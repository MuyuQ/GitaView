//! Git 执行、仓库读取与网络操作（N02 探针 + N05 提取）。
//!
//! 固定参数调用用户安装的 Git；单条命令 deadline 覆盖进程退出与管道读取。
//! 整理自 src-tauri/src/git/commands.rs（未提交参考实现），提取时不改变比较规则。
//! 探针仅供平台原型接线使用；N09 冻结正式 ABI 时移除导出。

use crate::error::{CoreError, ErrorObject};
use crate::git::discovery::StopFlag;
use crate::git::operation_lock::{OperationLocks, RepoOperationGuard};
use crate::git::process::{prepare_pipe, read_pipe, ProcessTree, SpawnMode};
use crate::git::remote::normalize_remote_url;
use crate::model::{ReadState, Relation, RepoActions, RepoStatus};
use crate::registry::RepoRecord;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::OnceLock;
use std::thread;
use std::time::{Duration, Instant};

/// 本地状态读取超时：仓库在本地磁盘上，30 秒已经非常宽裕。
pub const GIT_STATUS_TIMEOUT: Duration = Duration::from_secs(30);
/// 网络操作超时（fetch/pull/push）：慢网络上的大仓库合法耗时远超 30 秒，
/// 强杀 pull 可能留下 MERGE_HEAD / 锁文件，因此使用独立的长超时。
pub const GIT_NETWORK_TIMEOUT: Duration = Duration::from_secs(600);

/// 执行策略：只允许调整超时，不开放任意程序或参数（N05）。
/// 测试注入短超时验证 deadline 行为；生产使用默认值。
/// 生成模式不由策略字段决定：状态读取链统一 Direct，网络操作统一 Suspended
///（直接体现在调用入口，避免出现不被执行器消费的死字段）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExecPolicy {
    pub status_timeout: Duration,
    pub network_timeout: Duration,
}

impl Default for ExecPolicy {
    fn default() -> Self {
        Self {
            status_timeout: GIT_STATUS_TIMEOUT,
            network_timeout: GIT_NETWORK_TIMEOUT,
        }
    }
}

const GIT_TIMEOUT_SENTINEL: &str = "git command timed out";
const SPAWN_FAILED_PREFIX: &str = "failed to spawn command";
const PROGRAM_NOT_FOUND_PREFIX: &str = "command executable not found";

static RESOLVED_GIT: OnceLock<PathBuf> = OnceLock::new();

/// 解析 git 可执行文件并缓存。
///
/// macOS 图形界面启动（Finder/Dock）只继承最小 PATH，仅装 Homebrew git 的
/// 用户会全部仓库读取失败。解析顺序：PATH → 常见安装位置 → 登录 shell（非 Windows）。
/// 找不到时返回 "git"，实际调用以 GIT_NOT_FOUND 报错，不安装、不弹终端。
pub fn git_program() -> PathBuf {
    RESOLVED_GIT
        .get_or_init(|| {
            let resolved = resolve_git_program().unwrap_or_else(|| PathBuf::from("git"));
            crate::diagnostics::log("git.resolved", resolved.to_string_lossy());
            resolved
        })
        .clone()
}

fn resolve_git_program() -> Option<PathBuf> {
    if probe_git_program("git").is_ok() {
        return None;
    }
    for candidate in [
        "/opt/homebrew/bin/git",
        "/usr/local/bin/git",
        "/usr/bin/git",
    ] {
        if probe_git_program(candidate).is_ok() {
            return Some(PathBuf::from(candidate));
        }
    }
    probe_login_shell_git()
}

fn probe_git_program(program: &str) -> Result<(), ()> {
    let mut command = Command::new(program);
    command.args(["--version"]);
    command.env("GIT_TERMINAL_PROMPT", "0");
    configure_git_child_process(&mut command);
    matches!(
        run_command_with_timeout(command, GIT_STATUS_TIMEOUT, SpawnMode::Direct),
        Ok(output) if output.status.success()
    )
    .then_some(())
    .ok_or(())
}

