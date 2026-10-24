//! N08 视图查询测试：以 contracts/native/v1 fixture 为数据源，验证计数作用域、
//! 筛选协调、搜索、排序与摘要不变式。两端宿主（N10 起）只消费 ViewResult。

use gitaview_core::codec::decode_app_snapshot;
use gitaview_core::model::ReadState;
use gitaview_core::settings::GroupSettings;
use gitaview_core::view::{query_view, RelationFilter, ViewQuery};
use std::fs;
use std::path::{Path, PathBuf};

fn contracts_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../contracts/native/v1")
}

fn load_snapshot(name: &str) -> gitaview_core::model::AppSnapshot {
    let bytes = fs::read(contracts_dir().join("fixtures").join(name)).unwrap();
    decode_app_snapshot(&bytes).unwrap()
}

fn groups(ids: &[(&str, &str)]) -> Vec<GroupSettings> {
    ids.iter()
        .map(|(id, name)| GroupSettings {
            id: (*id).into(),
            name: (*name).into(),
        })
        .collect()
}

fn row_ids(result: &gitaview_core::view::ViewResult) -> Vec<String> {
    result.rows.iter().map(|row| row.id.clone()).collect()
}

const MIXED_GROUPS: &[(&str, &str)] = &[("group-backend", "后端"), ("group-docs", "文档")];

#[test]
fn mixed8_all_view_counts_sorts_and_summarizes() {
    let snapshot = load_snapshot("snapshot-mixed-8.json");
    let result = query_view(&snapshot, &groups(MIXED_GROUPS), ViewQuery::default());

    // 全部仓库摘要：synced 2 / local_ahead 1 / remote_ahead 1 / diverged 1 /
    // no_remote 2 / error 1；紧凑四桶 2/2/2/2。
    assert_eq!(result.summary_counts.total, 8);
    assert_eq!(result.summary_counts.synced, 2);
    assert_eq!(result.summary_counts.local_ahead, 1);
    assert_eq!(result.summary_counts.remote_ahead, 1);
    assert_eq!(result.summary_counts.diverged, 1);
    assert_eq!(result.summary_counts.no_remote, 2);
    assert_eq!(result.summary_counts.error, 1);
    assert_eq!(result.summary_counts.pending, 0);
    assert_eq!(result.summary_compact.synced, 2);
    assert_eq!(result.summary_compact.syncable, 2);
    assert_eq!(result.summary_compact.needs_attention, 2);
    assert_eq!(result.summary_compact.no_remote, 2);

    // 状态计数（未选组 = 全部仓库）与摘要一致。
    assert_eq!(result.status_counts, result.summary_counts);

    // 排序：error → diverged → remote_ahead → local_ahead → synced（名称序）
    // → no_remote（名称序，最后）。
    assert_eq!(
        row_ids(&result),
        vec![
            "repo-ci",
            "repo-api",
            "repo-docs",
            "repo-mobile",
            "repo-cli",
            "repo-web",
            "repo-legacy",
            "repo-notes",
        ]
    );

    // 组计数基于全部仓库。
    assert_eq!(
        result
            .groups
            .iter()
            .map(|group| (group.id.as_str(), group.count))
            .collect::<Vec<_>>(),
        vec![("group-backend", 1), ("group-docs", 2)]
    );

    // 行携带 actions；两端不重复实现动作规则。
    let api = result.rows.iter().find(|row| row.id == "repo-api").unwrap();
    assert!(api.actions.can_pull && api.actions.can_push && api.actions.can_fetch);
    let legacy = result
        .rows
        .iter()
        .find(|row| row.id == "repo-legacy")
        .unwrap();
    assert_eq!(
        legacy.actions,
        gitaview_core::model::RepoActions::directory_only()
    );
}

#[test]
fn group_filter_scopes_status_counts_and_rows_but_not_summary() {
    let snapshot = load_snapshot("snapshot-mixed-8.json");
    let result = query_view(
        &snapshot,
        &groups(MIXED_GROUPS),
        ViewQuery {
            group_id: Some("group-docs".into()),
            ..ViewQuery::default()
        },
    );

    // 行只含所选组：docs-site（remote_ahead）与 website（synced）。
    assert_eq!(row_ids(&result), vec!["repo-docs", "repo-web"]);
    // 状态计数只基于所选组。
    assert_eq!(result.status_counts.total, 2);
    assert_eq!(result.status_counts.remote_ahead, 1);
    assert_eq!(result.status_counts.synced, 1);
    assert_eq!(result.status_counts.error, 0);
    // 摘要与组计数仍覆盖全部仓库（筛选不改变全部摘要）。
    assert_eq!(result.summary_counts.total, 8);
    assert_eq!(result.summary_counts.error, 1);
    assert_eq!(result.effective.group_id.as_deref(), Some("group-docs"));
}

