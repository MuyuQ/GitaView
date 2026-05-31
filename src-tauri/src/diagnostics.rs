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
        message.replace('\n', "\\n")
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
}