fn probe_login_shell_git() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        // Windows GUI 启动继承用户 PATH，Git for Windows 安装器会写入，
        // 登录 shell 探测无意义。
        None
    }

    #[cfg(not(target_os = "windows"))]
    {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
        let mut command = Command::new(&shell);
        command.args(["-lc", "command -v git"]);
        let output =
            run_command_with_timeout(command, GIT_STATUS_TIMEOUT, SpawnMode::Direct).ok()?;
        if !output.status.success() {
            return None;
        }
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if path.is_empty() || !Path::new(&path).exists() {
            return None;
        }
        Some(PathBuf::from(path))
    }
}

fn run_command_with_timeout(
    mut command: Command,
    timeout: Duration,
    mode: SpawnMode,
) -> Result<Output, String> {
    let start = Instant::now();
    let spawned =
        ProcessTree::spawn_with_mode(command.stdout(Stdio::piped()).stderr(Stdio::piped()), mode);
    let (mut child, tree) = spawned.map_err(|error| spawn_failure_message(&command, &error))?;
    let result = (|| {
        let mut stdout = child.stdout.take().ok_or("failed to capture stdout")?;
        let mut stderr = child.stderr.take().ok_or("failed to capture stderr")?;
        prepare_pipe(&stdout).map_err(|err| err.to_string())?;
        prepare_pipe(&stderr).map_err(|err| err.to_string())?;
        let mut out = Vec::new();
        let mut err = Vec::new();
        let (mut out_done, mut err_done) = (false, false);
        let mut status = None;
        loop {
            if start.elapsed() >= timeout {
                return Err(GIT_TIMEOUT_SENTINEL.to_string());
            }
            if !out_done {
                out_done = read_pipe(&mut stdout, &mut out).map_err(|err| err.to_string())?;
            }
            if !err_done {
                err_done = read_pipe(&mut stderr, &mut err).map_err(|err| err.to_string())?;
            }
            if status.is_none() {
                status = child.try_wait().map_err(|err| err.to_string())?;
            }
            if out_done && err_done {
                if let Some(status) = status {
                    return Ok(Output {
                        status,
                        stdout: out,
                        stderr: err,
                    });
                }
            }
            thread::sleep(Duration::from_millis(5));
        }
    })();
    if result.is_err() {
        tree.terminate();
        let _ = child.kill();
        let _ = child.wait();
    }
    result
}

fn spawn_failure_message(command: &Command, error: &std::io::Error) -> String {
    let missing_working_dir = command.get_current_dir().is_some_and(|dir| !dir.is_dir());
    let prefix = if error.kind() == std::io::ErrorKind::NotFound && !missing_working_dir {
        PROGRAM_NOT_FOUND_PREFIX
    } else {
        SPAWN_FAILED_PREFIX
    };
    format!("{prefix}: {error}")
}

fn classify_command_error(error: String) -> GitCommandError {
    if error == GIT_TIMEOUT_SENTINEL {
        GitCommandError::Timeout
    } else if error.starts_with(PROGRAM_NOT_FOUND_PREFIX) {
        GitCommandError::NotFound
    } else {
        GitCommandError::Failed(error)
    }
}

#[cfg(target_os = "windows")]
fn configure_git_child_process(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(target_os = "windows"))]
fn configure_git_child_process(_command: &mut Command) {}

/// 内部执行错误：区分停止、超时、找不到 Git 和普通失败，供映射稳定错误码。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitCommandError {
    /// 引擎停止：命令未启动或扫描中止。
    Stopping,
    NotFound,
    Timeout,
    Failed(String),
}

impl From<GitCommandError> for CoreError {
    fn from(error: GitCommandError) -> Self {
        match error {
            GitCommandError::Stopping => CoreError::EngineStopping,
            GitCommandError::NotFound => CoreError::GitNotFound,
            GitCommandError::Timeout => CoreError::GitTimeout,
            GitCommandError::Failed(_) => CoreError::GitFailed,
        }
    }
}

