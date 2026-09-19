use std::io::Read;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::sync::OnceLock;
use std::thread;
use std::time::{Duration, Instant};

use crate::domain::status::RemoteRelation;
use crate::git::remote::normalize_remote_url;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitBranchState {
    pub branch: String,
    pub remote_branch: Option<String>,
    pub relation: RemoteRelation,
    pub ahead: u32,
    pub behind: u32,
    pub has_remote: bool,
    pub remote_url: Option<String>,
}

/// 本地状态读取超时：仓库在本地磁盘上，30 秒已经非常宽裕
pub const GIT_STATUS_TIMEOUT: Duration = Duration::from_secs(30);

/// 网络操作超时（fetch/pull/push）：慢网络上的大仓库合法耗时远超 30 秒，
/// 强杀 pull 可能留下 MERGE_HEAD / 锁文件，因此使用独立的长超时。
pub const GIT_NETWORK_TIMEOUT: Duration = Duration::from_secs(600);

const GIT_TIMEOUT_SENTINEL: &str = "git command timed out";

static RESOLVED_GIT: OnceLock<&'static str> = OnceLock::new();

/// 解析 git 可执行文件并缓存。
///
/// macOS 图形界面启动（Finder/Dock）只继承最小 PATH，仅装 Homebrew git 的
/// 用户会全部仓库读取失败。解析顺序：PATH → 常见安装位置 → 登录 shell。
pub fn git_program() -> &'static str {
    RESOLVED_GIT.get_or_init(|| match resolve_git_program() {
        Some(path) => {
            // git 可能安装在用户目录（如 ~/.cargo/bin），路径脱敏后记录
            crate::diagnostics::log(
                "git.resolved",
                format!(
                    "program={}",
                    crate::diagnostics::redact_path(Path::new(&path))
                ),
            );
            Box::leak(path.into_boxed_str())
        }
        None => {
            crate::diagnostics::log("git.resolved", "program=<PATH>");
            "git"
        }
    })
}

fn resolve_git_program() -> Option<String> {
    if probe_git_program("git").is_ok() {
        return None;
    }
    for candidate in [
        "/opt/homebrew/bin/git",
        "/usr/local/bin/git",
        "/usr/bin/git",
    ] {
        if probe_git_program(candidate).is_ok() {
            return Some(candidate.to_string());
        }
    }
    probe_login_shell_git()
}

fn probe_git_program(program: &str) -> Result<(), ()> {
    let mut command = Command::new(program);
    command.args(["--version"]);
    command.env("GIT_TERMINAL_PROMPT", "0");
    configure_git_child_process(&mut command);
    match command.stdout(Stdio::null()).stderr(Stdio::null()).status() {
        Ok(status) if status.success() => Ok(()),
        _ => Err(()),
    }
}

fn probe_login_shell_git() -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        // Windows GUI 启动继承用户 PATH，Git for Windows 安装器会写入，
        // 登录 shell 探测无意义
        None
    }

    #[cfg(not(target_os = "windows"))]
    {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
        let output = Command::new(&shell)
            .args(["-lc", "command -v git"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if path.is_empty() || !Path::new(&path).exists() {
            return None;
        }
        Some(path)
    }
}

fn run_command_with_timeout(mut command: Command, timeout: Duration) -> Result<Output, String> {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| format!("failed to spawn command: {}", err))?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| "failed to capture stdout".to_string())?;
    let mut stderr = child
        .stderr
        .take()
        .ok_or_else(|| "failed to capture stderr".to_string())?;
    let stdout_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).map(|_| bytes)
    });
    let stderr_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        stderr.read_to_end(&mut bytes).map(|_| bytes)
    });

    let start = std::time::Instant::now();
    loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|err| format!("failed to wait: {}", err))?
        {
            let stdout = stdout_reader
                .join()
                .map_err(|_| "stdout reader panicked".to_string())?
                .map_err(|err| format!("failed to collect stdout: {err}"))?;
            let stderr = stderr_reader
                .join()
                .map_err(|_| "stderr reader panicked".to_string())?
                .map_err(|err| format!("failed to collect stderr: {err}"))?;
            return Ok(Output {
                status,
                stdout,
                stderr,
            });
        }
        if start.elapsed() >= timeout {
            terminate_child_tree(&mut child);
            return Err(GIT_TIMEOUT_SENTINEL.to_string());
        }
        thread::sleep(Duration::from_millis(50));
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

