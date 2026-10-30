//! 诊断日志（补 N05/N07 遗留，整理自旧 crate diagnostics 的职责）。
//!
//! 规则：
//! - 引擎创建时以宿主注入的 logDirectory 初始化（进程内首个引擎生效，后续忽略）；
//! - 轮转：单文件超过 5MB 改名为 .1（保留 2 代）；
//! - **脱敏**：不输出完整仓库路径（用仓库 ID / 占位符）、不输出 origin 完整 URL
//!   （仅 scheme://host）、不输出原始 git stderr 全文（首行 + 路径占位）；
//! - 时间为 UTC ISO-8601（无依赖手写转换）；
//! - 有界队列后台写入：队列满或写入失败时丢弃诊断，绝不反向阻塞业务；
//! - **不得在持有引擎状态锁时写日志**（文件 IO 会放大锁争用——卡顿教训），
//!   调用方先取数据、释放锁、再记录。

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{sync_channel, SyncSender, TrySendError};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const MAX_LOG_BYTES: u64 = 5 * 1024 * 1024;
const ROTATION_KEEP: u32 = 2;

struct Logger {
    file_path: PathBuf,
}

static LOGGER: OnceLock<Logger> = OnceLock::new();
static WRITE_LOCK: Mutex<()> = Mutex::new(());
const QUEUE_CAPACITY: usize = 256;
enum LogMessage {
    Line(String),
    Flush(SyncSender<()>),
}
static LOG_QUEUE: OnceLock<Option<SyncSender<LogMessage>>> = OnceLock::new();

#[cfg(test)]
type BeforeLogHook = Box<dyn Fn(&str)>;
#[cfg(test)]
thread_local! {
    static BEFORE_LOG: std::cell::RefCell<Option<BeforeLogHook>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub(crate) fn before_log_for_test(hook: impl Fn(&str) + 'static) {
    BEFORE_LOG.with(|slot| *slot.borrow_mut() = Some(Box::new(hook)));
}

#[cfg(test)]
pub(crate) fn block_sink_for_test() -> std::sync::MutexGuard<'static, ()> {
    WRITE_LOCK.lock().unwrap_or_else(|error| error.into_inner())
}

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

/// 等待此前入队的诊断完成，超时返回 false；不暂停或关闭日志器。
/// 仅用于后台/测试收尾，不得从 UI 线程或持有业务锁时调用。
pub fn flush(timeout: Duration) -> bool {
    let Some(Some(queue)) = LOG_QUEUE.get() else {
        return true;
    };
    flush_queue(queue, timeout)
}

fn flush_queue(queue: &SyncSender<LogMessage>, timeout: Duration) -> bool {
    let started = Instant::now();
    let (sender, receiver) = sync_channel(1);
    let mut message = LogMessage::Flush(sender);
    loop {
        match queue.try_send(message) {
            Ok(()) => {
                return receiver
                    .recv_timeout(timeout.saturating_sub(started.elapsed()))
                    .is_ok()
            }
            Err(TrySendError::Disconnected(_)) => return false,
            Err(TrySendError::Full(returned)) => {
                message = returned;
                let remaining = timeout.saturating_sub(started.elapsed());
                if remaining.is_zero() {
                    return false;
                }
                std::thread::sleep(remaining.min(Duration::from_millis(1)));
            }
        }
    }
}

fn write_line(level: &str, event: &str, detail: &str) {
    #[cfg(test)]
    BEFORE_LOG.with(|slot| {
        if let Some(hook) = slot.borrow().as_ref() {
            hook(event);
        }
    });
    let Some(logger) = LOGGER.get() else {
        return;
    };
    let queue = LOG_QUEUE.get_or_init(|| {
        spawn_log_worker(Logger {
            file_path: logger.file_path.clone(),
        })
        .map(|(sender, _worker)| sender)
    });
    if let Some(queue) = queue {
        // Never wait for disk, the file lock, or room in the diagnostic queue.
        let _ = queue.try_send(LogMessage::Line(format_line(level, event, detail)));
    }
}

