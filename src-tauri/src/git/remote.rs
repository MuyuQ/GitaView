pub fn normalize_remote_url(raw: &str) -> Option<String> {
    let value = raw.trim();
    if value.is_empty() {
        return None;
    }
    let lower_value = value.to_ascii_lowercase();
    if lower_value.starts_with("git@github.com:") {
        let rest = &value["git@github.com:".len()..];
        return Some(format!(
            "https://github.com/{}",
            rest.trim_end_matches(".git")
        ));
    }
    if let Some((scheme, rest)) = value.split_once("://") {
        let normalized_scheme = scheme.to_ascii_lowercase();
        if normalized_scheme == "https" || normalized_scheme == "http" {
            return Some(format!(
                "{}://{}",
                normalized_scheme,
                rest.trim_end_matches(".git")
            ));
        }
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
    fn normalizes_remote_url_scheme_and_github_host_case_insensitively() {
        assert_eq!(
            normalize_remote_url("HTTPS://gitlab.com/owner/repo.git"),
            Some("https://gitlab.com/owner/repo".to_string()),
        );
        assert_eq!(
            normalize_remote_url("git@GitHub.com:owner/repo.git"),
            Some("https://github.com/owner/repo".to_string()),
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
