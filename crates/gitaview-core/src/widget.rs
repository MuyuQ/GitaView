//! Widget 安全投影（N02 原型；完整视图投影在 N08 扩展）。
//!
//! 投影规则（实施说明 §6.5 / §6.3）：
//! - 摘要始终覆盖全部仓库；紧凑四桶 synced / syncable / needs_attention / no_remote，
//!   error 计入 needs_attention，pending 单独计数；四桶之和 + pending = total。
//! - 关注列表最多 10 项：读取失败、分叉、远程领先、本地领先；
//!   同档按名称 lowercase 字典序、再按 ID 打破并列。
//! - 不包含 path、完整远端 URL、凭据或原始 Git 输出。
//! - 过期判断：开启定时刷新时，距最后成功超过两个配置间隔为过期；
//!   等于阈值尚不标记过期；关闭定时刷新为手动模式，不作过期判断。

use crate::model::{
    AppSnapshot, CompactCounts, ReadState, Relation, RepoStatus, WidgetCounts, WidgetRepo,
    WidgetSnapshot,
};

/// 关注列表上限。
pub const ATTENTION_LIMIT: usize = 10;

/// 关注优先级：error 最先，然后 diverged、remote_ahead、local_ahead。
/// synced / no_remote / pending 不进入关注列表。
fn attention_rank(read_state: ReadState, relation: Option<Relation>) -> Option<u8> {
    match (read_state, relation) {
        (ReadState::Error, _) => Some(0),
        (ReadState::Ok, Some(Relation::Diverged)) => Some(1),
        (ReadState::Ok, Some(Relation::RemoteAhead)) => Some(2),
        (ReadState::Ok, Some(Relation::LocalAhead)) => Some(3),
        _ => None,
    }
}

/// 视图排序（N08 全量复用）：读取失败、pending、diverged、remote_ahead、
/// local_ahead、synced、no_remote；同档按名称 lowercase 再按 ID。
pub fn view_sort_rank(read_state: ReadState, relation: Option<Relation>) -> u8 {
    match (read_state, relation) {
        (ReadState::Error, _) => 0,
        (ReadState::Pending, _) => 1,
        (ReadState::Ok, Some(Relation::Diverged)) => 2,
        (ReadState::Ok, Some(Relation::RemoteAhead)) => 3,
        (ReadState::Ok, Some(Relation::LocalAhead)) => 4,
        (ReadState::Ok, Some(Relation::Synced)) => 5,
        (ReadState::Ok, Some(Relation::NoRemote)) => 6,
        // 读取成功但缺关系属于数据错误：排最前，不伪装成 no_remote。
        (ReadState::Ok, None) => 0,
    }
}

pub fn compare_repos(
    left: &(ReadState, Option<Relation>, String, String),
    right: &(ReadState, Option<Relation>, String, String),
) -> std::cmp::Ordering {
    view_sort_rank(left.0, left.1)
        .cmp(&view_sort_rank(right.0, right.1))
        .then_with(|| left.2.to_lowercase().cmp(&right.2.to_lowercase()))
        .then_with(|| left.3.cmp(&right.3))
}

/// 摘要计数（N08 抽出，供 Widget 投影与视图查询共用，两端不重复实现）。
/// 摘要始终覆盖传入的全部仓库；紧凑四桶 error 计入 needs_attention，
/// pending 单独计数，四桶之和 + pending = total。
pub fn summarize(repos: &[RepoStatus]) -> (WidgetCounts, CompactCounts) {
    let mut counts = WidgetCounts {
        total: repos.len() as u32,
        synced: 0,
        local_ahead: 0,
        remote_ahead: 0,
        diverged: 0,
        no_remote: 0,
        error: 0,
        pending: 0,
    };
    for repo in repos {
        match repo.read_state {
            ReadState::Pending => counts.pending += 1,
            ReadState::Error => counts.error += 1,
            ReadState::Ok => match repo.relation {
                Some(Relation::Synced) => counts.synced += 1,
                Some(Relation::LocalAhead) => counts.local_ahead += 1,
                Some(Relation::RemoteAhead) => counts.remote_ahead += 1,
                Some(Relation::Diverged) => counts.diverged += 1,
                Some(Relation::NoRemote) => counts.no_remote += 1,
                None => counts.error += 1,
            },
        }
    }
    let compact = CompactCounts {
        synced: counts.synced,
        syncable: counts.local_ahead + counts.remote_ahead,
        needs_attention: counts.diverged + counts.error,
        no_remote: counts.no_remote,
    };
    (counts, compact)
}

