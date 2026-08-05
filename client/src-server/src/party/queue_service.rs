//! The live queue service: shared store, HTTP routes, the ingest worker, and
//! auto-advance. The pure queue model lives in `queue.rs`; this module wires it
//! to the event bus, the ingest pipeline, and remote playback.

use std::sync::Mutex;

use serde::Deserialize;
use serde_json::{json, Value};

use crate::commands::{ApiError, CmdResult};
use crate::party::ingest;
use crate::party::playback::issue_play;
use crate::party::queue::{PartyQueue, QueueStatus};
use crate::state::AppState;

/// Event name for every queue broadcast. The guest/admin pages subscribe to it.
const QUEUE_EVENT: &str = "party.queue";

/// Shared, persisted queue plus the ingest-worker running flag, behind one lock.
pub struct PartyQueueStore {
    inner: Mutex<Inner>,
}

struct Inner {
    queue: PartyQueue,
    worker_running: bool,
}

impl Default for PartyQueueStore {
    fn default() -> Self {
        Self::new()
    }
}

impl PartyQueueStore {
    pub fn new() -> Self {
        let mut queue = PartyQueue::load();
        sanitize_on_load(&mut queue);
        queue.save();
        Self {
            inner: Mutex::new(Inner {
                queue,
                worker_running: false,
            }),
        }
    }

    pub fn snapshot(&self) -> PartyQueue {
        self.inner.lock().unwrap().queue.clone()
    }

    /// Apply `f` to the queue, persist, and return `(f's result, snapshot)`.
    /// The lock is never held across an await (callers broadcast afterwards).
    pub fn mutate<R>(&self, f: impl FnOnce(&mut PartyQueue) -> R) -> (R, PartyQueue) {
        let mut guard = self.inner.lock().unwrap();
        let result = f(&mut guard.queue);
        guard.queue.save();
        (result, guard.queue.clone())
    }

    /// If the ingest worker is idle and there is queued work, mark it running
    /// and return true (the caller then spawns the worker task).
    fn try_acquire_worker(&self) -> bool {
        let mut guard = self.inner.lock().unwrap();
        if !guard.worker_running && guard.queue.first_queued().is_some() {
            guard.worker_running = true;
            true
        } else {
            false
        }
    }

    /// Called by the worker when it runs out of work. Returns true if new work
    /// arrived in the race window (keep looping); false after clearing the flag.
    fn keep_worker_running(&self) -> bool {
        let mut guard = self.inner.lock().unwrap();
        if guard.queue.first_queued().is_some() {
            true
        } else {
            guard.worker_running = false;
            false
        }
    }
}

/// Reset states that only the running server can exit, so a crash mid-flight
/// does not leave an entry stuck forever. In-flight ingest restarts; a song
/// that was "playing" when we died drops back to ready so it can replay.
fn sanitize_on_load(queue: &mut PartyQueue) {
    for entry in &mut queue.entries {
        entry.status = match entry.status {
            QueueStatus::Downloading | QueueStatus::Analyzing => QueueStatus::Queued,
            QueueStatus::Playing => QueueStatus::Ready,
            other => other,
        };
    }
}

fn broadcast(state: &AppState, snapshot: &PartyQueue) {
    state.events.emit(QUEUE_EVENT, snapshot);
}

// ── Routes ───────────────────────────────────────────────────────────────

pub async fn party_queue_list(state: &AppState) -> CmdResult {
    serde_json::to_value(state.party_queue.snapshot()).map_err(serialise_err)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AddArgs {
    /// A YouTube URL / search string. Mutually exclusive-ish with `file_hash`;
    /// if both are given, `file_hash` wins (it's an existing library song).
    #[serde(default)]
    query: Option<String>,
    #[serde(default)]
    file_hash: Option<String>,
    #[serde(default)]
    requested_by: Option<String>,
}

pub async fn party_queue_add(state: &AppState, payload: Value) -> CmdResult {
    let args: AddArgs = deserialize(payload)?;
    let requested_by = args
        .requested_by
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Guest".to_string());

    // Resolve what we can up front so the queue entry shows a real title.
    let (query, file_hash, title, artist, status) = if let Some(hash) = args
        .file_hash
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        // Existing library song.
        let song = app_core::song_by_hash(hash);
        let (title, artist, analyzed) = match &song {
            Some(s) => (s.title.clone(), s.artist.clone(), s.is_analyzed),
            None => {
                return Err(ApiError(
                    axum::http::StatusCode::NOT_FOUND,
                    format!("unknown song hash {hash}"),
                ))
            }
        };
        let status = if analyzed {
            QueueStatus::Ready
        } else {
            QueueStatus::Queued
        };
        (None, Some(hash.to_string()), title, artist, status)
    } else if let Some(q) = args
        .query
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        // YouTube query: title starts as the query, refined once resolved.
        (Some(q.to_string()), None, q.to_string(), String::new(), QueueStatus::Queued)
    } else {
        return Err(ApiError(
            axum::http::StatusCode::BAD_REQUEST,
            "party_queue_add needs a query or fileHash".to_string(),
        ));
    };

    let (id, snapshot) = state.party_queue.mutate(|q| {
        q.add(query, file_hash, title, artist, requested_by, status)
    });
    broadcast(state, &snapshot);

    // A newly ready library song may be playable immediately; otherwise kick
    // the ingest worker.
    maybe_start_next(state).await;
    ensure_worker(state);

    let entry = snapshot.get(&id).cloned();
    serde_json::to_value(json!({ "id": id, "entry": entry })).map_err(serialise_err)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IdArgs {
    id: String,
}

