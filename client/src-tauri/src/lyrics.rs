use app_core::{
    load_lyrics_file, save_lyrics_and_realign, search_lrclib_for_hash, LrclibCandidate, LyricsFile,
};

#[tauri::command]
pub fn load_lyrics(file_hash: String) -> Option<LyricsFile> {
    load_lyrics_file(&file_hash)
}

#[tauri::command]
pub async fn search_lrclib_lyrics(file_hash: String) -> Vec<LrclibCandidate> {
    tauri::async_runtime::spawn_blocking(move || search_lrclib_for_hash(&file_hash))
        .await
        .unwrap_or_default()
}

#[tauri::command]
pub fn save_lyrics(file_hash: String, lines: Vec<String>) -> Result<(), String> {
    save_lyrics_and_realign(&file_hash, lines)
}
