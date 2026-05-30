use app_core::{AppConfig, SongsStore};
use axum::{extract::State, Json};
use serde::Serialize;

use crate::state::AppState;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Bootstrap {
    config: AppConfig,
    songs_meta: app_core::SongsMeta,
}

/// Replaces the `initialization_script` Tauri injects on window creation:
/// the web client awaits this once and seeds `window.__NIGHTINGALE_*` from
/// the response before mounting React.
pub async fn handle(State(_state): State<AppState>) -> Json<Bootstrap> {
    let config = AppConfig::load();
    let songs_meta = SongsStore::load_meta();
    Json(Bootstrap { config, songs_meta })
}
