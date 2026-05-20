use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::cache::config_path;

/// Where the user wants Nightingale to source songs from. Persisted in
/// `config.json` and consumed by both the scanner and the analyzer.
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(export)]
pub enum LibrarySource {
    Folder {
        path: PathBuf,
    },
    Jellyfin {
        base_url: String,
        user_id: String,
        username: String,
        /// Access token returned by `/Users/AuthenticateByName`.
        access_token: String,
        /// Stable per-install identifier we hand to Jellyfin in the
        /// `X-Emby-Authorization` header. Generated once at connect time.
        device_id: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AppConfig {
    #[serde(default = "default_data_path_option")]
    pub data_path: Option<PathBuf>,
    /// Deprecated. Kept for one-shot migration into `library_source`; never
    /// written by code that has been through `with_defaults`.
    pub last_folder: Option<PathBuf>,
    #[serde(default)]
    pub library_source: Option<LibrarySource>,
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
            library_source: None,
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
        // One-shot promotion of the legacy `last_folder` field into the new
        // `library_source` enum so old installs keep scanning the same folder.
        if self.library_source.is_none() {
            if let Some(path) = self.last_folder.take() {
                self.library_source = Some(LibrarySource::Folder { path });
            }
        }
        self
    }

    /// Pre-Jellyfin builds never wrote `last_folder` — the chosen folder lived
    /// only in `library_meta.folder` inside the DB. Recover it here so users
    /// upgrading with a pre-existing library don't lose their source (and the
    /// Rescan button doesn't get stuck disabled).
    fn migrate_from_library_db(&mut self) -> bool {
        if self.library_source.is_some() {
            return false;
        }

        let Ok((folder, _)) = crate::library_db::read_library_meta() else {
            return false;
        };

        if folder.is_empty() {
            return false;
        }

        self.library_source = Some(LibrarySource::Folder {
            path: PathBuf::from(folder),
        });
        
        true
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

        let (mut config, mut should_save) = match loaded {
            Some(cfg) => {
                let had_data_path = cfg.data_path.is_some();
                let had_library_source = cfg.library_source.is_some();
                let had_legacy_folder = cfg.last_folder.is_some();
                let needs_save =
                    !had_data_path || (!had_library_source && had_legacy_folder);
                (cfg.with_defaults(), needs_save)
            }
            None => (Self::default().with_defaults(), true),
        };

        if config.migrate_from_library_db() {
            should_save = true;
        }

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
