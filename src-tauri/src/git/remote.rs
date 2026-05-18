pub fn normalize_remote_url(raw: &str) -> Option<String> {
    let value = raw.trim();
    if value.is_empty() {
        return None;
    }
    if let Some(rest) = value.strip_prefix("git@github.com:") {
        return Some(format!(
            "https://github.com/{}",
            rest.trim_end_matches(".git")
        ));
    }
    if value.starts_with("https://") || value.starts_with("http://") {
        return Some(value.trim_end_matches(".git").to_string());
    }
    None
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
    fn rejects_unsupported_remote_urls_for_opening() {
        assert_eq!(
            normalize_remote_url("ssh://git@example.com/owner/repo.git"),
            None
        );
        assert_eq!(normalize_remote_url("file:///tmp/repo.git"), None);
    }

    #[test]
    fn rejects_empty_remote_urls() {
        assert_eq!(normalize_remote_url("   "), None);
    }
}
