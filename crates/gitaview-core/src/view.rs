//! 共享视图查询（N08）：组计数、状态计数、筛选、搜索与排序的唯一实现，
//! 两个原生 UI 只消费结果，不各自维护关系计数和动作规则。
//!
//! 规则（实施说明 §8.2）：
//! - 组计数基于全部仓库；状态计数只基于所选组，不随状态筛选或搜索缩小。
//! - 行列表依次应用：分组 → 状态 / 读取失败筛选 → 搜索。
//! - 搜索 name / path / branch / group name，按 Unicode lowercase 后包含匹配
//!   （不承诺拼音检索）。
//! - 排序：读取失败、pending、diverged、remote_ahead、local_ahead、synced、
//!   no_remote；同档名称 lowercase 字典序，再按 ID 打破并列。
//! - 协调：所选组消失 → 切回全部；所选状态在所选组不存在 → 切回全部状态；
//!   搜索零结果不清空搜索。
//! - 摘要始终覆盖全部仓库，不受筛选影响；pending 仅出现在"全部状态"。

use crate::model::{AppSnapshot, CompactCounts, ReadState, Relation, RepoStatus, WidgetCounts};
use crate::settings::GroupSettings;
use crate::widget::{compare_repos, summarize};
use serde::{Deserialize, Serialize};

/// 状态筛选。pending 不是关系：仅 `All` 包含 pending 行（不新增关系筛选项）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RelationFilter {
    #[default]
    All,
    Error,
    Synced,
    LocalAhead,
    RemoteAhead,
    Diverged,
    NoRemote,
}

impl RelationFilter {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Error => "error",
            Self::Synced => "synced",
            Self::LocalAhead => "local_ahead",
            Self::RemoteAhead => "remote_ahead",
            Self::Diverged => "diverged",
            Self::NoRemote => "no_remote",
        }
    }

    fn matches(self, repo: &RepoStatus) -> bool {
        match self {
            Self::All => true,
            Self::Error => repo.read_state == ReadState::Error,
            Self::Synced => {
                repo.read_state == ReadState::Ok && repo.relation == Some(Relation::Synced)
            }
            Self::LocalAhead => {
                repo.read_state == ReadState::Ok && repo.relation == Some(Relation::LocalAhead)
            }
            Self::RemoteAhead => {
                repo.read_state == ReadState::Ok && repo.relation == Some(Relation::RemoteAhead)
            }
            Self::Diverged => {
                repo.read_state == ReadState::Ok && repo.relation == Some(Relation::Diverged)
            }
            Self::NoRemote => {
                repo.read_state == ReadState::Ok && repo.relation == Some(Relation::NoRemote)
            }
        }
    }
}

/// 视图查询输入：groupId=null 表示全部仓库。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ViewQuery {
    #[serde(default)]
    pub group_id: Option<String>,
    #[serde(default)]
    pub relation_filter: RelationFilter,
    #[serde(default)]
    pub query: String,
}

impl Default for ViewQuery {
    fn default() -> Self {
        Self {
            group_id: None,
            relation_filter: RelationFilter::All,
            query: String::new(),
        }
    }
}

/// 组计数（基于全部仓库）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GroupCount {
    pub id: String,
    pub name: String,
    pub count: u32,
}

/// 视图查询结果。同一 stateRevision 的两次查询可以有不同筛选（有效筛选随输入协调）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ViewResult {
    pub session_id: String,
    pub state_revision: u64,
    pub generation: u64,
    pub settings_revision: u64,
    /// 协调后的有效筛选。
    pub effective: ViewQuery,
    /// 全部仓库摘要计数（不受筛选影响）。
    pub summary_counts: WidgetCounts,
    pub summary_compact: CompactCounts,
    /// 各分组计数（基于全部仓库）。
    pub groups: Vec<GroupCount>,
    /// 所选组内的状态计数（不随状态筛选或搜索缩小）。
    pub status_counts: WidgetCounts,
    /// 排序后的行（依次应用分组、状态筛选、搜索）。
    pub rows: Vec<RepoStatus>,
}

