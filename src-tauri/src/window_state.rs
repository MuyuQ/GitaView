//! 桌面 widget 窗口位置的持久化。
//!
//! 规格 §8 "Window persistence"：用户拖好的 widget 位置在重启后恢复。
//! 前端在窗口移动（防抖）后经 `save_window_state` 命令写入；
//! 应用启动时在 setup 阶段恢复（先于前端首次帧同步，因此锚定逻辑会保留它）。

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use tauri::Manager;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowPosition {
    pub x: i32,
    pub y: i32,
}

pub fn window_state_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|err| err.to_string())?
        .join("window-state.json"))
}

/// 读取保存的位置；文件缺失或损坏返回 None（不隔离：位置数据可随时重写）
pub fn load_window_position(path: &Path) -> Option<WindowPosition> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return None,
        Err(err) => {
            crate::diagnostics::log(
                "window_state.load.error",
                format!("error_len={}", err.to_string().len()),
            );
            return None;
        }
    };
    match serde_json::from_slice::<WindowPosition>(&bytes) {
        Ok(position) => Some(position),
        Err(err) => {
            crate::diagnostics::log(
                "window_state.load.corrupt",
                format!("reason_len={}", err.to_string().len()),
            );
            None
        }
    }
}

pub fn save_window_position(path: &Path, position: &WindowPosition) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let text = serde_json::to_string(position).map_err(|err| err.to_string())?;
    let temp_path = path.with_extension("json.tmp");
    let write_result = fs::write(&temp_path, text).and_then(|()| fs::rename(&temp_path, path));
    if let Err(err) = write_result {
        let _ = fs::remove_file(&temp_path);
        return Err(err.to_string());
    }
    Ok(())
}

/// 显示器矩形（x, y, width, height，物理像素）
pub type MonitorBounds = (i32, i32, u32, u32);

fn bounds_contain(bounds: &MonitorBounds, position: &WindowPosition) -> bool {
    let (x, y, width, height) = *bounds;
    position.x >= x
        && position.y >= y
        && position.x < x + width as i32
        && position.y < y + height as i32
}

/// 位置落在任一显示器内则原样保留；否则钳制到主显示器（首个矩形）内。
/// 处理显示器拔除/分辨率变化后保存的位置越界。
pub fn clamp_position(position: WindowPosition, monitors: &[MonitorBounds]) -> WindowPosition {
    if monitors
        .iter()
        .any(|bounds| bounds_contain(bounds, &position))
    {
        return position;
    }
    let Some(&(mx, my, width, height)) = monitors.first() else {
        return position;
    };
    let margin = 40i32;
    let max_x = mx + width as i32 - margin;
    let max_y = my + height as i32 - margin;
    WindowPosition {
        x: position.x.clamp(mx, max_x.max(mx)),
        y: position.y.clamp(my, max_y.max(my)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_temp_dir(name: &str) -> PathBuf {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("{name}_{suffix}"))
    }

    #[test]
    fn missing_file_loads_as_none() {
        let path = unique_temp_dir("gitaview_wstate_missing").join("window-state.json");
        assert!(load_window_position(&path).is_none());
    }

    #[test]
    fn save_then_load_round_trips() {
        let temp = unique_temp_dir("gitaview_wstate_roundtrip");
        let path = temp.join("window-state.json");
        let position = WindowPosition { x: -320, y: 480 };

        save_window_position(&path, &position).expect("save should succeed");
        assert_eq!(load_window_position(&path), Some(position));
        assert!(!temp.join("window-state.json.tmp").exists());
        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn corrupt_file_loads_as_none_and_path_stays_writable() {
        let temp = unique_temp_dir("gitaview_wstate_corrupt");
        let path = temp.join("window-state.json");
        fs::create_dir_all(&temp).unwrap();
        fs::write(&path, "not json").unwrap();

        assert!(load_window_position(&path).is_none());

        save_window_position(&path, &WindowPosition { x: 10, y: 20 }).expect("resave should work");
        assert_eq!(
            load_window_position(&path),
            Some(WindowPosition { x: 10, y: 20 })
        );
        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn clamp_keeps_position_inside_any_monitor() {
        let monitors = vec![(0, 0, 1920, 1080), (-1440, 0, 1440, 900)];
        let position = WindowPosition { x: -1400, y: 100 };
        assert_eq!(clamp_position(position, &monitors), position);
    }

    #[test]
    fn clamp_pulls_offscreen_position_into_primary_monitor() {
        let monitors = vec![(0, 0, 1920, 1080)];
        // 显示器拔除后遗留的越界位置
        let position = WindowPosition { x: -3000, y: 4000 };
        let clamped = clamp_position(position, &monitors);
        assert!(clamped.x >= 0 && clamped.x < 1920);
        assert!(clamped.y >= 0 && clamped.y < 1080);
    }

    #[test]
    fn clamp_without_monitors_keeps_position() {
        let position = WindowPosition { x: 12, y: 34 };
        assert_eq!(clamp_position(position, &[]), position);
    }
}
