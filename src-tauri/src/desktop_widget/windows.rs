//! Windows 平台桌面 widget 层级实现
//!
//! 将 GitaView 窗口附着到桌面图标宿主窗口（Progman 或 WorkerW），
//! 实现真正的桌面 widget 行为：
//! - 位于桌面图标之上
//! - 位于普通应用窗口之下
//! - "显示桌面" 操作不会隐藏 widget
//!
//! 实现原理：
//! 1. Windows 桌面图标由 Explorer 托管在特殊的窗口层级中
//! 2. 默认情况下桌面图标窗口是 Progman 的子窗口
//! 3. 当有壁纸切换等功能时，Explorer 会创建 WorkerW 窗口
//! 4. 我们需要找到包含 SHELLDLL_DefView 的窗口作为桌面图标宿主
//! 5. 将 GitaView 作为该宿主的子窗口，即可实现桌面 widget 行为

use std::cell::Cell;
use std::time::Duration;
use tauri::{Manager, WebviewWindow};
use windows::core::{w, BOOL, PCWSTR};
use windows::Win32::Foundation::{GetLastError, HWND, LPARAM, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::ScreenToClient;
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, FindWindowW, GetParent, GetWindow, GetWindowLongPtrW, IsWindow,
    SendMessageTimeoutW, SetParent, SetWindowLongPtrW, SetWindowPos, ShowWindowAsync, GWL_EXSTYLE,
    GWL_STYLE, GW_CHILD, GW_HWNDNEXT, HWND_TOP, SMTO_ABORTIFHUNG, SWP_NOACTIVATE, SWP_NOMOVE,
    SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW, SW_SHOW, WS_CHILD, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW,
    WS_POPUP, WS_VISIBLE,
};

/// 消息码：通知 Progman 创建 WorkerW 窗口
const WM_CREATE_DESKTOP_WORKER: u32 = 0x052C;
const WATCHDOG_INTERVAL: Duration = Duration::from_secs(5);

/// 应用桌面 widget 层级到指定窗口 (Windows)
///
/// 实现步骤：
/// 1. 获取 GitaView 窗口的 HWND
/// 2. 查找 Progman 窗口
/// 3. 发送消息请求 Explorer 创建/刷新 WorkerW
/// 4. 枚举顶层窗口，找到包含 SHELLDLL_DefView 的窗口
/// 5. 将 GitaView 重设父窗口到桌面图标宿主
/// 6. 调整窗口样式（添加 child/visible，移除 popup）
/// 7. 设置 z-order 为顶层（在桌面图标之上）
///
/// 此函数是幂等的，重复调用不会产生副作用。
pub fn apply_desktop_widget_layer(window: &WebviewWindow) -> Result<(), String> {
    // 步骤 1：获取 GitaView 窗口的 HWND
    let hwnd = window
        .hwnd()
        .map_err(|e| format!("获取窗口句柄失败: {}", e))?;
    let gitaview_hwnd = hwnd;

    // 步骤 2：查找 Progman 窗口
    let progman = unsafe { FindWindowW(w!("Progman"), PCWSTR::null()) }
        .map_err(|e| format!("无法找到 Progman 窗口: {}", e))?;

    // 步骤 3：发送消息请求 Explorer 创建/刷新 WorkerW
    // 这会导致 Explorer 创建 WorkerW 窗口（如果尚未存在）
    let mut _result: usize = 0;
    let _ = unsafe {
        SendMessageTimeoutW(
            progman,
            WM_CREATE_DESKTOP_WORKER,
            WPARAM(0),
            LPARAM(0),
            SMTO_ABORTIFHUNG,
            1000, // 超时 1 秒
            Some(&mut _result),
        )
    };

    // 步骤 4：定位桌面图标宿主窗口
    // 枚举所有顶层窗口，找到包含 SHELLDLL_DefView 的窗口
    let desktop_host = find_desktop_icon_host(progman)?;
    if desktop_host.is_invalid() {
        return Err("无法找到桌面图标宿主窗口".to_string());
    }

    // 步骤 5：先调整样式，再重设父窗口
    // Win32 要求调用 SetParent 前由调用方切换 WS_CHILD / WS_POPUP。
    adjust_window_styles(gitaview_hwnd)?;

    let _previous_parent = unsafe { SetParent(gitaview_hwnd, Some(desktop_host)) }
        .map_err(|e| format!("SetParent 失败: {}", e))?;

    // 步骤 6：设置 z-order
    // 将窗口放在桌面图标宿主的子窗口顶部
    unsafe {
        SetWindowPos(
            gitaview_hwnd,
            Some(HWND_TOP),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
        )
    }
    .map_err(|e| format!("设置窗口 z-order 失败: {}", e))?;

    eprintln!(
        "[desktop_widget] 已应用桌面层级: HWND {:?} -> 父窗口 HWND {:?}",
        gitaview_hwnd, desktop_host
    );

    Ok(())
}

