use std::sync::Arc;

use crate::events::EventBus;
use crate::jukebox::JukeboxStore;

#[derive(Clone)]
pub struct AppState {
    pub events: Arc<EventBus>,
    pub jukebox: Arc<JukeboxStore>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            events: Arc::new(EventBus::new()),
            jukebox: Arc::new(JukeboxStore::new()),
        }
    }
}