/// 带策略与停止检查的执行入口：每条 Git 命令启动前检查 stop。
pub fn run_git_monitored(
    repo_path: &Path,
    args: &[&str],
    timeout: Duration,
    stop: &StopFlag,
) -> Result<String, GitCommandError> {
    run_git_with_timeout(repo_path, args, timeout, stop, SpawnMode::Suspended)
}

/// 状态读取入口：Direct 低开销路径（挂起入 Job 后仅捕获该进程的线程）。
pub fn run_git_status(
    repo_path: &Path,
    args: &[&str],
    timeout: Duration,
    stop: &StopFlag,
) -> Result<String, GitCommandError> {
    run_git_with_timeout(repo_path, args, timeout, stop, SpawnMode::Direct)
}

fn run_git_with_timeout(
    repo_path: &Path,
    args: &[&str],
    timeout: Duration,
    stop: &StopFlag,
    mode: SpawnMode,
) -> Result<String, GitCommandError> {
    if stop.is_stopped() {
        return Err(GitCommandError::Stopping);
    }
    let mut command = Command::new(git_program());
    command.args(args);
    command.current_dir(repo_path);
    command.env("GIT_TERMINAL_PROMPT", "0");
    command.env("GCM_INTERACTIVE", "Never");
    configure_git_child_process(&mut command);

    let started = Instant::now();
    let output = run_command_with_timeout(command, timeout, mode);
    let duration = started.elapsed();
    let output = match output {
        Ok(output) => output,
        Err(err) => {
            let kind = classify_command_error(err);
            crate::diagnostics::log_duration(
                "git.command.error",
                duration,
                format!(
                    "args={} kind={}",
                    args.first().copied().unwrap_or("git"),
                    kind_code(&kind)
                ),
            );
            return Err(kind);
        }
    };
    if output.status.success() {
        crate::diagnostics::log_duration(
            "git.command",
            duration,
            format!(
                "args={} stdout_len={}",
                args.first().copied().unwrap_or("git"),
                output.stdout.len()
            ),
        );
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        // 只记 stderr 首行并脱敏（路径 / URL / 用户目录），保证日志可分享。
        let first_line = stderr.lines().next().unwrap_or("");
        crate::diagnostics::log_duration(
            "git.command.failed",
            duration,
            format!(
                "args={} stderr={}",
                args.first().copied().unwrap_or("git"),
                crate::diagnostics::redact_message(first_line, Some(repo_path))
            ),
        );
        Err(GitCommandError::Failed(format!(
            "git {} failed: {}",
            args.join(" "),
            stderr
        )))
    }
}

fn kind_code(kind: &GitCommandError) -> &'static str {
    match kind {
        GitCommandError::Stopping => "STOPPING",
        GitCommandError::NotFound => "GIT_NOT_FOUND",
        GitCommandError::Timeout => "GIT_TIMEOUT",
        GitCommandError::Failed(_) => "GIT_FAILED",
    }
}

/// 比较分支解析：属于本地状态读取链，经调用方传入的 `git` 闭包执行
///（branch_state_monitored 的进程限定线程快照路径），不单独走全系统线程快照。
fn comparison_ref(
    git: &dyn Fn(&[&str]) -> Result<String, GitCommandError>,
    branch: &str,
    has_remote: bool,
) -> Option<String> {
    if let Ok(upstream) = git(&["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"]) {
        if upstream.starts_with("origin/") {
            return Some(upstream);
        }
    }

    if !has_remote {
        return None;
    }

    // origin 同名分支后备：本地分支改名后仍比较 origin 上的目标分支。
    let origin_branch = format!("origin/{branch}");
    let full_origin_ref = format!("refs/remotes/{origin_branch}");
    git(&["rev-parse", "--verify", "--quiet", &full_origin_ref])
        .ok()
        .map(|_| origin_branch)
}

/// 分支比较状态。提取自参考实现，字段语义与旧版一致；hasCommit 为探针补充。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitBranchState {
    pub branch: String,
    pub remote_branch: Option<String>,
    pub relation: Relation,
    pub ahead: u32,
    pub behind: u32,
    pub has_remote: bool,
    pub has_commit: bool,
    pub remote_url: Option<String>,
}

