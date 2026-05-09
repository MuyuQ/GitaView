pub mod domain {
    pub mod status;
}

pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("failed to run GitaView");
}
