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
use crate::scanner::metadata::{AnalysisStatus, Song, SongLibrary, TranscriptSource};

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
    crate::vendor::analyzer_dir().join("analyze.py")
}

fn find_python() -> String {
    crate::vendor::python_path().to_string_lossy().to_string()
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

fn fetch_lrclib_lyrics(song: &Song, cache: &CacheDir) -> Option<PathBuf> {
    let title = song.display_title();
    let artist = song.display_artist();
    let duration = song.duration_secs.round() as u64;

    if title.is_empty() || artist == "Unknown Artist" {
        return None;
    }

    let agent = ureq::Agent::new_with_defaults();

    eprintln!("[lrclib] Searching: \"{title}\" by \"{artist}\" ({}s, album=\"{}\")", duration, song.album);

    let url = format!(
        "https://lrclib.net/api/search?track_name={}&artist_name={}",
        urlencoding::encode(title),
        urlencoding::encode(artist),
    );
    let resp = match agent
        .get(&url)
        .header("User-Agent", "Nightingale/1.0")
        .call()
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[lrclib] Search request failed: {e}");
            return None;
        }
    };
    let results: Vec<serde_json::Value> = match resp.into_body().read_json() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[lrclib] Failed to parse search results: {e}");
            return None;
        }
    };

    let with_lyrics: Vec<_> = results
        .into_iter()
        .filter(|r| {
            r.get("plainLyrics")
                .and_then(|v| v.as_str())
                .is_some_and(|s| !s.is_empty())
                || r.get("syncedLyrics")
                    .and_then(|v| v.as_str())
                    .is_some_and(|s| !s.is_empty())
        })
        .collect();

    eprintln!("[lrclib] Search returned {} results with lyrics", with_lyrics.len());

    let album_lower = song.album.to_lowercase();
    let record = with_lyrics
        .into_iter()
        .min_by_key(|r| {
            let has_synced = r.get("syncedLyrics")
                .and_then(|v| v.as_str())
                .is_some_and(|s| !s.is_empty());
            let album_match = r.get("albumName")
                .and_then(|v| v.as_str())
                .is_some_and(|a| a.to_lowercase() == album_lower);
            let d = r.get("duration").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let duration_penalty = ((d - song.duration_secs).abs() * 10.0) as i64;

            let synced_bonus: i64 = if has_synced { 0 } else { 10_000 };
            let album_bonus: i64 = if album_match { 0 } else { 5_000 };

            synced_bonus + album_bonus + duration_penalty
        });

    if let Some(ref r) = record {
        let d = r.get("duration").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let name = r.get("trackName").and_then(|v| v.as_str()).unwrap_or("?");
        let album = r.get("albumName").and_then(|v| v.as_str()).unwrap_or("?");
        let has_synced = r.get("syncedLyrics").and_then(|v| v.as_str()).is_some_and(|s| !s.is_empty());
        eprintln!(
            "[lrclib] Picked \"{}\" from \"{}\" (duration {:.0}s, delta {:.1}s, synced={})",
            name, album, d, (d - song.duration_secs).abs(), has_synced
        );
    }

    let Some(record) = record else {
        eprintln!("[lrclib] No lyrics found");
        return None;
    };

    let synced_str = record
        .get("syncedLyrics")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty());

    let synced_lines = synced_str.and_then(|s| parse_lrc(s));

    let lyrics_json = if let Some(ref timed) = synced_lines {
        eprintln!("[lrclib] Extracted {} synced lines with timestamps", timed.len());
        serde_json::json!({"lines": timed})
    } else {
        let plain_str = record
            .get("plainLyrics")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty());

        let Some(plain) = plain_str else {
            eprintln!("[lrclib] Record has no plainLyrics, skipping");
            return None;
        };

        let lines: Vec<String> = plain
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect();

        if lines.is_empty() {
            eprintln!("[lrclib] Extracted 0 lines, skipping");
            return None;
        }

        eprintln!("[lrclib] Extracted {} plain lines (no timestamps)", lines.len());
        serde_json::json!({"lines": lines})
    };

    let out = cache.lyrics_path(&song.file_hash);
    match std::fs::write(&out, serde_json::to_string_pretty(&lyrics_json).unwrap()) {
        Ok(_) => {
            eprintln!("[lrclib] Lyrics saved to {}", out.display());
            Some(out)
        }
        Err(e) => {
            eprintln!("[lrclib] Failed to write lyrics: {e}");
            None
        }
    }
}

fn parse_lrc(lrc: &str) -> Option<Vec<serde_json::Value>> {
    let mut result = Vec::new();
    for line in lrc.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Some(close) = line.find(']') else {
            continue;
        };
        let tag = &line[1..close];
        let text = line[close + 1..].trim();
        if text.is_empty() {
            continue;
        }
        let parts: Vec<&str> = tag.splitn(2, ':').collect();
        if parts.len() != 2 {
            continue;
        }
        let Ok(mins) = parts[0].parse::<f64>() else {
            continue;
        };
        let Ok(secs) = parts[1].parse::<f64>() else {
            continue;
        };
        let start = mins * 60.0 + secs;
        result.push(serde_json::json!({"text": text, "start": start}));
    }
    if result.len() >= 2 {
        Some(result)
    } else {
        None
    }
}