#[test]
fn relation_filter_selects_matching_rows_and_excludes_pending() {
    let snapshot = load_snapshot("snapshot-mixed-8.json");
    let query = |filter| ViewQuery {
        relation_filter: filter,
        ..ViewQuery::default()
    };

    let diverged = query_view(
        &snapshot,
        &groups(MIXED_GROUPS),
        query(RelationFilter::Diverged),
    );
    assert_eq!(row_ids(&diverged), vec!["repo-api"]);

    let errors = query_view(
        &snapshot,
        &groups(MIXED_GROUPS),
        query(RelationFilter::Error),
    );
    assert_eq!(row_ids(&errors), vec!["repo-ci"]);

    let no_remote = query_view(
        &snapshot,
        &groups(MIXED_GROUPS),
        query(RelationFilter::NoRemote),
    );
    assert_eq!(row_ids(&no_remote), vec!["repo-legacy", "repo-notes"]);

    // pending 只出现在"全部状态"。
    let pending_snapshot = load_snapshot("snapshot-pending-first.json");
    let all = query_view(&pending_snapshot, &[], ViewQuery::default());
    assert_eq!(all.rows.len(), 2);
    assert!(all
        .rows
        .iter()
        .all(|row| row.read_state == ReadState::Pending));
    let synced = query_view(&pending_snapshot, &[], query(RelationFilter::Synced));
    // 协调：所选状态在范围内不存在 → 切回全部状态（仍显示 pending 行）。
    assert_eq!(synced.effective.relation_filter, RelationFilter::All);
    assert_eq!(synced.rows.len(), 2);
}

#[test]
fn search_matches_name_path_branch_and_group_name_case_folded() {
    let snapshot = load_snapshot("snapshot-mixed-8.json");
    let groups = groups(MIXED_GROUPS);

    // 名称匹配。
    let by_name = query_view(
        &snapshot,
        &groups,
        ViewQuery {
            query: "MOBILE".into(),
            ..ViewQuery::default()
        },
    );
    assert_eq!(row_ids(&by_name), vec!["repo-mobile"]);

    // 路径匹配。
    let by_path = query_view(
        &snapshot,
        &groups,
        ViewQuery {
            query: "projects/api".into(),
            ..ViewQuery::default()
        },
    );
    assert_eq!(row_ids(&by_path), vec!["repo-api"]);

    // 分支匹配（多行按排序规则）。
    let by_branch = query_view(
        &snapshot,
        &groups,
        ViewQuery {
            query: "feature/login".into(),
            ..ViewQuery::default()
        },
    );
    assert_eq!(row_ids(&by_branch), vec!["repo-mobile"]);

    // 分组名匹配（中文，Unicode lowercase 后包含）。
    let by_group = query_view(
        &snapshot,
        &groups,
        ViewQuery {
            query: "文档".into(),
            ..ViewQuery::default()
        },
    );
    assert_eq!(row_ids(&by_group), vec!["repo-docs", "repo-web"]);

    // 零结果：行空，但搜索词保留、摘要不变。
    let empty = query_view(
        &snapshot,
        &groups,
        ViewQuery {
            query: "zzz-no-match".into(),
            ..ViewQuery::default()
        },
    );
    assert!(empty.rows.is_empty());
    assert_eq!(empty.effective.query, "zzz-no-match");
    assert_eq!(empty.summary_counts.total, 8);
}

#[test]
fn unicode_names_search_and_sort() {
    let snapshot = load_snapshot("snapshot-unicode.json");
    let groups = groups(&[("组-产品", "产品线")]);

    let result = query_view(
        &snapshot,
        &groups,
        ViewQuery {
            query: "前端".into(),
            ..ViewQuery::default()
        },
    );
    assert_eq!(row_ids(&result), vec!["repo-frontend"]);

    // 全量排序：diverged（前端应用）→ local_ahead（文档中心）→ no_remote（备用仓库）。
    let all = query_view(&snapshot, &groups, ViewQuery::default());
    assert_eq!(
        row_ids(&all),
        vec!["repo-frontend", "repo-docs-cn", "repo-archive-cn"]
    );
}

