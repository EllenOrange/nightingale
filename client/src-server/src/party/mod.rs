//! Party layer: the multi-guest karaoke additions that turn Nightingale's
//! self-hosted web mode into a house-party jukebox. Kept in its own module tree
//! so the diff against upstream stays legible (see PHASE1_PLAN.md).
//!
//! Everything reachable from a `POST /api/cmd/party_*` route is dispatched
//! through [`dispatch`], which `commands::handle_cmd` calls before its own
//! table. This keeps the party command surface isolated from the ~490-line
//! upstream dispatcher.

use serde_json::Value;

use crate::commands::{ApiError, CmdResult};
use crate::state::AppState;

pub mod controls;
pub mod ingest;
pub mod playback;
pub mod qr;
pub mod queue;
pub mod queue_service;
pub mod search;

/// Route a `party_*` command. Returns `Err(NOT_FOUND)` for an unknown party
/// command so a typo surfaces instead of silently succeeding.
pub async fn dispatch(state: &AppState, name: &str, payload: Value) -> CmdResult {
    match name {
        "party_play" => playback::party_play(state, payload).await,
        "party_song_by_hash" => playback::party_song_by_hash(payload).await,
        "party_ingest" => ingest::party_ingest(state, payload).await,
        "party_search_lrclib" => search::party_search_lrclib(payload).await,
        "party_youtube_candidates" => search::party_youtube_candidates(payload).await,
        "party_queue_list" => queue_service::party_queue_list(state).await,
        "party_queue_add" => queue_service::party_queue_add(state, payload).await,
        "party_queue_remove" => queue_service::party_queue_remove(state, payload).await,
        "party_queue_clear" => queue_service::party_queue_clear(state).await,
        "party_queue_reorder" => queue_service::party_queue_reorder(state, payload).await,
        "party_song_ended" => queue_service::party_song_ended(state, payload).await,
        "party_skip" | "party_control_skip" => queue_service::party_skip(state).await,
        "party_control_pause" => controls::party_control_pause(state, true).await,
        "party_control_resume" => controls::party_control_pause(state, false).await,
        "party_control_restart" => controls::party_control_restart(state).await,
        "party_set_guide_vocal" => controls::party_set_guide_vocal(state, payload).await,
        "party_set_volume" => controls::party_set_volume(state, payload).await,
        "party_set_key" => controls::party_set_key(state, payload).await,
        _ => Err(ApiError(
            axum::http::StatusCode::NOT_FOUND,
            format!("unknown party command {name}"),
        )),
    }
}
