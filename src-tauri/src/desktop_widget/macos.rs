//! macOS 平台桌面 widget 层级实现
//!
//! 通过设置 NSWindow 的 level 和 collection behavior，
//! 实现桌面 widget 行为：
//! - 窗口层级位于桌面图标之上
//! - 位于普通应用窗口之下
//! - 在所有 Space 中可见
//! - Mission Control 中不作为普通窗口处理

use tauri::{Manager, WebviewWindow};

/// 应用桌面 widget 层级到指定窗口 (macOS)
///
/// 实现步骤：
/// 1. 通过 `with_webview` 获取平台原生 webview
/// 2. 从 webview 获取 `ns_window` 并转换为 `NSWindow`
/// 3. 设置窗口层级为 `kCGDesktopIconWindowLevel + 1`
/// 4. 设置 collection behavior：
///    - `CanJoinAllSpaces`: 在所有 Space 中可见
///    - `Stationary`: 在 Mission Control 中不移动
///    - `IgnoresCycle`: 不参与窗口切换 (Cmd+`)
#[cfg(target_os = "macos")]
pub fn apply_desktop_widget_layer(window: &WebviewWindow) -> Result<(), String> {
    use objc2_app_kit::{NSWindow, NSWindowCollectionBehavior};
    use objc2_core_graphics::kCGDesktopIconWindowLevel;

    // 使用 with_webview 访问平台原生 webview
    window
        .with_webview(|webview| {
            // 获取 NSWindow 指针并转换为 NSWindow 引用
            // ns_window() 返回 *mut c_void，需要转换为 NSWindow
            let ns_window_ptr = webview.ns_window();
            let ns_window: &NSWindow = unsafe { &*ns_window_ptr.cast() };

            // 设置窗口层级：桌面图标层 + 1
            // 这使窗口位于桌面图标之上，但在普通应用窗口之下
            let desktop_icon_level = kCGDesktopIconWindowLevel;
            ns_window.setLevel(desktop_icon_level + 1);

            // 设置 collection behavior
            // - CanJoinAllSpaces: 窗口在所有 Space 中可见
            // - Stationary: 窗口在 Mission Control 中保持静止，不会被重新排列
            // - IgnoresCycle: 窗口不参与 Cmd+` 窗口切换循环
            let behavior = NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::Stationary
                | NSWindowCollectionBehavior::IgnoresCycle;
            ns_window.setCollectionBehavior(behavior);
        })
        .map_err(|e| format!("无法访问 webview: {}", e))?;

    Ok(())
}

/// 重新应用桌面 widget 层级到主窗口 (macOS)
///
/// 查找名为 "main" 的窗口并重新应用桌面层级行为。
pub fn reapply_desktop_widget_layer(app: &tauri::AppHandle) -> Result<(), String> {
    // 查找名为 "main" 的窗口
    let Some(window) = app.get_webview_window("main") else {
        return Err("未找到 main 窗口".to_string());
    };

    // 应用桌面层级
    apply_desktop_widget_layer(&window)
}