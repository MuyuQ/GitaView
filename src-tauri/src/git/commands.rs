use std::io::Read;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::domain::status::RemoteRelation;
use crate::git::remote::normalize_remote_url;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitBranchState {
    pub branch: String,
    pub relation: RemoteRelation,
    pub ahead: u32,
    pub behind: u32,
    pub has_remote: bool,
    pub remote_url: Option<String>,
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
            return Err("git command timed out".to_string());
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
    let started = Instant::now();
    let command_label = args.join(" ");
    let mut command = Command::new("git");
    command.args(args);
    command.current_dir(repo_path);
    command.env("GIT_TERMINAL_PROMPT", "0");
    command.env("GCM_INTERACTIVE", "Never");
    configure_git_child_process(&mut command);

    let output = run_command_with_timeout(command, GIT_OPERATION_TIMEOUT)?;
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

pub(crate) fn origin_fetch_args() -> Vec<String> {
    vec!["fetch".to_string(), "origin".to_string()]
}

pub(crate) fn origin_pull_args(branch: &str) -> Vec<String> {
    vec!["pull".to_string(), "origin".to_string(), branch.to_string()]
}

pub(crate) fn origin_push_args(branch: &str) -> Vec<String> {
    vec![
        "push".to_string(),
        "origin".to_string(),
        format!("HEAD:refs/heads/{branch}"),
    ]
}

pub(crate) fn run_git_args(repo_path: &Path, args: Vec<String>) -> Result<String, String> {
    let borrowed_args = args.iter().map(String::as_str).collect::<Vec<_>>();
    run_git(repo_path, &borrowed_args)
}

pub fn format_git_failure(args: &[&str], stderr: &[u8]) -> String {
    let stderr_str = String::from_utf8_lossy(stderr).trim().to_string();
    format!("git {} failed: {}", args.join(" "), stderr_str)
}

pub const GIT_OPERATION_TIMEOUT: Duration = Duration::from_secs(30);

fn comparison_ref(repo_path: &Path, branch: &str, has_remote: bool) -> Option<String> {
    if let Ok(upstream) = run_git(
        repo_path,
        &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"],
    ) {
        if upstream.starts_with("origin/") {
            return Some(upstream);
        }
    }

    if !has_remote {
        return None;
    }

    let origin_branch = format!("origin/{branch}");
    let full_origin_ref = format!("refs/remotes/{origin_branch}");
    run_git(
        repo_path,
        &["rev-parse", "--verify", "--quiet", &full_origin_ref],
    )
    .ok()
    .map(|_| origin_branch)
}

pub fn branch_state(repo_path: &Path) -> Result<GitBranchState, String> {
    let inside = run_git(repo_path, &["rev-parse", "--is-inside-work-tree"])?;
    if inside != "true" {
        return Err("不是有效的 Git 工作区".to_string());
    }
    let branch = run_git(repo_path, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    let raw_remote_url = run_git(repo_path, &["config", "--get", "remote.origin.url"]).ok();
    let has_origin_remote = raw_remote_url
        .as_ref()
        .is_some_and(|url| !url.trim().is_empty());
    let remote_url = raw_remote_url.and_then(|url| normalize_remote_url(&url));
    if branch == "HEAD" {
        return Ok(GitBranchState {
            branch,
            relation: RemoteRelation::NoRemote,
            ahead: 0,
            behind: 0,
            has_remote: has_origin_remote,
            remote_url,
        });
    }

    let Some(compare_ref) = comparison_ref(repo_path, &branch, has_origin_remote) else {
        return Ok(GitBranchState {
            branch,
            relation: RemoteRelation::NoRemote,
            ahead: 0,
            behind: 0,
            has_remote: has_origin_remote,
            remote_url,
        });
    };

    let rev_range = format!("HEAD...{compare_ref}");
    let counts = run_git(
        repo_path,
        &["rev-list", "--left-right", "--count", &rev_range],
    )?;
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
        (0, 0) => RemoteRelation::Synced,
        (_, 0) => RemoteRelation::LocalAhead,
        (0, _) => RemoteRelation::RemoteAhead,
        _ => RemoteRelation::Diverged,
    };

    Ok(GitBranchState {
        branch,
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
        assert_eq!(origin_pull_args("main"), vec!["pull", "origin", "main"]);
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
