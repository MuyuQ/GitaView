use crate::domain::settings::AppSettings;
use std::sync::Mutex;
use tauri::Manager;

/// 串行化所有设置读-改-写序列：
/// 两个并发变更命令（如设置保存与添加仓库）交错 load/save 时会互相覆盖，
/// 该锁保证整个"读取 → 修改 → 保存"序列的原子性。
static SETTINGS_MUTATION_LOCK: Mutex<()> = Mutex::new(());

pub fn settings_path(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|err| err.to_string())?
        .join("settings.json"))
}

/// 在锁保护下完成一次设置的读-改-写，返回保存后的（normalized）设置。
/// mutate 闭包内不得再次调用 load/save。
pub fn mutate_app_settings(
    app: &tauri::AppHandle,
    mutate: impl FnOnce(&mut AppSettings) -> Result<(), String>,
) -> Result<AppSettings, String> {
    let _guard = SETTINGS_MUTATION_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut settings = load_app_settings(app)?;
    mutate(&mut settings)?;
    save_app_settings(app, &settings)
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
