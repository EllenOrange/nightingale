pub mod cache;
pub mod transcript;

use std::collections::VecDeque;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;

use cache::CacheDir;
use crate::scanner::metadata::{AnalysisStatus, SongLibrary};

pub struct AnalyzerPlugin;

impl Plugin for AnalyzerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AnalysisQueue>()
            .add_systems(Update, (process_queue, poll_active_job));
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
) -> Arc<Mutex<ProgressInfo>> {
    let progress = Arc::new(Mutex::new(ProgressInfo {
        percent: 0,
        message: "Starting analyzer...".into(),
        finished: None,
    }));

    let progress_clone = Arc::clone(&progress);
    let script = find_analyzer_script();
    let python = find_python();

    std::thread::spawn(move || {
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

    progress
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

    let progress = spawn_analyzer(
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

    let song_index = queue.active.as_ref().unwrap().song_index;
    let success = finished_info.finished.unwrap();

    if success && cache.transcript_exists(&library.songs[song_index].file_hash) {
        info!("Analysis complete for: {}", library.songs[song_index].path.display());
        library.songs[song_index].analysis_status = AnalysisStatus::Ready;
    } else {
        error!("Analysis failed: {}", finished_info.message);
        library.songs[song_index].analysis_status =
            AnalysisStatus::Failed(finished_info.message);
    }

    queue.active = None;
}