/// 默认入口（探针与测试用）：默认策略、无停止标志。
pub fn branch_state(repo_path: &Path) -> Result<GitBranchState, GitCommandError> {
    branch_state_monitored(repo_path, &ExecPolicy::default(), &StopFlag::new())
}

/// 带策略与停止检查的分支状态读取：引擎刷新路径使用。
pub fn branch_state_monitored(
    repo_path: &Path,
    policy: &ExecPolicy,
    stop: &StopFlag,
) -> Result<GitBranchState, GitCommandError> {
    // 分支状态读取全部是本地读命令：Direct 路径不遍历全系统线程。
    let git = |args: &[&str]| run_git_status(repo_path, args, policy.status_timeout, stop);
    let inside = git(&["rev-parse", "--is-inside-work-tree"])?;
    if inside != "true" {
        return Err(GitCommandError::Failed("不是有效的 Git 工作区".into()));
    }
    let branch = git(&["symbolic-ref", "--short", "HEAD"])
        .or_else(|_| git(&["rev-parse", "--abbrev-ref", "HEAD"]))?;
    let has_commit = git(&["rev-parse", "--verify", "HEAD"]).is_ok();
    let raw_remote_url = git(&["config", "--get", "remote.origin.url"]).ok();
    let has_origin_remote = raw_remote_url
        .as_ref()
        .is_some_and(|url| !url.trim().is_empty());
    let remote_url = raw_remote_url.and_then(|url| normalize_remote_url(&url));
    // 尚无提交或 detached：无法比较，按 no_remote 处理（has_remote 仍如实记录）。
    if branch == "HEAD" || !has_commit {
        return Ok(GitBranchState {
            branch,
            remote_branch: None,
            relation: Relation::NoRemote,
            ahead: 0,
            behind: 0,
            has_remote: has_origin_remote,
            has_commit,
            remote_url,
        });
    }

    let Some(compare_ref) = comparison_ref(&git, &branch, has_origin_remote) else {
        return Ok(GitBranchState {
            branch,
            remote_branch: None,
            relation: Relation::NoRemote,
            ahead: 0,
            behind: 0,
            has_remote: has_origin_remote,
            has_commit,
            remote_url,
        });
    };

    let rev_range = format!("HEAD...{compare_ref}");
    let counts = git(&["rev-list", "--left-right", "--count", &rev_range])?;
    let mut parts = counts.split_whitespace();
    let ahead = parts
        .next()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(0);
    let behind = parts
        .next()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(0);
    let relation = match (ahead, behind) {
        (0, 0) => Relation::Synced,
        (_, 0) => Relation::LocalAhead,
        (0, _) => Relation::RemoteAhead,
        _ => Relation::Diverged,
    };

    Ok(GitBranchState {
        remote_branch: compare_ref.strip_prefix("origin/").map(str::to_owned),
        branch,
        relation,
        ahead,
        behind,
        has_remote: has_origin_remote,
        has_commit,
        remote_url,
    })
}

// —— 网络操作（fetch / pull / push，N05）——

/// 网络操作种类。v1 只管理 origin。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepoGitOperation {
    Fetch,
    Pull,
    Push,
}

impl RepoGitOperation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fetch => "fetch",
            Self::Pull => "pull",
            Self::Push => "push",
        }
    }
}

/// 固定参数：不拼接 shell 字符串，目标分支来自已验证的 origin 比较。
pub fn origin_fetch_args() -> Vec<String> {
    vec!["fetch".to_string(), "origin".to_string()]
}

pub fn origin_pull_args(branch: &str) -> Vec<String> {
    vec![
        "pull".to_string(),
        "origin".to_string(),
        format!("refs/heads/{branch}"),
    ]
}

pub fn origin_push_args(branch: &str) -> Vec<String> {
    vec![
        "push".to_string(),
        "origin".to_string(),
        format!("HEAD:refs/heads/{branch}"),
    ]
}

