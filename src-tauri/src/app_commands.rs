use crate::app_settings::{load_app_settings, save_app_settings};
use crate::domain::repo::RepoStatusDto;
use crate::domain::settings::AppSettings;
use crate::git::commands::{branch_state, run_git};
use crate::git::remote::normalize_remote_url;
use crate::repo_operation::{validate_repo_git_operation, RepoGitOperation};
use crate::repo_registry::{find_repo, repo_id_from_path};
use crate::system_open::{open_directory, open_http_url};
use dunce;
use std::time::Instant;
use tauri::Manager;

fn require_confirmation(action: &str, confirmed: bool) -> Result<(), String> {
    if confirmed {
        Ok(())
    } else {
        Err(format!("{action} 需要确认"))
    }
}

#[tauri::command]
pub async fn get_settings(app: tauri::AppHandle) -> Result<AppSettings, String> {
    let started = Instant::now();
    crate::diagnostics::log("command.get_settings.start", "");
    let result = load_app_settings(&app);
    match &result {
        Ok(settings) => crate::diagnostics::log_duration(
            "command.get_settings.ok",
            started.elapsed(),
            format!("repos={}", settings.repos.len()),
        ),
        Err(err) => crate::diagnostics::log_duration(
            "command.get_settings.error",
            started.elapsed(),
            format!("error_len={}", err.len()),
        ),
    }
    result
}

#[tauri::command]
pub async fn save_settings(
    app: tauri::AppHandle,
    settings: AppSettings,
) -> Result<AppSettings, String> {
    let started = Instant::now();
    crate::diagnostics::log(
        "command.save_settings.start",
        format!("repos={}", settings.repos.len()),
    );
    let saved = save_app_settings(&app, &settings)?;
    crate::diagnostics::log_duration(
        "command.save_settings.ok",
        started.elapsed(),
        format!("repos={}", saved.repos.len()),
    );
    Ok(saved)
}

#[tauri::command]
pub async fn scan_directory(path: String) -> Result<Vec<String>, String> {
    let started = Instant::now();
    let root = std::path::PathBuf::from(&path);
    crate::diagnostics::log(
        "command.scan_directory.start",
        format!("path={}", crate::diagnostics::redact_path(&root)),
    );
    if !root.is_dir() {
        crate::diagnostics::log_duration(
            "command.scan_directory.error",
            started.elapsed(),
            "invalid directory",
        );
        return Err("请选择有效的目录".to_string());
    }
    let repos = tauri::async_runtime::spawn_blocking(move || {
        crate::git::discovery::scan_repositories(&root)
    })
    .await
    .map_err(|err| err.to_string())?;
    crate::diagnostics::log_duration(
        "command.scan_directory.ok",
        started.elapsed(),
        format!("repos={}", repos.len()),
    );
    Ok(repos
        .into_iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect())
}

#[tauri::command]
pub async fn add_repository(
    app: tauri::AppHandle,
    path: String,
) -> Result<crate::domain::repo::RepoRecord, String> {
    use crate::domain::repo::RepoRecord;
    let repo_path = std::path::PathBuf::from(&path);
    crate::diagnostics::log(
        "command.add_repository.start",
        format!("path={}", crate::diagnostics::redact_path(&repo_path)),
    );
    if !crate::git::discovery::is_git_repo(&repo_path) {
        crate::diagnostics::log("command.add_repository.error", "invalid git repository");
        return Err("请选择有效的 Git 仓库目录".to_string());
    }
    let repo_path = dunce::canonicalize(&repo_path).map_err(|err| err.to_string())?;
    let mut settings = load_app_settings(&app)?;
    if let Some(existing) = settings
        .repos
        .iter()
        .find(|repo| repo.path.as_path() == repo_path.as_path())
    {
        return Ok(existing.clone());
    }
    let name = repo_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("unknown")
        .to_string();
    let record = RepoRecord {
        id: repo_id_from_path(&repo_path, &settings),
        name,
        path: repo_path,
        group: "全部分组".to_string(),
    };
    settings.repos.push(record.clone());
    save_app_settings(&app, &settings)?;
    crate::diagnostics::log(
        "command.add_repository.ok",
        format!(
            "id={} path={}",
            record.id,
            crate::diagnostics::redact_path(&record.path)
        ),
    );
    Ok(record)
}

#[tauri::command]
pub async fn remove_repository(app: tauri::AppHandle, repo_id: String) -> Result<(), String> {
    crate::diagnostics::log(
        "command.remove_repository.start",
        format!("repo_id={repo_id}"),
    );
    let mut settings = load_app_settings(&app)?;
    settings.repos.retain(|repo| repo.id != repo_id);
    for group in &mut settings.groups {
        group.repo_ids.retain(|id| id != &repo_id);
    }
    save_app_settings(&app, &settings)?;
    crate::diagnostics::log("command.remove_repository.ok", "");
    Ok(())
}

