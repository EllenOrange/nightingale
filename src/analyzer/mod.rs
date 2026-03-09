pub mod cache;
pub mod transcript;

use std::collections::VecDeque;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use bevy::app::AppExit;
use bevy::prelude::*;

use cache::CacheDir;
use crate::scanner::metadata::{AnalysisStatus, SongLibrary};

pub struct AnalyzerPlugin;

impl Plugin for AnalyzerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AnalysisQueue>()
            .add_systems(Update, (process_queue, poll_active_job))
            .add_systems(Last, kill_analyzer_on_exit);
    }
}

#[derive(Resource)]
pub struct PlayTarget {
    pub song_index: usize,
}

#[derive(Debug, Clone)]
pub struct ProgressInfo {
    pub percent: u32,
    pub message: String,
    pub finished: Option<bool>,
}

pub struct ActiveJob {
    pub song_index: usize,
    pub progress: Arc<Mutex<ProgressInfo>>,
    pub child_pid: Arc<AtomicU32>,
    pub thread_handle: Option<std::thread::JoinHandle<()>>,
}

#[derive(Resource, Default)]
pub struct AnalysisQueue {
    pub queue: VecDeque<usize>,
    pub active: Option<ActiveJob>,
}

impl AnalysisQueue {
    pub fn enqueue(&mut self, song_index: usize) {
        if self.active.as_ref().is_some_and(|a| a.song_index == song_index) {
            return;
        }
        if !self.queue.contains(&song_index) {
            self.queue.push_back(song_index);
        }
    }

    pub fn active_progress(&self, song_index: usize) -> Option<ProgressInfo> {
        self.active.as_ref().and_then(|a| {
            if a.song_index == song_index {
                Some(a.progress.lock().unwrap().clone())
            } else {
                None
            }
        })
    }
}

fn find_analyzer_script() -> PathBuf {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()));

    let candidates = [
        Some(PathBuf::from("analyzer/analyze.py")),
        exe_dir.map(|d| d.join("analyzer/analyze.py")),
    ];

    for candidate in candidates.iter().flatten() {
        if candidate.is_file() {
            return candidate.clone();
        }
    }

    PathBuf::from("analyzer/analyze.py")
}

fn find_python() -> String {
    let venv_python = PathBuf::from("analyzer/.venv/bin/python");
    if venv_python.is_file() {
        return venv_python.to_string_lossy().to_string();
    }
    "python3".to_string()
}

fn parse_progress_line(line: &str) -> Option<(u32, String)> {
    let prefix = "[nightingale:PROGRESS:";
    let start = line.find(prefix)?;
    let after_prefix = &line[start + prefix.len()..];
    let end_bracket = after_prefix.find(']')?;
    let pct_str = &after_prefix[..end_bracket];
    let pct: u32 = pct_str.parse().ok()?;
    let msg = after_prefix[end_bracket + 1..].trim().to_string();
    Some((pct, msg))
}

