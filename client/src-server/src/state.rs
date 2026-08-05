use std::sync::{Arc, Mutex};

use crate::events::EventBus;
use crate::jukebox::JukeboxStore;
use crate::party::queue_service::{PartyQueueStore, PlaybackProgress};

#[derive(Clone)]
pub struct AppState {
    pub events: Arc<EventBus>,
    pub jukebox: Arc<JukeboxStore>,
    pub party_queue: Arc<PartyQueueStore>,
    /// Last playback position the TV reported, used by the auto-advance watchdog
    /// to defer to real progress (so pause/restart never cut a song short) and
    /// to fall back to wall-clock only when the TV goes silent. `None` until the
    /// first heartbeat.
    pub progress: Arc<Mutex<Option<PlaybackProgress>>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            events: Arc::new(EventBus::new()),
            jukebox: Arc::new(JukeboxStore::new()),
            party_queue: Arc::new(PartyQueueStore::new()),
            progress: Arc::new(Mutex::new(None)),
        }
    }
}
