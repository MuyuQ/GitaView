use crate::domain::settings::AppSettings;
use std::sync::Mutex;
use tauri::Manager;

/// 串行化所有设置读-改-写序列：
/// 两个并发变更命令（如设置保存与添加仓库）交错 load/save 时会互相覆盖，
/// 该锁保证整个"读取 → 修改 → 保存"序列的原子性。
static SETTINGS_MUTATION_LOCK: Mutex<()> = Mutex::new(());

pub fn check_settings_snapshot(
    current: &AppSettings,
    expected: &AppSettings,
) -> Result<(), String> {
    // Compare persisted fields only; legacy fields are read but intentionally not serialized.
    let current = serde_json::to_value(current).map_err(|err| err.to_string())?;
    let expected =
        serde_json::to_value(expected.clone().normalized()).map_err(|err| err.to_string())?;
    if current != expected {
        return Err("SETTINGS_CONFLICT".to_string());
    }
    Ok(())
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignored_legacy_fields_do_not_cause_permanent_conflicts() {
        let mut value = serde_json::to_value(AppSettings::default()).unwrap();
        value["appearance"]["compactMode"] = true.into();
        let current: AppSettings = serde_json::from_value(value).unwrap();
        let expected = serde_json::from_value(serde_json::to_value(&current).unwrap()).unwrap();
        assert!(check_settings_snapshot(&current, &expected).is_ok());
    }

    #[test]
    fn rejects_a_snapshot_taken_before_a_repository_addition() {
        let expected = AppSettings::default();
        let mut current = expected.clone();
        current.repos.push(crate::domain::repo::RepoRecord {
            id: "new".into(),
            name: "new".into(),
            path: "/new".into(),
            group: "全部分组".into(),
        });
        assert_eq!(
            check_settings_snapshot(&current, &expected),
            Err("SETTINGS_CONFLICT".into())
        );
        assert_eq!(current.repos.len(), 1);
        assert!(check_settings_snapshot(&expected, &expected).is_ok());
    }
}
