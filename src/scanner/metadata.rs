use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;

use bevy::prelude::*;
use lofty::prelude::*;

#[derive(Debug, Clone, Resource)]
pub struct SongLibrary {
    pub songs: Vec<Song>,
}

#[derive(Debug, Clone)]
pub struct Song {
    pub path: PathBuf,
    pub file_hash: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_secs: f64,
    pub album_art: Option<Arc<Vec<u8>>>,
    pub analysis_status: AnalysisStatus,
    pub language: Option<String>,
    pub is_video: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranscriptSource {
    Lyrics,
    Generated,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnalysisStatus {
    NotAnalyzed,
    Queued,
    Analyzing,
    Ready(TranscriptSource),
    Failed(String),
}

impl Song {
    pub fn from_path(
        path: &Path,
        file_hash: String,
        analysis_status: AnalysisStatus,
        language: Option<String>,
        is_video: bool,
    ) -> Self {
        let (title, artist, album, duration_secs, album_art) = if is_video {
            read_video_metadata(path)
        } else {
            read_metadata(path)
        };
        Self {
            path: path.to_path_buf(),
            file_hash,
            title,
            artist,
            album,
            duration_secs,
            album_art,
            analysis_status,
            language,
            is_video,
        }
    }

    pub fn display_title(&self) -> &str {
        if self.title.is_empty() {
            self.path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Unknown")
        } else {
            &self.title
        }
    }

    pub fn display_artist(&self) -> &str {
        if self.artist.is_empty() {
            "Unknown Artist"
        } else {
            &self.artist
        }
    }
}

fn read_metadata(path: &Path) -> (String, String, String, f64, Option<Arc<Vec<u8>>>) {
    let tagged = match lofty::read_from_path(path) {
        Ok(t) => t,
        Err(_) => return (String::new(), String::new(), String::new(), 0.0, None),
    };

    let properties = tagged.properties();
    let duration_secs = properties.duration().as_secs_f64();

    let tag = match tagged.primary_tag().or_else(|| tagged.first_tag()) {
        Some(t) => t,
        None => {
            return (
                String::new(),
                String::new(),
                String::new(),
                duration_secs,
                None,
            )
        }
    };

    let title = tag.title().map(|s| s.to_string()).unwrap_or_default();
    let artist = tag.artist().map(|s| s.to_string()).unwrap_or_default();
    let album = tag.album().map(|s| s.to_string()).unwrap_or_default();

    let album_art = tag.pictures().first().map(|pic| Arc::new(pic.data().to_vec()));

    (title, artist, album, duration_secs, album_art)
}

fn read_video_metadata(path: &Path) -> (String, String, String, f64, Option<Arc<Vec<u8>>>) {
    let ffmpeg = crate::vendor::ffmpeg_path();

    // Just probe the header -- no output file means ffmpeg reads metadata and exits immediately.
    let probe = crate::vendor::silent_command(&ffmpeg)
        .args(["-i", &path.to_string_lossy()])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output();

    let mut title = String::new();
    let mut artist = String::new();
    let mut album = String::new();
    let mut duration_secs = 0.0;

    if let Ok(output) = probe {
        let stderr = String::from_utf8_lossy(&output.stderr);
        for line in stderr.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("Duration:") {
                if let Some(ts) = rest.split(',').next() {
                    duration_secs = parse_ffmpeg_duration(ts.trim());
                }
            }
            if let Some(val) = strip_meta_tag(trimmed, "title") {
                title = val;
            }
            if let Some(val) = strip_meta_tag(trimmed, "artist") {
                artist = val;
            }
            if let Some(val) = strip_meta_tag(trimmed, "album") {
                album = val;
            }
        }
    }

    let album_art = extract_video_thumbnail(&ffmpeg, path);

    (title, artist, album, duration_secs, album_art)
}

fn strip_meta_tag(line: &str, tag: &str) -> Option<String> {
    let lower = line.to_lowercase();
    if lower.starts_with(tag) {
        let after = &line[tag.len()..];
        let after = after.trim_start();
        if let Some(val) = after.strip_prefix(':') {
            let val = val.trim();
            if !val.is_empty() {
                return Some(val.to_string());
            }
        }
    }
    None
}

fn parse_ffmpeg_duration(s: &str) -> f64 {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() == 3 {
        let h: f64 = parts[0].parse().unwrap_or(0.0);
        let m: f64 = parts[1].parse().unwrap_or(0.0);
        let s: f64 = parts[2].parse().unwrap_or(0.0);
        h * 3600.0 + m * 60.0 + s
    } else {
        0.0
    }
}

fn extract_video_thumbnail(ffmpeg: &Path, video_path: &Path) -> Option<Arc<Vec<u8>>> {
    let output = crate::vendor::silent_command(ffmpeg)
        .args([
            "-i",
            &video_path.to_string_lossy(),
            "-vframes", "1",
            "-f", "image2pipe",
            "-c:v", "mjpeg",
            "-vf", "scale=300:-1",
            "-v", "error",
            "pipe:1",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;

    if output.status.success() && !output.stdout.is_empty() {
        Some(Arc::new(output.stdout))
    } else {
        None
    }
}
