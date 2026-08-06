//! The party request queue: the shared, multi-guest list of songs to play.
//!
//! This file owns the *pure* queue model (entries, the status state machine,
//! ordering, persistence). The async worker that drives ingest and the routes
//! that mutate it live in `queue_service.rs` / `mod.rs`; keeping the data model
//! free of IO makes the state machine and persistence unit-testable (T3.1,
//! T3.2) without a running server.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// Lifecycle of one queued request. Serializes to camelCase strings for the
/// frontend (`queued`, `downloading`, ...).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum QueueStatus {
    /// Accepted, not yet started.
    Queued,
    /// yt-dlp is fetching the media.
    Downloading,
    /// Stems + lyric alignment in progress.
    Analyzing,
    /// Analyzed and playable, waiting its turn.
    Ready,
    /// Currently on the TV.
    Playing,
    /// Finished playing.
    Done,
    /// Terminal failure; `error` carries the reason.
    Error,
}

impl QueueStatus {
    /// Whether `self -> next` is a legal transition. Encodes the state machine
    /// from PHASE1_PLAN.md Step 3: the happy path is
    /// `queued -> downloading -> analyzing -> ready -> playing -> done`, a
    /// library song already analyzed may jump straight to `ready`, `error` is
    /// reachable from any non-terminal state, and a failed entry may be retried
    /// back to `queued`.
    pub fn can_transition_to(self, next: QueueStatus) -> bool {
        use QueueStatus::*;
        if next == Error {
            // Error reachable from any live state, but Done is terminal.
            return self != Done && self != Error;
        }
        match (self, next) {
            (Queued, Downloading) => true,
            // A library song needing analysis skips the download step.
            (Queued, Analyzing) => true,
            // A library song that is already analyzed skips ingest entirely.
            (Queued, Ready) => true,
            (Downloading, Analyzing) => true,
            // Analysis cache hit can surface as ready without a visible analyzing step.
            (Downloading, Ready) => true,
            (Analyzing, Ready) => true,
            (Ready, Playing) => true,
            (Playing, Done) => true,
            // Retry a failed entry.
            (Error, Queued) => true,
            _ => false,
        }
    }

    /// Terminal states never transition further (except Error, handled above).
    pub fn is_terminal(self) -> bool {
        matches!(self, QueueStatus::Done)
    }
}

/// One request in the queue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueEntry {
    pub id: String,
    /// The YouTube URL / search string to ingest, if this came from a search.
    /// `None` for a song added straight from the local library by hash.
    #[serde(default)]
    pub query: Option<String>,
    /// Known once the media is in the library and scanned.
    #[serde(default)]
    pub file_hash: Option<String>,
    pub title: String,
    pub artist: String,
    pub requested_by: String,
    pub status: QueueStatus,
    /// Unix milliseconds when the entry was added.
    pub added_at: u64,
    #[serde(default)]
    pub error: Option<String>,
}

/// The whole shared queue. Ordering is significant (play order).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartyQueue {
    pub entries: Vec<QueueEntry>,
    /// Monotonic id source, persisted so ids stay unique across restarts.
    #[serde(default)]
    next_id: u64,
}

static ID_COUNTER: AtomicU64 = AtomicU64::new(0);

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

impl PartyQueue {
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a new entry, returning its id. `added_at` is stamped now.
    pub fn add(
        &mut self,
        query: Option<String>,
        file_hash: Option<String>,
        title: String,
        artist: String,
        requested_by: String,
        status: QueueStatus,
    ) -> String {
        self.next_id += 1;
        // Combine the persisted per-queue counter with a process-global counter
        // so ids are unique even if two queues are alive in one process (tests).
        let id = format!("q{}_{}", self.next_id, ID_COUNTER.fetch_add(1, Ordering::Relaxed));
        self.entries.push(QueueEntry {
            id: id.clone(),
            query,
            file_hash,
            title,
            artist,
            requested_by,
            status,
            added_at: now_millis(),
            error: None,
        });
        id
    }

    pub fn get(&self, id: &str) -> Option<&QueueEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    fn get_mut(&mut self, id: &str) -> Option<&mut QueueEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    /// Attempt a status transition. Returns false (and does nothing) if the
    /// transition is illegal or the id is unknown.
    pub fn set_status(&mut self, id: &str, next: QueueStatus) -> bool {
        let Some(entry) = self.get_mut(id) else {
            return false;
        };
        if !entry.status.can_transition_to(next) {
            return false;
        }
        entry.status = next;
        if next != QueueStatus::Error {
            entry.error = None;
        }
        true
    }

