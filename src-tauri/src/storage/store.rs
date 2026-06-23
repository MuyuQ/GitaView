use std::fs;
use std::path::{Path, PathBuf};

use crate::domain::settings::AppSettings;

pub fn load_settings(path: &Path) -> Result<AppSettings, String> {
    if !path.exists() {
        return Ok(AppSettings::default());
    }
    let text = fs::read_to_string(path).map_err(|err| err.to_string())?;
    serde_json::from_str::<AppSettings>(&text)
        .map(|settings| settings.normalized())
        .map_err(|err| err.to_string())
}

pub fn save_settings(path: &Path, settings: &AppSettings) -> Result<AppSettings, String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let normalized = settings.clone().normalized();
    let text = serde_json::to_string_pretty(&normalized).map_err(|err| err.to_string())?;
    let temp_path = settings_temp_path(path);
    let _ = fs::remove_file(&temp_path);
    fs::write(&temp_path, text).map_err(|err| err.to_string())?;
    replace_file(&temp_path, path)?;
    Ok(normalized)
}

fn settings_temp_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("settings.json");
    path.with_file_name(format!(".{file_name}.tmp"))
}

#[cfg(target_os = "windows")]
fn replace_file(source: &Path, destination: &Path) -> Result<(), String> {
    use windows::core::HSTRING;
    use windows::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };

    let source = HSTRING::from(source.to_string_lossy().as_ref());
    let destination = HSTRING::from(destination.to_string_lossy().as_ref());
    unsafe {
        MoveFileExW(
            &source,
            &destination,
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    }
    .map_err(|err| err.to_string())
}

#[cfg(not(target_os = "windows"))]
fn replace_file(source: &Path, destination: &Path) -> Result<(), String> {
    fs::rename(source, destination).map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_settings_returns_default() {
        let path = std::env::temp_dir().join("gitaview_missing_settings.json");
        let _ = fs::remove_file(&path);
        let settings = load_settings(&path).unwrap();
        assert_eq!(settings.default_group, "全部分组");
        assert_eq!(settings.version, 1);
        assert!(settings.safety.confirm_pull);
    }

    #[test]
    fn load_settings_repairs_missing_default_group() {
        let path = std::env::temp_dir().join("gitaview_settings_repair.json");
        let _ = fs::remove_file(&path);
        fs::write(
            &path,
            r#"{
              "repos": [],
              "groups": [],
              "defaultGroup": "全部分组",
              "refresh": { "lightweightRefreshEnabled": true, "intervalMinutes": 5 },
              "safety": { "confirmPull": true, "confirmPush": true },
              "appearance": { "compactMode": false }
            }"#,
        )
        .unwrap();
        let settings = load_settings(&path).unwrap();
        assert!(settings.groups.iter().any(|group| group.name == "全部分组"));
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn load_settings_defaults_missing_confirm_push() {
        let path = std::env::temp_dir().join("gitaview_missing_confirm_push.json");
        let _ = fs::remove_file(&path);
        fs::write(
            &path,
            r#"{
              "repos": [],
              "groups": [{ "name": "全部分组", "repoIds": [] }],
              "defaultGroup": "全部分组",
              "refresh": { "lightweightRefreshEnabled": true, "intervalMinutes": 5 },
              "safety": { "confirmPull": true },
              "appearance": { "compactMode": false }
            }"#,
        )
        .unwrap();
        let settings = load_settings(&path).unwrap();
        assert!(settings.safety.confirm_pull);
        assert!(settings.safety.confirm_push);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn load_settings_clamps_refresh_interval() {
        let path = std::env::temp_dir().join("gitaview_refresh_interval_clamp.json");
        let _ = fs::remove_file(&path);
        fs::write(
            &path,
            r#"{
              "repos": [],
              "groups": [{ "name": "全部分组", "repoIds": [] }],
              "defaultGroup": "全部分组",
              "refresh": { "lightweightRefreshEnabled": true, "intervalMinutes": 0 },
              "safety": { "confirmPull": true, "confirmPush": true },
              "appearance": { "compactMode": false }
            }"#,
        )
        .unwrap();
        let settings = load_settings(&path).unwrap();
        assert_eq!(settings.refresh.interval_minutes, 1);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn load_settings_defaults_missing_allow_widget_drag() {
        let path = std::env::temp_dir().join("gitaview_missing_allow_widget_drag.json");
        let _ = fs::remove_file(&path);
        fs::write(
            &path,
            r#"{
              "repos": [],
              "groups": [{ "name": "全部分组", "repoIds": [] }],
              "defaultGroup": "全部分组",
              "refresh": { "lightweightRefreshEnabled": true, "intervalMinutes": 5 },
              "safety": { "confirmPull": true, "confirmPush": true },
              "appearance": { "compactMode": false }
            }"#,
        )
        .unwrap();
        let settings = load_settings(&path).unwrap();
        assert!(settings.appearance.allow_widget_drag);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn save_settings_rebuilds_group_repo_ids() {
        let path = std::env::temp_dir().join("gitaview_save_group_repo_ids.json");
        let _ = fs::remove_file(&path);
        let mut settings = AppSettings::default();
        settings.groups.push(crate::domain::settings::GroupRecord {
            name: "业务".to_string(),
            repo_ids: vec!["stale".to_string()],
        });
        settings.repos.push(crate::domain::repo::RepoRecord {
            id: "repo-a".to_string(),
            name: "repo-a".to_string(),
            path: std::path::PathBuf::from("C:/repo-a"),
            group: "业务".to_string(),
        });

        let saved = save_settings(&path, &settings).unwrap();
        let saved_group = saved
            .groups
            .iter()
            .find(|group| group.name == "业务")
            .unwrap();
        assert_eq!(saved_group.repo_ids, vec!["repo-a"]);

        let loaded = load_settings(&path).unwrap();
        let group = loaded
            .groups
            .iter()
            .find(|group| group.name == "业务")
            .unwrap();
        assert_eq!(group.repo_ids, vec!["repo-a"]);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn save_settings_drops_legacy_always_on_top() {
        let path = std::env::temp_dir().join("gitaview_save_drops_always_on_top.json");
        let _ = fs::remove_file(&path);
        fs::write(
            &path,
            r#"{
              "repos": [],
              "groups": [{ "name": "全部分组", "repoIds": [] }],
              "defaultGroup": "全部分组",
              "refresh": { "lightweightRefreshEnabled": true, "intervalMinutes": 5 },
              "safety": { "confirmPull": true, "confirmPush": true },
              "appearance": { "compactMode": false, "allowWidgetDrag": true, "alwaysOnTop": true }
            }"#,
        )
        .unwrap();

        let settings = load_settings(&path).unwrap();
        save_settings(&path, &settings).unwrap();
        let saved_text = fs::read_to_string(&path).unwrap();

        assert!(!saved_text.contains("alwaysOnTop"));
        assert!(!saved_text.contains("compactMode"));
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn save_settings_removes_atomic_write_temporary_file() {
        let path = std::env::temp_dir().join("gitaview_atomic_settings.json");
        let temp_path = settings_temp_path(&path);
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(&temp_path);

        save_settings(&path, &AppSettings::default()).unwrap();

        assert!(path.exists());
        assert!(!temp_path.exists());
        let _ = fs::remove_file(&path);
    }
}
