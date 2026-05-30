use app_core::AppConfig;

use crate::microphones::set_monitor_gain;

#[tauri::command]
pub fn load_config() -> AppConfig {
    AppConfig::load()
}

#[tauri::command]
pub fn save_config(config: AppConfig) -> AppConfig {
    let was_auto_analyze = AppConfig::load().auto_analyze();
    config.save();
    set_monitor_gain(config.mic_monitor_gain());
    if config.auto_analyze() && !was_auto_analyze {
        app_core::enqueue_all(&app_core::LibraryMenuFilters::default());
    }
    config
}
