//! 引擎：生命周期、统一刷新调度与可见状态发布（N07）。
//!
//! 关键约定（实施说明 §6.3 / §8）：
//! - 三种序号各司其职：requestId（刷新请求）/ completedRequestId（已覆盖该请求的
//!   完成扫描）/ generation（被接受的扫描结果，含全局失败）/ stateRevision（可见状态
//!   通知序号）。订阅等待用 stateRevision，不用 generation。
//! - 一次最多一个全量收集；扫描期间的新请求合并为最多一次后续扫描；扫描开始时捕获
//!   最新请求号，接受后把 completedRequestId 推进到捕获值。
//! - 采集时抓取配置 revision；发布前在同一同步步骤内复核：revision 已变化 → 丢弃
//!   结果、不推进 completedRequestId，请求仍由下一轮覆盖（停止阶段不补扫）。
//! - 配置提交立即重投影：删除项消失、新增项 pending、分组生效；不增加 generation；
//!   同时请求一次重扫。
//! - 定时用单调时钟；休眠唤醒合并一次刷新；手动模式不因唤醒自动扫描。
//! - Running → Stopping → Stopped；request_stop 幂等并唤醒所有等待者；wait_stopped
//!   等待扫描线程与在途业务调用结束（不在持锁时 join）。
//! - 锁顺序固定：引擎状态锁 → 设置服务锁，无反向获取；Git 执行与磁盘落盘期间
//!   不持有引擎状态锁。