/// 前置校验（实施说明 §7 表）。传入的 relation / has_origin 必须来自锁内
/// 重新读取的状态，不能只用旧快照 actions。
///
/// - 有 origin 即可 Fetch（含无比较分支、尚无提交）；
/// - remote_ahead / diverged 可 Pull；local_ahead / diverged 可 Push；
/// - synced 两者都拒绝；origin 但无比较分支 → COMPARISON_REQUIRED；
/// - 无 origin → ORIGIN_REQUIRED。
pub fn validate_repo_operation(
    operation: RepoGitOperation,
    relation: Relation,
    has_origin: bool,
) -> Result<(), CoreError> {
    use Relation::*;
    match (operation, relation) {
        (RepoGitOperation::Fetch, _) if has_origin => Ok(()),
        (RepoGitOperation::Fetch, _) => Err(CoreError::OriginRequired),
        (RepoGitOperation::Pull, RemoteAhead | Diverged) => Ok(()),
        (RepoGitOperation::Pull, Synced | LocalAhead) => Err(CoreError::OperationUnavailable),
        (RepoGitOperation::Pull, NoRemote) if has_origin => Err(CoreError::ComparisonRequired),
        (RepoGitOperation::Pull, NoRemote) => Err(CoreError::OriginRequired),
        (RepoGitOperation::Push, LocalAhead | Diverged) => Ok(()),
        (RepoGitOperation::Push, Synced | RemoteAhead) => Err(CoreError::OperationUnavailable),
        (RepoGitOperation::Push, NoRemote) if has_origin => Err(CoreError::ComparisonRequired),
        (RepoGitOperation::Push, NoRemote) => Err(CoreError::OriginRequired),
    }
}

/// 网络操作执行结果：命令成功结束。新的仓库状态由后续刷新产生，
/// 不能把本结果当作扫描完成。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RepoOperationOutcome {
    pub operation: RepoGitOperation,
}

/// 网络操作输入。调用方（引擎）先用短暂设置读定位路径并确认仓库仍存在，
/// 再进入本函数；路径由不可变副本持有。
pub struct RepoOperationInput<'a> {
    pub repo_path: &'a Path,
    pub operation: RepoGitOperation,
    /// 仅 Pull / Push 需要；缺失或 false 在启动 Git 前拒绝。
    pub confirmed: bool,
}

/// 执行网络操作。顺序：确认校验 → 仓库操作槽（REPO_BUSY）→ 锁内重读状态 →
/// 前置校验 → 固定参数执行（网络超时）。成功 / 非零退出 / 超时都应请求刷新
/// （由引擎层 guard 保证，N07）；本函数不自动重试写操作。
pub fn execute_repo_operation(
    locks: &OperationLocks,
    input: RepoOperationInput<'_>,
    policy: &ExecPolicy,
    stop: &StopFlag,
) -> Result<RepoOperationOutcome, CoreError> {
    if matches!(
        input.operation,
        RepoGitOperation::Pull | RepoGitOperation::Push
    ) && !input.confirmed
    {
        return Err(CoreError::ConfirmationRequired);
    }
    let _guard: RepoOperationGuard = locks.try_acquire(input.repo_path)?;
    execute_repo_operation_with_slot(input, policy, stop)
}

/// 已持有仓库操作槽的执行体（S05）：引擎在槽内完成配置复核后调用本函数；
/// 不再获取槽（同一非重入槽不得重复获取），确认校验由获取方负责。
pub fn execute_repo_operation_with_slot(
    input: RepoOperationInput<'_>,
    policy: &ExecPolicy,
    stop: &StopFlag,
) -> Result<RepoOperationOutcome, CoreError> {
    if stop.is_stopped() {
        return Err(CoreError::EngineStopping);
    }
    let state = branch_state_monitored(input.repo_path, policy, stop).map_err(CoreError::from)?;
    validate_repo_operation(input.operation, state.relation, state.has_remote)?;

    let target = state.remote_branch;
    let args: Vec<String> = match input.operation {
        RepoGitOperation::Fetch => origin_fetch_args(),
        RepoGitOperation::Pull => {
            origin_pull_args(target.as_deref().ok_or(CoreError::ComparisonRequired)?)
        }
        RepoGitOperation::Push => {
            origin_push_args(target.as_deref().ok_or(CoreError::ComparisonRequired)?)
        }
    };
    let borrowed_args = args.iter().map(String::as_str).collect::<Vec<_>>();
    run_git_monitored(
        input.repo_path,
        &borrowed_args,
        policy.network_timeout,
        stop,
    )
    .map_err(CoreError::from)?;
    Ok(RepoOperationOutcome {
        operation: input.operation,
    })
}