fn terminate_child_tree(child: &mut std::process::Child) {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;

        let _ = Command::new("taskkill")
            .args(["/PID", &child.id().to_string(), "/T", "/F"])
            .creation_flags(CREATE_NO_WINDOW)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    #[cfg(unix)]
    {
        let _ = Command::new("kill")
            .args(["-KILL", &format!("-{}", child.id())])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    let _ = child.kill();
    let _ = child.wait();
}

pub fn run_git(repo_path: &Path, args: &[&str]) -> Result<String, String> {
    run_git_with_timeout(repo_path, args, GIT_STATUS_TIMEOUT)
}

pub(crate) fn run_git_args_with_timeout(
    repo_path: &Path,
    args: Vec<String>,
    timeout: Duration,
) -> Result<String, String> {
    let borrowed_args = args.iter().map(String::as_str).collect::<Vec<_>>();
    run_git_with_timeout(repo_path, &borrowed_args, timeout)
}

fn run_git_with_timeout(
    repo_path: &Path,
    args: &[&str],
    timeout: Duration,
) -> Result<String, String> {
    let started = Instant::now();
    let command_label = args.join(" ");
    let mut command = Command::new(git_program());
    command.args(args);
    command.current_dir(repo_path);
    command.env("GIT_TERMINAL_PROMPT", "0");
    command.env("GCM_INTERACTIVE", "Never");
    configure_git_child_process(&mut command);

    let output = run_command_with_timeout(command, timeout).map_err(|err| {
        if err == GIT_TIMEOUT_SENTINEL {
            timeout_error_message(repo_path, args.first().copied().unwrap_or("git"), timeout)
        } else {
            format!("启动 git 失败（program={}）: {err}", git_program())
        }
    })?;
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        crate::diagnostics::log_duration(
            "git.command.ok",
            started.elapsed(),
            format!(
                "cwd={} command={} stdout_len={}",
                crate::diagnostics::redact_path(repo_path),
                command_label,
                stdout.len()
            ),
        );
        Ok(stdout)
    } else {
        let err = format_git_failure(args, &output.stderr);
        crate::diagnostics::log_duration(
            "git.command.error",
            started.elapsed(),
            format!(
                "cwd={} command={} error_len={}",
                crate::diagnostics::redact_path(repo_path),
                command_label,
                err.len()
            ),
        );
        Err(err)
    }
}

/// 超时错误信息附带恢复指引：被强杀的 pull 可能留下未完成的合并状态
fn timeout_error_message(repo_path: &Path, op: &str, timeout: Duration) -> String {
    let mut message = format!("git {op} 超时（{} 秒），已中止", timeout.as_secs());
    if matches!(op, "fetch" | "pull" | "push") {
        if repo_path.join(".git").join("MERGE_HEAD").exists() {
            message.push_str(
                "；检测到未完成的合并状态（MERGE_HEAD），可运行 git merge --abort 恢复后重试",
            );
        } else {
            message.push_str("；若重试时报 lock 文件冲突，请稍候再试");
        }
    }
    message
}

pub(crate) fn origin_fetch_args() -> Vec<String> {
    vec!["fetch".to_string(), "origin".to_string()]
}

pub(crate) fn origin_pull_args(branch: &str) -> Vec<String> {
    vec![
        "pull".to_string(),
        "origin".to_string(),
        format!("refs/heads/{branch}"),
    ]
}

pub(crate) fn origin_push_args(branch: &str) -> Vec<String> {
    vec![
        "push".to_string(),
        "origin".to_string(),
        format!("HEAD:refs/heads/{branch}"),
    ]
}

