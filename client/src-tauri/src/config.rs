use app_core::AppConfig;

use crate::microphones::set_monitor_gain;

#[tauri::command]
pub fn load_config() -> AppConfig {
    AppConfig::load()
}

#[tauri::command]
pub fn save_config(config: AppConfig) -> AppConfig {
    config.save();
    set_monitor_gain(config.mic_monitor_gain());
    config
}