// —— 探针 DTO（原型）——

/// 仓库记录 + 分支读取结果 → 契约 RepoStatus 行。
/// 读取失败映射为 readState=error + 稳定错误码，不伪装成 no_remote；
/// 动作规则统一走 `RepoActions::for_read`，两端不重复实现。
pub fn repo_status_from_state(
    record: &RepoRecord,
    result: Result<GitBranchState, GitCommandError>,
) -> RepoStatus {
    let path_text = record.path.to_string_lossy().into_owned();
    let mut row = RepoStatus::pending(&record.id, &record.name, path_text);
    row.group_id = record.group_id.clone();
    match result {
        Ok(state) => {
            row.read_state = ReadState::Ok;
            row.relation = Some(state.relation);
            row.branch = Some(state.branch);
            row.has_commit = Some(state.has_commit);
            row.has_origin = Some(state.has_remote);
            row.comparison_branch = state.remote_branch;
            row.ahead = Some(state.ahead);
            row.behind = Some(state.behind);
            row.remote_url = state.remote_url.clone();
            row.actions = RepoActions::for_read(
                ReadState::Ok,
                Some(state.relation),
                state.has_remote,
                state.remote_url.is_some(),
            );
        }
        Err(error) => {
            row.read_state = ReadState::Error;
            row.error = Some(error.into());
        }
    }
    row
}

impl From<GitCommandError> for ErrorObject {
    fn from(error: GitCommandError) -> Self {
        CoreError::from(error).to_error_object()
    }
}

/// 探针请求：`{"path": "..."}`，绝对路径。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProbeRequest {
    pub path: String,
}

/// 探针结果：单仓库真实 Git 读取。结构对齐契约 RepoStatus 的单仓库子集。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProbeResult {
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorObject>,
}

impl ProbeResult {
    fn error(error: CoreError) -> Self {
        Self {
            read_state: ReadState::Error,
            relation: None,
            branch: None,
            has_commit: None,
            has_origin: None,
            comparison_branch: None,
            ahead: None,
            behind: None,
            remote_url: None,
            error: Some(error.to_error_object()),
        }
    }
}