use crate::diagnostics;
use crate::error::{CoreError, ErrorObject};
use crate::git::commands::{
    branch_state_monitored, execute_repo_operation_with_slot, ExecPolicy, RepoGitOperation,
    RepoOperationInput, RepoOperationOutcome,
};
use crate::git::discovery::{scan_repositories, StopFlag};
use crate::git::operation_lock::OperationLocks;
use crate::model::{
    AppSnapshot, RefreshPolicy, RefreshProgress, RefreshState, RepoStatus, SCHEMA_VERSION,
};
use crate::refresh::{index_by_id, RepoCollector};
use crate::settings::{
    RepoSettings, SettingsData, SettingsPersist, SettingsSaveInput, SettingsService, StartupWarning,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// 扫描线程空闲等待的分片：虚拟时钟的期限变化由下一次唤醒或分片轮询发现。
const SCHEDULER_POLL_SLICE: Duration = Duration::from_millis(100);

/// 引擎时钟：单调毫秒（调度）+ Unix 墙钟毫秒（展示）。测试注入假时钟。
pub trait EngineClock: Send + Sync {
    fn monotonic_ms(&self) -> u64;
    fn wall_ms(&self) -> u64;
}

/// 生产时钟：单调部分以进程内固定基准计算，只用于差值。
#[derive(Default)]
pub struct SystemClock {
    baseline: Mutex<Option<Instant>>,
}

impl EngineClock for SystemClock {
    fn monotonic_ms(&self) -> u64 {
        let mut baseline = self.baseline.lock().unwrap();
        let base = baseline.get_or_insert(Instant::now());
        base.elapsed().as_millis() as u64
    }

    fn wall_ms(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lifecycle {
    Running,
    Stopping,
    Stopped,
}

/// 引擎配置：路径由宿主注入，各宿主使用自己的数据目录。
pub struct EngineConfig {
    pub data_dir: PathBuf,
    /// 宿主注入的日志目录；engine_create 初始化诊断器（首个引擎生效）。
    pub log_dir: PathBuf,
    pub persist: Option<Arc<dyn SettingsPersist>>,
    pub policy: ExecPolicy,
    pub clock: Arc<dyn EngineClock>,
    pub collector: Arc<dyn RepoCollector>,
    /// 测试注入固定会话 ID；生产自动生成。
    pub session_id: Option<String>,
    /// 测试注入（S05 交错测试）：网络操作在"已定位路径、尚未取得操作槽"处
    /// 调用的屏障钩子；生产为 None。
    pub pre_slot_hook: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            data_dir: std::env::temp_dir().join("gitaview-engine-default"),
            log_dir: std::env::temp_dir()
                .join("gitaview-engine-default")
                .join("logs"),
            persist: None,
            policy: ExecPolicy::default(),
            clock: Arc::new(SystemClock::default()),
            collector: Arc::new(crate::refresh::GitCollector),
            session_id: None,
            pre_slot_hook: None,
        }
    }
}

pub struct EngineStartup {
    pub engine: Engine,
    pub warnings: Vec<StartupWarning>,
}

// —— FFI 载荷 DTO（N09 契约；宿主 Swift / C# 镜像同一形状）——

/// engine_create 输入：绝对私有路径由宿主解析注入。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EngineCreateInput {
    pub data_directory: String,
    /// 日志目录；当前版本接受并保留，核心尚无日志模块（见进度 N05 说明）。
    pub log_directory: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EngineCreateData {
    pub session_id: String,
    pub abi_version: u32,
    /// 启动警告 code 列表（本次启动结果，不持久化）。
    #[serde(default)]
    pub startup_warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScanDirectoryInput {
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScanDirectoryData {
    /// 候选路径，不自动添加；预算 / 死线 / 停止中止时 truncated=true。
    pub paths: Vec<String>,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RepoAddInput {
    pub path: String,
    #[serde(default)]
    pub group_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RepoAddData {
    pub repo: RepoSettings,
    pub settings_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RepoRemoveData {
    pub settings_revision: u64,
}

/// 网络操作执行结果：命令成功结束；新状态由后续刷新产生。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RepoOperationData {
    pub repo_id: String,
    pub operation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RefreshRequestData {
    pub session_id: String,
    pub request_id: u64,
}

/// 快照等待 cursor：sessionId + stateRevision（不能用 generation 代替）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SnapshotWaitInput {
    pub session_id: String,
    pub state_revision: u64,
}

/// repo_target 种类：宿主调用系统打开 API，核心只产出验证后的值。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepoTargetKind {
    Directory,
    Remote,
}

impl RepoTargetKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Directory => "directory",
            Self::Remote => "remote",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RepoTargetInput {
    pub repo_id: String,
    pub kind: RepoTargetKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RepoTargetData {
    pub kind: RepoTargetKind,
    /// directory：仓库路径；remote：浏览器安全的 http(s) origin URL。
    pub value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleDto {
    Running,
    Stopping,
    Stopped,
}

impl From<Lifecycle> for LifecycleDto {
    fn from(value: Lifecycle) -> Self {
        match value {
            Lifecycle::Running => Self::Running,
            Lifecycle::Stopping => Self::Stopping,
            Lifecycle::Stopped => Self::Stopped,
        }
    }
}

impl LifecycleDto {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Stopping => "stopping",
            Self::Stopped => "stopped",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LifecycleData {
    pub lifecycle: LifecycleDto,
}

/// 创建引擎：打开设置（警告透传给宿主）、建立初始 pending 快照并安排首次读取。
pub fn engine_create(config: EngineConfig) -> Result<EngineStartup, CoreError> {
    diagnostics::init(&config.log_dir);
    let started_at = std::time::Instant::now();
    let opened = match config.persist {
        Some(persist) => SettingsService::open_with(&config.data_dir, persist)?,
        None => SettingsService::open(&config.data_dir)?,
    };
    let settings = opened.service.get();
    let clock = config.clock;
    let next_due = settings
        .refresh
        .enabled
        .then(|| clock.monotonic_ms() + interval_ms(&settings.refresh));

    let repo_count = opened.service.get().repos.len();
    let inner = Arc::new(EngineInner {
        session_id: config.session_id.unwrap_or_else(generate_session_id),
        settings: opened.service,
        locks: OperationLocks::new(),
        exec_policy: config.policy,
        clock,
        collector: config.collector,
        stop_flag: StopFlag::new(),
        state: Mutex::new(EngineState {
            lifecycle: Lifecycle::Running,
            state_revision: 0,
            generation: 0,
            requested_request_id: 1, // 初始化即安排首次读取
            completed_request_id: 0,
            is_refreshing: false,
            next_due_mono: next_due,
            settings: settings.clone(),
            collected: HashMap::new(),
            attempted_at: None,
            last_success_at: None,
            refresh_state: RefreshState::Pending,
            refresh_error: None,
            scan_done: false,
        }),
        changed: Condvar::new(),
        in_flight: AtomicUsize::new(0),
        scan_handle: Mutex::new(None),
        config_writer: Mutex::new(()),
        pre_slot_hook: config.pre_slot_hook,
        #[cfg(test)]
        before_business_registration: Mutex::new(None),
    });

    let engine = Engine {
        inner: Arc::clone(&inner),
    };
    let scan_inner = Arc::clone(&inner);
    let handle = std::thread::Builder::new()
        .name("gitaview-refresh".into())
        .spawn(move || scan_loop(scan_inner))
        .map_err(|_| CoreError::SettingsIo)?;
    *inner
        .scan_handle
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(handle);
    diagnostics::log_duration(
        "engine.create",
        started_at.elapsed(),
        format!(
            "session={} repos={} warnings={:?}",
            inner.session_id,
            repo_count,
            opened
                .warnings
                .iter()
                .map(|warning| warning.as_str())
                .collect::<Vec<_>>()
        ),
    );
    Ok(EngineStartup {
        engine,
        warnings: opened.warnings,
    })
}

fn generate_session_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    format!("sess-{now:016x}-{count:04x}")
}

fn interval_ms(policy: &RefreshPolicy) -> u64 {
    u64::from(policy.interval_minutes.max(1)) * 60_000
}

struct EngineState {
    lifecycle: Lifecycle,
    state_revision: u64,
    generation: u64,
    requested_request_id: u64,
    completed_request_id: u64,
    is_refreshing: bool,
    next_due_mono: Option<u64>,
    /// 引擎内配置镜像：所有变更经引擎提交，保证与投影一致。
    settings: SettingsData,
    /// 最近一次成功采集的行（ok / error），配置重投影复用；未采集的仓库显示 pending。
    collected: HashMap<String, RepoStatus>,
    attempted_at: Option<u64>,
    last_success_at: Option<u64>,
    refresh_state: RefreshState,
    refresh_error: Option<ErrorObject>,
    scan_done: bool,
}

struct EngineInner {
    session_id: String,
    settings: SettingsService,
    locks: OperationLocks,
    exec_policy: ExecPolicy,
    clock: Arc<dyn EngineClock>,
    collector: Arc<dyn RepoCollector>,
    stop_flag: StopFlag,
    state: Mutex<EngineState>,
    changed: Condvar,
    /// 在途业务调用计数；生命周期控制调用不计入（§9）。
    in_flight: AtomicUsize,
    scan_handle: Mutex<Option<std::thread::JoinHandle<()>>>,
    /// 配置写入串行器：设置服务事务与引擎镜像提交在同一临界区内完成，
    /// 并发提交不会把镜像回退到旧 revision。磁盘落盘在此临界区内，
    /// 但不占用刷新状态锁（锁顺序：config_writer → state，无反向）。
    config_writer: Mutex<()>,
    /// 测试注入（S05）：网络操作拿槽前调用的屏障钩子；生产 None。
    pre_slot_hook: Option<Arc<dyn Fn() + Send + Sync>>,
    #[cfg(test)]
    before_business_registration: Mutex<Option<Arc<dyn Fn() + Send + Sync>>>,
}

impl EngineInner {
    fn lock_state(&self) -> MutexGuard<'_, EngineState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// 由当前状态构建可见快照：行列表来自配置镜像 + collected（缺失 = pending），
/// 分组与名称按最新配置重投影。
fn build_snapshot(state: &EngineState, session_id: &str) -> AppSnapshot {
    let repos = state
        .settings
        .repos
        .iter()
        .map(|record| match state.collected.get(&record.id) {
            // 复用历史行前核对身份（S02）：collected 以 ID 为键，而 ID 在移除后
            // 可被同名不同路径的新仓库复用；路径不一致时按 pending 展示，
            // 绝不把另一仓库的分支 / 关系 / 差异套到新仓库。
            Some(row) if row.path == record.path => {
                let mut row = row.clone();
                row.name = record.name.clone();
                row.path = record.path.clone();
                row.group_id = record.group_id.clone();
                row
            }
            _ => {
                // 未采集或身份不符：pending 行按当前配置投影，未知 Git 字段为空，
                // 网络动作不可用。
                let mut row = RepoStatus::pending(&record.id, &record.name, &record.path);
                row.group_id = record.group_id.clone();
                row
            }
        })
        .collect();
    AppSnapshot {
        schema_version: SCHEMA_VERSION,
        session_id: session_id.to_string(),
        generation: state.generation,
        state_revision: state.state_revision,
        settings_revision: state.settings.revision,
        attempted_at_ms: state.attempted_at,
        last_success_at_ms: state.last_success_at,
        refresh_state: state.refresh_state,
        refresh_error: state.refresh_error.clone(),
        refresh_policy: state.settings.refresh,
        refresh: RefreshProgress {
            is_refreshing: state.is_refreshing,
            requested_request_id: state.requested_request_id,
            completed_request_id: state.completed_request_id,
        },
        repos,
    }
}

/// 扫描线程退出守卫：任何路径（含 panic）都标记 scan_done 并唤醒等待者。
struct ScanExit<'a> {
    inner: &'a EngineInner,
}

impl Drop for ScanExit<'_> {
    fn drop(&mut self) {
        let mut state = self.inner.lock_state();
        state.is_refreshing = false;
        state.scan_done = true;
        self.inner.changed.notify_all();
    }
}

fn scan_loop(inner: Arc<EngineInner>) {
    let _exit = ScanExit { inner: &inner };
    loop {
        // —— 调度决策（持状态锁）——
        let captured_request = {
            let mut state = inner.lock_state();
            loop {
                if state.lifecycle != Lifecycle::Running {
                    return;
                }
                let now = inner.clock.monotonic_ms();
                // 空闲且到期的定时请求：只补一次，不按错过间隔累计。
                if !state.is_refreshing && state.requested_request_id == state.completed_request_id
                {
                    if let Some(due) = state.next_due_mono {
                        if now >= due {
                            state.requested_request_id += 1;
                            state.state_revision += 1;
                            inner.changed.notify_all();
                        }
                    }
                }
                if !state.is_refreshing && state.requested_request_id > state.completed_request_id {
                    state.is_refreshing = true;
                    state.state_revision += 1;
                    inner.changed.notify_all();
                    break state.requested_request_id;
                }
                // 等待到下一次期限（分片轮询覆盖虚拟时钟跳跃），或被唤醒。
                let wait = state
                    .next_due_mono
                    .map(|due| due.saturating_sub(now))
                    .filter(|delta| *delta > 0)
                    .map(|delta| {
                        Duration::from_millis(delta.min(SCHEDULER_POLL_SLICE.as_millis() as u64))
                    })
                    .unwrap_or(SCHEDULER_POLL_SLICE);
                let (guard, _) = inner
                    .changed
                    .wait_timeout(state, wait)
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                state = guard;
            }
        };

        // —— 采集（不持状态锁）——
        let publish_started_mono = inner.clock.monotonic_ms();
        let settings_snapshot = inner.settings.get();
        let captured_revision = settings_snapshot.revision;
        let collect_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            inner.collector.collect(
                &settings_snapshot.repos,
                &inner.stop_flag,
                &inner.exec_policy,
            )
        }))
        .unwrap_or_else(|_| Err("仓库状态收集意外中断".to_string()));

        // —— 发布（持状态锁；revision 复核与接受是同一同步步骤）——
        let mut state = inner.lock_state();
        if state.lifecycle != Lifecycle::Running {
            state.is_refreshing = false;
            inner.changed.notify_all();
            return; // 停止阶段：不发布、不补扫
        }
        if state.settings.revision != captured_revision {
            // 配置已变化（按投影镜像复核，镜像与磁盘/服务在配置串行范围内一致）：
            // 丢弃过时结果，不推进 completedRequestId；
            // requested > completed 仍成立 → 下一轮覆盖未完成请求。
            let (repos, revision) = (settings_snapshot.repos.len(), captured_revision);
            state.is_refreshing = false;
            drop(state);
            diagnostics::log(
                "scan.discarded",
                format!("request={repos} settings_revision={revision} (配置变化，等待下轮覆盖)"),
            );
            inner.changed.notify_all();
            continue;
        }
        state.completed_request_id = captured_request;
        state.is_refreshing = false;
        state.generation += 1; // 全局失败也占一代
        let wall_now = inner.clock.wall_ms();
        state.attempted_at = Some(wall_now);
        let publish_info = (
            captured_request,
            settings_snapshot.repos.len(),
            collect_result.is_ok(),
        );
        match collect_result {
            Ok(rows) => {
                state.collected = index_by_id(rows);
                state.last_success_at = Some(wall_now);
                state.refresh_state = RefreshState::Ready;
                state.refresh_error = None;
            }
            Err(_) => {
                // 全局失败：保留 collected 历史行，发布时按最新配置重投影。
                state.refresh_state = RefreshState::Failed;
                state.refresh_error = Some(CoreError::GitFailed.to_error_object());
            }
        }
        state.next_due_mono = state
            .settings
            .refresh
            .enabled
            .then(|| inner.clock.monotonic_ms() + interval_ms(&state.settings.refresh));
        let generation_after = state.generation;
        state.state_revision += 1;
        drop(state);
        // 锁外记录：扫描完成（含全局失败）。
        diagnostics::log_duration(
            "scan.publish",
            Duration::from_millis(
                inner
                    .clock
                    .monotonic_ms()
                    .saturating_sub(publish_started_mono),
            ),
            format!(
                "request={} repos={} ok={} generation={}",
                publish_info.0, publish_info.1, publish_info.2, generation_after
            ),
        );
        inner.changed.notify_all();
    }
}

/// 快照等待结果。
#[derive(Debug, Clone, PartialEq)]
pub enum SnapshotWait {
    Ready(AppSnapshot),
    Timeout,
    Stopping,
}

#[derive(Clone)]
pub struct Engine {
    inner: Arc<EngineInner>,
}

struct InFlightGuard<'a> {
    inner: &'a EngineInner,
}

impl Drop for InFlightGuard<'_> {
    fn drop(&mut self) {
        self.inner.in_flight.fetch_sub(1, Ordering::SeqCst);
        self.inner.changed.notify_all();
    }
}

