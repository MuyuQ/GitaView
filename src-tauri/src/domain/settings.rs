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
    pub compact_mode: bool,
    #[serde(default = "default_true")]
    pub allow_widget_drag: bool,
    // 已移除 always_on_top 字段，现由前端原生桌面 widget 层控制
    // 使用 #[serde(default)] 忽略旧设置文件中的 alwaysOnTop/always_on_top 字段
    #[serde(default)]
    always_on_top: bool,
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            compact_mode: false,
            allow_widget_drag: true,
            always_on_top: false, // 私有字段，仅用于 serde 兼容
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
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
        self
    }
}
