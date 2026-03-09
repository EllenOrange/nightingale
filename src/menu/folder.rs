use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use bevy::prelude::*;

use crate::analyzer::AnalysisQueue;
use crate::scanner::metadata::{AnalysisStatus, Song, SongLibrary};
use crate::states::AppState;

#[derive(Resource)]
pub struct PendingFolderPick {
    pub result: Arc<Mutex<Option<Option<PathBuf>>>>,
}

#[derive(Resource)]
pub struct PendingRescan {
    pub result: Arc<Mutex<Option<Vec<Song>>>>,
}

pub fn poll_folder_result(
    mut commands: Commands,
    pending: Option<Res<PendingFolderPick>>,
    mut next_state: ResMut<NextState<AppState>>,
    mut config: ResMut<crate::config::AppConfig>,
    mut queue: ResMut<AnalysisQueue>,
) {
    let Some(pending) = pending else { return };

    let lock = pending.result.lock().unwrap();
    let Some(ref maybe_folder) = *lock else { return };
    let folder = maybe_folder.clone();
    drop(lock);
    commands.remove_resource::<PendingFolderPick>();

    if let Some(folder) = folder {
        info!("Selected folder: {}", folder.display());
        commands.insert_resource(SongLibrary { songs: vec![] });
        queue.queue.clear();
        queue.active = None;
        commands.insert_resource(crate::scanner::ScanRequest {
            folder: folder.clone(),
        });
        config.last_folder = Some(folder);
        config.save();
        next_state.set(AppState::Scanning);
    }
}

pub fn poll_rescan(
    mut commands: Commands,
    pending: Option<Res<PendingRescan>>,
    mut next_state: ResMut<NextState<AppState>>,
    mut library: ResMut<SongLibrary>,
    mut queue: ResMut<AnalysisQueue>,
) {
    let Some(pending) = pending else { return };

    let lock = pending.result.lock().unwrap();
    let Some(ref new_songs) = *lock else { return };

    let mut status_by_hash: std::collections::HashMap<String, AnalysisStatus> =
        std::collections::HashMap::new();
    for song in &library.songs {
        match &song.analysis_status {
            AnalysisStatus::Queued | AnalysisStatus::Analyzing => {
                status_by_hash.insert(song.file_hash.clone(), song.analysis_status.clone());
            }
            _ => {}
        }
    }

    let old_active_hash = queue
        .active
        .as_ref()
        .and_then(|a| library.songs.get(a.song_index))
        .map(|s| s.file_hash.clone());

    let old_queued_hashes: Vec<String> = queue
        .queue
        .iter()
        .filter_map(|&idx| library.songs.get(idx))
        .map(|s| s.file_hash.clone())
        .collect();

    let mut merged = new_songs.clone();
    for song in &mut merged {
        if let Some(status) = status_by_hash.get(&song.file_hash) {
            song.analysis_status = status.clone();
        }
    }

    let hash_to_new_idx: std::collections::HashMap<&str, usize> = merged
        .iter()
        .enumerate()
        .map(|(i, s)| (s.file_hash.as_str(), i))
        .collect();

    if let Some(ref mut active) = queue.active {
        if let Some(ref old_hash) = old_active_hash {
            if let Some(&new_idx) = hash_to_new_idx.get(old_hash.as_str()) {
                active.song_index = new_idx;
            }
        }
    }

    let mut new_queue = std::collections::VecDeque::new();
    for hash in &old_queued_hashes {
        if let Some(&new_idx) = hash_to_new_idx.get(hash.as_str()) {
            new_queue.push_back(new_idx);
        }
    }
    queue.queue = new_queue;

    library.songs = merged;

    drop(lock);
    commands.remove_resource::<PendingRescan>();

    next_state.set(AppState::Menu);
}
