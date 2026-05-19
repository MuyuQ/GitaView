use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

static LOG_PATH: OnceLock<PathBuf> = OnceLock::new();
static LOG_LOCK: Mutex<()> = Mutex::new(());

pub fn init(path: PathBuf) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = LOG_PATH.set(path.clone());
    log(
        "diagnostics.init",
        format!("log_path={} pid={}", path.display(), std::process::id()),
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
        message.replace('\n', "\\n")
    );
}

fn timestamp_ms() -> String {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => format!("{}.{:03}", duration.as_secs(), duration.subsec_millis()),
        Err(_) => "time_error".to_string(),
    }
}
