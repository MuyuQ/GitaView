pub mod app_commands;
pub mod app_settings;
pub mod desktop_widget;
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
}

pub mod storage {
    pub mod store;
}

use tauri::{include_image, Manager};

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let tray_icon = include_image!("./icons/icon.png");
            let tray_menu = tray_status::loading_tray_menu(app)?;

            let _tray = tauri::tray::TrayIconBuilder::with_id(tray_status::MAIN_TRAY_ID)
                .icon(tray_icon)
                .menu(&tray_menu)
                .tooltip("GitaView")
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    tray_status::TRAY_REFRESH_ID => {
                        tray_status::refresh_tray_menu_async(app.clone());
                    }
                    tray_status::TRAY_SHOW_ID => {
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

            tray_status::refresh_tray_menu_async(app.handle().clone());

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
            app_commands::exit_app,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run GitaView");
}
