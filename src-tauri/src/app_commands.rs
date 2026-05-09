use crate::domain::repo::RepoStatusDto;
use crate::domain::settings::AppSettings;
use crate::git::commands::{branch_state, change_label, relation_hint, run_git};
use tauri::Manager;

fn settings_path(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|err| err.to_string())?
        .join("settings.json"))
}

fn load_app_settings(app: &tauri::AppHandle) -> Result<AppSettings, String> {
    crate::storage::store::load_settings(&settings_path(app)?)
}

fn save_app_settings(app: &tauri::AppHandle, settings: &AppSettings) -> Result<(), String> {
    crate::storage::store::save_settings(&settings_path(app)?, settings)
}

fn repo_id_from_path(path: &std::path::Path, settings: &AppSettings) -> String {
    let base = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("repo")
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch.to_ascii_lowercase() } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    let base = if base.is_empty() { "repo".to_string() } else { base };
    if !settings.repos.iter().any(|repo| repo.id == base) {
        return base;
    }
    let mut suffix = 2;
    loop {
        let candidate = format!("{base}-{suffix}");
        if !settings.repos.iter().any(|repo| repo.id == candidate) {
            return candidate;
        }
        suffix += 1;
    }
}

fn find_repo<'a>(settings: &'a AppSettings, repo_id: &str) -> Result<&'a crate::domain::repo::RepoRecord, String> {
    settings
        .repos
        .iter()
        .find(|repo| repo.id == repo_id)
        .ok_or_else(|| format!("未找到仓库：{repo_id}"))
}

fn open_path(target: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut cmd = std::process::Command::new("cmd");
        cmd.args(["/C", "start", "", target]);
        cmd
    };

    #[cfg(target_os = "macos")]
    let mut command = {
        let mut cmd = std::process::Command::new("open");
        cmd.arg(target);
        cmd
    };

    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = {
        let mut cmd = std::process::Command::new("xdg-open");
        cmd.arg(target);
        cmd
    };

    command.spawn().map_err(|err| err.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn get_settings(app: tauri::AppHandle) -> Result<AppSettings, String> {
    load_app_settings(&app)
}

#[tauri::command]
pub async fn save_settings(app: tauri::AppHandle, settings: AppSettings) -> Result<AppSettings, String> {
    save_app_settings(&app, &settings)?;
    Ok(settings)
}

#[tauri::command]
pub async fn scan_directory(path: String) -> Result<Vec<String>, String> {
    let repos = crate::git::discovery::scan_repositories(std::path::Path::new(&path));
    Ok(repos.into_iter().map(|p| p.to_string_lossy().to_string()).collect())
}

#[tauri::command]
pub async fn add_repository(app: tauri::AppHandle, path: String) -> Result<crate::domain::repo::RepoRecord, String> {
    use crate::domain::repo::RepoRecord;
    let repo_path = std::path::PathBuf::from(&path);
    if !crate::git::discovery::is_git_repo(&repo_path) {
        return Err("请选择有效的 Git 仓库目录".to_string());
    }
    let repo_path = repo_path.canonicalize().map_err(|err| err.to_string())?;
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
    Ok(record)
}

#[tauri::command]
pub async fn remove_repository(app: tauri::AppHandle, repo_id: String) -> Result<(), String> {
    let mut settings = load_app_settings(&app)?;
    settings.repos.retain(|repo| repo.id != repo_id);
    for group in &mut settings.groups {
        group.repo_ids.retain(|id| id != &repo_id);
    }
    save_app_settings(&app, &settings)?;
    Ok(())
}

#[tauri::command]
pub async fn list_repo_statuses(app: tauri::AppHandle) -> Result<Vec<RepoStatusDto>, String> {
    let settings = load_app_settings(&app)?;
    let mut statuses = Vec::new();
    for repo in settings.repos {
        let state = branch_state(&repo.path)?;
        statuses.push(RepoStatusDto {
            id: repo.id,
            name: repo.name,
            path: repo.path.to_string_lossy().to_string(),
            group: repo.group,
            branch: state.branch.clone(),
            relation: state.relation,
            change_label: change_label(&state),
            hint: relation_hint(state.relation).to_string(),
            remote_url: state.remote_url,
        });
    }
    statuses.sort_by_key(|repo| repo.relation.sort_rank());
    Ok(statuses)
}

#[tauri::command]
pub async fn fetch_repo(app: tauri::AppHandle, repo_id: String) -> Result<String, String> {
    let settings = load_app_settings(&app)?;
    let repo = find_repo(&settings, &repo_id)?;
    run_git(&repo.path, &["fetch"])?;
    Ok("Fetch 已完成".to_string())
}

#[tauri::command]
pub async fn pull_repo(app: tauri::AppHandle, repo_id: String, confirmed: bool) -> Result<String, String> {
    let settings = load_app_settings(&app)?;
    if settings.safety.confirm_pull && !confirmed {
        return Err("Pull 需要确认".to_string());
    }
    let repo = find_repo(&settings, &repo_id)?;
    run_git(&repo.path, &["pull"])?;
    Ok("Pull 已完成".to_string())
}

#[tauri::command]
pub async fn push_repo(app: tauri::AppHandle, repo_id: String, confirmed: bool) -> Result<String, String> {
    let settings = load_app_settings(&app)?;
    if settings.safety.confirm_push && !confirmed {
        return Err("Push 需要确认".to_string());
    }
    let repo = find_repo(&settings, &repo_id)?;
    run_git(&repo.path, &["push"])?;
    Ok("Push 已完成".to_string())
}

#[tauri::command]
pub async fn open_repo_directory(app: tauri::AppHandle, repo_id: String) -> Result<(), String> {
    let settings = load_app_settings(&app)?;
    let repo = find_repo(&settings, &repo_id)?;
    let target = repo.path.to_string_lossy().to_string();
    open_path(&target)?;
    Ok(())
}

#[tauri::command]
pub async fn open_repo_remote(app: tauri::AppHandle, repo_id: String) -> Result<(), String> {
    let settings = load_app_settings(&app)?;
    let repo = find_repo(&settings, &repo_id)?;
    let remote = run_git(&repo.path, &["config", "--get", "remote.origin.url"])?;
    let remote = crate::git::commands::normalize_remote_url(&remote)
        .ok_or_else(|| "当前仓库没有远端地址".to_string())?;
    open_path(&remote)?;
    Ok(())
}
