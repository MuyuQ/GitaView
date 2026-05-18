use crate::domain::settings::AppSettings;
use tauri::Manager;

pub fn settings_path(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|err| err.to_string())?
        .join("settings.json"))
}

pub fn load_app_settings(app: &tauri::AppHandle) -> Result<AppSettings, String> {
    crate::storage::store::load_settings(&settings_path(app)?)
}

pub fn save_app_settings(
    app: &tauri::AppHandle,
    settings: &AppSettings,
) -> Result<AppSettings, String> {
    crate::storage::store::save_settings(&settings_path(app)?, settings)
}
