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
    let path = settings_path(app)?;
    let redacted_path = crate::diagnostics::redact_path(&path);
    crate::diagnostics::log("settings.load.start", &redacted_path);
    let result = crate::storage::store::load_settings(&path);
    match &result {
        Ok(settings) => crate::diagnostics::log(
            "settings.load.ok",
            format!("path={redacted_path} repos={}", settings.repos.len()),
        ),
        Err(err) => crate::diagnostics::log(
            "settings.load.error",
            format!("path={redacted_path} error_len={}", err.len()),
        ),
    }
    result
}

pub fn save_app_settings(
    app: &tauri::AppHandle,
    settings: &AppSettings,
) -> Result<AppSettings, String> {
    let path = settings_path(app)?;
    let redacted_path = crate::diagnostics::redact_path(&path);
    crate::diagnostics::log(
        "settings.save.start",
        format!("path={redacted_path} repos={}", settings.repos.len()),
    );
    let result = crate::storage::store::save_settings(&path, settings);
    match &result {
        Ok(saved) => crate::diagnostics::log(
            "settings.save.ok",
            format!("path={redacted_path} repos={}", saved.repos.len()),
        ),
        Err(err) => crate::diagnostics::log(
            "settings.save.error",
            format!("path={redacted_path} error_len={}", err.len()),
        ),
    }
    result
}
