use std::path::{Path, PathBuf};
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
    pub fn from_path(path: &Path, file_hash: String, analysis_status: AnalysisStatus) -> Self {
        let (title, artist, album, duration_secs, album_art) = read_metadata(path);
        Self {
            path: path.to_path_buf(),
            file_hash,
            title,
            artist,
            album,
            duration_secs,
            album_art,
            analysis_status,
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
