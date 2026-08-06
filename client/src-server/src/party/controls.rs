//! Admin transport + live audio controls. Each command mutates the shared
//! jukebox state and broadcasts it; the TV's playback session subscribes and
//! applies the change. No auth: this is a home-LAN convenience surface, not a
//! security boundary (see PHASE1_PLAN.md Step 4).

use serde::Deserialize;
use serde_json::{json, Value};

use crate::commands::{ApiError, CmdResult};
use crate::state::AppState;
use crate::ws::broadcast_jukebox;

fn bad(e: impl std::fmt::Display) -> ApiError {
    ApiError(axum::http::StatusCode::BAD_REQUEST, format!("invalid args: {e}"))
}

/// Pause or resume by setting the shared `paused` flag.
pub async fn party_control_pause(state: &AppState, paused: bool) -> CmdResult {
    let snapshot = state.jukebox.mutate(|s| s.paused = paused).await;
    broadcast_jukebox(state, &snapshot);
    Ok(json!({ "paused": paused }))
}

/// Restart the current song from the top without changing which song plays.
pub async fn party_control_restart(state: &AppState) -> CmdResult {
    let snapshot = state
        .jukebox
        .mutate(|s| s.restart_token = s.restart_token.wrapping_add(1))
        .await;
    broadcast_jukebox(state, &snapshot);
    Ok(json!({ "restartToken": snapshot.restart_token }))
}

#[derive(Debug, Deserialize)]
struct ValueArg {
    value: f32,
}

/// Guide-vocal mix level, clamped to 0.0..=1.0.
pub async fn party_set_guide_vocal(state: &AppState, payload: Value) -> CmdResult {
    let args: ValueArg = serde_json::from_value(payload).map_err(bad)?;
    let value = args.value.clamp(0.0, 1.0);
    let snapshot = state.jukebox.mutate(|s| s.guide_vocal = Some(value)).await;
    broadcast_jukebox(state, &snapshot);
    Ok(json!({ "guideVocal": value }))
}

/// Master volume, clamped to 0.0..=1.0.
pub async fn party_set_volume(state: &AppState, payload: Value) -> CmdResult {
    let args: ValueArg = serde_json::from_value(payload).map_err(bad)?;
    let value = args.value.clamp(0.0, 1.0);
    let snapshot = state.jukebox.mutate(|s| s.volume = Some(value)).await;
    broadcast_jukebox(state, &snapshot);
    Ok(json!({ "volume": value }))
}

#[derive(Debug, Deserialize)]
struct KeyArg {
    offset: i32,
}

/// Semitone key offset, clamped to a musical range (+/- one octave).
pub async fn party_set_key(state: &AppState, payload: Value) -> CmdResult {
    let args: KeyArg = serde_json::from_value(payload).map_err(bad)?;
    let offset = args.offset.clamp(-12, 12);
    let snapshot = state.jukebox.mutate(|s| s.key_offset = Some(offset)).await;
    broadcast_jukebox(state, &snapshot);
    Ok(json!({ "keyOffset": offset }))
}
