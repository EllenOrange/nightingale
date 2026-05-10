use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::cache::config_path;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AppConfig {
    #[serde(default = "default_data_path_option")]
    pub data_path: Option<PathBuf>,
    pub last_folder: Option<PathBuf>,
    pub last_theme: Option<usize>,
    pub guide_volume: Option<f64>,
    pub fullscreen: Option<bool>,
    pub dark_mode: Option<bool>,
    pub mic_active: Option<bool>,
    pub mic_mirroring: Option<bool>,
    pub mic_mirror_gain: Option<f64>,
    pub preferred_mic: Option<String>,
    pub whisper_model: Option<String>,
    pub beam_size: Option<u32>,
    pub batch_size: Option<u32>,
    pub last_video_flavor: Option<usize>,
    pub separator: Option<String>,
    pub asr_engine: Option<String>,
    pub language_overrides: Option<HashMap<String, String>>,
}

fn default_data_path_option() -> Option<PathBuf> {
    Some(AppConfig::default_data_path())
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            data_path: default_data_path_option(),
            last_folder: None,
            last_theme: None,
            guide_volume: None,
            fullscreen: None,
            dark_mode: None,
            mic_active: None,
            mic_mirroring: None,
            mic_mirror_gain: None,
            preferred_mic: None,
            whisper_model: None,
            beam_size: None,
            batch_size: None,
            last_video_flavor: None,
            separator: None,
            asr_engine: None,
            language_overrides: None,
        }
    }
}

impl AppConfig {
    pub fn default_data_path() -> PathBuf {
        crate::cache::default_nightingale_dir()
    }

    pub fn effective_data_path(&self) -> PathBuf {
        self.data_path
            .clone()
            .unwrap_or_else(Self::default_data_path)
    }

    fn with_defaults(mut self) -> Self {
        if self.data_path.is_none() {
            self.data_path = Some(Self::default_data_path());
        }
        self
    }

    pub fn load() -> Self {
        let path = config_path();

        let loaded = if path.is_file() {
            std::fs::read_to_string(&path)
                .ok()
                .and_then(|s| serde_json::from_str::<Self>(&s).ok())
        } else {
            None
        };

        let (config, should_save) = match loaded {
            Some(cfg) => {
                let had_data_path = cfg.data_path.is_some();
                (cfg.with_defaults(), !had_data_path)
            }
            None => {
                (Self::default().with_defaults(), true)
            }
        };

        if should_save {
            config.save();
        }

        config
    }

    pub fn save(&self) {
        let path = config_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(&path, json);
        }
    }

    pub fn whisper_model(&self) -> &str {
        self.whisper_model.as_deref().unwrap_or("large-v3")
    }

    pub fn beam_size(&self) -> u32 {
        self.beam_size.unwrap_or(8)
    }

    pub fn batch_size(&self) -> u32 {
        self.batch_size.unwrap_or(8)
    }

    pub fn separator(&self) -> &str {
        self.separator.as_deref().unwrap_or("karaoke")
    }

    pub fn asr_engine(&self) -> &str {
        self.asr_engine.as_deref().unwrap_or("whisper")
    }

    pub fn mic_mirror_gain(&self) -> f32 {
        self.mic_mirror_gain
            .map(|v| v as f32)
            .unwrap_or(0.65)
            .clamp(0.0, 2.0)
    }

    pub fn language_override(&self, file_hash: &str) -> Option<&str> {
        self.language_overrides
            .as_ref()
            .and_then(|m| m.get(file_hash))
            .map(|s| s.as_str())
    }

    pub fn set_language_override(&mut self, file_hash: String, lang: String) {
        self.language_overrides
            .get_or_insert_with(HashMap::new)
            .insert(file_hash, lang);
    }
}
