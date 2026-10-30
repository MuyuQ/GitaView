//! JSON 文件存取：原子替换写 + 损坏隔离。
//! 临时文件 + MoveFileEx(REPLACE_EXISTING|WRITE_THROUGH)（Windows）/ rename（Unix）；
//! 损坏文件 rename 到 `.corrupt-<时间戳>` 留档，隔离失败保留现场并报错。

use crate::error::CoreError;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// 读取结果：文件不存在不是错误（首次启动）。
pub enum LoadOutcome {
    Missing,
    Present(Vec<u8>),
}

/// 读取原始字节，让设置解码统一处理编码和 JSON 损坏。
/// IO 错误（权限、被锁定、路径是目录等）→ SETTINGS_IO，不与文件不存在混淆。
pub fn load_json(path: &Path) -> Result<LoadOutcome, CoreError> {
    match fs::read(path) {
        Ok(bytes) => Ok(LoadOutcome::Present(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(LoadOutcome::Missing),
        Err(_) => Err(CoreError::SettingsIo),
    }
}

/// 隔离损坏文件：rename 到 `<name>.corrupt-<unix 秒>`，保留现场供手动恢复。
/// rename 失败（占用 / 权限）→ SETTINGS_IO，不静默丢弃用户数据。
pub fn quarantine(path: &Path) -> Result<PathBuf, CoreError> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let backup = path.with_extension(format!("corrupt-{timestamp}"));
    fs::rename(path, &backup).map_err(|_| CoreError::SettingsIo)?;
    Ok(backup)
}

/// 原子写：临时文件 → 替换。目录不存在则先创建（失败 → SETTINGS_IO）。
pub fn save_json_atomic(path: &Path, text: &str) -> Result<(), CoreError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|_| CoreError::SettingsIo)?;
    }
    let temp_path = temp_path(path);
    let _ = fs::remove_file(&temp_path);
    fs::write(&temp_path, text).map_err(|_| CoreError::SettingsIo)?;
    let result = replace_file(&temp_path, path);
    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    result
}

fn temp_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("settings.json");
    path.with_file_name(format!(".{file_name}.tmp"))
}

#[cfg(target_os = "windows")]
fn replace_file(source: &Path, destination: &Path) -> Result<(), CoreError> {
    use windows::core::HSTRING;
    use windows::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };

    let source = HSTRING::from(source.to_string_lossy().as_ref());
    let destination = HSTRING::from(destination.to_string_lossy().as_ref());
    unsafe {
        MoveFileExW(
            &source,
            &destination,
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    }
    .map_err(|_| CoreError::SettingsIo)
}

#[cfg(not(target_os = "windows"))]
fn replace_file(source: &Path, destination: &Path) -> Result<(), CoreError> {
    fs::rename(source, destination).map_err(|_| CoreError::SettingsIo)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("{name}_{suffix}"))
    }

    #[test]
    fn missing_file_is_not_an_error() {
        let temp = unique_temp_dir("gitaview_core_store_missing");
        fs::create_dir_all(&temp).unwrap();
        assert!(matches!(
            load_json(&temp.join("settings.json")),
            Ok(LoadOutcome::Missing)
        ));
        fs::remove_dir_all(temp).unwrap();
    }

    #[test]
    fn unreadable_path_maps_to_settings_io() {
        // 路径是目录：读取必然失败，且不是 NotFound。
        let temp = unique_temp_dir("gitaview_core_store_dir");
        fs::create_dir_all(&temp).unwrap();
        assert!(
            matches!(load_json(&temp), Err(CoreError::SettingsIo)),
            "reading a directory must be an IO failure, not missing"
        );
    }

    #[test]
    fn corrupt_file_is_quarantined_and_preserved() {
        let temp = unique_temp_dir("gitaview_core_store_quarantine");
        let path = temp.join("settings.json");
        fs::create_dir_all(&temp).unwrap();
        fs::write(&path, "{ not json").unwrap();

        let backup = quarantine(&path).unwrap();

        assert!(!path.exists(), "corrupt file moved away");
        assert!(
            backup.exists(),
            "quarantine keeps the original for recovery"
        );
        fs::remove_dir_all(temp).unwrap();
    }

    #[test]
    fn atomic_save_leaves_no_temp_file_and_replaces_existing() {
        let temp = unique_temp_dir("gitaview_core_store_atomic");
        let path = temp.join("settings.json");
        fs::create_dir_all(&temp).unwrap();

        save_json_atomic(&path, "{\"a\":1}").unwrap();
        save_json_atomic(&path, "{\"a\":2}").unwrap();

        assert_eq!(fs::read_to_string(&path).unwrap(), "{\"a\":2}");
        assert!(
            !temp.join(".settings.json.tmp").exists(),
            "temp file must not survive atomic replace"
        );
        fs::remove_dir_all(temp).unwrap();
    }
}
