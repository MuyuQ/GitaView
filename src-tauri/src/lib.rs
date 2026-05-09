pub mod app_commands;

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
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let _tray = tauri::tray::TrayIconBuilder::new()
                .tooltip("GitaView")
                .show_menu_on_left_click(false)
                .build(app)?;
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
            app_commands::open_repo_directory,
            app_commands::open_repo_remote,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run GitaView");
}
