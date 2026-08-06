use std::sync::atomic::{AtomicU64, Ordering};

use serde::Serialize;
use tokio::sync::RwLock;

pub type ClientId = u64;

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

pub fn next_client_id() -> ClientId {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

/// Single shared playback state for the jukebox session. The server is the
/// source of truth; every browser sees the same snapshot rebroadcast over WS.
#[derive(Clone, Debug, Default, Serialize)]
pub struct JukeboxState {
    pub current_song: Option<String>,
    pub paused: bool,
    pub position_ms: u64,
    pub pitch_hz: Option<f32>,
    pub rms: Option<f32>,
    pub mic_owner: Option<ClientId>,
    pub controller: Option<ClientId>,
    pub theme: Option<usize>,
    pub score: u32,

    // ── Party layer ─────────────────────────────────────────────────────────
    /// The song the TV browser tab should be playing. Set by `party_play`.
    pub requested_song_hash: Option<String>,
    /// Monotonic counter bumped on every `party_play`. The frontend triggers a
    /// navigate when this changes, so replaying the same song still fires (the
    /// hash alone would look unchanged).
    pub play_token: u64,
    /// Bumped by `party_control_restart` to re-trigger the current song from the
    /// top without changing which song plays.
    pub restart_token: u64,

    // Live audio controls set from the admin page. `None` means "no remote
    // override, keep the TV's local default", so the server never forces a
    // control the admin has not touched (e.g. muting volume to 0).
    /// Guide-vocal mix level, 0.0 to 1.0.
    pub guide_vocal: Option<f32>,
    /// Semitone key offset applied to the current song.
    pub key_offset: Option<i32>,
    /// Master volume, 0.0 to 1.0.
    pub volume: Option<f32>,
}

#[derive(Default)]
pub struct JukeboxStore {
    state: RwLock<JukeboxState>,
}

impl JukeboxStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn snapshot(&self) -> JukeboxState {
        self.state.read().await.clone()
    }

    pub async fn mutate<F>(&self, f: F) -> JukeboxState
    where
        F: FnOnce(&mut JukeboxState),
    {
        let mut guard = self.state.write().await;
        f(&mut guard);
        guard.clone()
    }
}
