//! 契约数据模型（contracts/native/v1 原型子集，N02）。
//!
//! 规则（实施说明 §6.1）：
//! - JSON 键 camelCase；枚举小写 snake_case；UTF-8。
//! - schemaVersion 与 abiVersion 均为 1，独立于产品版本。
//! - 时间为 Unix 毫秒 u64；未知时间显式 null。
//! - 五种关系是读取成功的结果；读取失败是独立 readState，不是第六种关系。

use crate::error::ErrorObject;
use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: u64 = 1;

/// 本地与 origin 比较后的五种关系。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Relation {
    Synced,
    LocalAhead,
    RemoteAhead,
    Diverged,
    NoRemote,
}

impl Relation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Synced => "synced",
            Self::LocalAhead => "local_ahead",
            Self::RemoteAhead => "remote_ahead",
            Self::Diverged => "diverged",
            Self::NoRemote => "no_remote",
        }
    }
}

/// 仓库读取状态。`pending` 表示尚未读取，`error` 表示读取失败；
/// 两者都不是 Git 关系。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadState {
    Pending,
    Ok,
    Error,
}

impl ReadState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Ok => "ok",
            Self::Error => "error",
        }
    }
}

/// 全局刷新状态：只描述最近一次完成的尝试。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefreshState {
    Pending,
    Ready,
    Failed,
}

/// 操作可用性。UI 以此禁用按钮，核心在执行时再次校验。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RepoActions {
    pub can_open_directory: bool,
    pub can_open_remote: bool,
    pub can_fetch: bool,
    pub can_pull: bool,
    pub can_push: bool,
}

impl RepoActions {
    /// 全部禁用（目录入口除外）的占位动作。
    pub fn directory_only() -> Self {
        Self {
            can_open_directory: true,
            can_open_remote: false,
            can_fetch: false,
            can_pull: false,
            can_push: false,
        }
    }

    /// 按读取结果计算动作可用性（实施说明 §7 表；两端不重复实现规则）。
    /// - 读取失败 / pending：只保留目录入口；
    /// - 无 origin：网络动作全部禁用；
    /// - 有 origin：可 Fetch；remote_ahead / diverged 可 Pull；
    ///   local_ahead / diverged 可 Push；
    /// - 打开远端入口仅在有浏览器安全 URL 时可用。
    pub fn for_read(
        read_state: ReadState,
        relation: Option<Relation>,
        has_origin: bool,
        has_safe_remote_url: bool,
    ) -> Self {
        let mut actions = Self::directory_only();
        if read_state != ReadState::Ok || !has_origin {
            return actions;
        }
        actions.can_open_remote = has_safe_remote_url;
        actions.can_fetch = true;
        if matches!(
            relation,
            Some(Relation::RemoteAhead) | Some(Relation::Diverged)
        ) {
            actions.can_pull = true;
        }
        if matches!(
            relation,
            Some(Relation::LocalAhead) | Some(Relation::Diverged)
        ) {
            actions.can_push = true;
        }
        actions
    }
}

/// 快照中的仓库行。pending / error 时未知 Git 字段显式为 null。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RepoStatus {
    pub id: String,
    pub name: String,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_id: Option<String>,
    pub read_state: ReadState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relation: Option<Relation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub has_commit: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub has_origin: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comparison_branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ahead: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub behind: Option<u32>,
    /// 浏览器可打开的 origin URL（http/https），不安全的为 null。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorObject>,
    pub actions: RepoActions,
}

impl RepoStatus {
    /// pending / error 的占位行：目录入口保留，网络动作禁用。
    pub fn pending(
        id: impl Into<String>,
        name: impl Into<String>,
        path: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            path: path.into(),
            group_id: None,
            read_state: ReadState::Pending,
            relation: None,
            branch: None,
            has_commit: None,
            has_origin: None,
            comparison_branch: None,
            ahead: None,
            behind: None,
            remote_url: None,
            error: None,
            actions: RepoActions::directory_only(),
        }
    }
}

/// 定时刷新策略。间隔 1–60 分钟；enabled=false 表示手动模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RefreshPolicy {
    pub enabled: bool,
    pub interval_minutes: u32,
}

impl Default for RefreshPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            interval_minutes: 5,
        }
    }
}

