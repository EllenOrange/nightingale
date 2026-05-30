use app_core::{
    AnalysisQueue, AppConfig, JellyfinHealth, JellyfinLoginResult, LibraryMenuItems, LibrarySource,
    LoadSongsParams, NavidromeHealth, NavidromeLoginResult, SongsMeta, SongsStore,
};

#[tauri::command]
pub fn trigger_scan() {
    app_core::start_scan();
}

#[tauri::command]
pub fn set_library_source(source: LibrarySource) -> AppConfig {
    let mut config = AppConfig::load();
    config.library_source = Some(source);
    config.last_folder = None;
    config.save();
    app_core::start_scan();
    config
}

#[tauri::command]
pub fn clear_library_source() -> AppConfig {
    let mut config = AppConfig::load();
    config.library_source = None;
    config.last_folder = None;
    config.save();
    config
}

#[tauri::command]
pub fn jellyfin_login(
    base_url: String,
    username: String,
    password: String,
) -> Result<JellyfinLoginResult, String> {
    app_core::jellyfin_login(&base_url, &username, &password, None).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn jellyfin_ping() -> JellyfinHealth {
    app_core::jellyfin_ping_current()
}

#[tauri::command]
pub fn navidrome_login(
    base_url: String,
    username: String,
    password: String,
) -> Result<NavidromeLoginResult, String> {
    app_core::navidrome_login(&base_url, &username, &password).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn navidrome_ping() -> NavidromeHealth {
    app_core::navidrome_ping_current()
}

#[tauri::command]
pub fn load_songs(params: LoadSongsParams) -> SongsStore {
    SongsStore::load(&params)
}

#[tauri::command]
pub fn load_songs_meta() -> SongsMeta {
    SongsStore::load_meta()
}

#[tauri::command]
pub fn load_analysis_queue() -> AnalysisQueue {
    AnalysisQueue::load()
}

#[tauri::command]
pub fn load_library_menu_items() -> Result<LibraryMenuItems, String> {
    app_core::load_library_menu_items().map_err(|e| e.to_string())
}
