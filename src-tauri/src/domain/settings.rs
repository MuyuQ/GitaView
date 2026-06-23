use serde::{Deserialize, Serialize};

use super::repo::RepoRecord;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GroupRecord {
    pub name: String,
    pub repo_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RefreshSettings {
    pub lightweight_refresh_enabled: bool,
    pub interval_minutes: u32,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SafetySettings {
    #[serde(default = "default_true")]
    pub confirm_pull: bool,
    #[serde(default = "default_true")]
    pub confirm_push: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppearanceSettings {
    #[serde(default = "default_true")]
    pub allow_widget_drag: bool,
    // 已移除 compact_mode 和 always_on_top 字段
    // 使用 #[serde(default, skip_serializing)] 读取旧配置但不再写回
    #[serde(default, skip_serializing)]
    compact_mode: bool,
    #[serde(default, skip_serializing)]
    always_on_top: bool,
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            allow_widget_drag: true,
            compact_mode: false,  // 私有字段，仅用于 serde 兼容
            always_on_top: false, // 私有字段，仅用于 serde 兼容
        }
    }
}

fn default_version() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    #[serde(default = "default_version")]
    pub version: u32,
    pub repos: Vec<RepoRecord>,
    pub groups: Vec<GroupRecord>,
    pub default_group: String,
    pub refresh: RefreshSettings,
    pub safety: SafetySettings,
    #[serde(default)]
    pub appearance: AppearanceSettings,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            version: default_version(),
            repos: Vec::new(),
            groups: vec![GroupRecord {
                name: "全部分组".to_string(),
                repo_ids: Vec::new(),
            }],
            default_group: "全部分组".to_string(),
            refresh: RefreshSettings {
                lightweight_refresh_enabled: true,
                interval_minutes: 5,
            },
            safety: SafetySettings {
                confirm_pull: true,
                confirm_push: true,
            },
            appearance: AppearanceSettings::default(),
        }
    }
}

impl AppSettings {
    /// version 字段由 serde 反序列化时的 default_version() 降级到 1；
    /// 未来 schema 迁移（如 version 1 → 2）的逻辑应在此方法中实现。
    pub fn normalized(mut self) -> Self {
        if self.default_group.trim().is_empty() {
            self.default_group = "全部分组".to_string();
        }
        if !self
            .groups
            .iter()
            .any(|group| group.name == self.default_group)
        {
            self.groups.insert(
                0,
                GroupRecord {
                    name: self.default_group.clone(),
                    repo_ids: Vec::new(),
                },
            );
        }
        for repo in &mut self.repos {
            if repo.group.trim().is_empty()
                || !self.groups.iter().any(|group| group.name == repo.group)
            {
                repo.group = self.default_group.clone();
            }
        }
        for group in &mut self.groups {
            group.repo_ids = self
                .repos
                .iter()
                .filter(|repo| repo.group == group.name)
                .map(|repo| repo.id.clone())
                .collect();
        }
        self.refresh.interval_minutes = self.refresh.interval_minutes.clamp(1, 60);
        self.safety.confirm_pull = true;
        self.safety.confirm_push = true;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalization_restores_mandatory_confirmation_flags() {
        let mut settings = AppSettings::default();
        settings.safety.confirm_pull = false;
        settings.safety.confirm_push = false;

        let normalized = settings.normalized();

        assert!(normalized.safety.confirm_pull);
        assert!(normalized.safety.confirm_push);
    }

    #[test]
    fn default_settings_has_version_1() {
        let settings = AppSettings::default();
        assert_eq!(settings.version, 1);
    }

    #[test]
    fn deserialize_missing_version_defaults_to_1() {
        let json = r#"{
            "repos": [],
            "groups": [{ "name": "全部分组", "repoIds": [] }],
            "defaultGroup": "全部分组",
            "refresh": { "lightweightRefreshEnabled": true, "intervalMinutes": 5 },
            "safety": { "confirmPull": true, "confirmPush": true },
            "appearance": { "allowWidgetDrag": true }
        }"#;
        let settings: AppSettings = serde_json::from_str(json).unwrap();
        assert_eq!(settings.version, 1);
    }

    #[test]
    fn version_field_survives_round_trip() {
        let settings = AppSettings {
            version: 2,
            ..AppSettings::default()
        };
        let json = serde_json::to_string(&settings).unwrap();
        let restored: AppSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.version, 2);
        assert_eq!(restored, settings);
    }
}