fn spawn_analyzer(
    song_path: PathBuf,
    cache_path: PathBuf,
    file_hash: String,
    song: Song,
    whisper_model: String,
    beam_size: u32,
    batch_size: u32,
    separator: String,
) -> (Arc<Mutex<ProgressInfo>>, Arc<AtomicU32>, std::thread::JoinHandle<()>) {
    let progress = Arc::new(Mutex::new(ProgressInfo {
        percent: 0,
        message: "Searching for lyrics...".into(),
        finished: None,
    }));
    let child_pid = Arc::new(AtomicU32::new(0));

    let progress_clone = Arc::clone(&progress);
    let pid_clone = Arc::clone(&child_pid);
    let script = find_analyzer_script();
    let python = find_python();

    let thread_handle = std::thread::spawn(move || {
        let cache = CacheDir { path: cache_path.clone() };
        let lyrics_path = fetch_lrclib_lyrics(&song, &cache);

        {
            let mut p = progress_clone.lock().unwrap();
            p.message = "Starting analyzer...".into();
        }

        let models = crate::vendor::models_dir();
        let ffmpeg = crate::vendor::ffmpeg_path();
        let ffmpeg_dir = ffmpeg.parent().unwrap_or(std::path::Path::new("."));
        let path_env = if let Some(existing) = std::env::var_os("PATH") {
            let mut paths = std::env::split_paths(&existing).collect::<Vec<_>>();
            paths.insert(0, ffmpeg_dir.to_path_buf());
            std::env::join_paths(paths).unwrap_or(existing)
        } else {
            ffmpeg_dir.as_os_str().to_os_string()
        };
        let mut current_batch_size = batch_size;

        loop {
            let mut cmd = Command::new(&python);
            cmd.env("PATH", &path_env)
                .env("TORCH_HOME", models.join("torch"))
                .env("HF_HOME", models.join("huggingface"))
                .env("FFMPEG_PATH", &ffmpeg)
                .env("PYTHONWARNINGS", "ignore")
                .env("PYTORCH_ENABLE_MPS_FALLBACK", "1")
                .env("HF_HUB_DISABLE_SYMLINKS_WARNING", "1")
                .env("HF_HUB_DISABLE_SYMLINKS", "1")
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
                .arg(current_batch_size.to_string())
                .arg("--separator")
                .arg(&separator);

            if let Some(ref lp) = lyrics_path {
                cmd.arg("--lyrics").arg(lp);
            }

            let child = cmd
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
                            if status.success() {
                                let mut p = progress_clone.lock().unwrap();
                                p.finished = Some(true);
                                break;
                            }

                            let err_lines = stderr_lines.lock().unwrap();
                            let all_stderr = err_lines.join("\n");
                            let is_oom = all_stderr.contains("CUDA out of memory")
                                || all_stderr.contains("OutOfMemoryError");

                            if is_oom && current_batch_size > 1 {
                                let new_batch = current_batch_size / 2;
                                eprintln!(
                                    "[analyzer] CUDA OOM detected, retrying with batch_size={new_batch} (was {current_batch_size})"
                                );
                                current_batch_size = new_batch;
                                let mut p = progress_clone.lock().unwrap();
                                p.percent = 0;
                                p.message = format!("CUDA OOM — retrying with batch size {new_batch}...");
                                continue;
                            }

                            let last_err = err_lines
                                .iter()
                                .rev()
                                .find(|l| !l.trim().is_empty())
                                .cloned()
                                .unwrap_or_else(|| format!("exit code: {status}"));
                            let mut p = progress_clone.lock().unwrap();
                            p.finished = Some(false);
                            p.message = last_err;
                            break;
                        }
                        Err(e) => {
                            let mut p = progress_clone.lock().unwrap();
                            p.finished = Some(false);
                            p.message = format!("Error: {e}");
                            break;
                        }
                    }
                }
            Err(e) => {
                let mut p = progress_clone.lock().unwrap();
                p.finished = Some(false);
                p.message = format!("Failed to start: {e}");
                break;
            }
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
        song.clone(),
        config.whisper_model().to_string(),
        config.beam_size(),
        config.batch_size(),
        config.separator().to_string(),
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
        let hash = &library.songs[song_index].file_hash;
        let source = match transcript::Transcript::load(&cache.transcript_path(hash)) {
            Ok(t) if t.source == "lyrics" => TranscriptSource::Lyrics,
            _ => TranscriptSource::Generated,
        };
        library.songs[song_index].analysis_status = AnalysisStatus::Ready(source);
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