pub fn format_git_failure(args: &[&str], stderr: &[u8]) -> String {
    let stderr_str = String::from_utf8_lossy(stderr).trim().to_string();
    format!("git {} failed: {}", args.join(" "), stderr_str)
}

pub const GIT_OPERATION_TIMEOUT: Duration = Duration::from_secs(30);

/// for-each-ref 输出行字段分隔符（tab，git 引用名不允许包含）
const REF_FIELD_SEP: char = '\t';

/// 一次 for-each-ref 取回当前分支、upstream 与 ahead/behind 轨迹，
/// 替代旧的 rev-parse×2 + rev-list 组合，把 happy path 的 git spawn
/// 从每仓库 5 次降到 2 次。track 取 nobracket 形式：""（同步）、
/// "ahead 1, behind 2"、upstream 缺失时为 "gone"。
/// 不用 `status --porcelain=v2 --branch` 的原因：status 会全量扫描工作树，
/// 大仓库下远慢于 for-each-ref，而本应用只需要分支关系，不需要文件变更。
const CURRENT_BRANCH_FORMAT: &str =
    "%(HEAD)%09%(refname:short)%09%(upstream:short)%09%(upstream:track,nobracket)";

struct CurrentBranchLine {
    branch: String,
    upstream: Option<String>,
    track: String,
}

fn current_branch_line(repo_path: &Path) -> Result<CurrentBranchLine, String> {
    let output = run_git(
        repo_path,
        &[
            "for-each-ref",
            &format!("--format={CURRENT_BRANCH_FORMAT}"),
            "refs/heads",
        ],
    )?;
    let current = output
        .lines()
        .find(|line| line.starts_with('*'))
        .ok_or_else(|| "HEAD".to_string())?;
    // 行首是 "*<tab>"：同时去掉星标与其后的字段分隔符
    let mut fields = current
        .strip_prefix('*')
        .and_then(|rest| rest.strip_prefix(REF_FIELD_SEP))
        .unwrap_or_default()
        .split(REF_FIELD_SEP);
    let branch = fields.next().unwrap_or_default().trim().to_string();
    if branch.is_empty() {
        return Err("HEAD".to_string());
    }
    let upstream = fields
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    let track = fields.next().unwrap_or_default().trim().to_string();
    Ok(CurrentBranchLine {
        branch,
        upstream,
        track,
    })
}

fn parse_track_counts(track: &str) -> Option<(u32, u32)> {
    let mut ahead = 0u32;
    let mut behind = 0u32;
    let mut seen = false;
    for part in track.split(',') {
        let part = part.trim();
        let Some((label, value)) = part.split_once(' ') else {
            continue;
        };
        let Ok(value) = value.trim().parse::<u32>() else {
            continue;
        };
        match label {
            "ahead" => {
                ahead = value;
                seen = true;
            }
            "behind" => {
                behind = value;
                seen = true;
            }
            _ => {}
        }
    }
    seen.then_some((ahead, behind))
}

/// upstream 缺失（[gone]）或未配置时回退到 origin/<branch>：
/// 与旧行为一致——仅当该引用真实存在时才参与比较（rev-list 失败即视为无远端）。
fn fallback_origin_ahead_behind(repo_path: &Path, branch: &str) -> Option<(u32, u32)> {
    let rev_range = format!("HEAD...origin/{branch}");
    let counts = run_git(
        repo_path,
        &["rev-list", "--left-right", "--count", &rev_range],
    )
    .ok()?;
    let mut parts = counts.split_whitespace();
    let ahead = parts
        .next()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(0);
    let behind = parts
        .next()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(0);
    Some((ahead, behind))
}