fn spawn_log_worker(
    logger: Logger,
) -> Option<(SyncSender<LogMessage>, std::thread::JoinHandle<()>)> {
    let (sender, receiver) = sync_channel::<LogMessage>(QUEUE_CAPACITY);
    let worker = std::thread::Builder::new()
        .name("gitaview-diagnostics".into())
        .spawn(move || {
            for message in receiver {
                match message {
                    LogMessage::Line(line) => append_line(&logger, &line),
                    LogMessage::Flush(completion) => {
                        let _ = completion.try_send(());
                    }
                }
            }
        })
        .ok()?;
    Some((sender, worker))
}

#[cfg(test)]
fn write_line_to(logger: &Logger, level: &str, event: &str, detail: &str) {
    append_line(logger, &format_line(level, event, detail));
}

fn format_line(level: &str, event: &str, detail: &str) -> String {
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    format!("{} {} {} {}\n", utc_string(now_ms), level, event, detail)
}

fn append_line(logger: &Logger, line: &str) {
    // 仅后台写入器等待文件锁，调用方不执行磁盘操作。
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
    fn flush_deadline_does_not_wait_forever_on_a_blocked_sink() {
        let dir = unique_temp_dir();
        fs::create_dir_all(&dir).unwrap();
        let (queue, worker) = spawn_log_worker(Logger {
            file_path: dir.join("gitaview-core.log"),
        })
        .unwrap();
        let sink = block_sink_for_test();
        queue
            .try_send(LogMessage::Line("queued before the flush barrier\n".into()))
            .unwrap();
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let flush_sender = queue.clone();
        let flushing = std::thread::spawn(move || {
            done_tx
                .send(flush_queue(&flush_sender, Duration::from_millis(20)))
                .unwrap();
        });
        let timed_out = done_rx.recv_timeout(Duration::from_secs(1)).ok();
        drop(sink);
        flushing.join().unwrap();
        assert!(flush_queue(&queue, Duration::from_secs(5)));
        queue
            .try_send(LogMessage::Line("queued after the flush timeout\n".into()))
            .unwrap();
        drop(queue);
        worker.join().unwrap();
        let contents = fs::read_to_string(dir.join("gitaview-core.log")).unwrap();
        assert!(
            contents == "queued before the flush barrier\nqueued after the flush timeout\n",
            "the isolated log must contain exactly the two queued lines"
        );
        fs::remove_dir_all(dir).unwrap();
        assert_eq!(timed_out, Some(false));
    }

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
        fs::create_dir_all(&dir).unwrap();
        // Engine tests may initialize LOGGER first. Exercise the production
        // write/rotation path with a logger owned solely by this test.
        let logger = std::sync::Arc::new(Logger {
            file_path: dir.join("gitaview-core.log"),
        });
        for index in 0..40 {
            write_line_to(
                &logger,
                "INFO",
                "test.event",
                &format!("entry-{index} x{}", "y".repeat(200_000)),
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
            .map(|_| {
                let logger = logger.clone();
                std::thread::spawn(move || write_line_to(&logger, "INFO", "test.concurrent", "x"))
            })
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
        temp_dir_at(suffix)
    }

    fn temp_dir_at(timestamp: u128) -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        let sequence = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "gitaview_diag_{}_{timestamp}_{sequence}",
            std::process::id()
        ))
    }

    #[test]
    fn parallel_log_tests_get_distinct_directories_at_the_same_timestamp() {
        let workers: Vec<_> = (0..8)
            .map(|_| std::thread::spawn(|| (0..100).map(|_| temp_dir_at(42)).collect::<Vec<_>>()))
            .collect();
        let paths: std::collections::HashSet<_> = workers
            .into_iter()
            .flat_map(|worker| worker.join().unwrap())
            .collect();
        assert_eq!(paths.len(), 800);
    }
}