/// 执行视图查询。`groups` 来自当前设置（搜索 group name 与组存在性判断需要）。
pub fn query_view(
    snapshot: &AppSnapshot,
    groups: &[GroupSettings],
    query: ViewQuery,
) -> ViewResult {
    let (summary_counts, summary_compact) = summarize(&snapshot.repos);

    // 组计数：基于全部仓库；只统计已定义分组。
    let group_counts = groups
        .iter()
        .map(|group| GroupCount {
            id: group.id.clone(),
            name: group.name.clone(),
            count: snapshot
                .repos
                .iter()
                .filter(|repo| repo.group_id.as_deref() == Some(group.id.as_str()))
                .count() as u32,
        })
        .collect::<Vec<_>>();

    // 协调 1：所选组不存在（被删除）→ 切回全部。
    let effective_group = query
        .group_id
        .clone()
        .filter(|id| groups.iter().any(|group| &group.id == id));

    // 分组后的行（状态计数与行筛选的共同基础）。
    let grouped: Vec<&RepoStatus> = snapshot
        .repos
        .iter()
        .filter(|repo| match &effective_group {
            Some(group_id) => repo.group_id.as_deref() == Some(group_id.as_str()),
            None => true,
        })
        .collect();

    // 状态计数：只基于所选组，不随状态筛选或搜索缩小。
    let owned_grouped: Vec<RepoStatus> = grouped.iter().map(|repo| (*repo).clone()).collect();
    let (status_counts, _) = summarize(&owned_grouped);

    // 协调 2：所选状态在所选组中不存在 → 切回全部状态。
    let effective_filter = if query.relation_filter != RelationFilter::All
        && !grouped
            .iter()
            .any(|repo| query.relation_filter.matches(repo))
    {
        RelationFilter::All
    } else {
        query.relation_filter
    };

    // 行筛选：状态 → 搜索（name / path / branch / group name，lowercase 包含）。
    let group_name_of = |repo: &RepoStatus| -> Option<String> {
        repo.group_id
            .as_ref()
            .and_then(|id| groups.iter().find(|group| &group.id == id))
            .map(|group| group.name.clone())
    };
    let needle = query.query.trim().to_lowercase();
    let mut rows: Vec<RepoStatus> = grouped
        .into_iter()
        .filter(|repo| effective_filter.matches(repo))
        .filter(|repo| {
            if needle.is_empty() {
                return true;
            }
            let haystacks = [
                Some(repo.name.to_lowercase()),
                Some(repo.path.to_lowercase()),
                repo.branch.as_ref().map(|b| b.to_lowercase()),
                group_name_of(repo).map(|name| name.to_lowercase()),
            ];
            haystacks
                .iter()
                .flatten()
                .any(|haystack| haystack.contains(&needle))
        })
        .cloned()
        .collect();

    rows.sort_by(|left, right| {
        compare_repos(
            &(
                left.read_state,
                left.relation,
                left.name.clone(),
                left.id.clone(),
            ),
            &(
                right.read_state,
                right.relation,
                right.name.clone(),
                right.id.clone(),
            ),
        )
    });

    ViewResult {
        session_id: snapshot.session_id.clone(),
        state_revision: snapshot.state_revision,
        generation: snapshot.generation,
        settings_revision: snapshot.settings_revision,
        effective: ViewQuery {
            group_id: effective_group,
            relation_filter: effective_filter,
            // 搜索零结果不清空搜索：无条件保留。
            query: query.query,
        },
        summary_counts,
        summary_compact,
        groups: group_counts,
        status_counts,
        rows,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relation_filter_serializes_snake_case() {
        assert_eq!(
            serde_json::to_string(&RelationFilter::LocalAhead).unwrap(),
            "\"local_ahead\""
        );
        assert_eq!(
            serde_json::from_str::<RelationFilter>("\"no_remote\"").unwrap(),
            RelationFilter::NoRemote
        );
        assert!(serde_json::from_str::<RelationFilter>("\"pending\"").is_err());
        assert_eq!(
            serde_json::from_str::<RelationFilter>("\"all\"").unwrap(),
            RelationFilter::All
        );
    }

    #[test]
    fn view_query_defaults_to_all_with_empty_search() {
        let query = ViewQuery::default();
        assert_eq!(query.group_id, None);
        assert_eq!(query.relation_filter, RelationFilter::All);
        assert_eq!(query.query, "");
        let json = serde_json::to_value(&query).unwrap();
        assert_eq!(json["relationFilter"], "all");
    }
}
