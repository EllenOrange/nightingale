use std::path::PathBuf;

use bevy::prelude::*;

#[derive(Debug, Clone, Resource)]
pub struct SongLibrary {
    pub songs: Vec<Song>,
    pub root_folder: PathBuf,
}

#[derive(Debug, Clone)]
pub struct Song {
    pub path: PathBuf,
    pub file_hash: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_secs: f64,
    pub album_art: Option<Vec<u8>>,
    pub analysis_status: AnalysisStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnalysisStatus {
    NotAnalyzed,
    Queued,
    Analyzing,
    Ready,
    Failed(String),
}

impl Song {
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