/// 重新应用桌面 widget 层级到主窗口 (Windows)
///
/// 查找名为 "main" 的窗口并调用 apply_desktop_widget_layer。
/// 适用于 Explorer 重启后或窗口重建后恢复桌面层级。
pub fn reapply_desktop_widget_layer(app: &tauri::AppHandle) -> Result<(), String> {
    let Some(window) = app.get_webview_window("main") else {
        return Err("未找到 main 窗口".to_string());
    };

    apply_desktop_widget_layer(&window)
}

pub fn sync_desktop_widget_frame(
    window: &WebviewWindow,
    x: Option<i32>,
    y: Option<i32>,
    width: u32,
    height: u32,
) -> Result<(), String> {
    let hwnd = window
        .hwnd()
        .map_err(|err| format!("获取窗口句柄失败: {err}"))?;
    let (frame_x, frame_y, move_flag) = match (x, y) {
        (Some(x), Some(y)) => {
            let (x, y) = child_position_from_screen(hwnd, x, y)?;
            (x, y, SWP_NOZORDER | SWP_NOACTIVATE)
        }
        _ => (0, 0, SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOMOVE),
    };
    unsafe {
        SetWindowPos(
            hwnd,
            None,
            frame_x,
            frame_y,
            width as i32,
            height as i32,
            move_flag,
        )
    }
    .map_err(|err| format!("同步窗口位置失败: {err}"))
}

fn child_position_from_screen(hwnd: HWND, x: i32, y: i32) -> Result<(i32, i32), String> {
    let parent = unsafe { GetParent(hwnd) }.ok();
    let Some(parent) = parent.filter(|parent| !parent.is_invalid()) else {
        return Ok((x, y));
    };
    if !unsafe { IsWindow(Some(parent)) }.as_bool() {
        return Ok((x, y));
    }
    let mut point = POINT { x, y };
    if !unsafe { ScreenToClient(parent, &mut point) }.as_bool() {
        return Err("无法将屏幕坐标转换为桌面宿主坐标".to_string());
    }
    Ok((point.x, point.y))
}

pub fn start_desktop_widget_watchdog(app: tauri::AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(WATCHDOG_INTERVAL);
        if let Err(err) = ensure_desktop_widget_layer(&app) {
            crate::diagnostics::log(
                "desktop_widget.watchdog.error",
                format!("error_len={}", err.len()),
            );
        }
    });
}

fn ensure_desktop_widget_layer(app: &tauri::AppHandle) -> Result<(), String> {
    let Some(window) = app.get_webview_window("main") else {
        return Err("未找到 main 窗口".to_string());
    };
    let hwnd = window
        .hwnd()
        .map_err(|err| format!("获取窗口句柄失败: {err}"))?;
    let parent = unsafe { GetParent(hwnd) }.ok();
    if parent
        .filter(|parent| !parent.is_invalid())
        .is_some_and(|parent| unsafe { IsWindow(Some(parent)) }.as_bool())
    {
        return Ok(());
    }
    crate::diagnostics::log("desktop_widget.watchdog.reapply", "");
    apply_desktop_widget_layer(&window)
}

/// 查找桌面图标宿主窗口
///
/// 策略：
/// 1. 枚举所有顶层窗口
/// 2. 查找包含 SHELLDLL_DefView 子窗口的窗口
/// 3. 优先使用 SHELLDLL_DefView 的直接父窗口
/// 4. 如果找不到，回退到 Progman
fn find_desktop_icon_host(fallback: HWND) -> Result<HWND, String> {
    // 使用线程局部存储来传递结果
    thread_local! {
        static FOUND_HOST: Cell<HWND> = Cell::new(HWND::default());
    }

    FOUND_HOST.with(|cell| {
        cell.set(HWND::default());

        // 定义回调函数 - 返回 BOOL
        unsafe extern "system" fn callback(hwnd: HWND, _lparam: LPARAM) -> BOOL {
            // 检查此窗口是否包含 SHELLDLL_DefView
            if has_child_with_class(hwnd, "SHELLDLL_DefView\0") {
                FOUND_HOST.with(|cell| cell.set(hwnd));
                return BOOL(0); // 停止枚举
            }
            BOOL(1) // 继续枚举
        }

        let result = unsafe { EnumWindows(Some(callback), LPARAM(0)) };

        let found = cell.get();
        let last_error = if result.is_err() {
            unsafe { GetLastError() }.0
        } else {
            0
        };

        resolve_enum_windows_host(fallback, found, result.is_err(), last_error)
    })
}