/// 由应用快照生成 Widget 安全投影。
pub fn project_widget(snapshot: &AppSnapshot) -> WidgetSnapshot {
    let (counts, compact) = summarize(&snapshot.repos);

    let mut attention: Vec<WidgetRepo> = Vec::new();
    for repo in &snapshot.repos {
        if attention_rank(repo.read_state, repo.relation).is_some() {
            attention.push(WidgetRepo {
                id: repo.id.clone(),
                name: repo.name.clone(),
                read_state: repo.read_state,
                relation: repo.relation,
                error_code: repo.error.as_ref().map(|error| error.code.clone()),
            });
        }
    }

    attention.sort_by(|left, right| {
        attention_rank(left.read_state, left.relation)
            .cmp(&attention_rank(right.read_state, right.relation))
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
            .then_with(|| left.id.cmp(&right.id))
    });
    attention.truncate(ATTENTION_LIMIT);

    WidgetSnapshot {
        schema_version: snapshot.schema_version,
        session_id: snapshot.session_id.clone(),
        generation: snapshot.generation,
        state_revision: snapshot.state_revision,
        settings_revision: snapshot.settings_revision,
        attempted_at_ms: snapshot.attempted_at_ms,
        last_success_at_ms: snapshot.last_success_at_ms,
        refresh_state: snapshot.refresh_state,
        refresh_error_code: snapshot
            .refresh_error
            .as_ref()
            .map(|error| error.code.clone()),
        refresh_policy: snapshot.refresh_policy,
        counts,
        compact,
        attention,
    }
}

/// 数据新鲜度判断结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Staleness {
    /// 手动模式：不做过期判断，展示手动刷新与时间。
    Manual,
    /// 在阈值内（含恰好等于阈值）。
    Fresh,
    /// 距最后成功超过两个配置间隔。
    Stale,
    /// 成功时间在未来（时钟回拨）：显示时间异常，等待刷新，不作无符号减法。
    FutureTime,
}

impl WidgetSnapshot {
    /// 以当前 Unix 毫秒时间计算新鲜度。
    /// 开启定时刷新：now >= last + 2*interval 为 Stale（等于阈值仍 Fresh）。
    pub fn staleness(&self, now_ms: u64) -> Staleness {
        if !self.refresh_policy.enabled {
            return Staleness::Manual;
        }
        let Some(last) = self.last_success_at_ms else {
            // 从未成功过：由 refreshState 表达（pending / failed），不是过期。
            return Staleness::Fresh;
        };
        if last > now_ms {
            return Staleness::FutureTime;
        }
        let interval_ms = u64::from(self.refresh_policy.interval_minutes.max(1)) * 60_000;
        let threshold = interval_ms.saturating_mul(2);
        // 等于阈值仍 Fresh；严格大于阈值才过期。
        if now_ms - last > threshold {
            Staleness::Stale
        } else {
            Staleness::Fresh
        }
    }
}