/// 探针入口：读取真实指定仓库，返回结构化结果（不产生 mock 数据）。
pub fn probe_repository(path: &str) -> ProbeResult {
    let repo_path = Path::new(path);
    if !repo_path.is_absolute() || !repo_path.exists() {
        return ProbeResult::error(CoreError::InvalidPath);
    }
    match branch_state(repo_path) {
        Ok(state) => ProbeResult {
            read_state: ReadState::Ok,
            relation: Some(state.relation),
            branch: Some(state.branch),
            has_commit: Some(state.has_commit),
            has_origin: Some(state.has_remote),
            comparison_branch: state.remote_branch,
            ahead: Some(state.ahead),
            behind: Some(state.behind),
            remote_url: state.remote_url,
            error: None,
        },
        Err(error) => ProbeResult::error(error.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_or_job_failure_is_not_missing_git() {
        let command = Command::new("git");
        let error = spawn_failure_message(
            &command,
            &std::io::Error::from(std::io::ErrorKind::PermissionDenied),
        );
        assert!(matches!(
            classify_command_error(error),
            GitCommandError::Failed(_)
        ));
    }

    #[test]
    fn missing_working_directory_is_not_missing_git() {
        let mut command = Command::new("git");
        command.current_dir(unique_temp_dir("gitaview_missing_cwd"));
        let error = spawn_failure_message(
            &command,
            &std::io::Error::from(std::io::ErrorKind::NotFound),
        );
        assert!(matches!(
            classify_command_error(error),
            GitCommandError::Failed(_)
        ));
    }

    #[test]
    fn missing_executable_still_maps_to_git_not_found() {
        let command = Command::new(format!(
            "gitaview-missing-executable-{}",
            std::process::id()
        ));
        let error = run_command_with_timeout(command, Duration::from_secs(2), SpawnMode::Direct)
            .unwrap_err();
        assert_eq!(classify_command_error(error), GitCommandError::NotFound);
    }
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(name: &str) -> std::path::PathBuf {
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

    #[test]
    fn probe_reports_synced_diverged_and_missing_relations() {
        let temp = unique_temp_dir("gitaview_core_probe_relations");
        let remote = temp.join("remote.git");
        let synced = temp.join("synced");
        let diverged = temp.join("diverged");
        fs::create_dir_all(&remote).unwrap();
        fs::create_dir_all(&synced).unwrap();
        fs::create_dir_all(&diverged).unwrap();
        test_git(&remote, &["init", "--bare", "-b", "main"]);
        test_git(&synced, &["init", "-b", "main"]);
        test_git(&synced, &["config", "user.email", "test@example.test"]);
        test_git(&synced, &["config", "user.name", "Test"]);
        test_git(&synced, &["commit", "--allow-empty", "-m", "initial"]);
        test_git(
            &synced,
            &["remote", "add", "origin", remote.to_str().unwrap()],
        );
        test_git(&synced, &["push", "-u", "origin", "main"]);

        let synced_result = probe_repository(synced.to_str().unwrap());
        assert_eq!(synced_result.read_state, ReadState::Ok);
        assert_eq!(synced_result.relation, Some(Relation::Synced));
        assert_eq!(synced_result.branch.as_deref(), Some("main"));
        assert_eq!(synced_result.comparison_branch.as_deref(), Some("main"));
        assert_eq!(synced_result.ahead, Some(0));
        assert_eq!(synced_result.behind, Some(0));
        assert_eq!(synced_result.has_origin, Some(true));

        test_git(&diverged, &["init", "-b", "main"]);
        test_git(&diverged, &["config", "user.email", "test@example.test"]);
        test_git(&diverged, &["config", "user.name", "Test"]);
        test_git(&diverged, &["commit", "--allow-empty", "-m", "initial"]);
        test_git(
            &diverged,
            &["remote", "add", "origin", remote.to_str().unwrap()],
        );
        test_git(&diverged, &["fetch", "origin"]);
        test_git(&diverged, &["reset", "--hard", "origin/main~0"]);
        test_git(&diverged, &["commit", "--allow-empty", "-m", "local-only"]);
        // 构造分叉：本地领先 1，再让远端前进 1（通过第三个 clone push）。
        let peer = temp.join("peer");
        test_git(
            &temp,
            &["clone", remote.to_str().unwrap(), peer.to_str().unwrap()],
        );
        test_git(&peer, &["config", "user.email", "test@example.test"]);
        test_git(&peer, &["config", "user.name", "Test"]);
        test_git(&peer, &["commit", "--allow-empty", "-m", "remote-only"]);
        test_git(&peer, &["push", "origin", "main"]);
        test_git(&diverged, &["fetch", "origin"]);
        let diverged_result = probe_repository(diverged.to_str().unwrap());
        assert_eq!(diverged_result.relation, Some(Relation::Diverged));
        assert_eq!(diverged_result.ahead, Some(1));
        assert_eq!(diverged_result.behind, Some(1));

        // 无 origin：读取成功但关系为 no_remote。
        let lonely = temp.join("lonely");
        fs::create_dir_all(&lonely).unwrap();
        test_git(&lonely, &["init", "-b", "main"]);
        test_git(&lonely, &["config", "user.email", "test@example.test"]);
        test_git(&lonely, &["config", "user.name", "Test"]);
        test_git(&lonely, &["commit", "--allow-empty", "-m", "initial"]);
        let lonely_result = probe_repository(lonely.to_str().unwrap());
        assert_eq!(lonely_result.read_state, ReadState::Ok);
        assert_eq!(lonely_result.relation, Some(Relation::NoRemote));
        assert_eq!(lonely_result.has_origin, Some(false));

        fs::remove_dir_all(temp).unwrap();
    }

    #[test]
    fn probe_reports_unborn_with_origin_as_no_remote_but_fetchable() {
        let temp = unique_temp_dir("gitaview_core_probe_unborn");
        let repo = temp.join("repo");
        fs::create_dir_all(&repo).unwrap();
        test_git(&repo, &["init", "-b", "main"]);
        // origin URL 是本地路径，不是浏览器可打开 URL；normalize 为 null。
        test_git(
            &repo,
            &["remote", "add", "origin", "file:///tmp/nowhere.git"],
        );
        let result = probe_repository(repo.to_str().unwrap());
        assert_eq!(result.read_state, ReadState::Ok);
        assert_eq!(result.relation, Some(Relation::NoRemote));
        assert_eq!(result.has_origin, Some(true));
        assert_eq!(result.remote_url, None);
        assert_eq!(result.ahead, Some(0));

        fs::remove_dir_all(temp).unwrap();
    }

    #[test]
    fn probe_maps_invalid_path_and_non_git_directory_to_stable_errors() {
        let missing = probe_repository("Z:/definitely/missing/path");
        assert_eq!(missing.read_state, ReadState::Error);
        assert_eq!(missing.error.as_ref().unwrap().code, "INVALID_PATH");

        let temp = unique_temp_dir("gitaview_core_probe_plain_dir");
        fs::create_dir_all(&temp).unwrap();
        let plain = probe_repository(temp.to_str().unwrap());
        assert_eq!(plain.read_state, ReadState::Error);
        assert_eq!(plain.error.as_ref().unwrap().code, "GIT_FAILED");
        fs::remove_dir_all(temp).unwrap();
    }

    #[test]
    fn run_git_captures_stdout() {
        let cwd = std::env::current_dir().unwrap();
        let output =
            run_git_monitored(&cwd, &["--version"], GIT_STATUS_TIMEOUT, &StopFlag::new()).unwrap();
        assert!(output.starts_with("git version"));
    }

    #[test]
    fn timeout_covers_pipes_held_by_descendants() {
        #[cfg(target_os = "windows")]
        let mut command = {
            let mut command = Command::new("cmd");
            command.args(["/C", "start /b ping -n 6 127.0.0.1"]);
            command
        };
        #[cfg(unix)]
        let mut command = {
            let mut command = Command::new("sh");
            command.args(["-c", "sleep 5 & exit 0"]);
            command
        };
        configure_git_child_process(&mut command);
        let started = Instant::now();
        let result =
            run_command_with_timeout(command, Duration::from_millis(300), SpawnMode::Suspended);
        assert!(started.elapsed() < Duration::from_secs(3));
        assert_eq!(result.unwrap_err(), GIT_TIMEOUT_SENTINEL);
    }

    #[test]
    fn direct_spawn_timeout_kills_process_via_job() {
        // Direct 路径（读操作）仍需保留整棵 Job 的超时终止语义。
        #[cfg(target_os = "windows")]
        let mut command = {
            let mut command = Command::new("cmd");
            command.args(["/C", "ping -n 30 127.0.0.1"]);
            command
        };
        #[cfg(unix)]
        let mut command = {
            let mut command = Command::new("sh");
            command.args(["-c", "sleep 30"]);
            command
        };
        configure_git_child_process(&mut command);
        let started = Instant::now();
        let result =
            run_command_with_timeout(command, Duration::from_millis(300), SpawnMode::Direct);
        assert!(started.elapsed() < Duration::from_secs(3));
        assert_eq!(result.unwrap_err(), GIT_TIMEOUT_SENTINEL);
    }
}
