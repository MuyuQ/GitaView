//! 不支持的平台实现
//!
//! 对于非 Windows/macOS 平台（如 Linux），
//! 桌面 widget 层级不被支持，返回 Ok(()) 保持普通窗口行为。

use tauri::{Manager, WebviewWindow};

/// 应用桌面 widget 层级到指定窗口 (不支持的平台)
///
/// 记录不支持平台的消息，返回 Ok(())。
/// 保持普通窗口行为不变。
pub fn apply_desktop_widget_layer(_window: &WebviewWindow) -> Result<(), String> {
    // 当前平台不支持桌面 widget 层级，保持普通窗口行为
    eprintln!("桌面 widget 层级在当前平台不受支持，将使用普通窗口行为");
    Ok(())
}

/// 重新应用桌面 widget 层级到主窗口 (不支持的平台)
///
/// 记录不支持平台的消息，返回 Ok(())。
pub fn reapply_desktop_widget_layer(app: &tauri::AppHandle) -> Result<(), String> {
    // 验证窗口是否存在（保持与其他平台一致的错误处理）
    let Some(_window) = app.get_webview_window("main") else {
        return Err("未找到 main 窗口".to_string());
    };

    // 当前平台不支持桌面 widget 层级
    eprintln!("桌面 widget 层级在当前平台不受支持，将使用普通窗口行为");
    Ok(())
}