impl Engine {
    pub fn session_id(&self) -> &str {
        &self.inner.session_id
    }

    pub fn lifecycle(&self) -> Lifecycle {
        self.inner.lock_state().lifecycle
    }

    fn begin_business_call(&self) -> Result<InFlightGuard<'_>, CoreError> {
        let state = self.inner.lock_state();
        if state.lifecycle != Lifecycle::Running {
            return Err(CoreError::EngineStopping);
        }
        #[cfg(test)]
        if let Some(hook) = &*self.inner.before_business_registration.lock().unwrap() {
            hook();
        }
        // Admission and registration share the lifecycle lock: stopping must not
        // observe zero calls after a business operation has already been admitted.
        self.inner.in_flight.fetch_add(1, Ordering::SeqCst);
        drop(state);
        Ok(InFlightGuard { inner: &self.inner })
    }

    // —— 可见状态 ——

    pub fn snapshot_get(&self) -> AppSnapshot {
        let state = self.inner.lock_state();
        build_snapshot(&state, &self.inner.session_id)
    }

    /// 视图查询（N08）：组计数 / 状态计数 / 筛选 / 搜索 / 排序由核心统一计算，
    /// 宿主只消费结果。同一 stateRevision 下不同筛选各自有效。
    pub fn view_query(&self, query: crate::view::ViewQuery) -> crate::view::ViewResult {
        let state = self.inner.lock_state();
        let snapshot = build_snapshot(&state, &self.inner.session_id);
        crate::view::query_view(&snapshot, &state.settings.groups, query)
    }

    /// 目录扫描（N09）：发现候选，不自动添加；预算 / 死线 / 停止中止 → truncated。
    pub fn scan_directory(&self, path: &std::path::Path) -> Result<ScanDirectoryData, CoreError> {
        let _guard = self.begin_business_call()?;
        if !path.is_absolute() || !path.is_dir() {
            return Err(CoreError::InvalidPath);
        }
        let outcome = scan_repositories(path, &self.inner.stop_flag);
        Ok(ScanDirectoryData {
            paths: outcome
                .paths
                .iter()
                .map(|found| found.to_string_lossy().into_owned())
                .collect(),
            truncated: outcome.truncated,
        })
    }

    /// 打开目标（N09）：directory 复核目录存在；remote 重读并验证当前 origin URL。
    /// 宿主拿到值后调用系统打开 API；协议仅导航，本函数不执行打开。
    pub fn repo_target(
        &self,
        repo_id: &str,
        kind: RepoTargetKind,
    ) -> Result<RepoTargetData, CoreError> {
        let _guard = self.begin_business_call()?;
        let settings = self.inner.settings.get();
        let record = settings
            .repos
            .iter()
            .find(|repo| repo.id == repo_id)
            .ok_or(CoreError::RepoNotFound)?;
        match kind {
            RepoTargetKind::Directory => {
                let path = record.path_buf();
                if !path.is_dir() {
                    return Err(CoreError::InvalidPath);
                }
                Ok(RepoTargetData {
                    kind,
                    value: record.path.clone(),
                })
            }
            RepoTargetKind::Remote => {
                let state = branch_state_monitored(
                    &record.path_buf(),
                    &self.inner.exec_policy,
                    &self.inner.stop_flag,
                )
                .map_err(CoreError::from)?;
                if !state.has_remote {
                    return Err(CoreError::OriginRequired);
                }
                let url = state.remote_url.ok_or(CoreError::OperationUnavailable)?;
                Ok(RepoTargetData { kind, value: url })
            }
        }
    }

    /// 等待公开状态变化。引擎停止中 → Stopping（优先于其他分支，§9）；
    /// 不同会话或已有更新立即返回；相同版本后台等待；期限到 → Timeout。
    pub fn snapshot_wait(
        &self,
        session_id: &str,
        state_revision: u64,
        timeout: Duration,
    ) -> SnapshotWait {
        let deadline = Instant::now() + timeout;
        let mut state = self.inner.lock_state();
        loop {
            if state.lifecycle != Lifecycle::Running {
                return SnapshotWait::Stopping;
            }
            if session_id != self.inner.session_id || state.state_revision > state_revision {
                return SnapshotWait::Ready(build_snapshot(&state, &self.inner.session_id));
            }
            let now = Instant::now();
            if now >= deadline {
                return SnapshotWait::Timeout;
            }
            let (guard, _) = self
                .inner
                .changed
                .wait_timeout(state, deadline - now)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            state = guard;
        }
    }

    // —— 刷新 ——

    /// 手动 / 通用刷新请求：只请求不等待；完成与否看 completedRequestId。
    pub fn refresh_request(&self) -> Result<(String, u64), CoreError> {
        let mut state = self.inner.lock_state();
        if state.lifecycle != Lifecycle::Running {
            return Err(CoreError::EngineStopping);
        }
        state.requested_request_id += 1;
        let request_id = state.requested_request_id;
        state.state_revision += 1;
        drop(state);
        self.inner.changed.notify_all();
        Ok((self.inner.session_id.clone(), request_id))
    }

    /// 宿主原生恢复 / 唤醒：定时开启则合并一次刷新；手动模式不自发扫描；
    /// 不按休眠期间遗漏的间隔补 N 次。
    pub fn notify_system_resume(&self) -> Result<(), CoreError> {
        let mut state = self.inner.lock_state();
        if state.lifecycle != Lifecycle::Running {
            return Err(CoreError::EngineStopping);
        }
        if state.settings.refresh.enabled {
            state.requested_request_id += 1;
            state.state_revision += 1;
            drop(state);
            self.inner.changed.notify_all();
        }
        Ok(())
    }

    // —— 配置（引擎中介：提交成功后立即重投影并请求重扫）——

    pub fn get_settings(&self) -> SettingsData {
        self.inner.lock_state().settings.clone()
    }

    pub fn save_settings(&self, input: SettingsSaveInput) -> Result<SettingsData, CoreError> {
        let _guard = self.begin_business_call()?;
        // 事务 + 镜像提交在同一配置串行范围：并发提交不会把引擎镜像回退到
        // 旧 revision（服务内部事务串行只保证磁盘不丢写，不保证镜像次序）。
        let _config = self
            .inner
            .config_writer
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let saved = self.inner.settings.save(input)?;
        self.commit_config_change(saved.clone());
        Ok(saved)
    }

    pub fn repo_add(
        &self,
        path: &std::path::Path,
        group_id: Option<&str>,
    ) -> Result<(RepoSettings, SettingsData), CoreError> {
        let _guard = self.begin_business_call()?;
        let _config = self
            .inner
            .config_writer
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let (record, saved) = self.inner.settings.repo_add(path, group_id)?;
        self.commit_config_change(saved.clone());
        Ok((record, saved))
    }

    pub fn repo_remove(&self, repo_id: &str) -> Result<SettingsData, CoreError> {
        let _guard = self.begin_business_call()?;
        let _config = self
            .inner
            .config_writer
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let saved = self
            .inner
            .settings
            .repo_remove(repo_id, &self.inner.locks)?;
        self.commit_config_change(saved.clone());
        Ok(saved)
    }

    /// 配置提交后的同步步骤：镜像新配置、重投影（删除消失 / 新增 pending / 分组生效）、
    /// 递增 stateRevision 并请求一次重扫。不增加 generation；collected 历史与
    /// lastSuccessAt 保持。
    fn commit_config_change(&self, settings: SettingsData) {
        let mut state = self.inner.lock_state();
        if settings.revision < state.settings.revision {
            // 过时镜像拒绝覆盖（防御纵深：镜像只允许按 revision 前进；
            // 磁盘与服务内部已保留新配置）。
            let (stale, current) = (settings.revision, state.settings.revision);
            drop(state);
            diagnostics::log(
                "config.commit.stale",
                format!("revision={stale} current={current}（过时镜像不覆盖）"),
            );
            return;
        }
        let diagnostic = (
            settings.revision,
            settings.repos.len(),
            settings.groups.len(),
        );
        state.settings = settings;
        // 剔除已删除仓库的采集缓存（S02）：collected 以 ID 为键，而移除后
        // 同名新仓库会复用 ID，不清缓存会让新仓库继承旧行。
        let managed_ids: std::collections::HashSet<String> = state
            .settings
            .repos
            .iter()
            .map(|repo| repo.id.clone())
            .collect();
        state.collected.retain(|id, _| managed_ids.contains(id));
        // Configuration reprojection is not a completed refresh attempt. Preserve
        // its state/error pair until the next accepted scan publishes a result.
        state.state_revision += 1;
        if state.lifecycle == Lifecycle::Running {
            state.requested_request_id += 1;
            state.next_due_mono =
                state.settings.refresh.enabled.then(|| {
                    self.inner.clock.monotonic_ms() + interval_ms(&state.settings.refresh)
                });
        }
        drop(state);
        self.inner.changed.notify_all();
        diagnostics::log(
            "config.commit",
            format!(
                "revision={} repos={} groups={}",
                diagnostic.0, diagnostic.1, diagnostic.2
            ),
        );
    }

    // —— Git 操作 ——

    pub fn repo_fetch(&self, repo_id: &str) -> Result<RepoOperationOutcome, CoreError> {
        self.run_repo_operation(repo_id, RepoGitOperation::Fetch, true)
    }

    pub fn repo_pull(
        &self,
        repo_id: &str,
        confirmed: bool,
    ) -> Result<RepoOperationOutcome, CoreError> {
        self.run_repo_operation(repo_id, RepoGitOperation::Pull, confirmed)
    }

    pub fn repo_push(
        &self,
        repo_id: &str,
        confirmed: bool,
    ) -> Result<RepoOperationOutcome, CoreError> {
        self.run_repo_operation(repo_id, RepoGitOperation::Push, confirmed)
    }

    fn run_repo_operation(
        &self,
        repo_id: &str,
        operation: RepoGitOperation,
        confirmed: bool,
    ) -> Result<RepoOperationOutcome, CoreError> {
        let _guard = self.begin_business_call()?;
        // 确认校验最先执行：未确认的 Pull / Push 不进入仓库槽、不读 Git 状态
        //（与拆分前的 execute_repo_operation 顺序一致）。
        if matches!(operation, RepoGitOperation::Pull | RepoGitOperation::Push) && !confirmed {
            return Err(CoreError::ConfirmationRequired);
        }
        // 短暂设置读定位路径（不持状态锁）。
        let path = self
            .inner
            .settings
            .get()
            .repos
            .iter()
            .find(|repo| repo.id == repo_id)
            .ok_or(CoreError::RepoNotFound)?
            .path_buf();
        diagnostics::log(
            "repo.operation",
            format!(
                "repo={} operation={} confirmed={}",
                repo_id,
                operation.as_str(),
                confirmed
            ),
        );
        // 测试钩子（S05 交错测试）：暂停在"已定位路径、尚未取得操作槽"。
        if let Some(hook) = &self.inner.pre_slot_hook {
            hook();
        }
        // 取得仓库操作槽（网络忙 → REPO_BUSY），并在槽内复核配置（S05）：
        // 定位与拿槽之间仓库可能已被移除；已移除 → REPO_NOT_FOUND，
        // 不在该路径上启动 Git。复核为短暂设置读，不持设置锁做网络调用。
        let slot = self.inner.locks.try_acquire(&path)?;
        let managed = self
            .inner
            .settings
            .get()
            .repos
            .iter()
            .any(|repo| repo.id == repo_id && Path::new(&repo.path) == path);
        if !managed {
            return Err(CoreError::RepoNotFound);
        }
        let result = execute_repo_operation_with_slot(
            RepoOperationInput {
                repo_path: &path,
                operation,
                confirmed,
            },
            &self.inner.exec_policy,
            &self.inner.stop_flag,
        );
        drop(slot);
        if let Ok(outcome) = &result {
            diagnostics::log(
                "repo.operation.done",
                format!("repo={} operation={}", repo_id, outcome.operation.as_str()),
            );
        }
        // finally：Git 实际执行过（成功 / 非零退出 / 超时）都请求刷新；
        // 预检拒绝（确认、忙、校验、已移除）不请求。Stopping 时忽略：下次启动重扫。
        let ran_git =
            result.is_ok() || matches!(result, Err(CoreError::GitFailed | CoreError::GitTimeout));
        if ran_git {
            let _ = self.refresh_request();
        }
        result
    }

    // —— 生命周期 ——

    /// 幂等停止：拒绝新工作、停止派发、唤醒全部等待者。
    pub fn request_stop(&self) {
        let changed = {
            let mut state = self.inner.lock_state();
            if state.lifecycle == Lifecycle::Running {
                state.lifecycle = Lifecycle::Stopping;
                state.state_revision += 1;
                true
            } else {
                false
            }
        };
        self.inner.stop_flag.stop();
        self.inner.changed.notify_all();
        if changed {
            diagnostics::log("engine.stop_request", "Running → Stopping");
        }
    }

    /// 等待扫描线程与在途业务调用结束；成功后进入 Stopped 并回收线程。
    /// 期限到仍清理完 → false（仍 Stopping，不代表可以 destroy）。
    pub fn wait_stopped(&self, timeout: Duration) -> bool {
        {
            let mut state = self.inner.lock_state();
            if state.lifecycle != Lifecycle::Stopped {
                let deadline = Instant::now() + timeout;
                while !(state.scan_done && self.inner.in_flight.load(Ordering::SeqCst) == 0) {
                    let now = Instant::now();
                    if now >= deadline {
                        return false;
                    }
                    let (guard, _) = self
                        .inner
                        .changed
                        .wait_timeout(state, deadline - now)
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    state = guard;
                }
                state.lifecycle = Lifecycle::Stopped;
                self.inner.changed.notify_all();
            }
        }
        // join 在锁外：线程已标记退出，join 立即返回。
        let handle = self
            .inner
            .scan_handle
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take();
        if let Some(handle) = handle {
            let _ = handle.join();
        }
        true
    }
}

