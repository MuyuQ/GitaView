use std::path::Path;
use std::process::Command;
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

pub fn run_git(repo_path: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_path)
        .output()
        .map_err(|err| format!("failed to run git {:?}: {}", args, err))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

pub const GIT_OPERATION_TIMEOUT: Duration = Duration::from_secs(30);

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
}