/// 全局失败时保留历史 repos 并按最新配置重投影（§6.3）由引擎在 N07 实现；
/// 此处仅提供计数一致性自检，供测试使用。
pub fn counts_close_over_total(counts: &WidgetCounts) -> bool {
    let sum = counts.synced
        + counts.local_ahead
        + counts.remote_ahead
        + counts.diverged
        + counts.no_remote
        + counts.error
        + counts.pending;
    sum == counts.total
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorObject;
    use crate::model::{
        AppSnapshot, ReadState, RefreshPolicy, RefreshProgress, RefreshState, Relation,
        RepoActions, RepoStatus,
    };

    fn repo(id: &str, name: &str, read_state: ReadState, relation: Option<Relation>) -> RepoStatus {
        let mut row = RepoStatus::pending(id, name, format!("/private/{id}"));
        row.read_state = read_state;
        row.relation = relation;
        row
    }

    fn snapshot(repos: Vec<RepoStatus>) -> AppSnapshot {
        AppSnapshot {
            schema_version: 1,
            session_id: "test-session".into(),
            generation: 4,
            state_revision: 12,
            settings_revision: 2,
            attempted_at_ms: Some(1_792_800_000_000),
            last_success_at_ms: Some(1_792_799_700_000),
            refresh_state: RefreshState::Ready,
            refresh_error: None,
            refresh_policy: RefreshPolicy::default(),
            refresh: RefreshProgress {
                is_refreshing: false,
                requested_request_id: 7,
                completed_request_id: 7,
            },
            repos,
        }
    }

    #[test]
    fn counts_close_over_total_and_compact_buckets() {
        let mut error_row = repo("repo-ci", "ci", ReadState::Error, None);
        error_row.error = Some(ErrorObject::new("GIT_FAILED"));
        let snap = snapshot(vec![
            repo("a", "a", ReadState::Ok, Some(Relation::Synced)),
            repo("b", "b", ReadState::Ok, Some(Relation::Synced)),
            repo("c", "c", ReadState::Ok, Some(Relation::LocalAhead)),
            repo("d", "d", ReadState::Ok, Some(Relation::RemoteAhead)),
            repo("e", "e", ReadState::Ok, Some(Relation::Diverged)),
            repo("f", "f", ReadState::Ok, Some(Relation::NoRemote)),
            repo("g", "g", ReadState::Ok, Some(Relation::NoRemote)),
            error_row,
        ]);
        let widget = project_widget(&snap);
        assert_eq!(widget.counts.total, 8);
        assert_eq!(widget.counts.synced, 2);
        assert_eq!(widget.counts.local_ahead, 1);
        assert_eq!(widget.counts.remote_ahead, 1);
        assert_eq!(widget.counts.diverged, 1);
        assert_eq!(widget.counts.no_remote, 2);
        assert_eq!(widget.counts.error, 1);
        assert_eq!(widget.counts.pending, 0);
        assert!(counts_close_over_total(&widget.counts));
        assert_eq!(widget.compact.synced, 2);
        assert_eq!(widget.compact.syncable, 2);
        assert_eq!(widget.compact.needs_attention, 2);
        assert_eq!(widget.compact.no_remote, 2);
    }

    #[test]
    fn attention_orders_by_tier_then_name_and_truncates() {
        let mut many = vec![
            repo("ci", "ci", ReadState::Error, None),
            repo(
                "api",
                "backend-api",
                ReadState::Ok,
                Some(Relation::Diverged),
            ),
            repo(
                "docs",
                "docs-site",
                ReadState::Ok,
                Some(Relation::RemoteAhead),
            ),
            repo(
                "mobile",
                "mobile-app",
                ReadState::Ok,
                Some(Relation::LocalAhead),
            ),
            repo("aaa", "a-local", ReadState::Ok, Some(Relation::LocalAhead)),
            repo("sync", "synced-repo", ReadState::Ok, Some(Relation::Synced)),
            repo(
                "none",
                "no-remote-repo",
                ReadState::Ok,
                Some(Relation::NoRemote),
            ),
            repo("pend", "pending-repo", ReadState::Pending, None),
        ];
        for index in 0..12 {
            many.push(repo(
                &format!("div-{index}"),
                &format!("zz-div-{index:02}"),
                ReadState::Ok,
                Some(Relation::Diverged),
            ));
        }
        let widget = project_widget(&snapshot(many));
        let ids: Vec<&str> = widget
            .attention
            .iter()
            .map(|item| item.id.as_str())
            .collect();
        // error 最先；diverged 档按名称排序：backend-api 在 zz-div-* 之前。
        assert_eq!(ids[0], "ci");
        assert_eq!(ids[1], "api");
        assert_eq!(ids[2], "div-0");
        assert_eq!(widget.attention.len(), ATTENTION_LIMIT);
        assert_eq!(ids[ATTENTION_LIMIT - 1], "div-7");
        // synced / no_remote / pending 不进入关注列表
        assert!(!ids.contains(&"sync"));
        assert!(!ids.contains(&"none"));
        assert!(!ids.contains(&"pend"));
        // 名称排序在 local_ahead 档内生效
        let widget_small = project_widget(&snapshot(vec![
            repo("m", "mobile", ReadState::Ok, Some(Relation::LocalAhead)),
            repo("a", "api-local", ReadState::Ok, Some(Relation::LocalAhead)),
        ]));
        assert_eq!(
            widget_small
                .attention
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            vec!["a", "m"]
        );
    }

    #[test]
    fn widget_projection_leaks_no_paths_or_urls() {
        let mut row = repo(
            "repo-api",
            "api",
            ReadState::Ok,
            Some(Relation::RemoteAhead),
        );
        row.path = "/Users/private/project/api".into();
        row.remote_url = Some("https://github.com/example/api".into());
        row.error = Some(ErrorObject::with_message(
            "GIT_FAILED",
            "fatal: /Users/private/project/api stdout leak",
        ));
        row.actions = RepoActions {
            can_open_directory: true,
            can_open_remote: true,
            can_fetch: true,
            can_pull: true,
            can_push: false,
        };
        let json = serde_json::to_string(&project_widget(&snapshot(vec![row]))).unwrap();
        assert!(!json.contains("/Users/private"));
        assert!(!json.contains("github.com"));
        assert!(!json.contains("stdout leak"));
        assert!(json.contains("GIT_FAILED"));
    }

    #[test]
    fn staleness_threshold_is_exclusive_at_two_intervals() {
        let mut snap = snapshot(vec![]);
        snap.refresh_policy = RefreshPolicy {
            enabled: true,
            interval_minutes: 5,
        };
        snap.last_success_at_ms = Some(1_000_000);
        let widget = project_widget(&snap);
        let threshold = 2 * 5 * 60_000;
        assert_eq!(widget.staleness(1_000_000 + threshold), Staleness::Fresh);
        assert_eq!(
            widget.staleness(1_000_000 + threshold + 1),
            Staleness::Stale
        );
        assert_eq!(widget.staleness(999_999), Staleness::FutureTime);
        snap.refresh_policy.enabled = false;
        assert_eq!(
            project_widget(&snap).staleness(9_000_000_000),
            Staleness::Manual
        );
        snap.last_success_at_ms = None;
        snap.refresh_policy.enabled = true;
        assert_eq!(
            project_widget(&snap).staleness(9_000_000_000),
            Staleness::Fresh
        );
    }

    #[test]
    fn failed_projection_keeps_history_and_error_code() {
        let mut snap = snapshot(vec![repo(
            "repo-api",
            "api",
            ReadState::Ok,
            Some(Relation::RemoteAhead),
        )]);
        snap.refresh_state = RefreshState::Failed;
        snap.refresh_error = Some(ErrorObject::new("SETTINGS_IO"));
        let widget = project_widget(&snap);
        assert_eq!(widget.refresh_state, RefreshState::Failed);
        assert_eq!(widget.refresh_error_code.as_deref(), Some("SETTINGS_IO"));
        assert_eq!(widget.counts.remote_ahead, 1);
        assert_eq!(widget.attention.len(), 1);
    }
}
