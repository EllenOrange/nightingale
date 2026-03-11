use std::path::{Path, PathBuf};

use bevy::prelude::*;

#[derive(Resource, Debug, Clone)]
pub struct CacheDir {
    pub path: PathBuf,
}

impl CacheDir {
    pub fn new() -> Self {
        let path = dirs::home_dir()
            .expect("could not find home directory")
            .join(".nightingale")
            .join("cache");
        std::fs::create_dir_all(&path).expect("could not create cache directory");
        Self { path }
    }

    pub fn transcript_path(&self, hash: &str) -> PathBuf {
        self.path.join(format!("{hash}_transcript.json"))
    }

    pub fn instrumental_path(&self, hash: &str) -> PathBuf {
        self.path.join(format!("{hash}_instrumental.ogg"))
    }

    pub fn vocals_path(&self, hash: &str) -> PathBuf {
        self.path.join(format!("{hash}_vocals.ogg"))
    }

    pub fn lyrics_path(&self, hash: &str) -> PathBuf {
        self.path.join(format!("{hash}_lyrics.json"))
    }

    pub fn transcript_exists(&self, hash: &str) -> bool {
        self.transcript_path(hash).is_file()
            && self.instrumental_path(hash).is_file()
            && self.vocals_path(hash).is_file()
    }

    pub fn delete_song_cache(&self, hash: &str) {
        for path in [
            self.transcript_path(hash),
            self.instrumental_path(hash),
            self.vocals_path(hash),
            self.lyrics_path(hash),
        ] {
            if path.is_file() {
                let _ = std::fs::remove_file(&path);
            }
        }
    }

    pub fn clear_all(&self) {
        if self.path.is_dir() {
            let _ = std::fs::remove_dir_all(&self.path);
            let _ = std::fs::create_dir_all(&self.path);
        }
    }
}

pub fn dir_size(path: &Path) -> u64 {
    if !path.is_dir() {
        return 0;
    }
    walkdir::WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum()
}
