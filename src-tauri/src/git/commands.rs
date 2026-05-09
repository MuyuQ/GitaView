use std::path::Path;
use std::process::{Command, Output};
use std::thread;
use std::time::Duration;

use crate::domain::status::RemoteRelation;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitBranchState {
    pub branch: String,
    pub relation: RemoteRelation,
    pub ahead: u32,
    pub behind: u32,
    pub remote_url: Option<String>,
}

pub fn normalize_remote_url(raw: &str) -> Option<String> {
    let value = raw.trim();
    if value.is_empty() {
        return None;
    }
    if let Some(rest) = value.strip_prefix("git@github.com:") {
        return Some(format!("https://github.com/{}", rest.trim_end_matches(".git")));
    }
    if value.starts_with("https://") || value.starts_with("http://") {
        return Some(value.trim_end_matches(".git").to_string());
    }
    Some(value.to_string())
}

fn run_command_with_timeout(mut command: Command, timeout: Duration) -> Result<Output, String> {
    let mut child = command.spawn().map_err(|err| format!("failed to spawn command: {}", err))?;

    let start = std::time::Instant::now();
    loop {
        if child.try_wait().map_err(|err| format!("failed to wait: {}", err))?.is_some() {
            return child.wait_with_output().map_err(|err| format!("failed to collect output: {}", err));
        }
        if start.elapsed() >= timeout {
            let _ = child.kill();
            return Err("git command timed out".to_string());
        }
        thread::sleep(Duration::from_millis(50));
    }
}

pub fn run_git(repo_path: &Path, args: &[&str]) -> Result<String, String> {
    let mut command = Command::new("git");
    command.args(args);
    command.current_dir(repo_path);
    command.env("GIT_TERMINAL_PROMPT", "0");
    command.env("GCM_INTERACTIVE", "Never");

    let output = run_command_with_timeout(command, GIT_OPERATION_TIMEOUT)?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(format_git_failure(args, &output.stderr))
    }
}

pub fn format_git_failure(args: &[&str], stderr: &[u8]) -> String {
    let stderr_str = String::from_utf8_lossy(stderr).trim().to_string();
    format!("git {} failed: {}", args.join(" "), stderr_str)
}

pub const GIT_OPERATION_TIMEOUT: Duration = Duration::from_secs(30);

pub fn branch_state(repo_path: &Path) -> Result<GitBranchState, String> {
    let branch = run_git(repo_path, &["rev-parse", "--abbrev-ref", "HEAD"])
        .unwrap_or_else(|_| "HEAD".to_string());
    let remote_url = run_git(repo_path, &["config", "--get", "remote.origin.url"])
        .ok()
        .and_then(|url| normalize_remote_url(&url));

    if run_git(repo_path, &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"]).is_err() {
        return Ok(GitBranchState {
            branch,
            relation: RemoteRelation::NoRemote,
            ahead: 0,
            behind: 0,
            remote_url,
        });
    }

    let counts = run_git(repo_path, &["rev-list", "--left-right", "--count", "HEAD...@{u}"])?;
    let mut parts = counts.split_whitespace();
    let ahead = parts.next().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
    let behind = parts.next().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
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
        remote_url,
    })
}

pub fn change_label(state: &GitBranchState) -> String {
    match state.relation {
        RemoteRelation::Synced => "✓".to_string(),
        RemoteRelation::LocalAhead => format!("↑ {}", state.ahead),
        RemoteRelation::RemoteAhead => format!("↓ {}", state.behind),
        RemoteRelation::Diverged => format!("⇕ {}", state.ahead + state.behind),
        RemoteRelation::NoRemote => "∅".to_string(),
    }
}

pub fn relation_hint(relation: RemoteRelation) -> &'static str {
    match relation {
        RemoteRelation::Synced => "无需操作",
        RemoteRelation::LocalAhead => "可 Push",
        RemoteRelation::RemoteAhead => "可 Pull",
        RemoteRelation::Diverged => "需要人工处理",
        RemoteRelation::NoRemote => "未设置 upstream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_github_ssh_url() {
        assert_eq!(
            normalize_remote_url("git@github.com:owner/repo.git"),
            Some("https://github.com/owner/repo".to_string()),
        );
    }

    #[test]
    fn keeps_non_github_urls_openable() {
        assert_eq!(
            normalize_remote_url("https://gitlab.com/owner/repo.git"),
            Some("https://gitlab.com/owner/repo".to_string()),
        );
    }

    #[test]
    fn formats_change_labels_for_all_relations() {
        let mut state = GitBranchState {
            branch: "main".to_string(),
            relation: RemoteRelation::LocalAhead,
            ahead: 2,
            behind: 0,
            remote_url: None,
        };
        assert_eq!(change_label(&state), "↑ 2");

        state.relation = RemoteRelation::RemoteAhead;
        state.ahead = 0;
        state.behind = 3;
        assert_eq!(change_label(&state), "↓ 3");

        state.relation = RemoteRelation::Diverged;
        state.ahead = 2;
        state.behind = 3;
        assert_eq!(change_label(&state), "⇕ 5");
    }

    #[test]
    fn formats_git_failure_message() {
        assert_eq!(
            format_git_failure(&["fetch"], b"fatal: failed"),
            "git fetch failed: fatal: failed",
        );
    }
}
