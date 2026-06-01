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

pub mod domain {
    pub mod repo;
    pub mod settings;
    pub mod status;
}

pub mod git {
    pub mod commands;
    pub mod discovery;
    pub mod remote;
    pub mod status_text;
}

pub mod storage {
    pub mod store;
}

use tauri::{include_image, Manager};

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
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
            app_commands::exit_app,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run GitaView");
}