#[tauri::command]
pub async fn list_repo_statuses(app: tauri::AppHandle) -> Result<Vec<RepoStatusDto>, String> {
    let started = Instant::now();
    let tray_generation = crate::tray_status::begin_tray_menu_update();
    crate::diagnostics::log("command.list_repo_statuses.start", "");
    let settings = load_app_settings(&app)?;
    crate::diagnostics::log(
        "command.list_repo_statuses.settings",
        format!("repos={}", settings.repos.len()),
    );
    let statuses = tauri::async_runtime::spawn_blocking(move || {
        crate::repo_status::collect_repo_statuses(settings.repos)
    })
    .await
    .map_err(|err| err.to_string())??;
    match crate::tray_status::set_status_menu_if_current(&app, tray_generation, &statuses) {
        Ok(false) => crate::diagnostics::log(
            "command.list_repo_statuses.tray_stale",
            format!("generation={tray_generation}"),
        ),
        Err(err) => {
            eprintln!("更新托盘状态菜单失败: {err}");
            crate::diagnostics::log(
                "command.list_repo_statuses.tray_error",
                format!("error_len={}", err.len()),
            );
        }
        Ok(true) => {}
    }
    crate::diagnostics::log_duration(
        "command.list_repo_statuses.ok",
        started.elapsed(),
        format!("statuses={}", statuses.len()),
    );
    Ok(statuses)
}

#[tauri::command]
pub async fn fetch_repo(app: tauri::AppHandle, repo_id: String) -> Result<String, String> {
    let settings = load_app_settings(&app)?;
    let repo = find_repo(&settings, &repo_id)?;
    let repo_path = repo.path.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let state = branch_state(&repo_path)?;
        validate_repo_git_operation(RepoGitOperation::Fetch, state.relation, state.has_remote)?;
        run_git(&repo_path, &["fetch"])
    })
    .await
    .map_err(|err| err.to_string())??;
    Ok("Fetch 已完成".to_string())
}

#[tauri::command]
pub async fn pull_repo(
    app: tauri::AppHandle,
    repo_id: String,
    confirmed: bool,
) -> Result<String, String> {
    require_confirmation("Pull", confirmed)?;
    let settings = load_app_settings(&app)?;
    let repo = find_repo(&settings, &repo_id)?;
    let repo_path = repo.path.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let state = branch_state(&repo_path)?;
        validate_repo_git_operation(RepoGitOperation::Pull, state.relation, state.has_remote)?;
        run_git(&repo_path, &["pull"])
    })
    .await
    .map_err(|err| err.to_string())??;
    Ok("Pull 已完成".to_string())
}

#[tauri::command]
pub async fn push_repo(
    app: tauri::AppHandle,
    repo_id: String,
    confirmed: bool,
) -> Result<String, String> {
    require_confirmation("Push", confirmed)?;
    let settings = load_app_settings(&app)?;
    let repo = find_repo(&settings, &repo_id)?;
    let repo_path = repo.path.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let state = branch_state(&repo_path)?;
        validate_repo_git_operation(RepoGitOperation::Push, state.relation, state.has_remote)?;
        run_git(&repo_path, &["push"])
    })
    .await
    .map_err(|err| err.to_string())??;
    Ok("Push 已完成".to_string())
}

#[tauri::command]
pub async fn open_repo_directory(app: tauri::AppHandle, repo_id: String) -> Result<(), String> {
    let settings = load_app_settings(&app)?;
    let repo = find_repo(&settings, &repo_id)?;
    // 校验路径存在
    if !repo.path.exists() {
        return Err("仓库目录不存在".to_string());
    }
    open_directory(&repo.path)?;
    Ok(())
}

#[tauri::command]
pub async fn open_repo_remote(app: tauri::AppHandle, repo_id: String) -> Result<(), String> {
    let settings = load_app_settings(&app)?;
    let repo = find_repo(&settings, &repo_id)?;
    let repo_path = repo.path.clone();
    let remote = tauri::async_runtime::spawn_blocking(move || {
        run_git(&repo_path, &["config", "--get", "remote.origin.url"])
    })
    .await
    .map_err(|err| err.to_string())??;
    // 规范化 URL 并校验是否为可打开的 HTTP/HTTPS 地址
    let remote = normalize_remote_url(&remote)
        .ok_or_else(|| "当前仓库没有可打开的 HTTP/HTTPS 远端地址".to_string())?;
    open_http_url(&remote)?;
    Ok(())
}

#[tauri::command]
pub async fn sync_desktop_widget_frame(
    app: tauri::AppHandle,
    x: Option<i32>,
    y: Option<i32>,
    width: u32,
    height: u32,
) -> Result<(), String> {
    if width == 0 || height == 0 {
        return Err("窗口尺寸必须大于零".to_string());
    }
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "未找到 main 窗口".to_string())?;
    crate::desktop_widget::sync_desktop_widget_frame(&window, x, y, width, height)
}

#[tauri::command]
pub async fn exit_app(app: tauri::AppHandle) {
    app.exit(0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mutating_repository_actions_always_require_confirmation() {
        assert_eq!(
            require_confirmation("Pull", false),
            Err("Pull 需要确认".to_string())
        );
        assert_eq!(
            require_confirmation("Push", false),
            Err("Push 需要确认".to_string())
        );
        assert_eq!(require_confirmation("Pull", true), Ok(()));
        assert_eq!(require_confirmation("Push", true), Ok(()));
    }
}
