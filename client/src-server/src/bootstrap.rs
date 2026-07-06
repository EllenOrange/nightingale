use app_core::{AppConfig, SongsStore};
use axum::{extract::State, Json};
use serde::Serialize;

use crate::state::AppState;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Bootstrap {
    config: AppConfig,
    songs_meta: app_core::SongsMeta,
    /// The data folder is fixed by the operator (via `NIGHTINGALE_DATA_PATH`),
    /// so the setup wizard hides the data-folder picker.
    data_path_pinned: bool,
    /// The library folder is fixed by the operator (via
    /// `NIGHTINGALE_LIBRARY_PATH`), so the UI hides the folder-select action.
    library_pinned: bool,
}

/// Replaces the `initialization_script` Tauri injects on window creation:
/// the web client awaits this once and seeds `window.__NIGHTINGALE_*` from
/// the response before mounting React.
pub async fn handle(State(_state): State<AppState>) -> Json<Bootstrap> {
    let config = AppConfig::load();
    let songs_meta = SongsStore::load_meta();
    Json(Bootstrap {
        config,
        songs_meta,
        data_path_pinned: env_pinned("NIGHTINGALE_DATA_PATH"),
        library_pinned: env_pinned("NIGHTINGALE_LIBRARY_PATH"),
    })
}

/// True when `key` is set to a non-empty value in the process environment.
fn env_pinned(key: &str) -> bool {
    std::env::var_os(key).is_some_and(|v| !v.is_empty())
}