pub async fn party_queue_remove(state: &AppState, payload: Value) -> CmdResult {
    let args: IdArgs = deserialize(payload)?;
    let (removed, snapshot) = state.party_queue.mutate(|q| q.remove(&args.id));
    if removed {
        broadcast(state, &snapshot);
    }
    serde_json::to_value(json!({ "removed": removed })).map_err(serialise_err)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReorderArgs {
    id: String,
    position: usize,
}

pub async fn party_queue_clear(state: &AppState) -> CmdResult {
    let (_, snapshot) = state.party_queue.mutate(|q| q.entries.clear());
    broadcast(state, &snapshot);
    Ok(json!({ "cleared": true }))
}

pub async fn party_queue_reorder(state: &AppState, payload: Value) -> CmdResult {
    let args: ReorderArgs = deserialize(payload)?;
    let (ok, snapshot) = state
        .party_queue
        .mutate(|q| q.reorder(&args.id, args.position));
    if ok {
        broadcast(state, &snapshot);
    }
    serde_json::to_value(json!({ "reordered": ok })).map_err(serialise_err)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SongEndedArgs {
    #[serde(default)]
    file_hash: Option<String>,
}

/// The TV reports the current song finished. Mark the matching playing entry
/// done and advance to the next ready song. The hash guard prevents a stale
/// end (from a song we already moved past) from double-advancing.
pub async fn party_song_ended(state: &AppState, payload: Value) -> CmdResult {
    let args: SongEndedArgs = deserialize(payload)?;
    let (changed, snapshot) = state.party_queue.mutate(|q| {
        let Some(playing_id) = q.playing() else {
            return false;
        };
        // If a hash was supplied, only advance when it matches the playing entry.
        if let Some(hash) = &args.file_hash {
            let matches = q
                .get(&playing_id)
                .and_then(|e| e.file_hash.as_deref())
                .map(|h| h == hash)
                .unwrap_or(false);
            if !matches {
                return false;
            }
        }
        q.set_status(&playing_id, QueueStatus::Done)
    });
    if changed {
        broadcast(state, &snapshot);
        maybe_start_next(state).await;
    }
    Ok(json!({ "advanced": changed }))
}

/// Skip the current song (admin): mark it done and advance. Used in Step 4 too.
pub async fn party_skip(state: &AppState) -> CmdResult {
    let (changed, snapshot) = state.party_queue.mutate(|q| {
        if let Some(playing_id) = q.playing() {
            q.set_status(&playing_id, QueueStatus::Done)
        } else {
            false
        }
    });
    if changed {
        broadcast(state, &snapshot);
    }
    maybe_start_next(state).await;
    Ok(json!({ "skipped": changed }))
}

// ── Auto-advance ───────────────────────────────────────────────────────────

/// If nothing is playing and a ready entry exists, promote it to playing and
/// issue the remote play. No-op while a song is already playing.
async fn maybe_start_next(state: &AppState) {
    // Decide and mutate atomically-ish: grab the ready id only if idle.
    let (to_play, snapshot) = state.party_queue.mutate(|q| {
        if q.playing().is_some() {
            return None;
        }
        let Some(ready_id) = q.first_ready() else {
            return None;
        };
        // A ready entry must have a hash; guard anyway.
        let hash = q.get(&ready_id).and_then(|e| e.file_hash.clone());
        if hash.is_none() {
            return None;
        }
        q.set_status(&ready_id, QueueStatus::Playing);
        hash
    });

    if let Some(hash) = to_play {
        broadcast(state, &snapshot);
        issue_play(state, hash).await;
    }
}

// ── Ingest worker ──────────────────────────────────────────────────────────

/// Spawn the ingest worker if it is idle and there is queued work.
fn ensure_worker(state: &AppState) {
    if state.party_queue.try_acquire_worker() {
        let state = state.clone();
        tokio::spawn(async move {
            run_worker(&state).await;
        });
    }
}

/// Drain queued entries one at a time (serial: analysis concurrency is 1). The
/// blocking download/analysis runs on a blocking thread so the async runtime
/// stays free to serve the queue's live updates.
async fn run_worker(state: &AppState) {
    loop {
        let Some(id) = state.party_queue.snapshot().first_queued() else {
            if state.party_queue.keep_worker_running() {
                continue;
            }
            break;
        };

        process_entry(state, &id).await;

        if !state.party_queue.keep_worker_running() {
            break;
        }
    }
}

/// Take one queued entry through ingest to ready (or error), broadcasting each
/// transition. Then try to start playback in case this is the first song.
async fn process_entry(state: &AppState, id: &str) {
    let Some(entry) = state.party_queue.snapshot().get(id).cloned() else {
        return;
    };

    // Case 1: an existing library song, already analyzed -> ready immediately.
    if let Some(hash) = &entry.file_hash {
        if app_core::song_by_hash(hash)
            .map(|s| s.is_analyzed)
            .unwrap_or(false)
        {
            set_status(state, id, QueueStatus::Ready);
            maybe_start_next(state).await;
            return;
        }
    }

    // Case 2: a library song that needs analysis (hash known, no download).
    if let Some(hash) = entry.file_hash.clone() {
        set_status(state, id, QueueStatus::Analyzing);
        let outcome = tokio::task::spawn_blocking(move || {
            app_core::enqueue_one(&hash);
            ingest::wait_for_analysis(&hash)
        })
        .await;
        finish_after_analysis(state, id, entry.file_hash.clone(), outcome).await;
        return;
    }

    // Case 3: a YouTube query -> download + scan + analyze.
    let Some(query) = entry.query.clone() else {
        set_error(state, id, "queue entry has neither file hash nor query");
        return;
    };

    set_status(state, id, QueueStatus::Downloading);
    let dl = tokio::task::spawn_blocking(move || {
        let lib = ingest::folder_library_dir()
            .ok_or_else(|| "party ingest requires a Folder library source".to_string())?;
        let path = ingest::download(&query, &lib)?;
        ingest::scan_for_path(&path)
    })
    .await;

    let hash = match dl {
        Ok(Ok(hash)) => hash,
        Ok(Err(e)) => {
            set_error(state, id, format!("download/scan failed: {e}"));
            return;
        }
        Err(e) => {
            set_error(state, id, format!("ingest task crashed: {e}"));
            return;
        }
    };

    // Already analyzed (idempotent re-add of a known song)?
    if app_core::song_by_hash(&hash)
        .map(|s| s.is_analyzed)
        .unwrap_or(false)
    {
        resolve_and_ready(state, id, &hash).await;
        return;
    }

    set_status(state, id, QueueStatus::Analyzing);
    let analyze_hash = hash.clone();
    let outcome = tokio::task::spawn_blocking(move || {
        app_core::enqueue_one(&analyze_hash);
        ingest::wait_for_analysis(&analyze_hash)
    })
    .await;
    finish_after_analysis(state, id, Some(hash), outcome).await;
}

/// Common tail: interpret a `wait_for_analysis` result and either mark ready
/// (recording resolved metadata) or error.
async fn finish_after_analysis(
    state: &AppState,
    id: &str,
    hash: Option<String>,
    outcome: Result<Result<(), String>, tokio::task::JoinError>,
) {
    match outcome {
        Ok(Ok(())) => {
            if let Some(hash) = hash {
                resolve_and_ready(state, id, &hash).await;
            } else {
                set_status(state, id, QueueStatus::Ready);
                maybe_start_next(state).await;
            }
        }
        Ok(Err(e)) => set_error(state, id, e),
        Err(e) => set_error(state, id, format!("analysis task crashed: {e}")),
    }
}

/// Record the resolved song identity, mark ready, and try to start playback.
async fn resolve_and_ready(state: &AppState, id: &str, hash: &str) {
    let song = app_core::song_by_hash(hash);
    let (_, snapshot) = state.party_queue.mutate(|q| {
        // Keep the existing title/artist if the song row somehow vanished.
        let (title, artist) = match &song {
            Some(s) => (s.title.clone(), s.artist.clone()),
            None => q
                .get(id)
                .map(|e| (e.title.clone(), e.artist.clone()))
                .unwrap_or_default(),
        };
        q.set_resolved(id, hash.to_string(), title, artist);
        q.set_status(id, QueueStatus::Ready);
    });
    broadcast(state, &snapshot);
    maybe_start_next(state).await;
}

fn set_status(state: &AppState, id: &str, status: QueueStatus) {
    let (_, snapshot) = state.party_queue.mutate(|q| q.set_status(id, status));
    broadcast(state, &snapshot);
}

fn set_error(state: &AppState, id: &str, message: impl Into<String>) {
    let (_, snapshot) = state.party_queue.mutate(|q| q.set_error(id, message));
    broadcast(state, &snapshot);
}

fn deserialize<T: for<'de> Deserialize<'de>>(value: Value) -> Result<T, ApiError> {
    serde_json::from_value(value)
        .map_err(|e| ApiError(axum::http::StatusCode::BAD_REQUEST, format!("invalid args: {e}")))
}

fn serialise_err(e: serde_json::Error) -> ApiError {
    ApiError(
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        format!("serialise: {e}"),
    )
}