#[test]
fn missing_group_falls_back_to_all() {
    let snapshot = load_snapshot("snapshot-mixed-8.json");
    let result = query_view(
        &snapshot,
        &groups(MIXED_GROUPS),
        ViewQuery {
            group_id: Some("group-deleted".into()),
            ..ViewQuery::default()
        },
    );
    assert_eq!(result.effective.group_id, None);
    assert_eq!(result.rows.len(), 8);
    // 已删除组不出现在组计数中。
    assert!(!result.groups.iter().any(|g| g.id == "group-deleted"));
}

#[test]
fn filter_and_search_apply_in_sequence() {
    let snapshot = load_snapshot("snapshot-mixed-8.json");
    let result = query_view(
        &snapshot,
        &groups(MIXED_GROUPS),
        ViewQuery {
            relation_filter: RelationFilter::Synced,
            query: "web".into(),
            ..ViewQuery::default()
        },
    );
    // synced 中名称含 web 的只有 website。
    assert_eq!(row_ids(&result), vec!["repo-web"]);
    // 状态计数不受搜索缩小：仍包含 cli-tools。
    assert_eq!(result.status_counts.synced, 2);
}

#[test]
fn view_result_serializes_camel_case_for_contract() {
    let snapshot = load_snapshot("snapshot-mixed-8.json");
    let result = query_view(&snapshot, &groups(MIXED_GROUPS), ViewQuery::default());
    let json = serde_json::to_value(&result).unwrap();
    for key in [
        "sessionId",
        "stateRevision",
        "generation",
        "settingsRevision",
        "effective",
        "summaryCounts",
        "summaryCompact",
        "groups",
        "statusCounts",
        "rows",
    ] {
        assert!(json.get(key).is_some(), "missing {key}");
    }
    assert_eq!(json["effective"]["relationFilter"], "all");
    assert_eq!(json["groups"][0]["id"], "group-backend");
    assert_eq!(json["rows"][0]["id"], "repo-ci");
}

#[test]
fn engine_view_query_uses_current_state_and_groups() {
    // 引擎接线：配置提交后立即用最新组定义与 pending 行回答查询。
    // 采集器仅首次调用阻塞：首扫不完成（pending 状态确定），后续扫描放行，
    // 停止路径不会被第二次 collect 的长等待卡住。
    struct BlockedFirstCollector {
        gate: std::sync::Mutex<std::sync::mpsc::Receiver<()>>,
        blocked_once: std::sync::atomic::AtomicBool,
    }
    impl gitaview_core::refresh::RepoCollector for BlockedFirstCollector {
        fn collect(
            &self,
            _repos: &[gitaview_core::settings::RepoSettings],
            _stop: &gitaview_core::git::StopFlag,
            _policy: &gitaview_core::git::ExecPolicy,
        ) -> Result<Vec<gitaview_core::model::RepoStatus>, String> {
            if !self
                .blocked_once
                .swap(true, std::sync::atomic::Ordering::SeqCst)
            {
                let gate = self.gate.lock().unwrap();
                let _ = gate.recv_timeout(std::time::Duration::from_secs(60));
            }
            Ok(Vec::new())
        }
    }
    let (release, gate) = std::sync::mpsc::channel::<()>();
    let temp = std::env::temp_dir().join(format!(
        "gitaview_n08_engine_view_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let engine = gitaview_core::engine::engine_create(gitaview_core::engine::EngineConfig {
        data_dir: temp.clone(),
        log_dir: temp.join("logs"),
        collector: std::sync::Arc::new(BlockedFirstCollector {
            gate: std::sync::Mutex::new(gate),
            blocked_once: std::sync::atomic::AtomicBool::new(false),
        }),
        ..Default::default()
    })
    .unwrap()
    .engine;

    let repo = temp.join("repo");
    std::fs::create_dir_all(repo.join(".git")).unwrap();
    engine.repo_add(&repo, None).unwrap();

    // 首扫被阻塞：查询立即返回 pending 行（配置提交即重投影）。
    let result = engine.view_query(ViewQuery::default());
    assert_eq!(result.rows.len(), 1);
    assert_eq!(result.rows[0].read_state, ReadState::Pending);
    assert_eq!(result.rows[0].relation, None);
    assert_eq!(result.summary_counts.pending, 1);
    assert_eq!(result.summary_counts.total, 1);
    // 四桶 + pending = total。
    let bucket_sum = result.summary_compact.synced
        + result.summary_compact.syncable
        + result.summary_compact.needs_attention
        + result.summary_compact.no_remote
        + result.summary_counts.pending;
    assert_eq!(bucket_sum, result.summary_counts.total);

    release.send(()).unwrap();
    engine.request_stop();
    assert!(engine.wait_stopped(std::time::Duration::from_secs(10)));
    std::fs::remove_dir_all(temp).unwrap();
}