    /// Move an entry into `Error` with a message (always legal from a live state).
    pub fn set_error(&mut self, id: &str, message: impl Into<String>) -> bool {
        let Some(entry) = self.get_mut(id) else {
            return false;
        };
        if !entry.status.can_transition_to(QueueStatus::Error) {
            return false;
        }
        entry.status = QueueStatus::Error;
        entry.error = Some(message.into());
        true
    }

    /// Record the resolved library identity once ingest/scan produces it.
    pub fn set_resolved(&mut self, id: &str, file_hash: String, title: String, artist: String) {
        if let Some(entry) = self.get_mut(id) {
            entry.file_hash = Some(file_hash);
            entry.title = title;
            entry.artist = artist;
        }
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let before = self.entries.len();
        self.entries.retain(|e| e.id != id);
        self.entries.len() != before
    }

    /// Move `id` to `position` (clamped), preserving the order of the rest.
    pub fn reorder(&mut self, id: &str, position: usize) -> bool {
        let Some(from) = self.entries.iter().position(|e| e.id == id) else {
            return false;
        };
        let entry = self.entries.remove(from);
        let to = position.min(self.entries.len());
        self.entries.insert(to, entry);
        true
    }

    /// The id of the first entry in `Ready` status, for auto-advance.
    pub fn first_ready(&self) -> Option<String> {
        self.entries
            .iter()
            .find(|e| e.status == QueueStatus::Ready)
            .map(|e| e.id.clone())
    }

    /// The id of the entry currently `Playing`, if any.
    pub fn playing(&self) -> Option<String> {
        self.entries
            .iter()
            .find(|e| e.status == QueueStatus::Playing)
            .map(|e| e.id.clone())
    }

    /// The id of the first entry still `Queued`, for the ingest worker.
    pub fn first_queued(&self) -> Option<String> {
        self.entries
            .iter()
            .find(|e| e.status == QueueStatus::Queued)
            .map(|e| e.id.clone())
    }

    // ── Persistence ──────────────────────────────────────────────────────

    /// Default on-disk location under the data dir.
    pub fn default_path() -> PathBuf {
        app_core::nightingale_dir().join("party_queue.json")
    }

    pub fn load() -> Self {
        Self::load_from(&Self::default_path())
    }

    pub fn load_from(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) {
        if let Err(e) = self.save_to(&Self::default_path()) {
            tracing::warn!("[party] failed to persist queue: {e}");
        }
    }

