//! 诊断日志（补 N05/N07 遗留，整理自旧 crate diagnostics 的职责）。
//!
//! 规则：
//! - 引擎创建时以宿主注入的 logDirectory 初始化（进程内首个引擎生效，后续忽略）；
//! - 轮转：单文件超过 5MB 改名为 .1（保留 2 代）；
//! - **脱敏**：不输出完整仓库路径（用仓库 ID / 占位符）、不输出 origin 完整 URL
//!   （仅 scheme://host）、不输出原始 git stderr 全文（首行 + 路径占位）；
//! - 时间为 UTC ISO-8601（无依赖手写转换）；
//! - 日志写入绝不 panic：文件错误静默忽略（诊断不能反向影响业务）；
//! - **不得在持有引擎状态锁时写日志**（文件 IO 会放大锁争用——卡顿教训），
//!   调用方先取数据、释放锁、再记录。

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const MAX_LOG_BYTES: u64 = 5 * 1024 * 1024;
const ROTATION_KEEP: u32 = 2;

struct Logger {
    file_path: PathBuf,
}

static LOGGER: OnceLock<Logger> = OnceLock::new();
static WRITE_LOCK: Mutex<()> = Mutex::new(());

/// 以日志目录初始化进程级日志器。首个调用生效；写入失败静默。
pub fn init(dir: &Path) {
    let _ = fs::create_dir_all(dir);
    let _ = LOGGER.set(Logger {
        file_path: dir.join("gitaview-core.log"),
    });
}

pub fn log(event: &str, detail: impl AsRef<str>) {
    write_line("INFO", event, detail.as_ref());
}

pub fn warn(event: &str, detail: impl AsRef<str>) {
    write_line("WARN", event, detail.as_ref());
}

pub fn error(event: &str, detail: impl AsRef<str>) {
    write_line("ERROR", event, detail.as_ref());
}

pub fn log_duration(event: &str, duration: Duration, detail: impl AsRef<str>) {
    write_line(
        "INFO",
        event,
        &format!("duration_ms={} {}", duration.as_millis(), detail.as_ref()),
    );
}

fn write_line(level: &str, event: &str, detail: &str) {
    let Some(logger) = LOGGER.get() else {
        return;
    };
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let line = format!("{} {} {} {}\n", utc_string(now_ms), level, event, detail);
    // 写入 + 轮转全程串行；单条日志很小，文件 IO 在专有序列化下开销可忽略。
    let guard = WRITE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    rotate_if_needed(&logger.file_path);
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&logger.file_path)
    {
        let _ = file.write_all(line.as_bytes());
    }
    drop(guard);
}

fn rotate_if_needed(path: &Path) {
    let Ok(metadata) = fs::metadata(path) else {
        return;
    };
    if metadata.len() < MAX_LOG_BYTES {
        return;
    }
    for generation in (1..ROTATION_KEEP).rev() {
        let from = path.with_extension(format!("log.{}", generation));
        let to = path.with_extension(format!("log.{}", generation + 1));
        let _ = fs::rename(&from, &to);
    }
    let _ = fs::rename(path, path.with_extension("log.1"));
}

/// Unix 毫秒 → "YYYY-MM-DDTHH:MM:SS.mmmZ"（无依赖 UTC 转换，Howard Hinnant 算法）。
fn utc_string(unix_ms: u64) -> String {
    let seconds = unix_ms / 1000;
    let millis = unix_ms % 1000;
    let days = seconds / 86_400;
    let secs_of_day = seconds % 86_400;
    let (year, month, day) = civil_from_days(days as i64);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{millis:03}Z",
        secs_of_day / 3600,
        (secs_of_day % 3600) / 60,
        secs_of_day % 60
    )
}

fn civil_from_days(days_since_epoch: i64) -> (i64, u64, u64) {
    let z = days_since_epoch + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u64;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u64;
    (if month <= 2 { year + 1 } else { year }, month, day)
}

// —— 脱敏 ——

/// origin URL → scheme://host[:port]；userinfo / path / query 全部去除；
/// 非 http(s)（本地路径、ssh 等）→ "<opaque-url>"。
pub fn redact_url(raw: &str) -> String {
    let trimmed = raw.trim();
    let (scheme, rest) = match trimmed.split_once("://") {
        Some(parts) => parts,
        None => return "<opaque-url>".to_string(),
    };
    if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
        return "<opaque-url>".to_string();
    }
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let authority = match authority.rsplit_once('@') {
        Some((_userinfo, host)) => host,
        None => authority,
    };
    format!("{}://{}", scheme.to_ascii_lowercase(), authority)
}

