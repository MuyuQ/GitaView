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
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| format!("failed to spawn command: {}", err))?;

    let start = std::time::Instant::now();
    loop {
        if child
            .try_wait()
            .map_err(|err| format!("failed to wait: {}", err))?
            .is_some()
        {
            return child
                .wait_with_output()
                .map_err(|err| format!("failed to collect output: {}", err));
        }
        if start.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err("git command timed out".to_string());
        }
        thread::sleep(Duration::from_millis(50));
    }
}

pub fn run_git(repo_path: &Path, args: &[&str]) -> Result<String, String> {
    let started = Instant::now();
    let command_label = args.join(" ");
    let mut command = Command::new("git");
    command.args(args);
    command.current_dir(repo_path);
    command.env("GIT_TERMINAL_PROMPT", "0");
    command.env("GCM_INTERACTIVE", "Never");

    let output = run_command_with_timeout(command, GIT_OPERATION_TIMEOUT)?;
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        crate::diagnostics::log_duration(
            "git.command.ok",
            started.elapsed(),
            format!(
                "cwd={} command={} stdout_len={}",
                repo_path.display(),
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
                "cwd={} command={} error={}",
                repo_path.display(),
                command_label,
                err
            ),
        );
        Err(err)
    }
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
        return Some(upstream);
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
    fn branch_state_errors_for_missing_repo_path() {
        let missing = std::env::temp_dir().join("gitaview_missing_repo_path_for_branch_state");
        let _ = std::fs::remove_dir_all(&missing);
        assert!(branch_state(&missing).is_err());
    }
}
