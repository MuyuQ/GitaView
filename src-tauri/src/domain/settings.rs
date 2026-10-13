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

/// 当前实现支持的 schema 版本上限。未来迁移（1 → 2）时递增，
/// 并在 normalized() 中补写迁移逻辑。
pub const MAX_SUPPORTED_VERSION: u32 = 1;

impl AppSettings {
    /// 规范化：修复缺失的默认分组、把 repo 归位到存在的分组、重建分组成员、
    /// 收紧刷新间隔与安全开关，并对未来版本的配置做降级标记。
    pub fn normalized(mut self) -> Self {
        if self.version > MAX_SUPPORTED_VERSION {
            // 更新版本写入的配置被旧版本读到时，未知字段已被 serde 丢弃；
            // 把版本号压回受支持上限，避免"声明 v99、内容实为 v1"的混合状态。
            crate::diagnostics::log(
                "settings.version_downgraded",
                format!("from={} to={MAX_SUPPORTED_VERSION}", self.version),
            );
            self.version = MAX_SUPPORTED_VERSION;
        }
        self.dedup_repos_by_id();
        self.dedup_groups_by_name();
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

    /// 重复的 repo id 会让 find_repo 遮蔽、remove_repository 双删，只保留首个
    fn dedup_repos_by_id(&mut self) {
        let mut seen = std::collections::HashSet::new();
        self.repos.retain(|repo| seen.insert(repo.id.clone()));
    }

    /// 重复的分组名会形成影子分组；保留首个，其余丢弃（成员随后重建）
    fn dedup_groups_by_name(&mut self) {
        let mut seen = std::collections::HashSet::new();
        self.groups.retain(|group| seen.insert(group.name.clone()));
    }
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

    #[test]
    fn normalization_clamps_future_versions_to_supported_maximum() {
        let settings = AppSettings {
            version: 99,
            ..AppSettings::default()
        };
        assert_eq!(settings.normalized().version, MAX_SUPPORTED_VERSION);
    }

    #[test]
    fn normalization_dedups_repos_by_id_and_groups_by_name() {
        let mut settings = AppSettings::default();
        settings.repos.push(crate::domain::repo::RepoRecord {
            id: "repo-a".to_string(),
            name: "first".to_string(),
            path: std::path::PathBuf::from("C:/repo-a"),
            group: "全部分组".to_string(),
        });
        // 同 id 的影子仓库会让 remove_repository 双删，必须去掉
        settings.repos.push(crate::domain::repo::RepoRecord {
            id: "repo-a".to_string(),
            name: "shadow".to_string(),
            path: std::path::PathBuf::from("C:/repo-shadow"),
            group: "全部分组".to_string(),
        });
        settings.repos.push(crate::domain::repo::RepoRecord {
            id: "repo-b".to_string(),
            name: "second".to_string(),
            path: std::path::PathBuf::from("C:/repo-b"),
            group: "业务".to_string(),
        });
        // 同名影子分组会让 find_repo 命中错误分组，必须去掉
        settings.groups.push(GroupRecord {
            name: "全部分组".to_string(),
            repo_ids: vec!["stale".to_string()],
        });
        settings.groups.push(GroupRecord {
            name: "业务".to_string(),
            repo_ids: vec![],
        });
        settings.groups.push(GroupRecord {
            name: "业务".to_string(),
            repo_ids: vec!["stale".to_string()],
        });

        let normalized = settings.normalized();

        let ids: Vec<&str> = normalized
            .repos
            .iter()
            .map(|repo| repo.id.as_str())
            .collect();
        assert_eq!(ids, vec!["repo-a", "repo-b"]);
        let group_names: Vec<&str> = normalized
            .groups
            .iter()
            .map(|group| group.name.as_str())
            .collect();
        assert_eq!(group_names, vec!["全部分组", "业务"]);
    }
}