    /// Write atomically: serialize to a sibling temp file, then rename over the
    /// target so a crash mid-write can never leave a torn `party_queue.json`.
    pub fn save_to(&self, path: &Path) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, json.as_bytes())?;
        // fs::rename replaces an existing destination on both Windows and Unix.
        std::fs::rename(&tmp, path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use QueueStatus::*;

    // T3.1: the status state machine. Table-driven so every legal and illegal
    // transition is asserted. This is the highest-value unit test in Step 3:
    // the transitions are pure and easy to get subtly wrong.
    #[test]
    fn transition_table() {
        let all = [Queued, Downloading, Analyzing, Ready, Playing, Done, Error];
        // (from, to) pairs that are legal. Everything else must be rejected.
        let legal: &[(QueueStatus, QueueStatus)] = &[
            (Queued, Downloading),
            (Queued, Analyzing),
            (Queued, Ready),
            (Downloading, Analyzing),
            (Downloading, Ready),
            (Analyzing, Ready),
            (Ready, Playing),
            (Playing, Done),
            (Error, Queued),
            // error reachable from any live state
            (Queued, Error),
            (Downloading, Error),
            (Analyzing, Error),
            (Ready, Error),
            (Playing, Error),
        ];
        for &from in &all {
            for &to in &all {
                let expected = legal.contains(&(from, to));
                assert_eq!(
                    from.can_transition_to(to),
                    expected,
                    "transition {from:?} -> {to:?} expected legal={expected}"
                );
            }
        }
    }

    // The specific illegal transitions the plan calls out.
    #[test]
    fn rejects_the_named_illegal_transitions() {
        assert!(!Done.can_transition_to(Playing), "done -> playing must be illegal");
        assert!(!Queued.can_transition_to(Playing), "queued -> playing (skipping ready) must be illegal");
        assert!(!Done.can_transition_to(Error), "done is terminal");
    }

    #[test]
    fn set_status_enforces_machine() {
        let mut q = PartyQueue::new();
        let id = q.add(Some("x".into()), None, "T".into(), "A".into(), "guest".into(), Queued);

        // Illegal jump is rejected and leaves status unchanged.
        assert!(!q.set_status(&id, Playing));
        assert_eq!(q.get(&id).unwrap().status, Queued);

        // Legal path works.
        assert!(q.set_status(&id, Downloading));
        assert!(q.set_status(&id, Analyzing));
        assert!(q.set_status(&id, Ready));
        assert!(q.set_status(&id, Playing));
        assert!(q.set_status(&id, Done));
        // Terminal.
        assert!(!q.set_status(&id, Playing));
    }

    #[test]
    fn set_error_records_message_and_clears_on_retry() {
        let mut q = PartyQueue::new();
        let id = q.add(Some("x".into()), None, "T".into(), "A".into(), "guest".into(), Queued);
        assert!(q.set_status(&id, Downloading));
        assert!(q.set_error(&id, "boom"));
        assert_eq!(q.get(&id).unwrap().status, Error);
        assert_eq!(q.get(&id).unwrap().error.as_deref(), Some("boom"));
        // Retry back to queued clears the error.
        assert!(q.set_status(&id, Queued));
        assert_eq!(q.get(&id).unwrap().error, None);
    }

    #[test]
    fn unique_ids() {
        let mut q = PartyQueue::new();
        let a = q.add(None, None, "a".into(), "".into(), "g".into(), Queued);
        let b = q.add(None, None, "b".into(), "".into(), "g".into(), Queued);
        assert_ne!(a, b);
    }

    #[test]
    fn reorder_and_remove_preserve_order() {
        let mut q = PartyQueue::new();
        let a = q.add(None, None, "a".into(), "".into(), "g".into(), Ready);
        let b = q.add(None, None, "b".into(), "".into(), "g".into(), Ready);
        let c = q.add(None, None, "c".into(), "".into(), "g".into(), Ready);

        // Move c to the front.
        assert!(q.reorder(&c, 0));
        assert_eq!(
            q.entries.iter().map(|e| e.id.clone()).collect::<Vec<_>>(),
            vec![c.clone(), a.clone(), b.clone()]
        );

        // Remove a; order of the rest holds.
        assert!(q.remove(&a));
        assert_eq!(
            q.entries.iter().map(|e| e.id.clone()).collect::<Vec<_>>(),
            vec![c, b]
        );
        // Removing an unknown id is a no-op false.
        assert!(!q.remove("nope"));
    }

    #[test]
    fn first_ready_and_playing_selectors() {
        let mut q = PartyQueue::new();
        let a = q.add(None, None, "a".into(), "".into(), "g".into(), Playing);
        let b = q.add(None, None, "b".into(), "".into(), "g".into(), Ready);
        let _c = q.add(None, None, "c".into(), "".into(), "g".into(), Ready);
        assert_eq!(q.playing(), Some(a));
        assert_eq!(q.first_ready(), Some(b));
    }

    // T3.2: persistence round-trips exactly, and writes atomically (no torn
    // file, no leftover temp).
    #[test]
    fn persistence_round_trip_is_atomic() {
        let dir = std::env::temp_dir().join(format!("party_q_test_{}", ID_COUNTER.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("party_queue.json");

        let mut q = PartyQueue::new();
        q.add(Some("https://x/y".into()), Some("hash1".into()), "Song".into(), "Artist".into(), "Bob".into(), Ready);
        q.add(None, None, "Two".into(), "".into(), "Sue".into(), Queued);

        q.save_to(&path).unwrap();

        // No temp file should linger after a successful save.
        assert!(!path.with_extension("json.tmp").exists(), "temp file was not renamed away");

        let loaded = PartyQueue::load_from(&path);
        assert_eq!(loaded, q, "round-trip must be exact");

        // A missing file loads as empty rather than erroring.
        let empty = PartyQueue::load_from(&dir.join("does_not_exist.json"));
        assert_eq!(empty, PartyQueue::default());

        std::fs::remove_dir_all(&dir).ok();
    }
}
