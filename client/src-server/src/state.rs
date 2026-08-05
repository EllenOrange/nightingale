use std::sync::Arc;

use crate::events::EventBus;
use crate::jukebox::JukeboxStore;
use crate::party::queue_service::PartyQueueStore;

#[derive(Clone)]
pub struct AppState {
    pub events: Arc<EventBus>,
    pub jukebox: Arc<JukeboxStore>,
    pub party_queue: Arc<PartyQueueStore>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            events: Arc::new(EventBus::new()),
            jukebox: Arc::new(JukeboxStore::new()),
            party_queue: Arc::new(PartyQueueStore::new()),
        }
    }
}
