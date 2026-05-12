//! Desktop Widget Layer 模块
//!
//! 为 GitaView 提供原生桌面层级窗口行为：
//! - Windows: 将窗口附着到桌面图标宿主窗口，位于桌面图标之上但在普通应用窗口之下
//! - macOS: 设置 NSWindow level 为桌面图标层级之上，配置 Space/Mission Control 行为
//! - 其他平台: 不支持，保持普通窗口行为
//!
//! 调用必须是幂等的。

#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "macos")]
mod macos;

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
mod unsupported;

/// 应用桌面 widget 层级到指定窗口
///
/// 此函数将原生桌面层级行为应用到指定的 WebviewWindow。
/// 调用是幂等的 — 重复调用不会产生副作用。
///
/// # 参数
/// - `window`: Tauri 的 WebviewWindow 引用
///
/// # 返回
/// - `Ok(())`: 成功应用桌面层级，或不支持平台上返回成功
/// - `Err(String)`: 应用失败时的错误信息
#[cfg(target_os = "windows")]
pub fn apply_desktop_widget_layer(window: &tauri::WebviewWindow) -> Result<(), String> {
    windows::apply_desktop_widget_layer(window)
}

#[cfg(target_os = "macos")]
pub fn apply_desktop_widget_layer(window: &tauri::WebviewWindow) -> Result<(), String> {
    macos::apply_desktop_widget_layer(window)
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn apply_desktop_widget_layer(window: &tauri::WebviewWindow) -> Result<(), String> {
    unsupported::apply_desktop_widget_layer(window)
}

/// 重新应用桌面 widget 层级到主窗口
///
/// 查找名为 "main" 的窗口并重新应用桌面层级行为。
/// 适用于 Explorer 重启后或窗口重建后恢复桌面层级。
/// 调用是幂等的。
///
/// # 参数
/// - `app`: Tauri 的 AppHandle 引用
///
/// # 返回
/// - `Ok(())`: 成功重新应用桌面层级
/// - `Err(String)`: 未找到窗口或应用失败时的错误信息
#[cfg(target_os = "windows")]
pub fn reapply_desktop_widget_layer(app: &tauri::AppHandle) -> Result<(), String> {
    windows::reapply_desktop_widget_layer(app)
}

#[cfg(target_os = "macos")]
pub fn reapply_desktop_widget_layer(app: &tauri::AppHandle) -> Result<(), String> {
    macos::reapply_desktop_widget_layer(app)
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn reapply_desktop_widget_layer(app: &tauri::AppHandle) -> Result<(), String> {
    unsupported::reapply_desktop_widget_layer(app)
}