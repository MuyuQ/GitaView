//! N05 集成测试：网络操作（fetch / pull / push）的确认、锁、前置校验与执行，
//! 使用本地 bare origin + clone，不访问公网。

use gitaview_core::error::CoreError;
use gitaview_core::git::commands::{
    branch_state, execute_repo_operation, repo_status_from_state, validate_repo_operation,
    ExecPolicy, RepoGitOperation, RepoOperationInput,
};
use gitaview_core::git::discovery::StopFlag;
use gitaview_core::git::operation_lock::OperationLocks;
use gitaview_core::model::{ReadState, Relation, RepoActions};
use gitaview_core::registry::RepoRecord;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn unique_temp_dir(name: &str) -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("{name}_{suffix}"))
}

fn test_git(repo_path: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// 建立带本地 bare origin 的仓库；返回 (repo, remote)。
fn init_repo_with_origin(temp: &Path, name: &str) -> (PathBuf, PathBuf) {
    let remote = temp.join(format!("{name}.git"));
    let repo = temp.join(name);
    fs::create_dir_all(&remote).unwrap();
    fs::create_dir_all(&repo).unwrap();
    test_git(&remote, &["init", "--bare", "-b", "main"]);
    test_git(&repo, &["init", "-b", "main"]);
    test_git(&repo, &["config", "user.email", "test@example.test"]);
    test_git(&repo, &["config", "user.name", "Test"]);
    test_git(
        &repo,
        &["remote", "add", "origin", remote.to_str().unwrap()],
    );
    (repo, remote)
}

fn run_op(
    locks: &OperationLocks,
    repo: &Path,
    operation: RepoGitOperation,
    confirmed: bool,
) -> Result<gitaview_core::git::commands::RepoOperationOutcome, CoreError> {
    execute_repo_operation(
        locks,
        RepoOperationInput {
            repo_path: repo,
            operation,
            confirmed,
        },
        &ExecPolicy::default(),
        &StopFlag::new(),
    )
}

#[test]
fn pull_requires_confirmation_then_executes_and_syncs() {
    let temp = unique_temp_dir("gitaview_n05_pull");
    let (repo, remote) = init_repo_with_origin(&temp, "repo");
    test_git(&repo, &["commit", "--allow-empty", "-m", "initial"]);
    test_git(&repo, &["push", "-u", "origin", "main"]);

    let peer = temp.join("peer");
    test_git(
        &temp,
        &["clone", remote.to_str().unwrap(), peer.to_str().unwrap()],
    );
    test_git(&peer, &["config", "user.email", "test@example.test"]);
    test_git(&peer, &["config", "user.name", "Test"]);
    test_git(&peer, &["commit", "--allow-empty", "-m", "remote change"]);
    test_git(&peer, &["push", "origin", "main"]);
    test_git(&repo, &["fetch", "origin"]);
    assert_eq!(branch_state(&repo).unwrap().relation, Relation::RemoteAhead);

    let locks = OperationLocks::new();
    // 未确认：在启动 Git 前拒绝。
    assert_eq!(
        run_op(&locks, &repo, RepoGitOperation::Pull, false),
        Err(CoreError::ConfirmationRequired)
    );
    // 拒绝后锁被释放，确认后的执行可以继续。
    let outcome = run_op(&locks, &repo, RepoGitOperation::Pull, true).unwrap();
    assert_eq!(outcome.operation, RepoGitOperation::Pull);
    assert_eq!(branch_state(&repo).unwrap().relation, Relation::Synced);

    fs::remove_dir_all(temp).unwrap();
}

#[test]
fn push_requires_confirmation_and_executes_for_local_ahead() {
    let temp = unique_temp_dir("gitaview_n05_push");
    let (repo, remote) = init_repo_with_origin(&temp, "repo");
    test_git(&repo, &["commit", "--allow-empty", "-m", "initial"]);
    test_git(&repo, &["push", "-u", "origin", "main"]);
    test_git(&repo, &["commit", "--allow-empty", "-m", "local change"]);
    assert_eq!(branch_state(&repo).unwrap().relation, Relation::LocalAhead);

    let locks = OperationLocks::new();
    assert_eq!(
        run_op(&locks, &repo, RepoGitOperation::Push, false),
        Err(CoreError::ConfirmationRequired)
    );
    run_op(&locks, &repo, RepoGitOperation::Push, true).unwrap();
    // 远端已前进：本地重新比较回到 synced。
    assert_eq!(branch_state(&repo).unwrap().relation, Relation::Synced);
    test_git(&remote, &["show-ref", "--verify", "refs/heads/main"]);

    fs::remove_dir_all(temp).unwrap();
}

#[test]
fn fetch_works_for_unborn_repo_with_origin_without_confirmation() {
    let temp = unique_temp_dir("gitaview_n05_fetch_unborn");
    let (repo, _remote) = init_repo_with_origin(&temp, "repo");

    let locks = OperationLocks::new();
    // Fetch 不需要确认；有 origin 即可，尚无提交也允许。
    let outcome = run_op(&locks, &repo, RepoGitOperation::Fetch, false).unwrap();
    assert_eq!(outcome.operation, RepoGitOperation::Fetch);
    let state = branch_state(&repo).unwrap();
    assert_eq!(state.relation, Relation::NoRemote);
    assert!(state.has_remote);

    fs::remove_dir_all(temp).unwrap();
}

#[test]
fn network_rules_map_to_stable_error_codes() {
    let temp = unique_temp_dir("gitaview_n05_rules");
    let (repo, _remote) = init_repo_with_origin(&temp, "repo");
    test_git(&repo, &["commit", "--allow-empty", "-m", "initial"]);
    test_git(&repo, &["push", "-u", "origin", "main"]);
    // synced 状态：pull / push 都不允许。
    assert_eq!(branch_state(&repo).unwrap().relation, Relation::Synced);
    let locks = OperationLocks::new();
    assert_eq!(
        run_op(&locks, &repo, RepoGitOperation::Pull, true),
        Err(CoreError::OperationUnavailable)
    );
    assert_eq!(
        run_op(&locks, &repo, RepoGitOperation::Push, true),
        Err(CoreError::OperationUnavailable)
    );

    // 有 origin 但无比较分支：Pull / Push → COMPARISON_REQUIRED。
    let (unborn, _remote2) = init_repo_with_origin(&temp, "unborn");
    assert_eq!(
        run_op(&locks, &unborn, RepoGitOperation::Pull, true),
        Err(CoreError::ComparisonRequired)
    );
    assert_eq!(
        run_op(&locks, &unborn, RepoGitOperation::Push, true),
        Err(CoreError::ComparisonRequired)
    );

    // 无 origin：Fetch → ORIGIN_REQUIRED。
    let lonely = temp.join("lonely");
    fs::create_dir_all(&lonely).unwrap();
    test_git(&lonely, &["init", "-b", "main"]);
    test_git(&lonely, &["config", "user.email", "test@example.test"]);
    test_git(&lonely, &["config", "user.name", "Test"]);
    test_git(&lonely, &["commit", "--allow-empty", "-m", "initial"]);
    assert_eq!(
        run_op(&locks, &lonely, RepoGitOperation::Fetch, false),
        Err(CoreError::OriginRequired)
    );

    fs::remove_dir_all(temp).unwrap();
}

#[test]
fn same_repo_second_network_operation_returns_busy() {
    let temp = unique_temp_dir("gitaview_n05_busy");
    let (repo, _remote) = init_repo_with_origin(&temp, "repo");
    test_git(&repo, &["commit", "--allow-empty", "-m", "initial"]);
    test_git(&repo, &["push", "-u", "origin", "main"]);

    let locks = OperationLocks::new();
    let guard = locks.try_acquire(&repo).unwrap();
    assert_eq!(
        run_op(&locks, &repo, RepoGitOperation::Fetch, false),
        Err(CoreError::RepoBusy)
    );
    // 不同仓库不受影响。
    let (other, _remote2) = init_repo_with_origin(&temp, "other");
    assert!(run_op(&locks, &other, RepoGitOperation::Fetch, false).is_ok());
    drop(guard);
    assert!(run_op(&locks, &repo, RepoGitOperation::Fetch, false).is_ok());

    fs::remove_dir_all(temp).unwrap();
}

#[test]
fn stop_flag_rejects_operation_before_git() {
    let temp = unique_temp_dir("gitaview_n05_stop");
    let (repo, _remote) = init_repo_with_origin(&temp, "repo");

    let stop = StopFlag::new();
    stop.stop();
    let result = execute_repo_operation(
        &OperationLocks::new(),
        RepoOperationInput {
            repo_path: &repo,
            operation: RepoGitOperation::Fetch,
            confirmed: true,
        },
        &ExecPolicy::default(),
        &stop,
    );
    assert_eq!(result, Err(CoreError::EngineStopping));

    fs::remove_dir_all(temp).unwrap();
}

#[test]
fn injected_short_timeout_maps_to_git_timeout() {
    let temp = unique_temp_dir("gitaview_n05_timeout");
    let (repo, _remote) = init_repo_with_origin(&temp, "repo");
    test_git(&repo, &["commit", "--allow-empty", "-m", "initial"]);

    let policy = ExecPolicy {
        status_timeout: Duration::ZERO,
        network_timeout: Duration::ZERO,
    };
    let result = execute_repo_operation(
        &OperationLocks::new(),
        RepoOperationInput {
            repo_path: &repo,
            operation: RepoGitOperation::Fetch,
            confirmed: true,
        },
        &policy,
        &StopFlag::new(),
    );
    // 与等待 TIMEOUT（FFI 状态 2）区分：Git 命令超时是业务错误 GIT_TIMEOUT。
    assert_eq!(result, Err(CoreError::GitTimeout));

    fs::remove_dir_all(temp).unwrap();
}

#[test]
fn validate_rules_cover_the_full_operation_table() {
    use Relation::*;
    let ok = |op, relation, has_origin| validate_repo_operation(op, relation, has_origin).is_ok();
    // Fetch：有 origin 一律允许（含无比较分支）。
    for relation in [Synced, LocalAhead, RemoteAhead, Diverged, NoRemote] {
        assert!(ok(RepoGitOperation::Fetch, relation, true));
    }
    assert!(!ok(RepoGitOperation::Fetch, NoRemote, false));

    assert!(ok(RepoGitOperation::Pull, RemoteAhead, true));
    assert!(ok(RepoGitOperation::Pull, Diverged, true));
    assert!(!ok(RepoGitOperation::Pull, Synced, true));
    assert!(!ok(RepoGitOperation::Pull, LocalAhead, true));

    assert!(ok(RepoGitOperation::Push, LocalAhead, true));
    assert!(ok(RepoGitOperation::Push, Diverged, true));
    assert!(!ok(RepoGitOperation::Push, Synced, true));
    assert!(!ok(RepoGitOperation::Push, RemoteAhead, true));

    assert_eq!(
        validate_repo_operation(RepoGitOperation::Pull, NoRemote, true),
        Err(CoreError::ComparisonRequired)
    );
    assert_eq!(
        validate_repo_operation(RepoGitOperation::Push, NoRemote, false),
        Err(CoreError::OriginRequired)
    );
}

#[test]
fn repo_status_rows_carry_actions_and_error_codes() {
    let record = RepoRecord {
        id: "repo-x".into(),
        name: "X".into(),
        path: PathBuf::from("/private/x"),
        group_id: Some("group-a".into()),
    };

    let make_state = |relation| gitaview_core::git::GitBranchState {
        branch: "main".into(),
        remote_branch: Some("main".into()),
        relation,
        ahead: 1,
        behind: 0,
        has_remote: true,
        has_commit: true,
        remote_url: Some("https://github.com/example/x".into()),
    };

    let diverged = repo_status_from_state(&record, Ok(make_state(Relation::Diverged)));
    assert_eq!(diverged.read_state, ReadState::Ok);
    assert_eq!(diverged.relation, Some(Relation::Diverged));
    assert_eq!(
        diverged.actions,
        RepoActions {
            can_open_directory: true,
            can_open_remote: true,
            can_fetch: true,
            can_pull: true,
            can_push: true,
        }
    );

    let local_ahead = repo_status_from_state(&record, Ok(make_state(Relation::LocalAhead)));
    assert!(local_ahead.actions.can_push);
    assert!(!local_ahead.actions.can_pull);

    let remote_ahead = repo_status_from_state(&record, Ok(make_state(Relation::RemoteAhead)));
    assert!(remote_ahead.actions.can_pull);
    assert!(!remote_ahead.actions.can_push);

    let failed = repo_status_from_state(
        &record,
        Err(gitaview_core::git::GitCommandError::Failed("boom".into())),
    );
    assert_eq!(failed.read_state, ReadState::Error);
    assert_eq!(failed.relation, None);
    assert_eq!(failed.error.as_ref().unwrap().code, "GIT_FAILED");
    assert_eq!(failed.actions, RepoActions::directory_only());

    // 无 origin：读取成功但网络动作全禁用。
    let mut no_origin_state = make_state(Relation::NoRemote);
    no_origin_state.has_remote = false;
    no_origin_state.remote_url = None;
    let no_origin = repo_status_from_state(&record, Ok(no_origin_state));
    assert_eq!(no_origin.actions, RepoActions::directory_only());
}

#[test]
fn worktree_reports_its_own_branch_with_origin_flag() {
    let temp = unique_temp_dir("gitaview_n05_worktree");
    let (repo, _remote) = init_repo_with_origin(&temp, "repo");
    test_git(&repo, &["commit", "--allow-empty", "-m", "initial"]);
    test_git(&repo, &["push", "-u", "origin", "main"]);

    let worktree = temp.join("wt");
    test_git(
        &repo,
        &[
            "worktree",
            "add",
            worktree.to_str().unwrap(),
            "-b",
            "feature",
        ],
    );
    let state = branch_state(&worktree).unwrap();
    assert_eq!(state.branch, "feature");
    // 新分支没有 origin 比较目标：no_remote，但 origin 存在。
    assert_eq!(state.relation, Relation::NoRemote);
    assert!(state.has_remote);

    let main_state = branch_state(&repo).unwrap();
    assert_eq!(main_state.relation, Relation::Synced);
    assert_eq!(main_state.remote_branch.as_deref(), Some("main"));

    fs::remove_dir_all(temp).unwrap();
}

#[test]
fn detached_head_reads_as_no_remote_with_origin_flag() {
    let temp = unique_temp_dir("gitaview_n05_detached");
    let (repo, _remote) = init_repo_with_origin(&temp, "repo");
    test_git(&repo, &["commit", "--allow-empty", "-m", "initial"]);
    test_git(&repo, &["push", "-u", "origin", "main"]);
    test_git(&repo, &["checkout", "--detach", "HEAD"]);

    let state = branch_state(&repo).unwrap();
    assert_eq!(state.branch, "HEAD");
    assert_eq!(state.relation, Relation::NoRemote);
    assert_eq!(state.ahead, 0);
    assert_eq!(state.behind, 0);
    assert!(state.has_remote);
    assert_eq!(state.remote_branch, None);

    // detached 无比较目标：不允许 Pull / Push。
    let locks = OperationLocks::new();
    assert_eq!(
        run_op(&locks, &repo, RepoGitOperation::Pull, true),
        Err(CoreError::ComparisonRequired)
    );

    fs::remove_dir_all(temp).unwrap();
}

#[test]
fn stale_upstream_falls_back_to_matching_origin_branch() {
    let temp = unique_temp_dir("gitaview_n05_stale_upstream");
    let (repo, _remote) = init_repo_with_origin(&temp, "repo");
    test_git(&repo, &["commit", "--allow-empty", "-m", "initial"]);
    test_git(&repo, &["push", "-u", "origin", "main"]);
    // 本地前进一笔后把 upstream 指向不存在的目标：应回退到 origin/main 比较。
    test_git(&repo, &["commit", "--allow-empty", "-m", "local change"]);
    test_git(&repo, &["config", "branch.main.remote", "origin"]);
    test_git(&repo, &["config", "branch.main.merge", "refs/heads/master"]);

    let state = branch_state(&repo).unwrap();
    assert_eq!(state.relation, Relation::LocalAhead);
    assert_eq!(state.ahead, 1);
    assert_eq!(state.behind, 0);
    assert_eq!(state.remote_branch.as_deref(), Some("main"));

    fs::remove_dir_all(temp).unwrap();
}

#[test]
fn local_branch_renamed_still_uses_origin_target_for_operations() {
    let temp = unique_temp_dir("gitaview_n05_renamed");
    let (repo, remote) = init_repo_with_origin(&temp, "repo");
    test_git(&repo, &["commit", "--allow-empty", "-m", "initial"]);
    test_git(&repo, &["push", "-u", "origin", "main"]);
    // 本地改名后继续提交：比较与 Pull/Push 目标都是 origin/main。
    test_git(&repo, &["branch", "-m", "feature"]);
    test_git(&repo, &["commit", "--allow-empty", "-m", "local"]);

    let state = branch_state(&repo).unwrap();
    assert_eq!(state.branch, "feature");
    assert_eq!(state.remote_branch.as_deref(), Some("main"));
    assert_eq!(state.relation, Relation::LocalAhead);

    let locks = OperationLocks::new();
    run_op(&locks, &repo, RepoGitOperation::Push, true).unwrap();
    // 推送目标是 refs/heads/main，不是本地分支名。
    test_git(&remote, &["show-ref", "--verify", "refs/heads/main"]);
    assert_eq!(branch_state(&repo).unwrap().relation, Relation::Synced);

    fs::remove_dir_all(temp).unwrap();
}

#[test]
fn actions_rule_table_matches_contract() {
    use gitaview_core::model::RepoActions;
    let dir_only = RepoActions::directory_only();
    // pending / error：目录入口。
    assert_eq!(
        RepoActions::for_read(ReadState::Pending, None, true, true),
        dir_only
    );
    assert_eq!(
        RepoActions::for_read(ReadState::Error, None, true, true),
        dir_only
    );
    // 无 origin：一切网络动作禁用。
    assert_eq!(
        RepoActions::for_read(ReadState::Ok, Some(Relation::Diverged), false, false),
        dir_only
    );
    // 有 origin、不安全 URL：可 Fetch，不可打开远端。
    let actions = RepoActions::for_read(ReadState::Ok, Some(Relation::Synced), true, false);
    assert!(
        actions.can_fetch && !actions.can_open_remote && !actions.can_pull && !actions.can_push
    );
}