/// 刷新请求进度。等待中的请求用 requested > completed 表示。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RefreshProgress {
    pub is_refreshing: bool,
    pub requested_request_id: u64,
    pub completed_request_id: u64,
}

/// 应用快照（宿主窗口 / 菜单消费）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppSnapshot {
    pub schema_version: u64,
    pub session_id: String,
    pub generation: u64,
    pub state_revision: u64,
    pub settings_revision: u64,
    pub attempted_at_ms: Option<u64>,
    pub last_success_at_ms: Option<u64>,
    pub refresh_state: RefreshState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_error: Option<ErrorObject>,
    pub refresh_policy: RefreshPolicy,
    pub refresh: RefreshProgress,
    pub repos: Vec<RepoStatus>,
}

// —— Widget 安全投影（§6.5）——

/// Widget 全量摘要计数。五种关系 + error + pending 之和等于 total。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WidgetCounts {
    pub total: u32,
    pub synced: u32,
    pub local_ahead: u32,
    pub remote_ahead: u32,
    pub diverged: u32,
    pub no_remote: u32,
    pub error: u32,
    pub pending: u32,
}

/// 紧凑四桶摘要：error 计入 needs_attention；pending 单独展示。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompactCounts {
    pub synced: u32,
    pub syncable: u32,
    pub needs_attention: u32,
    pub no_remote: u32,
}

/// 关注列表条目：不含 path、完整远端 URL 或原始 Git 输出。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WidgetRepo {
    pub id: String,
    pub name: String,
    pub read_state: ReadState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relation: Option<Relation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
}

/// Widget 快照：由 Rust 投影生成，两端消费同一种 JSON。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WidgetSnapshot {
    pub schema_version: u64,
    pub session_id: String,
    pub generation: u64,
    pub state_revision: u64,
    pub settings_revision: u64,
    pub attempted_at_ms: Option<u64>,
    pub last_success_at_ms: Option<u64>,
    pub refresh_state: RefreshState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_error_code: Option<String>,
    pub refresh_policy: RefreshPolicy,
    pub counts: WidgetCounts,
    pub compact: CompactCounts,
    /// 关注列表：读取失败、分叉、远程领先、本地领先，最多 10 项。
    pub attention: Vec<WidgetRepo>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relation_serializes_to_lowercase_snake_case() {
        assert_eq!(
            serde_json::to_string(&Relation::LocalAhead).unwrap(),
            "\"local_ahead\""
        );
        assert_eq!(
            serde_json::from_str::<Relation>("\"no_remote\"").unwrap(),
            Relation::NoRemote
        );
        assert!(serde_json::from_str::<Relation>("\"error\"").is_err());
    }

    #[test]
    fn pending_row_has_null_git_fields_and_disabled_network_actions() {
        let repo = RepoStatus::pending("repo-x", "x", "/tmp/x");
        let json = serde_json::to_value(&repo).unwrap();
        assert_eq!(json["readState"], "pending");
        assert_eq!(json["relation"], serde_json::Value::Null);
        assert!(json.get("branch").is_none() || json["branch"].is_null());
        assert_eq!(json["actions"]["canFetch"], false);
        assert_eq!(json["actions"]["canOpenDirectory"], true);
    }

    #[test]
    fn app_snapshot_field_names_are_camel_case() {
        let snapshot = AppSnapshot {
            schema_version: 1,
            session_id: "s".into(),
            generation: 0,
            state_revision: 0,
            settings_revision: 0,
            attempted_at_ms: None,
            last_success_at_ms: None,
            refresh_state: RefreshState::Pending,
            refresh_error: None,
            refresh_policy: RefreshPolicy::default(),
            refresh: RefreshProgress {
                is_refreshing: false,
                requested_request_id: 0,
                completed_request_id: 0,
            },
            repos: vec![],
        };
        let json = serde_json::to_value(&snapshot).unwrap();
        for key in [
            "schemaVersion",
            "sessionId",
            "generation",
            "stateRevision",
            "settingsRevision",
            "attemptedAtMs",
            "lastSuccessAtMs",
            "refreshState",
            "refreshPolicy",
            "refresh",
            "repos",
        ] {
            assert!(json.get(key).is_some(), "missing key {key}");
        }
    }
}
