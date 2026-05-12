pub mod app_commands;
pub mod desktop_widget;

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

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let _tray = tauri::tray::TrayIconBuilder::new()
                .tooltip("GitaView")
                .show_menu_on_left_click(false)
                .build(app)?;

            // 应用桌面 widget 层级行为（仅在 Windows 和 macOS）
            // 失败时只记录错误，保持应用作为普通窗口继续运行
            if let Err(err) = crate::desktop_widget::reapply_desktop_widget_layer(app.handle()) {
                eprintln!("应用桌面 widget 层失败，将作为普通窗口运行: {err}");
            }

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
