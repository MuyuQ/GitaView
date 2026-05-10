use std::fs;
use std::path::Path;

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

pub fn save_settings(path: &Path, settings: &AppSettings) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let text = serde_json::to_string_pretty(settings).map_err(|err| err.to_string())?;
    fs::write(path, text).map_err(|err| err.to_string())
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
        ).unwrap();
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
        ).unwrap();
        let settings = load_settings(&path).unwrap();
        assert!(settings.safety.confirm_pull);
        assert!(settings.safety.confirm_push);
        let _ = fs::remove_file(&path);
    }
}
