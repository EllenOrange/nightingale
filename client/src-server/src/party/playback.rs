//! Remote-controlled playback: the spike that lets a WS message start a song on
//! the TV browser tab. The server never plays audio itself (the browser does),
//! so this only publishes a *play intent* into the shared jukebox state; the
//! frontend `use-remote-playback` hook observes it and navigates to /playback.

use serde::Deserialize;
use serde_json::{json, Value};

use crate::commands::{ApiError, CmdResult};
use crate::state::AppState;
use crate::ws::broadcast_jukebox;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PartyPlayArgs {
    file_hash: String,
}

/// `POST /api/cmd/party_play {"fileHash": "..."}`
///
/// Sets the requested song and bumps `play_token`, then broadcasts the new
/// jukebox snapshot to every connected client. Bumping the token (rather than
/// only setting the hash) means replaying the same song still triggers the
/// frontend, which keys its navigate on the token.
pub async fn party_play(state: &AppState, payload: Value) -> CmdResult {
    let args: PartyPlayArgs = serde_json::from_value(payload)
        .map_err(|e| ApiError(axum::http::StatusCode::BAD_REQUEST, format!("invalid args: {e}")))?;

    let play_token = issue_play(state, args.file_hash).await;
    Ok(json!({ "playToken": play_token }))
}

/// Publish a play intent for `file_hash`: set it as the requested song, bump
/// `play_token`, and broadcast. Shared by the `party_play` command and the
/// queue's auto-advance. Returns the new play token.
pub async fn issue_play(state: &AppState, file_hash: String) -> u64 {
    let snapshot = state
        .jukebox
        .mutate(|s| {
            s.requested_song_hash = Some(file_hash);
            s.play_token = s.play_token.wrapping_add(1);
            // A fresh song always starts playing, even if the previous one was
            // paused when it was skipped; otherwise the next song would load
            // and immediately pause.
            s.paused = false;
        })
        .await;

    broadcast_jukebox(state, &snapshot);
    snapshot.play_token
}

/// `POST /api/cmd/party_song_by_hash {"fileHash": "..."}`
///
/// Resolve a hash back to a full `Song` so the frontend can hand it to the
/// playback route (which needs the whole object in router state, not just a
/// hash). Returns `null` when the hash is unknown.
pub async fn party_song_by_hash(payload: Value) -> CmdResult {
    let args: PartyPlayArgs = serde_json::from_value(payload)
        .map_err(|e| ApiError(axum::http::StatusCode::BAD_REQUEST, format!("invalid args: {e}")))?;

    let song = app_core::song_by_hash(&args.file_hash);
    serde_json::to_value(song)
        .map_err(|e| ApiError(axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("serialise: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    // T1.1: two successive party_play calls with the same hash must produce
    // different play_token values, and the mutation must be visible via
    // snapshot(). This is the core guarantee the frontend relies on to re-fire
    // a play for a repeated song.
    #[tokio::test]
    async fn party_play_bumps_token_and_sets_hash() {
        let state = AppState::new();
        let hash = "7df0c494f3c8a7ccb974b6265e5b4f0e";

        // `.ok()` sidesteps `Result::expect`, which would require `ApiError:
        // Debug` and thus a change to upstream code.
        let first = party_play(&state, json!({ "fileHash": hash }))
            .await
            .ok()
            .expect("first party_play should succeed");
        let first_token = first["playToken"].as_u64().expect("playToken is a number");

        let snap1 = state.jukebox.snapshot().await;
        assert_eq!(snap1.requested_song_hash.as_deref(), Some(hash));
        assert_eq!(snap1.play_token, first_token);

        let second = party_play(&state, json!({ "fileHash": hash }))
            .await
            .ok()
            .expect("second party_play should succeed");
        let second_token = second["playToken"].as_u64().expect("playToken is a number");

        assert_ne!(
            first_token, second_token,
            "replaying the same song must produce a fresh play_token"
        );

        let snap2 = state.jukebox.snapshot().await;
        assert_eq!(snap2.requested_song_hash.as_deref(), Some(hash));
        assert_eq!(snap2.play_token, second_token);
        assert_eq!(second_token, first_token + 1);
    }

    // camelCase is the wire convention (Tauri serde carried into the web API).
    // A snake_case key must be rejected so we never silently accept the wrong
    // shape.
    #[tokio::test]
    async fn party_play_rejects_missing_file_hash() {
        let state = AppState::new();
        let result = party_play(&state, json!({ "file_hash": "abc" })).await;
        assert!(result.is_err(), "snake_case file_hash must be rejected");
    }
}
