use std::path::PathBuf;

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
        self.path.join(format!("{hash}_instrumental.wav"))
    }

    pub fn vocals_path(&self, hash: &str) -> PathBuf {
        self.path.join(format!("{hash}_vocals.wav"))
    }

    pub fn lyrics_path(&self, hash: &str) -> PathBuf {
        self.path.join(format!("{hash}_lyrics.json"))
    }

    pub fn transcript_exists(&self, hash: &str) -> bool {
        self.transcript_path(hash).is_file()
            && self.instrumental_path(hash).is_file()
            && self.vocals_path(hash).is_file()
    }
}
