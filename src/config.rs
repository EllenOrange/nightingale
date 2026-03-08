use std::path::PathBuf;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Resource, Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    pub last_folder: Option<PathBuf>,
    pub last_theme: Option<usize>,
    pub guide_volume: Option<f64>,
    pub fullscreen: Option<bool>,
    pub dark_mode: Option<bool>,
    pub mic_active: Option<bool>,
}

impl AppConfig {
    fn config_path() -> PathBuf {
        dirs::home_dir()
            .expect("could not find home directory")
            .join(".karasad")
            .join("config.json")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
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
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(&path, json);
        }
    }

    pub fn is_fullscreen(&self) -> bool {
        self.fullscreen.unwrap_or(true)
    }

    pub fn is_dark_mode(&self) -> bool {
        self.dark_mode.unwrap_or(true)
    }
}
