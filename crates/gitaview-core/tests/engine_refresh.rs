//! N07 引擎状态机集成测试：假采集器 + 假时钟，覆盖任务清单的全部验证场景。
//! 不执行真实 Git（Git 执行在 network_operations / refresh 模块单独测试）。

use gitaview_core::engine::{
    engine_create, Engine, EngineClock, EngineConfig, Lifecycle, SnapshotWait,
};
use gitaview_core::error::CoreError;
use gitaview_core::git::ExecPolicy;
use gitaview_core::model::{AppSnapshot, ReadState, RefreshState, Relation, RepoStatus};
use gitaview_core::refresh::RepoCollector;
use gitaview_core::settings::{RepoSettings, SettingsSaveInput, SettingsService};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

// —— 测试替身 ——

struct FakeClock {
    mono: AtomicU64,
    wall: AtomicU64,
}

impl FakeClock {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            mono: AtomicU64::new(1_000_000),
            wall: AtomicU64::new(1_792_800_000_000),
        })
    }

    fn advance_millis(self: &Arc<Self>, millis: u64) {
        self.mono.fetch_add(millis, Ordering::SeqCst);
    }

    fn set_wall_ms(self: &Arc<Self>, value: u64) {
        self.wall.store(value, Ordering::SeqCst);
    }
}

impl EngineClock for FakeClock {
    fn monotonic_ms(&self) -> u64 {
        self.mono.load(Ordering::SeqCst)
    }

    fn wall_ms(&self) -> u64 {
        self.wall.load(Ordering::SeqCst)
    }
}

type CollectAction = Box<dyn Fn(&[RepoSettings]) -> Result<Vec<RepoStatus>, String> + Send>;

/// 脚本化采集器：前 N 次按脚本动作（先进先出），之后默认按当前配置全部读取成功。
struct ScriptedCollector {
    calls: AtomicUsize,
    script: Mutex<std::collections::VecDeque<CollectAction>>,
}

impl ScriptedCollector {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            calls: AtomicUsize::new(0),
            script: Mutex::new(std::collections::VecDeque::new()),
        })
    }

    fn push(self: &Arc<Self>, action: CollectAction) -> Arc<Self> {
        self.script.lock().unwrap().push_back(action);
        Arc::clone(self)
    }

    fn push_ok(self: &Arc<Self>) -> Arc<Self> {
        self.push(Box::new(|repos| Ok(ok_rows(repos))))
    }

    fn push_fail(self: &Arc<Self>) -> Arc<Self> {
        self.push(Box::new(|_| Err("collect failed".to_string())))
    }

    /// 阻塞动作：等待 release 信号后返回成功行。
    fn push_blocked(self: &Arc<Self>) -> (Arc<Self>, mpsc::Sender<()>) {
        let (sender, receiver) = mpsc::channel::<()>();
        let blocked = self.push(Box::new(move |repos| {
            receiver
                .recv_timeout(Duration::from_secs(30))
                .expect("test gate released");
            Ok(ok_rows(repos))
        }));
        (blocked, sender)
    }

    fn call_count(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl RepoCollector for ScriptedCollector {
    fn collect(
        &self,
        repos: &[RepoSettings],
        _stop: &gitaview_core::git::StopFlag,
        _policy: &ExecPolicy,
    ) -> Result<Vec<RepoStatus>, String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let action = self.script.lock().unwrap().pop_front();
        match action {
            Some(action) => action(repos),
            None => Ok(ok_rows(repos)),
        }
    }
}

fn ok_rows(repos: &[RepoSettings]) -> Vec<RepoStatus> {
    repos
        .iter()
        .map(|repo| {
            let mut row = RepoStatus::pending(&repo.id, &repo.name, &repo.path);
            row.read_state = ReadState::Ok;
            row.relation = Some(Relation::Synced);
            row.branch = Some("main".into());
            row.has_commit = Some(true);
            row.has_origin = Some(false);
            row.ahead = Some(0);
            row.behind = Some(0);
            row
        })
        .collect()
}

// —— 工具 ——

fn unique_temp_dir(name: &str) -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("{name}_{suffix}"))
}

fn fake_repo(temp: &Path, name: &str) -> PathBuf {
    let repo = temp.join(name);
    fs::create_dir_all(repo.join(".git")).unwrap();
    repo
}

/// 建引擎前预置仓库：初始扫描直接读取它们，测试不依赖 add 的次序。
fn seed_repos(temp: &Path, names: &[&str]) {
    let service = SettingsService::open(temp).unwrap().service;
    for name in names {
        let repo = fake_repo(temp, name);
        service.repo_add(&repo, None).unwrap();
    }
}

fn test_engine(temp: &Path, collector: Arc<ScriptedCollector>, clock: Arc<FakeClock>) -> Engine {
    engine_create(EngineConfig {
        data_dir: temp.to_path_buf(),
        log_dir: temp.join("logs"),
        persist: None,
        policy: ExecPolicy::default(),
        clock,
        collector,
        session_id: Some("test-session".into()),
        pre_slot_hook: None,
    })
    .unwrap()
    .engine
}