fn resolve_enum_windows_host(
    fallback: HWND,
    found: HWND,
    enum_failed: bool,
    last_error: u32,
) -> Result<HWND, String> {
    if !found.is_invalid() {
        return Ok(found);
    }

    if enum_failed && last_error != 0 {
        return Err(format!("EnumWindows 失败: 错误码 {}", last_error));
    }

    eprintln!("[desktop_widget] 未找到 WorkerW 宿主窗口，回退到 Progman");
    Ok(fallback)
}

/// 检查窗口是否有指定类名的子窗口
fn has_child_with_class(parent: HWND, class_name: &str) -> bool {
    // 将类名转换为 UTF-16
    let class_name_wide: Vec<u16> = class_name.encode_utf16().collect();

    // 遍历子窗口
    let mut child = unsafe { GetWindow(parent, GW_CHILD) }.ok();
    while let Some(hwnd) = child {
        // 检查类名
        if is_window_class(hwnd, &class_name_wide) {
            return true;
        }
        // 继续下一个兄弟窗口
        child = unsafe { GetWindow(hwnd, GW_HWNDNEXT) }.ok();
    }
    false
}

/// 检查窗口的类名是否匹配
fn is_window_class(hwnd: HWND, class_name: &[u16]) -> bool {
    let mut buffer = [0u16; 256];
    let len = unsafe { windows::Win32::UI::WindowsAndMessaging::GetClassNameW(hwnd, &mut buffer) };
    if len == 0 {
        return false;
    }

    // 比较类名（不包括 null 终止符）
    let window_class = &buffer[..len as usize];
    let class_name_without_null = &class_name[..class_name.len().saturating_sub(1)];
    window_class == class_name_without_null
}

/// 调整窗口样式以适应桌面 widget 模式
///
/// 需要做的调整：
/// - 添加 WS_CHILD 样式（作为子窗口）
/// - 添加 WS_VISIBLE 样式
/// - 移除 WS_POPUP 样式（与 WS_CHILD 冲突）
/// - 保持 WS_EX_TOOLWINDOW 样式（不在任务栏显示）
/// - 移除 WS_EX_APPWINDOW 样式（不作为独立应用窗口）
fn adjust_window_styles(hwnd: HWND) -> Result<(), String> {
    // 获取当前样式
    let current_style = unsafe { GetWindowLongPtrW(hwnd, GWL_STYLE) };
    let current_ex_style = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) };

    // 计算新样式
    // 添加 WS_CHILD | WS_VISIBLE
    // 移除 WS_POPUP（与 WS_CHILD 冲突）
    let new_style =
        (current_style | WS_VISIBLE.0 as isize | WS_CHILD.0 as isize) & !(WS_POPUP.0 as isize);

    // 扩展样式：添加 WS_EX_TOOLWINDOW，移除 WS_EX_APPWINDOW
    let new_ex_style =
        (current_ex_style | WS_EX_TOOLWINDOW.0 as isize) & !(WS_EX_APPWINDOW.0 as isize);

    // 应用新样式
    unsafe {
        SetWindowLongPtrW(hwnd, GWL_STYLE, new_style);
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new_ex_style);
    }

    // 确保窗口可见
    unsafe {
        let _ = ShowWindowAsync(hwnd, SW_SHOW);
    }

    eprintln!(
        "[desktop_widget] 窗口样式已调整: style {:x} -> {:x}, ex_style {:x} -> {:x}",
        current_style, new_style, current_ex_style, new_ex_style
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn found_desktop_host_ignores_stale_enum_windows_error() {
        let fallback = HWND::default();
        let found = HWND(std::ptr::dangling_mut::<u8>().cast());

        let host = resolve_enum_windows_host(fallback, found, true, 123).unwrap();

        assert_eq!(host, found);
    }
}
