//! Local-folder media source. Walks a directory tree with `walkdir`, classifies
//! audio/video/USDX files, and feeds the results into the library DB. This is
//! a direct refactor of the original `scanner.rs` logic.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use tracing::warn;
use walkdir::WalkDir;

use crate::cache::CacheDir;
use crate::error::NightingaleError;
use crate::library_db;
use crate::song::{Song, build_song};
use crate::usdx;

use super::{MediaSource, SCAN_BATCH_SIZE, ScanContext, SourceKind, flush_batch};

const AUDIO_EXTENSIONS: &[&str] = &["mp3", "flac", "ogg", "opus", "wav", "m4a", "aac", "wma"];
const VIDEO_EXTENSIONS: &[&str] = &["mp4", "mkv", "avi", "webm", "mov", "m4v"];

#[derive(Debug, Clone, Copy)]
enum MediaKind {
    Audio,
    Video,
    Usdx,
}

pub struct FolderSource {
    root: PathBuf,
}

impl FolderSource {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }
}

impl MediaSource for FolderSource {
    fn kind(&self) -> SourceKind {
        SourceKind::Folder
    }

    fn label(&self) -> String {
        self.root.to_string_lossy().into_owned()
    }

    fn scan(&self, ctx: &ScanContext<'_>) -> Result<(), NightingaleError> {
        let media_files = collect_media_paths(&self.root);
        let folder_label = self.label();

        // Source-switch wipes are handled centrally in `scanner::start_scan`;
        // here we only need to prune rows whose paths disappeared between
        // scans of the same folder.
        let paths: Vec<String> = media_files
            .iter()
            .map(|(p, _)| p.to_string_lossy().into_owned())
            .collect();
        let _ = library_db::delete_songs_not_in_paths(&paths);
        let _ = library_db::update_library_meta(&folder_label, media_files.len());

        let already_processed: HashSet<String> =
            library_db::load_song_path_strings().unwrap_or_default();

        let pending: Vec<_> = media_files
            .into_iter()
            .filter(|(p, _)| !already_processed.contains(&p.to_string_lossy().into_owned()))
            .collect();

        let mut batch: Vec<Song> = Vec::new();
        let generation = ctx.generation;

        for (i, (path, kind)) in pending.iter().enumerate() {
            if !library_db::scan_generation_is_current(generation) {
                return Ok(());
            }
            let result = match kind {
                MediaKind::Audio => build_song(path, ctx.cache, false),
                MediaKind::Video => build_song(path, ctx.cache, true),
                MediaKind::Usdx => usdx::build_usdx_song(path, ctx.cache),
            };
            match result {
                Ok(song) => batch.push(song),
                Err(e) => {
                    warn!("Failed to process {}: {e}", path.display());
                }
            }
            if (i + 1) % SCAN_BATCH_SIZE == 0 {
                flush_batch(&mut batch, generation);
            }
        }

        flush_batch(&mut batch, generation);
        Ok(())
    }

    fn ensure_local_media(
        &self,
        song: &Song,
        _cache: &CacheDir,
    ) -> Result<PathBuf, NightingaleError> {
        Ok(song.path.clone())
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
