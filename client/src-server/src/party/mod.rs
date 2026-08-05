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

pub mod ingest;
pub mod playback;

/// Route a `party_*` command. Returns `Err(NOT_FOUND)` for an unknown party
/// command so a typo surfaces instead of silently succeeding.
pub async fn dispatch(state: &AppState, name: &str, payload: Value) -> CmdResult {
    match name {
        "party_play" => playback::party_play(state, payload).await,
        "party_song_by_hash" => playback::party_song_by_hash(payload).await,
        "party_ingest" => ingest::party_ingest(state, payload).await,
        _ => Err(ApiError(
            axum::http::StatusCode::NOT_FOUND,
            format!("unknown party command {name}"),
        )),
    }
}