fn wait_for_snapshot(
    engine: &Engine,
    what: &str,
    predicate: impl Fn(&AppSnapshot) -> bool,
) -> AppSnapshot {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let snapshot = engine.snapshot_get();
        if predicate(&snapshot) {
            return snapshot;
        }
        if Instant::now() >= deadline {
            panic!("timeout waiting for: {what}; last snapshot = {snapshot:#?}");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn wait_for_calls(collector: &ScriptedCollector, expected: usize) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while collector.call_count() < expected {
        assert!(
            Instant::now() < deadline,
            "collector never reached {expected} calls"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Windows 文件延迟释放 + 其他测试引擎仍写首个日志目录：删除需重试。
fn remove_dir_retry(path: &Path) {
    for _ in 0..20 {
        if fs::remove_dir_all(path).is_ok() {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!("remove_dir_all failed: {}", path.display());
}

fn stop_engine(engine: &Engine) {
    engine.request_stop();
    assert!(
        engine.wait_stopped(Duration::from_secs(10)),
        "engine must stop within timeout"
    );
}

// —— 场景 ——

#[test]
fn initial_snapshot_is_pending_without_faking_success() {
    let temp = unique_temp_dir("gitaview_n07_initial");
    let (collector, gate) = ScriptedCollector::new().push_blocked();
    let engine = test_engine(&temp, collector, FakeClock::new());

    let snapshot = engine.snapshot_get();
    assert_eq!(snapshot.generation, 0);
    assert_eq!(snapshot.refresh_state, RefreshState::Pending);
    assert_eq!(snapshot.attempted_at_ms, None);
    assert_eq!(snapshot.last_success_at_ms, None);
    // 初始即安排首次读取：有未完成请求。
    assert!(snapshot.refresh.requested_request_id == 1);
    assert_eq!(snapshot.refresh.completed_request_id, 0);
    // 释放阻塞采集，让停止路径确定性地结束（stop 与在途采集的时序竞态）。
    gate.send(()).ok();
    stop_engine(&engine);
    remove_dir_retry(&temp);
}

#[test]
fn fifty_requests_coalesce_into_one_followup_scan() {
    let temp = unique_temp_dir("gitaview_n07_coalesce");
    let (collector, release) = ScriptedCollector::new().push_blocked();
    let engine = test_engine(&temp, Arc::clone(&collector), FakeClock::new());

    wait_for_calls(&collector, 1);
    let after_first = engine.snapshot_get().refresh.requested_request_id;
    for _ in 0..50 {
        engine.refresh_request().unwrap();
    }
    let latest = engine.snapshot_get().refresh.requested_request_id;
    assert!(latest >= after_first + 50);

    release.send(()).unwrap();
    let final_snapshot =
        wait_for_snapshot(&engine, "completed covers the latest request", |snapshot| {
            snapshot.refresh.completed_request_id >= latest
        });
    wait_for_calls(&collector, 2);
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(
        collector.call_count(),
        2,
        "50 requests during a scan must coalesce into exactly one follow-up"
    );
    assert!(!final_snapshot.refresh.is_refreshing);
    assert_eq!(final_snapshot.refresh.completed_request_id, latest);
    stop_engine(&engine);
    remove_dir_retry(&temp);
}

#[test]
fn old_scan_does_not_complete_newer_requests_prematurely() {
    let temp = unique_temp_dir("gitaview_n07_oldscan");
    let (collector, release_a) = ScriptedCollector::new().push_blocked();
    let (collector, release_b) = collector.push_blocked();
    let engine = test_engine(&temp, Arc::clone(&collector), FakeClock::new());

    wait_for_calls(&collector, 1);
    let (_, request_b) = engine.refresh_request().unwrap();
    release_a.send(()).unwrap();

    // 扫描 A 完成只覆盖它捕获的请求（=1），不提前完成 request_b。
    let after_a = wait_for_snapshot(&engine, "scan A completes its own request", |snapshot| {
        snapshot.refresh.completed_request_id == 1
    });
    assert!(
        after_a.refresh.completed_request_id < request_b,
        "scan A must not complete requests issued during its run"
    );
    wait_for_calls(&collector, 2);

    release_b.send(()).unwrap();
    wait_for_snapshot(&engine, "scan B completes request_b", |snapshot| {
        snapshot.refresh.completed_request_id >= request_b
    });
    stop_engine(&engine);
    remove_dir_retry(&temp);
}

#[test]
fn discarded_scan_keeps_request_pending_until_next_completes() {
    let temp = unique_temp_dir("gitaview_n07_discard");
    let repo = fake_repo(&temp, "repo");
    let (collector, release_a) = ScriptedCollector::new().push_blocked();
    let collector = collector.push_ok();
    let engine = test_engine(&temp, Arc::clone(&collector), FakeClock::new());

    // 扫描 A（旧配置）运行期间：先添加、再删除仓库，revision 变化两次。
    engine.repo_add(&repo, None).unwrap();
    engine.repo_remove("repo").unwrap();
    wait_for_calls(&collector, 1);

    release_a.send(()).unwrap();
    // A 按旧 revision 丢弃：不发布、不推进 completed；下一轮覆盖请求并发布。
    let final_snapshot = wait_for_snapshot(
        &engine,
        "follow-up scan publishes and completes",
        |snapshot| {
            snapshot.refresh.completed_request_id >= snapshot.refresh.requested_request_id
                && snapshot.refresh_state == RefreshState::Ready
        },
    );
    wait_for_calls(&collector, 2);
    assert_eq!(
        final_snapshot.generation, 1,
        "only the accepted scan counts"
    );
    assert!(
        final_snapshot.repos.is_empty(),
        "stale rows must not publish"
    );
    stop_engine(&engine);
    remove_dir_retry(&temp);
}

#[test]
fn global_failure_completes_request_and_keeps_history() {
    let temp = unique_temp_dir("gitaview_n07_global_fail");
    seed_repos(&temp, &["repo"]);
    let collector = ScriptedCollector::new().push_ok().push_fail();
    let engine = test_engine(&temp, Arc::clone(&collector), FakeClock::new());

    let ready = wait_for_snapshot(&engine, "first scan succeeds", |snapshot| {
        snapshot.refresh_state == RefreshState::Ready
    });
    let success_time = ready.last_success_at_ms.unwrap();
    assert_eq!(ready.repos.len(), 1);
    let generation_before = ready.generation;

    let (_, request) = engine.refresh_request().unwrap();
    let failed = wait_for_snapshot(
        &engine,
        "second scan fails globally but completes the request",
        |snapshot| snapshot.refresh.completed_request_id >= request,
    );
    assert_eq!(failed.refresh_state, RefreshState::Failed);
    assert!(failed.refresh_error.is_some());
    // 历史行与成功时间保留。
    assert_eq!(failed.repos.len(), 1);
    assert_eq!(failed.repos[0].read_state, ReadState::Ok);
    assert_eq!(failed.last_success_at_ms, Some(success_time));
    assert!(failed.attempted_at_ms.unwrap() >= success_time);
    // 全局失败也占一代、结束请求忙状态。
    assert_eq!(failed.generation, generation_before + 1);
    assert!(!failed.refresh.is_refreshing);
    stop_engine(&engine);
    remove_dir_retry(&temp);
}

#[test]
fn config_commit_reprojects_immediately_and_wakes_subscribers() {
    let temp = unique_temp_dir("gitaview_n07_reproject");
    seed_repos(&temp, &["repo"]);
    let (collector, release_second) = ScriptedCollector::new().push_ok().push_blocked();
    let engine = test_engine(&temp, Arc::clone(&collector), FakeClock::new());

    let ready = wait_for_snapshot(&engine, "first scan ready", |snapshot| {
        snapshot.refresh_state == RefreshState::Ready
    });
    let cursor = ready.state_revision;

    let waiter_engine = engine.clone();
    let waiter = std::thread::spawn(move || {
        waiter_engine.snapshot_wait("test-session", cursor, Duration::from_secs(10))
    });
    std::thread::sleep(Duration::from_millis(100));

    // 新增仓库：提交返回前状态已重投影（第二扫描被门挡住）。
    let new_repo = fake_repo(&temp, "repo-new");
    engine.repo_add(&new_repo, None).unwrap();

    match waiter.join().unwrap() {
        SnapshotWait::Ready(snapshot) => {
            assert_eq!(snapshot.repos.len(), 2);
            let new_row = snapshot
                .repos
                .iter()
                .find(|row| row.id == "repo-new")
                .unwrap();
            assert_eq!(new_row.read_state, ReadState::Pending);
            assert_eq!(new_row.relation, None);
            assert_eq!(
                snapshot.last_success_at_ms, ready.last_success_at_ms,
                "history time is preserved"
            );
            assert_eq!(
                snapshot.generation, ready.generation,
                "config commit must not bump generation"
            );
        }
        other => panic!("expected Ready, got {other:?}"),
    }
    // 重投影先于第二扫描完成：门仍挡住第二扫描时，订阅者已拿到 pending 行
    //（上面 waiter 的 Ready 已证明）。这里仅确认第二扫描尚未发布新结果。
    assert!(engine
        .snapshot_get()
        .repos
        .iter()
        .any(|row| row.id == "repo-new" && row.read_state == ReadState::Pending));

    release_second.send(()).unwrap();
    wait_for_snapshot(&engine, "new repo read", |snapshot| {
        snapshot
            .repos
            .iter()
            .find(|row| row.id == "repo-new")
            .is_some_and(|row| row.read_state == ReadState::Ok)
    });
    stop_engine(&engine);
    remove_dir_retry(&temp);
}

#[test]
fn removed_repo_stays_gone_when_next_scan_fails() {
    let temp = unique_temp_dir("gitaview_n07_remove_fail");
    seed_repos(&temp, &["repo-a", "repo-b"]);
    let collector = ScriptedCollector::new().push_ok().push_fail();
    let engine = test_engine(&temp, Arc::clone(&collector), FakeClock::new());

    let ready = wait_for_snapshot(&engine, "both repos read", |snapshot| {
        snapshot.repos.len() == 2
            && snapshot
                .repos
                .iter()
                .all(|row| row.read_state == ReadState::Ok)
    });
    assert_eq!(ready.refresh_state, RefreshState::Ready);

    engine.repo_remove("repo-a").unwrap();
    // 删除后的首次扫描全局失败：repo-a 不得复活，repo-b 历史保留。
    let failed = wait_for_snapshot(&engine, "post-removal scan fails", |snapshot| {
        snapshot.refresh_state == RefreshState::Failed
    });
    let ids: Vec<&str> = failed.repos.iter().map(|row| row.id.as_str()).collect();
    assert_eq!(ids, vec!["repo-b"]);
    assert_eq!(failed.last_success_at_ms, ready.last_success_at_ms);
    stop_engine(&engine);
    remove_dir_retry(&temp);
}

#[test]
fn resume_coalesces_into_single_request_and_manual_mode_stays_manual() {
    let temp = unique_temp_dir("gitaview_n07_resume");
    let collector = ScriptedCollector::new().push_ok();
    let clock = FakeClock::new();
    let engine = test_engine(&temp, Arc::clone(&collector), Arc::clone(&clock));

    let ready = wait_for_snapshot(&engine, "first scan ready", |snapshot| {
        snapshot.refresh_state == RefreshState::Ready
    });
    let requested = ready.refresh.requested_request_id;

    // 单调时钟前进 5 个间隔后唤醒：只合并一次刷新，不补 5 次。
    clock.advance_millis(5 * 5 * 60_000);
    engine.notify_system_resume().unwrap();
    wait_for_snapshot(&engine, "exactly one merged refresh request", |snapshot| {
        snapshot.refresh.requested_request_id == requested + 1
            && snapshot.refresh.completed_request_id == requested + 1
    });
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(
        engine.snapshot_get().refresh.requested_request_id,
        requested + 1
    );

    // 手动模式：唤醒不自发扫描。
    engine
        .save_settings(SettingsSaveInput {
            expected_revision: engine.get_settings().revision,
            settings: gitaview_core::settings::SettingsEditable {
                groups: vec![],
                default_group_id: None,
                refresh: gitaview_core::model::RefreshPolicy {
                    enabled: false,
                    interval_minutes: 5,
                },
                repo_groups: vec![],
            },
        })
        .unwrap();
    // 等待配置触发的扫描完成，避免与手动模式断言竞争。
    let after_save = engine.snapshot_get().refresh.requested_request_id;
    wait_for_snapshot(&engine, "config-triggered scan settles", |snapshot| {
        snapshot.refresh.completed_request_id >= after_save
    });
    clock.advance_millis(10 * 60_000);
    engine.notify_system_resume().unwrap();
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(
        engine.snapshot_get().refresh.requested_request_id,
        after_save,
        "manual mode must not start scans on resume"
    );
    // 手动刷新仍然有效。
    let (_, manual) = engine.refresh_request().unwrap();
    wait_for_snapshot(&engine, "manual refresh still works", |snapshot| {
        snapshot.refresh.completed_request_id >= manual
    });
    stop_engine(&engine);
    remove_dir_retry(&temp);
}

#[test]
fn timer_requests_once_at_due_and_not_before() {
    let temp = unique_temp_dir("gitaview_n07_timer");
    let collector = ScriptedCollector::new().push_ok();
    let clock = FakeClock::new();
    let engine = test_engine(&temp, Arc::clone(&collector), Arc::clone(&clock));

    let ready = wait_for_snapshot(&engine, "first scan ready", |snapshot| {
        snapshot.refresh_state == RefreshState::Ready
    });
    assert_eq!(ready.refresh.completed_request_id, 1);
    let interval = 5 * 60_000u64;

    // 距期限 1ms：不触发。
    clock.advance_millis(interval - 1);
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(collector.call_count(), 1, "before due must not scan");

    // 越过期限：恰好一次自动扫描。
    clock.advance_millis(2);
    wait_for_calls(&collector, 2);
    wait_for_snapshot(&engine, "timer scan completes", |snapshot| {
        snapshot.refresh.completed_request_id == 2
    });
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(collector.call_count(), 2);
    stop_engine(&engine);
    remove_dir_retry(&temp);
}

#[test]
fn clock_setback_has_defined_display_semantics() {
    let temp = unique_temp_dir("gitaview_n07_setback");
    seed_repos(&temp, &["repo"]);
    let collector = ScriptedCollector::new().push_ok().push_fail();
    let clock = FakeClock::new();
    let engine = test_engine(&temp, Arc::clone(&collector), Arc::clone(&clock));

    let ready = wait_for_snapshot(&engine, "first success", |snapshot| {
        snapshot.refresh_state == RefreshState::Ready
    });
    let success_at = ready.last_success_at_ms.unwrap();

    // 墙钟回拨：失败尝试的 attempted 可能小于 lastSuccess；无下溢、不崩溃。
    clock.set_wall_ms(success_at - 1000);
    let (_, request) = engine.refresh_request().unwrap();
    let failed = wait_for_snapshot(
        &engine,
        "failed attempt recorded with older wall clock",
        |snapshot| snapshot.refresh.completed_request_id >= request,
    );
    assert_eq!(failed.refresh_state, RefreshState::Failed);
    assert_eq!(failed.attempted_at_ms, Some(success_at - 1000));
    assert_eq!(failed.last_success_at_ms, Some(success_at));

    // Widget 侧明确显示规则：成功时间在未来 → FutureTime（不显示永久新鲜）。
    let widget = gitaview_core::widget::project_widget(&failed);
    assert_eq!(
        widget.staleness(success_at - 1001),
        gitaview_core::widget::Staleness::FutureTime
    );
    stop_engine(&engine);
    remove_dir_retry(&temp);
}

#[test]
fn waiters_wake_on_stop_with_stopping_result() {
    let temp = unique_temp_dir("gitaview_n07_wait_stop");
    let (collector, release) = ScriptedCollector::new().push_blocked();
    let engine = test_engine(&temp, collector, FakeClock::new());

    // 首扫已开始（扫描开始本身会递增 stateRevision）后再捕获 cursor。
    let scan_started = wait_for_snapshot(&engine, "first scan starts", |snapshot| {
        snapshot.refresh.is_refreshing
    });
    let cursor = scan_started.state_revision;
    let waiter_engine = engine.clone();
    let started = Instant::now();
    let waiter = std::thread::spawn(move || {
        waiter_engine.snapshot_wait("test-session", cursor, Duration::from_secs(30))
    });
    std::thread::sleep(Duration::from_millis(100));

    engine.request_stop();
    assert_eq!(waiter.join().unwrap(), SnapshotWait::Stopping);
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "stop must wake waiting subscribers immediately"
    );
    // 停止不中断在途采集：释放门后在途调用结束、线程退出。
    release.send(()).unwrap();
    assert!(engine.wait_stopped(Duration::from_secs(10)));
    remove_dir_retry(&temp);
}

#[test]
fn repeated_stop_is_idempotent_and_stopped_engine_rejects_work() {
    let temp = unique_temp_dir("gitaview_n07_stop_idempotent");
    let collector = ScriptedCollector::new();
    let engine = test_engine(&temp, Arc::clone(&collector), FakeClock::new());

    wait_for_snapshot(&engine, "first scan", |snapshot| {
        snapshot.refresh_state == RefreshState::Ready
    });
    let calls_before_stop = collector.call_count();

    engine.request_stop();
    engine.request_stop(); // 幂等
    assert!(engine.wait_stopped(Duration::from_secs(10)));
    assert_eq!(engine.lifecycle(), Lifecycle::Stopped);
    engine.request_stop(); // Stopped 后仍幂等
    assert_eq!(engine.lifecycle(), Lifecycle::Stopped);

    // 停止后不启动新采集；业务操作全部拒绝。
    assert!(matches!(
        engine.refresh_request(),
        Err(CoreError::EngineStopping)
    ));
    assert!(matches!(
        engine.repo_fetch("any"),
        Err(CoreError::EngineStopping)
    ));
    assert!(matches!(
        engine.notify_system_resume(),
        Err(CoreError::EngineStopping)
    ));
    assert_eq!(
        engine.snapshot_wait("test-session", 0, Duration::from_millis(10)),
        SnapshotWait::Stopping
    );
    assert_eq!(
        engine.snapshot_wait("other-session", 0, Duration::from_millis(10)),
        SnapshotWait::Stopping
    );
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(collector.call_count(), calls_before_stop);
    remove_dir_retry(&temp);
}

#[test]
fn stop_during_blocked_scan_discards_result_and_stops() {
    let temp = unique_temp_dir("gitaview_n07_stop_during_scan");
    let (collector, release) = ScriptedCollector::new().push_blocked();
    let engine = test_engine(&temp, Arc::clone(&collector), FakeClock::new());

    wait_for_calls(&collector, 1);
    engine.request_stop();
    // 停止不中断已开始的采集（与 Git 命令的超时语义一致）：释放门让其在途调用结束。
    release.send(()).unwrap();
    assert!(engine.wait_stopped(Duration::from_secs(10)));
    // 结果不被接受：generation 不变、无行发布。
    let snapshot = engine.snapshot_get();
    assert_eq!(snapshot.generation, 0);
    assert!(snapshot.repos.is_empty());
    remove_dir_retry(&temp);
}

#[test]
fn foreign_session_wait_returns_immediately() {
    let temp = unique_temp_dir("gitaview_n07_foreign_session");
    let (collector, gate) = ScriptedCollector::new().push_blocked();
    let engine = test_engine(&temp, collector, FakeClock::new());

    let started = Instant::now();
    match engine.snapshot_wait("different-session", 999, Duration::from_secs(30)) {
        SnapshotWait::Ready(_) => {}
        other => panic!("expected immediate Ready, got {other:?}"),
    }
    assert!(started.elapsed() < Duration::from_secs(2));
    gate.send(()).ok();
    stop_engine(&engine);
    remove_dir_retry(&temp);
}

#[test]
fn snapshot_wait_times_out_when_nothing_changes() {
    let temp = unique_temp_dir("gitaview_n07_wait_timeout");
    let (collector, release) = ScriptedCollector::new().push_blocked();
    let engine = test_engine(&temp, collector, FakeClock::new());

    // 首扫开始后状态稳定，cursor 与当前版本一致 → 无变化可等。
    let scan_started = wait_for_snapshot(&engine, "first scan starts", |snapshot| {
        snapshot.refresh.is_refreshing
    });
    let cursor = scan_started.state_revision;
    let started = Instant::now();
    assert_eq!(
        engine.snapshot_wait("test-session", cursor, Duration::from_millis(200)),
        SnapshotWait::Timeout
    );
    assert!(started.elapsed() >= Duration::from_millis(180));
    release.send(()).unwrap();
    stop_engine(&engine);
    remove_dir_retry(&temp);
}

#[test]
fn startup_warnings_pass_through_engine_create() {
    let temp = unique_temp_dir("gitaview_n07_warnings");
    fs::create_dir_all(&temp).unwrap();
    fs::write(temp.join("settings.json"), "{ broken").unwrap();

    let startup = engine_create(EngineConfig {
        data_dir: temp.to_path_buf(),
        log_dir: temp.join("logs"),
        persist: None,
        policy: ExecPolicy::default(),
        clock: FakeClock::new(),
        collector: ScriptedCollector::new(),
        session_id: None,
        pre_slot_hook: None,
    })
    .unwrap();

    assert_eq!(
        startup.warnings,
        vec![gitaview_core::settings::StartupWarning::SettingsRecovered]
    );
    assert!(!startup.engine.session_id().is_empty());
    stop_engine(&startup.engine);
    remove_dir_retry(&temp);
}

// —— Git 操作与刷新联动（真实 Git；扫描用假采集器隔离）——

fn run_git(repo_path: &Path, args: &[&str]) {
    let output = std::process::Command::new("git")
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

fn init_repo_with_origin(temp: &Path, name: &str) -> PathBuf {
    let remote = temp.join(format!("{name}.git"));
    let repo = temp.join(name);
    fs::create_dir_all(&remote).unwrap();
    fs::create_dir_all(&repo).unwrap();
    run_git(&remote, &["init", "--bare", "-b", "main"]);
    run_git(&repo, &["init", "-b", "main"]);
    run_git(&repo, &["config", "user.email", "test@example.test"]);
    run_git(&repo, &["config", "user.name", "Test"]);
    run_git(
        &repo,
        &["remote", "add", "origin", remote.to_str().unwrap()],
    );
    run_git(&repo, &["commit", "--allow-empty", "-m", "initial"]);
    run_git(&repo, &["push", "-u", "origin", "main"]);
    repo
}

fn init_repo_with_broken_origin(temp: &Path, name: &str) -> PathBuf {
    let repo = temp.join(name);
    fs::create_dir_all(&repo).unwrap();
    run_git(&repo, &["init", "-b", "main"]);
    run_git(&repo, &["config", "user.email", "test@example.test"]);
    run_git(&repo, &["config", "user.name", "Test"]);
    run_git(&repo, &["commit", "--allow-empty", "-m", "initial"]);
    run_git(
        &repo,
        &[
            "remote",
            "add",
            "origin",
            "file:///nonexistent/gitaview-test.git",
        ],
    );
    repo
}

fn settle(engine: &Engine) -> u64 {
    let requested = engine.snapshot_get().refresh.requested_request_id;
    wait_for_snapshot(engine, "scans settle", |snapshot| {
        snapshot.refresh.completed_request_id >= requested
    });
    engine.snapshot_get().refresh.requested_request_id
}

#[test]
fn git_operations_request_refresh_on_success_and_git_failure() {
    let temp = unique_temp_dir("gitaview_n07_op_refresh");
    let good = init_repo_with_origin(&temp, "good");
    let broken = init_repo_with_broken_origin(&temp, "broken");
    let engine = test_engine(&temp, ScriptedCollector::new(), FakeClock::new());

    engine.repo_add(&good, None).unwrap();
    engine.repo_add(&broken, None).unwrap();
    let settled = settle(&engine);

    // 成功的 Fetch → 请求刷新。
    engine.repo_fetch("good").unwrap();
    assert_eq!(
        engine.snapshot_get().refresh.requested_request_id,
        settled + 1
    );
    let settled = settle(&engine);

    // Git 实际执行且失败（fetch 到不存在的远端）→ 同样请求刷新。
    assert_eq!(
        engine.repo_fetch("broken"),
        Err(CoreError::GitFailed),
        "fetch from a missing remote must fail with GIT_FAILED"
    );
    assert_eq!(
        engine.snapshot_get().refresh.requested_request_id,
        settled + 1,
        "git-executed failure must request a refresh"
    );
    let settled = settle(&engine);

    // 预检拒绝（未确认）→ 不请求刷新。
    assert_eq!(
        engine.repo_push("good", false),
        Err(CoreError::ConfirmationRequired)
    );
    assert_eq!(engine.snapshot_get().refresh.requested_request_id, settled);

    stop_engine(&engine);
    remove_dir_retry(&temp);
}

// —— Review 回归（R03 / R04 / R08）——

/// R08：首刷被阻塞时，配置已加入分组的新仓库必须以 pending 行计入该组
///（修复前 pending 行 group_id 固定 None，组计数与组内列表遗漏）。
#[test]
fn pending_rows_carry_configured_group_before_first_scan() {
    let temp = unique_temp_dir("gitaview_r08_pending_group");
    let (collector, gate) = ScriptedCollector::new().push_blocked();
    let engine = test_engine(&temp, collector, FakeClock::new());

    let revision = engine.get_settings().revision;
    engine
        .save_settings(SettingsSaveInput {
            expected_revision: revision,
            settings: gitaview_core::settings::SettingsEditable {
                groups: vec![gitaview_core::settings::GroupSettings {
                    id: "g".into(),
                    name: "工作".into(),
                }],
                default_group_id: None,
                refresh: gitaview_core::model::RefreshPolicy::default(),
                repo_groups: Vec::new(),
            },
        })
        .unwrap();
    let repo = fake_repo(&temp, "grouped");
    let (record, _) = engine.repo_add(&repo, Some("g")).unwrap();

    let snapshot = engine.snapshot_get();
    let row = snapshot
        .repos
        .iter()
        .find(|row| row.id == record.id)
        .expect("pending row present");
    assert_eq!(row.relation, None, "first scan blocked: row stays pending");
    assert_eq!(
        row.group_id.as_deref(),
        Some("g"),
        "pending row must project the configured group"
    );
    let view = engine.view_query(gitaview_core::view::ViewQuery {
        group_id: Some("g".into()),
        ..Default::default()
    });
    assert_eq!(view.status_counts.total, 1, "group count includes pending");

    // 放行被阻塞的首扫，让扫描线程能退出后再停引擎。
    gate.send(()).expect("scan gate open");
    stop_engine(&engine);
    remove_dir_retry(&temp);
}

/// R04：设置页删除分组提交的是空 repoGroups；修复前未列出的仓库保留
/// 悬挂归属，设置页重开时 Groups.First 抛异常。核心必须对全部仓库清归属。
#[test]
fn delete_group_with_empty_assignments_clears_all_dangling_ownership() {
    let temp = unique_temp_dir("gitaview_r04_delete_group");
    let engine = test_engine(&temp, ScriptedCollector::new(), FakeClock::new());

    let revision = engine.get_settings().revision;
    engine
        .save_settings(SettingsSaveInput {
            expected_revision: revision,
            settings: gitaview_core::settings::SettingsEditable {
                groups: vec![gitaview_core::settings::GroupSettings {
                    id: "g".into(),
                    name: "G".into(),
                }],
                default_group_id: Some("g".into()),
                refresh: gitaview_core::model::RefreshPolicy::default(),
                repo_groups: Vec::new(),
            },
        })
        .unwrap();
    let repo = fake_repo(&temp, "member");
    let (record, _) = engine.repo_add(&repo, Some("g")).unwrap();
    assert_eq!(
        engine.snapshot_get().repos[0].group_id.as_deref(),
        Some("g")
    );

    // 真实 UI 删除分组 payload：groups 清空 + repoGroups 空 + 归属列表不覆盖。
    let revision = engine.get_settings().revision;
    let saved = engine
        .save_settings(SettingsSaveInput {
            expected_revision: revision,
            settings: gitaview_core::settings::SettingsEditable {
                groups: Vec::new(),
                default_group_id: None,
                refresh: gitaview_core::model::RefreshPolicy::default(),
                repo_groups: Vec::new(),
            },
        })
        .unwrap();

    let settings = engine.get_settings();
    assert!(settings.groups.is_empty());
    assert_eq!(settings.default_group_id, None);
    let owned = settings
        .repos
        .iter()
        .find(|repo| repo.id == record.id)
        .expect("repo kept");
    assert_eq!(owned.group_id, None, "dangling ownership cleared");
    assert_eq!(engine.snapshot_get().repos[0].group_id, None);

    // 磁盘一致：按同一数据目录重开设置，无警告且归属为空。
    let reopened = SettingsService::open(&temp).unwrap();
    assert!(reopened.warnings.is_empty());
    assert_eq!(reopened.service.get().revision, saved.revision);
    assert_eq!(reopened.service.get().repos[0].group_id, None);

    stop_engine(&engine);
    remove_dir_retry(&temp);
}

/// R03：并发配置提交不得把引擎镜像回退到旧 revision（审查探针
/// CONFIG_REVISION_REGRESSIONS (24, 23) 的引擎层回归）。
#[test]
fn concurrent_config_commits_keep_mirror_monotonic() {
    let temp = unique_temp_dir("gitaview_r03_concurrent_config");
    let engine = test_engine(&temp, ScriptedCollector::new(), FakeClock::new());

    const THREADS: usize = 8;
    const ROUNDS: usize = 12;
    let observed = Arc::new(AtomicU64::new(0));
    let regressions = Arc::new(AtomicUsize::new(0));
    let barrier = Arc::new(std::sync::Barrier::new(THREADS));
    let mut handles = Vec::new();
    for t in 0..THREADS {
        let engine = engine.clone();
        let barrier = Arc::clone(&barrier);
        let observed = Arc::clone(&observed);
        let regressions = Arc::clone(&regressions);
        let temp = temp.clone();
        handles.push(std::thread::spawn(move || {
            for r in 0..ROUNDS {
                barrier.wait();
                let repo = fake_repo(&temp, &format!("t{t}_r{r}"));
                match engine.repo_add(&repo, None) {
                    Ok((_, saved)) => {
                        let previous = observed.fetch_max(saved.revision, Ordering::SeqCst);
                        if saved.revision < previous {
                            regressions.fetch_add(1, Ordering::SeqCst);
                        }
                        let snapshot = engine.snapshot_get();
                        let previous =
                            observed.fetch_max(snapshot.settings_revision, Ordering::SeqCst);
                        if snapshot.settings_revision < previous {
                            regressions.fetch_add(1, Ordering::SeqCst);
                        }
                    }
                    Err(_) => {
                        regressions.fetch_add(1, Ordering::SeqCst);
                    }
                }
            }
        }));
    }
    for handle in handles {
        handle.join().unwrap();
    }
    assert_eq!(
        regressions.load(Ordering::SeqCst),
        0,
        "mirror revision must never regress"
    );
    let settings = engine.get_settings();
    assert_eq!(settings.repos.len(), THREADS * ROUNDS, "no lost adds");
    assert_eq!(
        engine.snapshot_get().settings_revision,
        settings.revision,
        "mirror equals service revision at rest"
    );

    stop_engine(&engine);
    remove_dir_retry(&temp);
}

// —— 复查回归（S02 / S05）——

/// S02：移除 a/shared 后添加 b/shared（同名不同路径，ID 复用），新仓库必须
/// pending、Git 字段为空、网络动作不可用；不得继承旧仓库的采集状态。
#[test]
fn removed_repo_state_is_not_reused_for_same_name_new_repo() {
    let temp = unique_temp_dir("gitaview_s02_identity");
    let dir_a = unique_temp_dir("gitaview_s02_a");
    let dir_b = unique_temp_dir("gitaview_s02_b");
    let repo_a = fake_repo(&dir_a, "shared");
    let repo_b = fake_repo(&dir_b, "shared");

    // 脚本：首采（空配置）ok → A 的采集阻塞 → 后续采集阻塞。
    let (collector, gate_a) = ScriptedCollector::new().push_ok().push_blocked();
    let (collector, gate_hold) = collector.push_blocked();
    let calls = Arc::clone(&collector);
    let engine = test_engine(&temp, collector, FakeClock::new());

    // 首采（空配置）完成后添加 A → 触发的采集被阻塞。
    let (record_a, _) = engine.repo_add(&repo_a, None).unwrap();
    wait_for_calls(&calls, 2);

    // 放行 A 的采集：A 成功进入 collected（generation 2）。
    gate_a.send(()).expect("release A collect");
    let snapshot = wait_for_snapshot(&engine, "A collected", |s| s.generation >= 2);
    let row_a = snapshot
        .repos
        .iter()
        .find(|row| row.id == record_a.id)
        .expect("row A present");
    assert_eq!(row_a.relation, Some(Relation::Synced));

    // 采集阻塞期间：移除 A → 添加 b/shared（同名，ID 复用）。
    engine.repo_remove(&record_a.id).unwrap();
    let (record_b, _) = engine.repo_add(&repo_b, None).unwrap();
    assert_eq!(
        record_a.id, record_b.id,
        "ID 生成在移除后复用同名 ID（既有语义，正因如此必须清缓存 + 核对身份）"
    );

    let snapshot = engine.snapshot_get();
    assert_eq!(
        snapshot.repos.len(),
        1,
        "exactly the new repo remains (same ID, different path)"
    );
    let row_b = snapshot
        .repos
        .iter()
        .find(|row| row.id == record_b.id)
        .expect("row B present");
    assert_eq!(row_b.read_state, ReadState::Pending, "new repo is pending");
    assert_eq!(row_b.relation, None, "no inherited relation");
    assert_eq!(row_b.branch, None, "no inherited branch");
    assert_eq!(row_b.ahead, None, "no inherited ahead");
    assert!(!row_b.actions.can_pull, "pending row cannot pull");
    assert!(!row_b.actions.can_push, "pending row cannot push");

    // 放行被阻塞的采集（其捕获的配置已过时 → 发布按 revision 复核丢弃）：
    // B 不会被旧配置的采集结果污染。
    gate_hold.send(()).expect("release blocked collect");
    let _ = wait_for_snapshot(&engine, "blocked collect settles", |s| {
        !s.refresh.is_refreshing
    });
    stop_engine(&engine);
    remove_dir_retry(&temp);
    let _ = fs::remove_dir_all(&dir_a);
    let _ = fs::remove_dir_all(&dir_b);
}

/// S05：网络操作在"已定位路径、尚未取得操作槽"处被暂停时仓库被移除，
/// 恢复后必须返回 REPO_NOT_FOUND 且不启动 Git 命令。
#[test]
fn removed_repo_between_locate_and_slot_returns_not_found() {
    let temp = unique_temp_dir("gitaview_s05_slot_recheck");
    let (release, gate) = mpsc::channel::<()>();
    let gate = Arc::new(Mutex::new(gate));
    let hook = Arc::new(move || {
        gate.lock()
            .unwrap()
            .recv_timeout(Duration::from_secs(30))
            .expect("test released the pre-slot gate");
    });
    let engine = engine_create(EngineConfig {
        data_dir: temp.to_path_buf(),
        log_dir: temp.join("logs"),
        persist: None,
        policy: ExecPolicy::default(),
        clock: FakeClock::new(),
        collector: ScriptedCollector::new(),
        session_id: Some("test-session".into()),
        pre_slot_hook: Some(hook),
    })
    .unwrap()
    .engine;

    let repo = init_repo_with_origin(&temp, "victim");
    let (record, _) = engine.repo_add(&repo, None).unwrap();
    let _ = wait_for_snapshot(&engine, "initial scan", |s| s.generation >= 1);

    // fetch 停在钩子里（已定位路径、尚未拿槽），此时移除仓库。
    let engine_for_fetch = engine.clone();
    let repo_id = record.id.clone();
    let fetcher = std::thread::spawn(move || engine_for_fetch.repo_fetch(&repo_id));
    std::thread::sleep(Duration::from_millis(300));
    engine.repo_remove(&record.id).unwrap();
    assert!(engine.get_settings().repos.is_empty());
    release.send(()).expect("release fetch");

    let result = fetcher.join().unwrap();
    assert_eq!(
        result,
        Err(CoreError::RepoNotFound),
        "slot recheck must reject removed repo before any Git command"
    );

    stop_engine(&engine);
    remove_dir_retry(&temp);
}
