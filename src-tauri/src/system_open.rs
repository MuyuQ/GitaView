use std::path::Path;

pub(crate) fn open_directory(path: &Path) -> Result<(), String> {
    spawn_system_open(&path.to_string_lossy(), "无法打开目录")
}

pub(crate) fn open_http_url(url: &str) -> Result<(), String> {
    validate_http_url(url)?;
    spawn_system_open(url, "无法打开 URL")
}

fn validate_http_url(url: &str) -> Result<(), String> {
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err("只支持 HTTP/HTTPS URL".to_string());
    }
    // 拒绝控制字符与空白：`open`/`explorer` 可能吞掉或拆分参数，
    // 浏览器安全的远端 URL（见 remote.rs 规范化）不会包含这些字符
    if url.chars().any(|ch| (ch as u32) < 0x21 || ch == '\u{7f}') {
        return Err("URL 包含非法字符".to_string());
    }
    Ok(())
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

    let mut child = command
        .spawn()
        .map_err(|err| format!("{error_prefix}：{err}"))?;
    let status = child
        .wait()
        .map_err(|err| format!("{error_prefix}：{err}"))?;

    // Windows explorer 成功时也常返回非零退出码，不能当作失败依据；
    // macOS open / Linux xdg-open 的非零退出码代表真的失败了
    #[cfg(target_os = "windows")]
    let _ = status;
    #[cfg(not(target_os = "windows"))]
    if !status.success() {
        return Err(match status.code() {
            Some(code) => format!("{error_prefix}：进程退出码 {code}"),
            None => format!("{error_prefix}：进程被信号终止"),
        });
    }
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

    #[test]
    fn validate_http_url_rejects_control_characters_and_whitespace() {
        assert!(validate_http_url("https://example.com/a\nb").is_err());
        assert!(validate_http_url("https://example.com/a\tb").is_err());
        assert!(validate_http_url("https://example.com/a b").is_err());
        assert!(validate_http_url("https://example.com/\u{7f}").is_err());
        assert!(validate_http_url("https://example.com/ok_path?q=1").is_ok());
    }
}
