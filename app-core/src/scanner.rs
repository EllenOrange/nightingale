use std::collections::HashSet;
use std::path::{Path, PathBuf};

use tracing::warn;
use walkdir::WalkDir;

use crate::{
    cache::CacheDir,
    library_db,
    library_model::{LoadSongsParams, SongsMeta, SongsStore},
    song::{Song, build_song},
    usdx,
};

const AUDIO_EXTENSIONS: &[&str] = &["mp3", "flac", "ogg", "wav", "m4a", "aac", "wma"];
const VIDEO_EXTENSIONS: &[&str] = &["mp4", "mkv", "avi", "webm", "mov", "m4v"];

const SCAN_SAVE_BATCH_SIZE: usize = 25;

#[derive(Debug, Clone, Copy)]
enum MediaKind {
    Audio,
    Video,
    Usdx,
}

impl SongsStore {
    pub fn load_all() -> Self {
        let processed = library_db::load_all_songs().unwrap_or_default();
        let (folder, count) = library_db::read_library_meta().unwrap_or((String::new(), 0));
        let processed_count = processed.len();
        SongsStore {
            count,
            folder,
            processed,
            processed_count,
        }
    }

    pub fn load(params: &LoadSongsParams) -> Self {
        library_db::load_songs_page(params).unwrap_or_else(|_| SongsStore {
            count: 0,
            folder: String::new(),
            processed: Vec::new(),
            processed_count: 0,
        })
    }

    pub fn load_meta() -> SongsMeta {
        library_db::load_meta_sql().unwrap_or_default()
    }
}

fn classify_media_file(path: &Path) -> Option<MediaKind> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());

    let ext_str = ext.as_deref()?;

    if AUDIO_EXTENSIONS.contains(&ext_str) {
        Some(MediaKind::Audio)
    } else if VIDEO_EXTENSIONS.contains(&ext_str) {
        Some(MediaKind::Video)
    } else if ext_str == "usdx" {
        Some(MediaKind::Usdx)
    } else if ext_str == "txt" && usdx::looks_like_usdx(path) {
        Some(MediaKind::Usdx)
    } else {
        None
    }
}

fn collect_media_paths(folder: &Path) -> Vec<(PathBuf, MediaKind)> {
    let mut paths: Vec<(PathBuf, MediaKind)> = WalkDir::new(folder)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .filter_map(|e| {
            let kind = classify_media_file(e.path())?;
            Some((e.path().to_path_buf(), kind))
        })
        .collect();

    let claimed: HashSet<PathBuf> = paths
        .iter()
        .filter_map(|(p, kind)| matches!(kind, MediaKind::Usdx).then(|| p.clone()))
        .filter_map(|usdx_path| usdx::read_siblings(&usdx_path))
        .flat_map(|s| {
            [Some(s.audio), s.vocals, s.instrumental, s.video]
                .into_iter()
                .flatten()
        })
        .collect();

    paths.retain(|(p, kind)| matches!(kind, MediaKind::Usdx) || !claimed.contains(p));
    paths
}

fn flush_batch(batch: &mut Vec<Song>, generation: u64) {
    let _ = library_db::append_songs_for_scan(batch, generation);
    batch.clear();
}

pub fn start_scan(folder: &Path) {
    let scan_generation = library_db::bump_scan_generation();
    let media_files = collect_media_paths(folder);
    let folder_str = folder.to_string_lossy().into_owned();
    let (existing_folder, _) = library_db::read_library_meta().unwrap_or((String::new(), 0));
    let same_folder = existing_folder == folder_str;

    if same_folder {
        let paths: Vec<String> = media_files
            .iter()
            .map(|(p, _)| p.to_string_lossy().into_owned())
            .collect();
        let _ = library_db::delete_songs_not_in_paths(&paths);
        let _ = library_db::update_library_meta(&folder_str, media_files.len());
    } else {
        let _ = library_db::replace_all_songs_sorted(&[]);
        let _ = library_db::update_library_meta(&folder_str, media_files.len());
    }

    let already_processed: HashSet<String> =
        library_db::load_song_path_strings().unwrap_or_default();

    let pending: Vec<_> = media_files
        .into_iter()
        .filter(|(p, _)| !already_processed.contains(&p.to_string_lossy().into_owned()))
        .collect();

    std::thread::spawn(move || {
        let cache = CacheDir::new();

        let mut batch: Vec<Song> = Vec::new();

        for (i, (path, kind)) in pending.iter().enumerate() {
            if !library_db::scan_generation_is_current(scan_generation) {
                return;
            }
            let result = match kind {
                MediaKind::Audio => build_song(path, &cache, false),
                MediaKind::Video => build_song(path, &cache, true),
                MediaKind::Usdx => usdx::build_usdx_song(path, &cache),
            };
            match result {
                Ok(song) => batch.push(song),
                Err(e) => {
                    warn!("Failed to process {}: {e}", path.display());
                }
            }
            if (i + 1) % SCAN_SAVE_BATCH_SIZE == 0 && !batch.is_empty() {
                flush_batch(&mut batch, scan_generation);
            }
        }

        if !batch.is_empty() {
            flush_batch(&mut batch, scan_generation);
        }
    });
}
