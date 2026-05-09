use crate::domain::repo::RepoStatusDto;
use crate::domain::settings::AppSettings;

#[tauri::command]
pub async fn get_settings() -> Result<AppSettings, String> {
    Ok(AppSettings::default())
}

#[tauri::command]
pub async fn save_settings(_settings: AppSettings) -> Result<AppSettings, String> {
    Ok(AppSettings::default())
}

#[tauri::command]
pub async fn scan_directory(path: String) -> Result<Vec<String>, String> {
    let repos = crate::git::discovery::scan_repositories(std::path::Path::new(&path));
    Ok(repos.into_iter().map(|p| p.to_string_lossy().to_string()).collect())
}

#[tauri::command]
pub async fn add_repository(path: String) -> Result<serde_json::Value, String> {
    use crate::domain::repo::RepoRecord;
    let record = RepoRecord {
        id: path.replace('\\', "/").split('/').last().unwrap_or("unknown").to_string(),
        name: path.replace('\\', "/").split('/').last().unwrap_or("unknown").to_string(),
        path: std::path::PathBuf::from(&path),
        group: "全部分组".to_string(),
    };
    Ok(serde_json::to_value(record).unwrap())
}

#[tauri::command]
pub async fn remove_repository(_repo_id: String) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub async fn list_repo_statuses() -> Result<Vec<RepoStatusDto>, String> {
    Ok(Vec::new())
}

#[tauri::command]
pub async fn fetch_repo(_repo_id: String) -> Result<String, String> {
    Ok("Fetch 已完成".to_string())
}

#[tauri::command]
pub async fn pull_repo(_repo_id: String, confirmed: bool) -> Result<String, String> {
    if !confirmed {
        return Err("Pull 需要确认".to_string());
    }
    Ok("Pull 已完成".to_string())
}

#[tauri::command]
pub async fn open_repo_directory(_repo_id: String) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub async fn open_repo_remote(_repo_id: String) -> Result<(), String> {
    Ok(())
}
