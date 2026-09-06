pub mod app_commands;
pub mod app_settings;
pub mod desktop_widget;
pub mod diagnostics;
pub mod repo_operation;
pub mod repo_registry;
pub mod repo_status;
pub mod system_open;
pub mod tray_menu_rows;
pub mod tray_status;
pub mod widget_data;
pub mod window_state;

pub mod domain {
    pub mod repo;
    pub mod settings;
    pub mod status;
}

pub mod git {
    pub mod commands;
    pub mod discovery;
    pub mod operation_lock;
    pub mod remote;
    pub mod status_text;
}

pub mod storage {
    pub mod store;
}

use tauri::{include_image, Manager};
use tauri_plugin_deep_link::DeepLinkExt;

pub fn run() {
    tauri::Builder::default()
        // 必须最先注册：二次启动（含 gitaview:// 链接触发）立即转发到这里，
        // 避免两个实例竞争 settings.json 的读-改-写与桌面 widget 的窗口层级
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            diagnostics::log("single_instance.activate", "");
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_deep_link::init())
        .setup(|app| {
            let app_handle = app.handle().clone();
            #[cfg(target_os = "macos")]
            app.handle()
                .set_activation_policy(tauri::ActivationPolicy::Accessory)?;
            if let Ok(app_data_dir) = app.path().app_data_dir() {
                diagnostics::init(app_data_dir.join("gitaview.log"));
            }
            diagnostics::log(
                "app.setup.start",
                format!(
                    "exe={} cwd={}",
                    std::env::current_exe()
                        .map(|path| diagnostics::redact_path(&path))
                        .unwrap_or_else(|err| format!("error:{err}")),
                    std::env::current_dir()
                        .map(|path| diagnostics::redact_path(&path))
                        .unwrap_or_else(|err| format!("error:{err}")),
                ),
            );
            match app_settings::settings_path(&app_handle) {
                Ok(path) => {
                    diagnostics::log("app.setup.settings_path", diagnostics::redact_path(&path))
                }
                Err(err) => diagnostics::log("app.setup.settings_path_error", err),
            }
            if let Some(window) = app.get_webview_window("main") {
                diagnostics::log_window("app.setup.main_window", &window);
            } else {
                diagnostics::log("app.setup.main_window_missing", "main window not found");
            }
            if let Err(err) = desktop_widget::reapply_desktop_widget_layer(app.handle()) {
                diagnostics::log("app.setup.desktop_widget_error", &err);
                eprintln!("应用桌面 widget 层失败，将作为普通窗口运行: {err}");
            }
            desktop_widget::start_desktop_widget_watchdog(app.handle().clone());

            // 恢复上次的窗口位置（规格 §8 Window persistence）。
            // 必须在 setup 内完成：前端首次帧同步以当前窗口位置为锚点，
            // 这里先恢复，锚定逻辑就会保留它；越界位置由钳制拉回可见区域。
            if let Some(window) = app.get_webview_window("main") {
                if let Ok(state_path) = window_state::window_state_path(app.handle()) {
                    if let Some(position) = window_state::load_window_position(&state_path) {
                        let monitors = window
                            .available_monitors()
                            .unwrap_or_default()
                            .iter()
                            .map(|monitor| {
                                let origin = monitor.position();
                                let size = monitor.size();
                                (origin.x, origin.y, size.width, size.height)
                            })
                            .collect::<Vec<_>>();
                        let restored = window_state::clamp_position(position, &monitors);
                        match window
                            .set_position(tauri::PhysicalPosition::new(restored.x, restored.y))
                        {
                            Ok(()) => diagnostics::log(
                                "app.setup.window_state_restored",
                                format!("x={} y={}", restored.x, restored.y),
                            ),
                            Err(err) => {
                                diagnostics::log("app.setup.window_state_error", err.to_string())
                            }
                        }
                    }
                }
            }

            // Deep Link 处理
            let handle = app.handle().clone();
            app.deep_link().on_open_url(move |event| {
                for url in event.urls() {
                    diagnostics::log("deep_link.received", url.as_str());
                    if url.scheme() == "gitaview" {
                        if let Some(window) = handle.get_webview_window("main") {
                            if let Err(err) = window.show() {
                                diagnostics::log("deep_link.show_error", err.to_string());
                            }
                            if let Err(err) = window.set_focus() {
                                diagnostics::log("deep_link.focus_error", err.to_string());
                            }
                        } else {
                            diagnostics::log("deep_link.window_not_found", "");
                        }
                    }
                }
            });

            #[cfg(target_os = "macos")]
            let tray_icon = include_image!("./icons/tray-template.png");
            #[cfg(not(target_os = "macos"))]
            let tray_icon = include_image!("./icons/icon.png");
            let tray_menu = tray_status::loading_tray_menu(app)?;

            let tray_builder = tauri::tray::TrayIconBuilder::with_id(tray_status::MAIN_TRAY_ID)
                .icon(tray_icon)
                .menu(&tray_menu)
                .tooltip("GitaView");
            #[cfg(target_os = "macos")]
            let tray_builder = tray_builder
                .icon_as_template(true)
                .show_menu_on_left_click(true);
            #[cfg(not(target_os = "macos"))]
            let tray_builder = tray_builder.show_menu_on_left_click(false);

            let _tray = tray_builder
                .on_menu_event(|app, event| match event.id().as_ref() {
                    tray_status::TRAY_REFRESH_ID => {
                        tray_status::refresh_tray_menu_async(app.clone());
                    }
                    tray_status::TRAY_SHOW_ID => {
                        if let Err(err) = desktop_widget::reapply_desktop_widget_layer(app) {
                            eprintln!("重新应用桌面 widget 层失败: {err}");
                        }
                        if let Some(window) = app.get_webview_window("main") {
                            if let Err(err) = window.show() {
                                eprintln!("显示主窗口失败: {err}");
                            }
                            if let Err(err) = window.set_focus() {
                                eprintln!("聚焦主窗口失败: {err}");
                            }
                        }
                    }
                    tray_status::TRAY_QUIT_ID => app.exit(0),
                    _ => {}
                })
                .build(app)?;

            diagnostics::log("app.setup.tray_ready", "main tray created");
            tray_status::refresh_tray_menu_async(app.handle().clone());
            diagnostics::log("app.setup.end", "setup completed");

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_commands::get_settings,
            app_commands::save_settings,
            app_commands::scan_directory,
            app_commands::add_repository,
            app_commands::remove_repository,
            app_commands::list_repo_statuses,
            app_commands::fetch_repo,
            app_commands::pull_repo,
            app_commands::push_repo,
            app_commands::open_repo_directory,
            app_commands::open_repo_remote,
            app_commands::sync_desktop_widget_frame,
            app_commands::save_window_state,
            app_commands::exit_app,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run GitaView");
}