/// 消息脱敏：仓库绝对路径 → <repo>；用户目录前缀 → ~；http(s) URL → 脱敏 URL。
/// 用于 git stderr 首行等诊断细节，保证日志可分享而不泄露本地布局。
pub fn redact_message(message: &str, repo_path: Option<&Path>) -> String {
    let mut result = message.to_string();
    if let Some(path) = repo_path {
        let as_str = path.to_string_lossy();
        if !as_str.is_empty() {
            result = result.replace(as_str.as_ref(), "<repo>");
        }
    }
    result = redact_user_profile(&result, "C:\\Users\\");
    result = redact_user_profile(&result, "/Users/");
    result = redact_user_profile(&result, "/home/");
    redact_urls_in_place(&mut result);
    result
}

fn redact_user_profile(text: &str, prefix: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find(prefix) {
        result.push_str(&rest[..start]);
        result.push('~');
        let after = &rest[start + prefix.len()..];
        let end = after
            .find(['\\', '/', ' ', '"', ':'])
            .unwrap_or(after.len());
        result.push('/');
        // 被消费的原分隔符（/ 或 \）由统一 '/' 替代。
        rest = if end < after.len() {
            &after[end + 1..]
        } else {
            ""
        };
    }
    result.push_str(rest);
    result
}

fn redact_urls_in_place(text: &mut String) {
    for keyword in ["http://", "https://"] {
        let mut search_from = 0;
        while let Some(found) = text[search_from..].find(keyword) {
            let start = search_from + found;
            let rest = &text[start..];
            let end = rest.find([' ', '"', ')']).unwrap_or(rest.len());
            let redacted = redact_url(&rest[..end]);
            text.replace_range(start..start + end, &redacted);
            search_from = start + redacted.len();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utc_string_formats_iso8601() {
        // 2026-10-24T00:00:00Z（Python datetime 校准）
        assert_eq!(utc_string(1_792_800_000_000), "2026-10-24T00:00:00.000Z");
        assert_eq!(utc_string(0), "1970-01-01T00:00:00.000Z");
    }

    #[test]
    fn redact_url_keeps_host_only() {
        assert_eq!(
            redact_url("https://user:token@github.com/owner/repo.git?x=1"),
            "https://github.com"
        );
        assert_eq!(
            redact_url("HTTPS://GitHub.com/owner/repo"),
            "https://GitHub.com"
        );
        assert_eq!(redact_url("ssh://git@host/repo"), "<opaque-url>");
        assert_eq!(redact_url("file:///tmp/repo"), "<opaque-url>");
        assert_eq!(redact_url(""), "<opaque-url>");
    }

    #[test]
    fn redact_message_masks_paths_and_urls() {
        let repo = Path::new("E:\\Git_Repositories\\GitaView");
        let message = "fatal: E:\\Git_Repositories\\GitaView\\.git failed, see https://user:pw@gitlab.com/x/y";
        let redacted = redact_message(message, Some(repo));
        assert!(!redacted.contains("Git_Repositories"));
        assert!(!redacted.contains("user:pw"));
        assert!(redacted.contains("<repo>"));
        assert!(redacted.contains("https://gitlab.com"));
    }

    #[test]
    fn redact_user_profile_masks_home_prefix() {
        assert_eq!(
            redact_message("config at C:\\Users\\Muyu\\app.ini", None),
            "config at ~/app.ini"
        );
        assert_eq!(redact_message("at /Users/me/x", None), "at ~/x");
        assert_eq!(redact_message("at /home/u/x", None), "at ~/x");
    }

    #[test]
    fn log_writes_and_rotates() {
        let dir = unique_temp_dir();
        init(&dir);
        for index in 0..40 {
            log(
                "test.event",
                format!("entry-{index} x{}", "y".repeat(200_000)),
            );
        }
        let main = fs::metadata(dir.join("gitaview-core.log")).unwrap();
        assert!(
            main.len() <= MAX_LOG_BYTES + 250_000,
            "rotation must cap size"
        );
        assert!(
            dir.join("gitaview-core.log.1").exists(),
            "rotated file kept"
        );
        // 并发写入不 panic。
        let handles: Vec<_> = (0..4)
            .map(|_| std::thread::spawn(|| log("test.concurrent", "x")))
            .collect();
        for handle in handles {
            handle.join().unwrap();
        }
        fs::remove_dir_all(dir).ok();
    }

    fn unique_temp_dir() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("gitaview_diag_{suffix}"))
    }
}
