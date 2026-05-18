use std::path::Path;

pub(crate) fn open_directory(path: &Path) -> Result<(), String> {
    spawn_system_open(&path.to_string_lossy(), "无法打开目录")
}

pub(crate) fn open_http_url(url: &str) -> Result<(), String> {
    validate_http_url(url)?;
    spawn_system_open(url, "无法打开 URL")
}

fn validate_http_url(url: &str) -> Result<(), String> {
    if url.starts_with("http://") || url.starts_with("https://") {
        Ok(())
    } else {
        Err("只支持 HTTP/HTTPS URL".to_string())
    }
}

fn spawn_system_open(target: &str, error_prefix: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut cmd = std::process::Command::new("explorer");
        cmd.arg(target);
        cmd
    };

    #[cfg(target_os = "macos")]
    let mut command = {
        let mut cmd = std::process::Command::new("open");
        cmd.arg(target);
        cmd
    };

    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = {
        let mut cmd = std::process::Command::new("xdg-open");
        cmd.arg(target);
        cmd
    };

    command
        .spawn()
        .map_err(|err| format!("{error_prefix}：{err}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_http_url_accepts_http_and_https() {
        assert!(validate_http_url("http://example.com/repo").is_ok());
        assert!(validate_http_url("https://example.com/repo").is_ok());
    }

    #[test]
    fn validate_http_url_rejects_non_browser_safe_targets() {
        assert_eq!(
            validate_http_url("ssh://git@example.com/repo.git").unwrap_err(),
            "只支持 HTTP/HTTPS URL"
        );
        assert_eq!(
            validate_http_url("file:///tmp/repo.git").unwrap_err(),
            "只支持 HTTP/HTTPS URL"
        );
        assert_eq!(validate_http_url("").unwrap_err(), "只支持 HTTP/HTTPS URL");
    }
}
