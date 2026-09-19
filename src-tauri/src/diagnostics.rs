use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

static LOG_PATH: OnceLock<PathBuf> = OnceLock::new();
static LOG_LOCK: Mutex<()> = Mutex::new(());
const MAX_LOG_BYTES: u64 = 512 * 1024;

pub fn init(path: PathBuf) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = LOG_PATH.set(path.clone());
    log(
        "diagnostics.init",
        format!("log_path={} pid={}", redact_path(&path), std::process::id()),
    );
}

pub fn log(event: &str, message: impl AsRef<str>) {
    let Some(path) = LOG_PATH.get() else {
        return;
    };
    append_line(path, event, message.as_ref());
}

pub fn log_duration(event: &str, duration: Duration, message: impl AsRef<str>) {
    log(
        event,
        format!("duration_ms={} {}", duration.as_millis(), message.as_ref()),
    );
}

pub fn log_window(event: &str, window: &tauri::WebviewWindow) {
    let position = window
        .outer_position()
        .map(|position| format!("{},{}", position.x, position.y))
        .unwrap_or_else(|err| format!("error:{err}"));
    let size = window
        .outer_size()
        .map(|size| format!("{}x{}", size.width, size.height))
        .unwrap_or_else(|err| format!("error:{err}"));
    let visible = window
        .is_visible()
        .map(|visible| visible.to_string())
        .unwrap_or_else(|err| format!("error:{err}"));

    log(
        event,
        format!(
            "label={} visible={} position={} size={}",
            window.label(),
            visible,
            position,
            size
        ),
    );
}

fn append_line(path: &Path, event: &str, message: &str) {
    let Ok(_guard) = LOG_LOCK.lock() else {
        return;
    };
    rotate_if_needed(path, MAX_LOG_BYTES);
    let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) else {
        return;
    };
    let _ = writeln!(
        file,
        "{} pid={} thread={:?} [{}] {}",
        timestamp_ms(),
        std::process::id(),
        std::thread::current().id(),
        event,
        redact_home_paths(&message.replace('\n', "\\n"))
    );
}

fn rotate_if_needed(path: &Path, max_bytes: u64) {
    let Ok(metadata) = fs::metadata(path) else {
        return;
    };
    if metadata.len() < max_bytes {
        return;
    }
    let backup_path = path.with_extension("log.1");
    let _ = fs::remove_file(&backup_path);
    let _ = fs::rename(path, backup_path);
}

pub fn redact_path(path: &Path) -> String {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("<root>");
    format!("<path>/{name}")
}

/// 深链只记 scheme+host（含路径形状但不带 query/frag）：
/// `gitaview://open/repo/<id>?from=tray` → `gitaview://open/repo/<id>`
pub fn redact_url(url: &str) -> String {
    let (without_fragment, _) = url
        .split_once('#')
        .map(|(head, _)| (head, ()))
        .unwrap_or((url, ()));
    match without_fragment.split_once('?') {
        Some((head, _)) => head.to_string(),
        None => without_fragment.to_string(),
    }
}

/// 中心化兜底：append 前对消息里的用户主目录路径模式统一打码。
/// 覆盖 `/Users/<name>/…`、`/home/<name>/…` 与 `X:\Users\<name>\…`
/// （含正斜杠变体），防止个别调用点漏走 redact_path 泄漏用户名。
pub fn redact_home_paths(message: &str) -> String {
    let mut result = String::with_capacity(message.len());
    let mut rest = message;
    while !rest.is_empty() {
        if let Some((replacement, consumed)) = match_home_prefix(rest) {
            result.push_str(&replacement);
            rest = &rest[consumed..];
        } else {
            let ch = rest.chars().next().unwrap_or('\0');
            result.push(ch);
            rest = &rest[ch.len_utf8()..];
        }
    }
    result
}

fn match_home_prefix(rest: &str) -> Option<(String, usize)> {
    for prefix in ["/Users/", "/home/"] {
        if let Some(after) = rest.strip_prefix(prefix) {
            let segment = username_segment(after);
            if segment > 0 {
                return Some((format!("{prefix}<user>"), prefix.len() + segment));
            }
            return None;
        }
    }
    // Windows：`X:\Users\name` 或 `X:/Users/name`
    let bytes = rest.as_bytes();
    if bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'\\' || bytes[2] == b'/')
    {
        for sep in ['\\', '/'] {
            let users_prefix = format!("Users{sep}");
            if let Some(after) = rest[3..].strip_prefix(users_prefix.as_str()) {
                let segment = username_segment(after);
                if segment > 0 {
                    return Some((
                        format!("{}{users_prefix}<user>", &rest[..3]),
                        3 + users_prefix.len() + segment,
                    ));
                }
            }
        }
    }
    None
}

/// 用户名段在任一路径分隔符/引号/空白处结束；全为 ASCII 分隔符，字节索引安全
fn username_segment(after: &str) -> usize {
    after
        .find(['/', '\\', '"', '\'', ' ', '\n', '\t'])
        .unwrap_or(after.len())
}

fn timestamp_ms() -> String {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => format!("{}.{:03}", duration.as_secs(), duration.subsec_millis()),
        Err(_) => "time_error".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotates_log_before_appending_past_limit() {
        let root = std::env::temp_dir().join("gitaview_diagnostics_rotation");
        let path = root.join("gitaview.log");
        let backup = root.join("gitaview.log.1");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(&path, "0123456789").unwrap();

        rotate_if_needed(&path, 5);

        assert!(!path.exists());
        assert_eq!(fs::read_to_string(&backup).unwrap(), "0123456789");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn redacts_filesystem_paths_from_diagnostic_messages() {
        assert_eq!(
            redact_path(Path::new("C:/Users/Muyu/projects/GitaView")),
            "<path>/GitaView"
        );
        assert_eq!(
            redact_path(Path::new("/Users/muyu/projects")),
            "<path>/projects"
        );
    }

    #[test]
    fn redacts_url_queries_and_fragments() {
        assert_eq!(
            redact_url("gitaview://open/repo/abc?token=secret#frag"),
            "gitaview://open/repo/abc"
        );
        assert_eq!(redact_url("gitaview://open"), "gitaview://open");
        assert_eq!(redact_url("no-scheme"), "no-scheme");
    }

    #[test]
    fn redacts_user_home_paths_in_free_form_messages() {
        // 只打码用户名段，保留路径形状便于排障
        assert_eq!(
            redact_home_paths("cwd=/Users/alice/projects/demo command=git"),
            "cwd=/Users/<user>/projects/demo command=git"
        );
        assert_eq!(
            redact_home_paths("home /home/bob/x and C:\\Users\\carol\\repo ok"),
            "home /home/<user>/x and C:\\Users\\<user>\\repo ok"
        );
        assert_eq!(
            redact_home_paths("forward D:/Users/dave/log.txt"),
            "forward D:/Users/<user>/log.txt"
        );
        // 无用户名的 /Users/（如共享目录）不打码
        assert_eq!(redact_home_paths("/Users/"), "/Users/");
        // 普通消息原样通过
        assert_eq!(redact_home_paths("no paths here"), "no paths here");
    }
}
