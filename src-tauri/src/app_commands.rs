use crate::domain::repo::RepoStatusDto;
use crate::domain::settings::AppSettings;
use crate::domain::status::RemoteRelation;

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
    let name = path.replace('\\', "/").split('/').last().unwrap_or("unknown").to_string();
    let record = RepoRecord {
        id: name.clone(),
        name,
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
    // Return fixture data for demonstration until full backend integration
    Ok(vec![
        RepoStatusDto {
            id: "api".into(), name: "接口服务".into(), path: "E:/Git_Repositories/api".into(),
            group: "后端".into(), branch: "dev".into(), relation: RemoteRelation::Diverged,
            change_label: "⇕ 3".into(), hint: "需要人工处理".into(),
            remote_url: Some("https://github.com/example/api".into()),
        },
        RepoStatusDto {
            id: "web".into(), name: "前端".into(), path: "E:/Git_Repositories/web".into(),
            group: "Web".into(), branch: "main".into(), relation: RemoteRelation::RemoteAhead,
            change_label: "↓ 2".into(), hint: "可 Pull".into(),
            remote_url: Some("https://github.com/example/web".into()),
        },
        RepoStatusDto {
            id: "docs".into(), name: "文档".into(), path: "E:/Git_Repositories/docs".into(),
            group: "文档".into(), branch: "main".into(), relation: RemoteRelation::Synced,
            change_label: "✓".into(), hint: "无需操作".into(),
            remote_url: Some("https://github.com/example/docs".into()),
        },
        RepoStatusDto {
            id: "lab".into(), name: "实验仓库".into(), path: "E:/Git_Repositories/lab".into(),
            group: "实验".into(), branch: "topic".into(), relation: RemoteRelation::NoRemote,
            change_label: "∅".into(), hint: "未设置 upstream".into(),
            remote_url: None,
        },
    ])
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