fn spawn_analyzer(
    song_path: PathBuf,
    cache_path: PathBuf,
    file_hash: String,
    whisper_model: String,
    beam_size: u32,
    batch_size: u32,
) -> (Arc<Mutex<ProgressInfo>>, Arc<AtomicU32>, std::thread::JoinHandle<()>) {
    let progress = Arc::new(Mutex::new(ProgressInfo {
        percent: 0,
        message: "Starting analyzer...".into(),
        finished: None,
    }));
    let child_pid = Arc::new(AtomicU32::new(0));

    let progress_clone = Arc::clone(&progress);
    let pid_clone = Arc::clone(&child_pid);
    let script = find_analyzer_script();
    let python = find_python();

    let thread_handle = std::thread::spawn(move || {
        let child = Command::new(&python)
            .arg(&script)
            .arg(&song_path)
            .arg(&cache_path)
            .arg("--hash")
            .arg(&file_hash)
            .arg("--model")
            .arg(&whisper_model)
            .arg("--beam-size")
            .arg(beam_size.to_string())
            .arg("--batch-size")
            .arg(batch_size.to_string())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();

        match child {
            Ok(mut child) => {
                pid_clone.store(child.id(), Ordering::Relaxed);
                use std::io::{BufRead, BufReader};

                let stderr_lines: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
                let stderr_clone = Arc::clone(&stderr_lines);
                let stderr_thread = child.stderr.take().map(|stderr| {
                    std::thread::spawn(move || {
                        let reader = BufReader::new(stderr);
                        for line in reader.lines() {
                            if let Ok(line) = line {
                                eprintln!("[analyzer stderr] {}", line);
                                stderr_clone.lock().unwrap().push(line);
                            }
                        }
                    })
                });

                if let Some(stdout) = child.stdout.take() {
                    let reader = BufReader::new(stdout);
                    for line in reader.lines() {
                        if let Ok(line) = line {
                            if let Some((pct, msg)) = parse_progress_line(&line) {
                                let mut p = progress_clone.lock().unwrap();
                                p.percent = pct;
                                p.message = msg;
                            }
                            eprintln!("[analyzer] {}", line);
                        }
                    }
                }

                if let Some(handle) = stderr_thread {
                    let _ = handle.join();
                }

                match child.wait() {
                    Ok(status) => {
                        let mut p = progress_clone.lock().unwrap();
                        p.finished = Some(status.success());
                        if !status.success() {
                            let err_lines = stderr_lines.lock().unwrap();
                            let last_err = err_lines
                                .iter()
                                .rev()
                                .find(|l| !l.trim().is_empty())
                                .cloned()
                                .unwrap_or_else(|| format!("exit code: {status}"));
                            p.message = last_err;
                        }
                    }
                    Err(e) => {
                        let mut p = progress_clone.lock().unwrap();
                        p.finished = Some(false);
                        p.message = format!("Error: {e}");
                    }
                }
            }
            Err(e) => {
                let mut p = progress_clone.lock().unwrap();
                p.finished = Some(false);
                p.message = format!("Failed to start: {e}");
            }
        }
    });

    (progress, child_pid, thread_handle)
}

fn process_queue(
    mut queue: ResMut<AnalysisQueue>,
    library: Option<ResMut<SongLibrary>>,
    cache: Res<CacheDir>,
    config: Res<crate::config::AppConfig>,
) {
    let Some(mut library) = library else { return };
    if queue.active.is_some() || queue.queue.is_empty() {
        return;
    }

    let song_index = queue.queue.pop_front().unwrap();
    let song = &library.songs[song_index];

    info!(
        "Starting analysis of: {} (hash={})",
        song.path.display(),
        song.file_hash
    );

    let (progress, child_pid, thread_handle) = spawn_analyzer(
        song.path.clone(),
        cache.path.clone(),
        song.file_hash.clone(),
        config.whisper_model().to_string(),
        config.beam_size(),
        config.batch_size(),
    );

    library.songs[song_index].analysis_status = AnalysisStatus::Analyzing;

    queue.active = Some(ActiveJob {
        song_index,
        progress,
        child_pid,
        thread_handle: Some(thread_handle),
    });
}

fn poll_active_job(
    mut queue: ResMut<AnalysisQueue>,
    library: Option<ResMut<SongLibrary>>,
    cache: Res<CacheDir>,
) {
    let Some(mut library) = library else { return };
    let finished_info = {
        let Some(ref active) = queue.active else {
            return;
        };
        let info = active.progress.lock().unwrap().clone();
        if info.finished.is_none() {
            return;
        }
        info
    };

    let mut active = queue.active.take().unwrap();
    let song_index = active.song_index;
    let success = finished_info.finished.unwrap();

    if let Some(handle) = active.thread_handle.take() {
        let _ = handle.join();
    }

    if success && cache.transcript_exists(&library.songs[song_index].file_hash) {
        info!("Analysis complete for: {}", library.songs[song_index].path.display());
        library.songs[song_index].analysis_status = AnalysisStatus::Ready;
    } else {
        error!("Analysis failed: {}", finished_info.message);
        library.songs[song_index].analysis_status =
            AnalysisStatus::Failed(finished_info.message);
    }
}

fn kill_analyzer_on_exit(
    mut exit_events: MessageReader<AppExit>,
    queue: Res<AnalysisQueue>,
) {
    if exit_events.read().next().is_none() {
        return;
    }
    if let Some(ref active) = queue.active {
        let pid = active.child_pid.load(Ordering::Relaxed);
        if pid != 0 {
            info!("Killing analyzer subprocess (pid={pid})");
            #[cfg(unix)]
            {
                let _ = Command::new("kill")
                    .args(["-TERM", &pid.to_string()])
                    .spawn();
            }
            #[cfg(windows)]
            {
                let _ = Command::new("taskkill")
                    .args(["/PID", &pid.to_string(), "/F"])
                    .spawn();
            }
        }
    }
}
