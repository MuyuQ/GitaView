pub mod domain {
    pub mod repo;
    pub mod status;
}

pub mod git {
    pub mod commands;
    pub mod discovery;
}

pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("failed to run GitaView");
}
