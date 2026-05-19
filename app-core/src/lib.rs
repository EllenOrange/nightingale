mod analyzer;
mod cache;
mod config;
mod error;
mod library_model;
mod library_menu;
mod lyrics;
pub mod media_server;
mod playback;
mod profile;
mod scanner;
mod song;
mod library_db;
mod usdx;
mod vendor;
mod vendor_scripts;

pub use analyzer::{
    AnalysisQueue, delete_cache, enqueue_all, enqueue_one, reanalyze_full, reanalyze_transcript,
    shutdown_server,
};
pub use lyrics::{
    LrclibCandidate, LyricsFile, load_lyrics_file, save_lyrics_and_realign, search_lrclib_for_hash,
};
pub use cache::{
    CacheDir, CacheStats, change_app_data_path, clear_models, clear_videos,
    default_nightingale_dir, nightingale_dir,
};
pub use playback::{
    AudioPaths, ShiftResult, download_pixabay_videos, ensure_mp3_stems,
    ensure_playable_source_video, get_audio_paths, get_cached_pixabay_videos, load_transcript,
    prefetch_one_per_flavor, shift_key, shift_tempo,
};
pub use config::AppConfig;
pub use profile::ProfileStore;
pub use library_model::{LibraryMenuFilters, LoadSongsParams, SongsMeta, SongsStore};
pub use library_menu::{LibraryMenuItem, LibraryMenuItems, load_library_menu_items};
pub use scanner::start_scan;
pub use library_db::{init_library, library_db_path};
pub use vendor::{
    clear_vendor_dir, is_ready, mark_ready, refresh_analyzer_scripts_if_ready, step_create_venv,
    step_download_ffmpeg, step_download_uv, step_extract_scripts, step_install_packages,
    step_install_python,
};
