use std::path::PathBuf;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreRecord {
    pub profile: String,
    pub song_hash: String,
    pub score: u32,
    pub played_at: u64,
}

#[derive(Resource, Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProfileStore {
    pub active: Option<String>,
    pub profiles: Vec<String>,
    #[serde(default)]
    pub scores: Vec<ScoreRecord>,
}

impl ProfileStore {
    fn path() -> PathBuf {
        dirs::home_dir()
            .expect("could not find home directory")
            .join(".nightingale")
            .join("profiles.json")
    }

    pub fn load() -> Self {
        let path = Self::path();
        if path.is_file() {
            std::fs::read_to_string(&path)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default()
        } else {
            Self::default()
        }
    }

    pub fn save(&self) {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, json);
        }
    }

    pub fn create_profile(&mut self, name: String) {
        if !self.profiles.contains(&name) {
            self.profiles.push(name.clone());
        }
        self.active = Some(name);
        self.save();
    }

    pub fn switch_profile(&mut self, name: &str) {
        if self.profiles.contains(&name.to_string()) {
            self.active = Some(name.to_string());
            self.save();
        }
    }

    pub fn delete_profile(&mut self, name: &str) {
        self.profiles.retain(|n| n != name);
        self.scores.retain(|r| r.profile != name);
        if self.active.as_deref() == Some(name) {
            self.active = self.profiles.first().cloned();
        }
        self.save();
    }

    pub fn add_score(&mut self, song_hash: &str, score: u32) {
        let profile = match &self.active {
            Some(p) => p.clone(),
            None => return,
        };
        let played_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        self.scores.push(ScoreRecord {
            profile,
            song_hash: song_hash.to_string(),
            score,
            played_at,
        });
        self.save();
    }

    pub fn best_score(&self, song_hash: &str, profile: &str) -> Option<u32> {
        self.scores
            .iter()
            .filter(|r| r.song_hash == song_hash && r.profile == profile)
            .map(|r| r.score)
            .max()
    }

    pub fn top_scores_for_song(&self, song_hash: &str, limit: usize) -> Vec<(String, u32)> {
        let mut best: std::collections::HashMap<&str, u32> = std::collections::HashMap::new();
        for record in &self.scores {
            if record.song_hash == song_hash {
                let entry = best.entry(&record.profile).or_insert(0);
                if record.score > *entry {
                    *entry = record.score;
                }
            }
        }
        let mut sorted: Vec<(String, u32)> = best
            .into_iter()
            .map(|(name, score)| (name.to_string(), score))
            .collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1));
        sorted.truncate(limit);
        sorted
    }
}
