//! Pluggable library backends ("media sources").
//!
//! Each source knows how to enumerate songs for the library and how to make a
//! local file appear on disk when the analyzer needs to read bytes. The folder
//! source is the original "Select folder" behavior; the Jellyfin source talks
//! to a remote server over HTTP. The trait is sized so Navidrome/Subsonic and
//! friends can be added without touching downstream code.

use std::path::PathBuf;

use crate::cache::CacheDir;
use crate::config::{AppConfig, LibrarySource};
use crate::error::NightingaleError;
use crate::song::Song;

pub mod folder;
pub mod jellyfin;

pub use folder::FolderSource;
pub use jellyfin::{JellyfinAuth, JellyfinSource};

/// Coarse-grained discriminator surfaced to the UI / commands layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    Folder,
    Jellyfin,
}

/// Context passed to a source while it is running a scan. Implementations should
/// poll `is_current_generation` periodically and stop emitting writes once it
/// turns false — the user has triggered a new scan or switched sources.
pub struct ScanContext<'a> {
    pub generation: u64,
    pub cache: &'a CacheDir,
}

pub trait MediaSource: Send + Sync {
    fn kind(&self) -> SourceKind;

    /// Human-readable label that ends up in `library_meta.folder` and the UI.
    fn label(&self) -> String;

    /// Run a full library scan. Implementations are responsible for:
    /// - flushing songs to `library_db` in batches
    /// - removing entries that no longer exist upstream
    /// - updating `library_meta` with the active label + total count
    /// - bailing out when the scan generation has been bumped
    fn scan(&self, ctx: &ScanContext<'_>) -> Result<(), NightingaleError>;

    /// Make sure the song's source audio is present on disk and return a path
    /// the analyzer (ffmpeg + Python) can read. For `LocalFile` origins this
    /// just hands `song.path` back; remote sources download to cache.
    fn ensure_local_audio(
        &self,
        song: &Song,
        cache: &CacheDir,
    ) -> Result<PathBuf, NightingaleError>;
}

/// Resolve the configured library source, if any.
pub fn active_source() -> Result<Option<Box<dyn MediaSource>>, NightingaleError> {
    active_source_from_config(&AppConfig::load())
}

pub fn active_source_from_config(
    config: &AppConfig,
) -> Result<Option<Box<dyn MediaSource>>, NightingaleError> {
    let Some(src) = config.library_source.clone() else {
        return Ok(None);
    };
    match src {
        LibrarySource::Folder { path } => Ok(Some(Box::new(FolderSource::new(path)))),
        LibrarySource::Jellyfin {
            base_url,
            user_id,
            username: _,
            access_token,
            device_id,
        } => {
            let auth = JellyfinAuth {
                base_url,
                user_id,
                access_token,
                device_id,
            };
            Ok(Some(Box::new(JellyfinSource::new(auth))))
        }
    }
}