#[cfg(test)]
mod admission_tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn blocked_logging_does_not_block_config_queries_or_stop() {
        let temp = std::env::temp_dir().join(format!(
            "gitaview-slow-log-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let engine = engine_create(EngineConfig {
            data_dir: temp.clone(),
            log_dir: temp.join("logs"),
            ..EngineConfig::default()
        })
        .unwrap()
        .engine;
        let settings = engine.get_settings();
        let input = SettingsSaveInput {
            expected_revision: settings.revision,
            settings: crate::settings::SettingsEditable {
                groups: vec![],
                default_group_id: None,
                refresh: RefreshPolicy {
                    enabled: !settings.refresh.enabled,
                    interval_minutes: settings.refresh.interval_minutes,
                },
                repo_groups: vec![],
            },
        };
        let sink = diagnostics::block_sink_for_test();
        let (entered_tx, entered_rx) = mpsc::channel();
        let (saved_tx, saved_rx) = mpsc::channel();
        let save_engine = engine.clone();
        let save = std::thread::spawn(move || {
            diagnostics::before_log_for_test(move |event| {
                if event == "config.commit" {
                    entered_tx.send(()).unwrap();
                }
            });
            saved_tx
                .send(save_engine.save_settings(input).map(|data| data.revision))
                .unwrap();
        });
        entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        let (snapshot_tx, snapshot_rx) = mpsc::channel();
        let query_engine = engine.clone();
        let query = std::thread::spawn(move || {
            snapshot_tx
                .send(query_engine.snapshot_get().settings_revision)
                .unwrap();
        });
        let (stopped_tx, stopped_rx) = mpsc::channel();
        let stop_engine = engine.clone();
        let stop = std::thread::spawn(move || {
            stop_engine.request_stop();
            stopped_tx
                .send(stop_engine.wait_stopped(Duration::from_secs(3)))
                .unwrap();
        });
        let saved_while_blocked = saved_rx.recv_timeout(Duration::from_millis(300)).ok();
        let snapshot_while_blocked = snapshot_rx.recv_timeout(Duration::from_millis(300)).ok();
        let stopped_while_blocked = stopped_rx.recv_timeout(Duration::from_millis(300)).ok();
        drop(sink);
        save.join().unwrap();
        query.join().unwrap();
        stop.join().unwrap();
        assert!(engine.wait_stopped(Duration::from_secs(5)));
        assert!(diagnostics::flush(Duration::from_secs(5)));
        for attempt in 0..20 {
            match std::fs::remove_dir_all(&temp) {
                Ok(()) => break,
                Err(_) if attempt < 19 => std::thread::sleep(Duration::from_millis(10)),
                Err(error) => panic!("cleanup: {error}"),
            }
        }
        assert_eq!(
            saved_while_blocked,
            Some(Ok(settings.revision + 1)),
            "config commit waited for log I/O"
        );
        assert_eq!(
            snapshot_while_blocked,
            Some(settings.revision + 1),
            "snapshot query waited for log I/O"
        );
        assert_eq!(stopped_while_blocked, Some(true), "stop waited for log I/O");
    }

    #[test]
    fn stop_cannot_overtake_a_business_call_at_registration() {
        let temp = std::env::temp_dir().join(format!(
            "gitaview-admission-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let engine = engine_create(EngineConfig {
            data_dir: temp.clone(),
            log_dir: temp.join("logs"),
            ..EngineConfig::default()
        })
        .unwrap()
        .engine;

        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let release_rx = Mutex::new(release_rx);
        *engine.inner.before_business_registration.lock().unwrap() = Some(Arc::new(move || {
            entered_tx.send(()).unwrap();
            release_rx
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(5))
                .unwrap();
        }));
        let (admitted_tx, admitted_rx) = mpsc::channel();
        let (finish_tx, finish_rx) = mpsc::channel();
        let business_engine = engine.clone();
        let business = std::thread::spawn(move || {
            let _guard = business_engine.begin_business_call().unwrap();
            admitted_tx.send(()).unwrap();
            finish_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        });
        entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();

        let (stop_started_tx, stop_started_rx) = mpsc::channel();
        let (stopped_tx, stopped_rx) = mpsc::channel();
        let stopping_engine = engine.clone();
        let stopping = std::thread::spawn(move || {
            stop_started_tx.send(()).unwrap();
            stopping_engine.request_stop();
            stopped_tx
                .send(stopping_engine.wait_stopped(Duration::from_secs(5)))
                .unwrap();
        });
        stop_started_rx
            .recv_timeout(Duration::from_secs(5))
            .unwrap();
        let stopped_before_registration = stopped_rx.recv_timeout(Duration::from_millis(500)).ok();
        release_tx.send(()).unwrap();
        admitted_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        let stopped_during_business = if stopped_before_registration.is_none() {
            stopped_rx.recv_timeout(Duration::from_millis(100)).ok()
        } else {
            None
        };
        finish_tx.send(()).unwrap();
        business.join().unwrap();
        stopping.join().unwrap();
        let stopped = stopped_before_registration
            .or(stopped_during_business)
            .unwrap_or_else(|| stopped_rx.recv_timeout(Duration::from_secs(5)).unwrap());
        assert!(stopped);
        assert!(engine.wait_stopped(Duration::from_secs(1)));
        assert_eq!(engine.lifecycle(), Lifecycle::Stopped);
        assert!(matches!(
            engine.begin_business_call(),
            Err(CoreError::EngineStopping)
        ));
        assert!(diagnostics::flush(Duration::from_secs(5)));
        assert!(temp.starts_with(std::env::temp_dir()));
        std::fs::remove_dir_all(&temp).unwrap();
        assert!(
            stopped_before_registration.is_none(),
            "stop completed before call registration"
        );
        assert!(
            stopped_during_business.is_none(),
            "stop completed while the call was in flight"
        );
    }
}