pub fn branch_state(repo_path: &Path) -> Result<GitBranchState, String> {
    let raw_remote_url = run_git(repo_path, &["config", "--get", "remote.origin.url"]).ok();
    let has_origin_remote = raw_remote_url
        .as_ref()
        .is_some_and(|url| !url.trim().is_empty());
    let remote_url = raw_remote_url.and_then(|url| normalize_remote_url(&url));

    let line = match current_branch_line(repo_path) {
        Ok(line) => line,
        // detached HEAD：for-each-ref 没有 * 标记行；保持只读 no_remote 关系
        Err(message) if message == "HEAD" => {
            return Ok(GitBranchState {
                branch: "HEAD".to_string(),
                remote_branch: None,
                relation: RemoteRelation::NoRemote,
                ahead: 0,
                behind: 0,
                has_remote: has_origin_remote,
                remote_url,
            });
        }
        Err(err) => return Err(err),
    };

    // v1 只支持 origin 远端比较：非 origin upstream 一律 no_remote
    let origin_upstream = line
        .upstream
        .as_ref()
        .filter(|upstream| upstream.starts_with("origin/"));

    let (compare_ref, counts) = match origin_upstream {
        Some(upstream) if line.track != "gone" => {
            let counts = parse_track_counts(&line.track).unwrap_or((0, 0));
            (Some(upstream.clone()), counts)
        }
        // upstream 未配置、非 origin、或已 gone：回退 origin/<branch>
        _ if has_origin_remote => match fallback_origin_ahead_behind(repo_path, &line.branch) {
            Some(counts) => (Some(format!("origin/{}", line.branch)), counts),
            None => (None, (0, 0)),
        },
        _ => (None, (0, 0)),
    };

    let Some(compare_ref) = compare_ref else {
        return Ok(GitBranchState {
            branch: line.branch,
            remote_branch: None,
            relation: RemoteRelation::NoRemote,
            ahead: 0,
            behind: 0,
            has_remote: has_origin_remote,
            remote_url,
        });
    };

    let (ahead, behind) = counts;
    let relation = match (ahead, behind) {
        (0, 0) => RemoteRelation::Synced,
        (_, 0) => RemoteRelation::LocalAhead,
        (0, _) => RemoteRelation::RemoteAhead,
        _ => RemoteRelation::Diverged,
    };

    Ok(GitBranchState {
        remote_branch: compare_ref.strip_prefix("origin/").map(str::to_owned),
        branch: line.branch,
        relation,
        ahead,
        behind,
        has_remote: has_origin_remote,
        remote_url,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;
    use std::process::Command;
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
    fn actions_use_the_configured_origin_branch_when_local_name_differs() {
        let temp = unique_temp_dir("gitaview_different_upstream");
        let repo = temp.join("repo");
        let remote = temp.join("remote.git");
        fs::create_dir_all(&repo).unwrap();
        fs::create_dir_all(&remote).unwrap();
        test_git(&remote, &["init", "--bare"]);
        test_git(&repo, &["init", "-b", "main"]);
        test_git(&repo, &["config", "user.email", "test@example.test"]);
        test_git(&repo, &["config", "user.name", "Test"]);
        test_git(&repo, &["commit", "--allow-empty", "-m", "initial"]);
        test_git(
            &repo,
            &["remote", "add", "origin", remote.to_str().unwrap()],
        );
        test_git(&repo, &["push", "-u", "origin", "main"]);
        test_git(&repo, &["branch", "-m", "feature"]);
        test_git(&repo, &["commit", "--allow-empty", "-m", "local"]);
        let state = branch_state(&repo).unwrap();
        assert_eq!(state.branch, "feature");
        assert_eq!(state.remote_branch.as_deref(), Some("main"));
        let target = state.remote_branch.as_deref().unwrap();
        assert_eq!(
            origin_pull_args(target),
            vec!["pull", "origin", "refs/heads/main"]
        );
        run_git_args_with_timeout(&repo, origin_push_args(target), GIT_NETWORK_TIMEOUT).unwrap();
        assert!(run_git(&remote, &["show-ref", "--verify", "refs/heads/main"]).is_ok());
        assert!(run_git(&remote, &["show-ref", "--verify", "refs/heads/feature"]).is_err());
        let peer = temp.join("peer");
        test_git(
            &temp,
            &[
                "clone",
                "-b",
                "main",
                remote.to_str().unwrap(),
                peer.to_str().unwrap(),
            ],
        );
        test_git(&peer, &["config", "user.email", "test@example.test"]);
        test_git(&peer, &["config", "user.name", "Test"]);
        test_git(&peer, &["commit", "--allow-empty", "-m", "remote change"]);
        test_git(&peer, &["push", "origin", "main"]);
        run_git_args_with_timeout(&repo, origin_pull_args(target), GIT_NETWORK_TIMEOUT).unwrap();
        assert_eq!(
            run_git(&repo, &["rev-parse", "HEAD"]).unwrap(),
            run_git(&peer, &["rev-parse", "HEAD"]).unwrap()
        );
        assert_eq!(branch_state(&repo).unwrap().branch, "feature");
        fs::remove_dir_all(temp).unwrap();
    }

    #[test]
    fn formats_git_failure_message() {
        assert_eq!(
            format_git_failure(&["fetch"], b"fatal: failed"),
            "git fetch failed: fatal: failed",
        );
    }

    #[test]
    fn run_git_captures_stdout() {
        let cwd = std::env::current_dir().unwrap();
        let output = run_git(&cwd, &["--version"]).unwrap();
        assert!(output.starts_with("git version"));
    }

    #[test]
    fn origin_action_args_pin_remote_and_branch() {
        assert_eq!(origin_fetch_args(), vec!["fetch", "origin"]);
        assert_eq!(
            origin_pull_args("main"),
            vec!["pull", "origin", "refs/heads/main"]
        );
        assert_eq!(
            origin_push_args("main"),
            vec!["push", "origin", "HEAD:refs/heads/main"],
        );
    }

    #[test]
    fn branch_state_uses_matching_origin_branch_when_upstream_is_stale() {
        let temp = unique_temp_dir("gitaview_stale_upstream_test");
        let repo = temp.join("repo");
        let remote = temp.join("remote.git");
        fs::create_dir_all(&repo).unwrap();
        fs::create_dir_all(&remote).unwrap();

        test_git(&remote, &["init", "--bare"]);
        test_git(&repo, &["init", "-b", "main"]);
        test_git(&repo, &["config", "user.email", "gitaview@example.test"]);
        test_git(&repo, &["config", "user.name", "GitaView Test"]);
        fs::write(repo.join("README.md"), "initial\n").unwrap();
        test_git(&repo, &["add", "README.md"]);
        test_git(&repo, &["commit", "-m", "initial"]);
        test_git(
            &repo,
            &["remote", "add", "origin", remote.to_str().unwrap()],
        );
        test_git(&repo, &["push", "-u", "origin", "main"]);

        fs::write(repo.join("README.md"), "initial\nlocal\n").unwrap();
        test_git(&repo, &["commit", "-am", "local change"]);
        test_git(&repo, &["config", "branch.main.remote", "origin"]);
        test_git(&repo, &["config", "branch.main.merge", "refs/heads/master"]);

        let state = branch_state(&repo).unwrap();

        assert_eq!(state.relation, RemoteRelation::LocalAhead);
        assert_eq!(state.ahead, 1);
        assert_eq!(state.behind, 0);

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn branch_state_treats_non_origin_upstream_as_unsupported_in_v1() {
        let temp = unique_temp_dir("gitaview_non_origin_upstream_test");
        let repo = temp.join("repo");
        let remote = temp.join("upstream.git");
        fs::create_dir_all(&repo).unwrap();
        fs::create_dir_all(&remote).unwrap();

        test_git(&remote, &["init", "--bare"]);
        test_git(&repo, &["init", "-b", "main"]);
        test_git(&repo, &["config", "user.email", "gitaview@example.test"]);
        test_git(&repo, &["config", "user.name", "GitaView Test"]);
        fs::write(repo.join("README.md"), "initial\n").unwrap();
        test_git(&repo, &["add", "README.md"]);
        test_git(&repo, &["commit", "-m", "initial"]);
        test_git(
            &repo,
            &["remote", "add", "upstream", remote.to_str().unwrap()],
        );
        test_git(&repo, &["push", "-u", "upstream", "main"]);

        let state = branch_state(&repo).unwrap();

        assert_eq!(state.relation, RemoteRelation::NoRemote);
        assert!(!state.has_remote);
        assert_eq!(state.remote_url, None);

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn branch_state_treats_detached_head_as_unsupported_for_origin_actions() {
        let temp = unique_temp_dir("gitaview_detached_head_test");
        let repo = temp.join("repo");
        let remote = temp.join("remote.git");
        fs::create_dir_all(&repo).unwrap();
        fs::create_dir_all(&remote).unwrap();

        test_git(&remote, &["init", "--bare"]);
        test_git(&repo, &["init", "-b", "main"]);
        test_git(&repo, &["config", "user.email", "gitaview@example.test"]);
        test_git(&repo, &["config", "user.name", "GitaView Test"]);
        fs::write(repo.join("README.md"), "initial\n").unwrap();
        test_git(&repo, &["add", "README.md"]);
        test_git(&repo, &["commit", "-m", "initial"]);
        test_git(
            &repo,
            &["remote", "add", "origin", remote.to_str().unwrap()],
        );
        test_git(&repo, &["push", "-u", "origin", "main"]);
        test_git(
            &repo,
            &[
                "symbolic-ref",
                "refs/remotes/origin/HEAD",
                "refs/remotes/origin/main",
            ],
        );
        test_git(&repo, &["checkout", "--detach", "HEAD"]);

        let state = branch_state(&repo).unwrap();

        assert_eq!(state.branch, "HEAD");
        assert_eq!(state.relation, RemoteRelation::NoRemote);
        assert_eq!(state.ahead, 0);
        assert_eq!(state.behind, 0);
        assert!(state.has_remote);

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn branch_state_errors_for_missing_repo_path() {
        let missing = std::env::temp_dir().join("gitaview_missing_repo_path_for_branch_state");
        let _ = std::fs::remove_dir_all(&missing);
        assert!(branch_state(&missing).is_err());
    }

    #[test]
    fn branch_state_reports_diverged_counts_from_upstream_track() {
        let temp = unique_temp_dir("gitaview_diverged_track_test");
        let repo = temp.join("repo");
        let remote = temp.join("remote.git");
        fs::create_dir_all(&repo).unwrap();
        fs::create_dir_all(&remote).unwrap();

        test_git(&remote, &["init", "--bare"]);
        test_git(&repo, &["init", "-b", "main"]);
        test_git(&repo, &["config", "user.email", "gitaview@example.test"]);
        test_git(&repo, &["config", "user.name", "GitaView Test"]);
        fs::write(repo.join("README.md"), "base\n").unwrap();
        test_git(&repo, &["add", "README.md"]);
        test_git(&repo, &["commit", "-m", "base"]);
        test_git(
            &repo,
            &["remote", "add", "origin", remote.to_str().unwrap()],
        );
        test_git(&repo, &["push", "-u", "origin", "main"]);

        // 本地领先 1（改动独立文件，避免与远端改动冲突）
        fs::write(repo.join("local.txt"), "local\n").unwrap();
        test_git(&repo, &["add", "local.txt"]);
        test_git(&repo, &["commit", "-m", "local"]);
        // 远端领先 2
        let peer = temp.join("peer");
        test_git(
            &temp,
            &["clone", remote.to_str().unwrap(), peer.to_str().unwrap()],
        );
        test_git(&peer, &["config", "user.email", "gitaview@example.test"]);
        test_git(&peer, &["config", "user.name", "GitaView Test"]);
        fs::write(peer.join("remote1.txt"), "remote1\n").unwrap();
        test_git(&peer, &["add", "remote1.txt"]);
        test_git(&peer, &["commit", "-m", "remote1"]);
        fs::write(peer.join("remote2.txt"), "remote2\n").unwrap();
        test_git(&peer, &["add", "remote2.txt"]);
        test_git(&peer, &["commit", "-m", "remote2"]);
        test_git(&peer, &["push", "origin", "main"]);
        test_git(&repo, &["fetch", "origin"]);

        let state = branch_state(&repo).unwrap();

        assert_eq!(state.relation, RemoteRelation::Diverged);
        assert_eq!(state.ahead, 1);
        assert_eq!(state.behind, 2);
        assert_eq!(state.remote_branch.as_deref(), Some("main"));

        // rebase 整合远端后本地领先 1，推送恢复同步
        test_git(&repo, &["pull", "--rebase", "origin", "main"]);
        let rebased = branch_state(&repo).unwrap();
        assert_eq!(rebased.relation, RemoteRelation::LocalAhead);
        test_git(&repo, &["push", "origin", "main"]);
        let synced = branch_state(&repo).unwrap();
        assert_eq!(synced.relation, RemoteRelation::Synced);
        assert_eq!((synced.ahead, synced.behind), (0, 0));

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn branch_state_falls_back_to_no_remote_when_origin_branch_is_missing() {
        let temp = unique_temp_dir("gitaview_gone_upstream_test");
        let repo = temp.join("repo");
        let remote = temp.join("remote.git");
        fs::create_dir_all(&repo).unwrap();
        fs::create_dir_all(&remote).unwrap();

        test_git(&remote, &["init", "--bare"]);
        test_git(&repo, &["init", "-b", "main"]);
        test_git(&repo, &["config", "user.email", "gitaview@example.test"]);
        test_git(&repo, &["config", "user.name", "GitaView Test"]);
        test_git(&repo, &["commit", "--allow-empty", "-m", "initial"]);
        test_git(
            &repo,
            &["remote", "add", "origin", remote.to_str().unwrap()],
        );
        // 从未 push：upstream 未配置，origin/main 也不存在 → no_remote
        let state = branch_state(&repo).unwrap();

        assert_eq!(state.relation, RemoteRelation::NoRemote);
        assert!(state.has_remote, "配置了 origin 即视为有远端");
        assert_eq!(state.remote_branch, None);

        let _ = fs::remove_dir_all(&temp);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn command_timeout_runner_drains_large_stdout_before_exit() {
        let mut command = Command::new("cmd");
        command.args([
            "/C",
            "for /L %i in (1,1,20000) do @echo 0123456789012345678901234567890123456789",
        ]);

        let output = run_command_with_timeout(command, Duration::from_secs(5)).unwrap();

        assert!(output.status.success());
        assert!(output.stdout.len() > 500_000);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn command_timeout_runner_terminates_descendant_processes() {
        let temp = unique_temp_dir("gitaview_timeout_descendant");
        let marker = temp.join("descendant-survived.txt");
        let child_script = temp.join("child.ps1");
        let parent_script = temp.join("parent.ps1");
        fs::create_dir_all(&temp).unwrap();
        fs::write(
            &child_script,
            format!(
                "Start-Sleep -Seconds 2\nSet-Content -LiteralPath '{}' -Value alive\n",
                marker.display()
            ),
        )
        .unwrap();
        fs::write(
            &parent_script,
            format!(
                "Start-Process powershell -WindowStyle Hidden -ArgumentList '-NoProfile','-File','{}'\nStart-Sleep -Seconds 10\n",
                child_script.display()
            ),
        )
        .unwrap();
        let mut command = Command::new("powershell");
        command.args(["-NoProfile", "-File", parent_script.to_str().unwrap()]);

        assert!(run_command_with_timeout(command, Duration::from_millis(300)).is_err());
        thread::sleep(Duration::from_secs(3));

        assert!(!marker.exists(), "timed out descendant process survived");
        let _ = fs::remove_dir_all(&temp);
    }
}
