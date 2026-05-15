use crate::domain::repo::{RepoRecord, RepoStatusDto};
use crate::domain::settings::AppSettings;
use crate::domain::status::RemoteRelation;
use crate::git::commands::{branch_state, change_label, run_git, state_hint, GitBranchState};
use dunce;
use tauri::Manager;

const STATUS_REFRESH_BATCH_SIZE: usize = 4;

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

fn save_app_settings(
    app: &tauri::AppHandle,
    settings: &AppSettings,
) -> Result<AppSettings, String> {
    crate::storage::store::save_settings(&settings_path(app)?, settings)
}

fn repo_id_from_path(path: &std::path::Path, settings: &AppSettings) -> String {
    let base = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("repo")
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    let base = if base.is_empty() {
        "repo".to_string()
    } else {
        base
    };
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

fn find_repo<'a>(
    settings: &'a AppSettings,
    repo_id: &str,
) -> Result<&'a crate::domain::repo::RepoRecord, String> {
    settings
        .repos
        .iter()
        .find(|repo| repo.id == repo_id)
        .ok_or_else(|| format!("未找到仓库：{repo_id}"))
}

/// 打开本地目录（使用系统默认文件浏览器）
/// 仅用于打开已验证的仓库目录
fn open_directory(path: &std::path::Path) -> Result<(), String> {
    let path_str = path.to_string_lossy().to_string();

    #[cfg(target_os = "windows")]
    let mut command = {
        let mut cmd = std::process::Command::new("explorer");
        cmd.arg(&path_str);
        cmd
    };

    #[cfg(target_os = "macos")]
    let mut command = {
        let mut cmd = std::process::Command::new("open");
        cmd.arg(&path_str);
        cmd
    };

    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = {
        let mut cmd = std::process::Command::new("xdg-open");
        cmd.arg(&path_str);
        cmd
    };

    command
        .spawn()
        .map_err(|err| format!("无法打开目录：{}", err))?;
    Ok(())
}

/// 打开 HTTP/HTTPS URL（使用系统默认浏览器）
/// 仅用于打开已规范化的远端 URL
fn open_http_url(url: &str) -> Result<(), String> {
    // 校验 URL 必须是 HTTP 或 HTTPS
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("只支持 HTTP/HTTPS URL".to_string());
    }

    #[cfg(target_os = "windows")]
    let mut command = {
        let mut cmd = std::process::Command::new("explorer");
        cmd.arg(url);
        cmd
    };

    #[cfg(target_os = "macos")]
    let mut command = {
        let mut cmd = std::process::Command::new("open");
        cmd.arg(url);
        cmd
    };

    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = {
        let mut cmd = std::process::Command::new("xdg-open");
        cmd.arg(url);
        cmd
    };

    command
        .spawn()
        .map_err(|err| format!("无法打开 URL：{}", err))?;
    Ok(())
}

#[derive(Debug, Clone, Copy)]
enum RepoGitOperation {
    Fetch,
    Pull,
    Push,
}

fn validate_repo_git_operation(
    operation: RepoGitOperation,
    relation: RemoteRelation,
    has_remote: bool,
) -> Result<(), String> {
    match (operation, relation) {
        (_, RemoteRelation::Error) => Err("当前仓库状态异常，请先刷新或检查仓库路径".to_string()),
        (RepoGitOperation::Fetch, RemoteRelation::NoRemote) if has_remote => Ok(()),
        (_, RemoteRelation::NoRemote) => Err("当前仓库没有可操作的远端分支".to_string()),
        (RepoGitOperation::Fetch, _) => Ok(()),
        (RepoGitOperation::Pull, RemoteRelation::RemoteAhead | RemoteRelation::Diverged) => Ok(()),
        (RepoGitOperation::Pull, RemoteRelation::Synced) => {
            Err("当前仓库已同步，无需 Pull".to_string())
        }
        (RepoGitOperation::Pull, RemoteRelation::LocalAhead) => {
            Err("当前仓库只有本地提交，无需 Pull".to_string())
        }
        (RepoGitOperation::Push, RemoteRelation::LocalAhead | RemoteRelation::Diverged) => Ok(()),
        (RepoGitOperation::Push, RemoteRelation::Synced) => {
            Err("当前仓库已同步，无需 Push".to_string())
        }
        (RepoGitOperation::Push, RemoteRelation::RemoteAhead) => {
            Err("当前仓库落后远端，请先 Pull 或处理分叉".to_string())
        }
    }
}

fn repo_status_from_branch_result(
    repo: RepoRecord,
    result: Result<GitBranchState, String>,
) -> RepoStatusDto {
    match result {
        Ok(state) => RepoStatusDto {
            id: repo.id,
            name: repo.name,
            path: repo.path.to_string_lossy().to_string(),
            group: repo.group,
            branch: state.branch.clone(),
            relation: state.relation,
            change_label: change_label(&state),
            hint: state_hint(&state),
            has_remote: state.has_remote,
            remote_url: state.remote_url,
        },
        Err(err) => RepoStatusDto {
            id: repo.id,
            name: repo.name,
            path: repo.path.to_string_lossy().to_string(),
            group: repo.group,
            branch: "未知".to_string(),
            relation: RemoteRelation::Error,
            change_label: "!".to_string(),
            hint: format!("读取失败：{err}"),
            has_remote: false,
            remote_url: None,
        },
    }
}

fn sort_repo_statuses(statuses: &mut [RepoStatusDto]) {
    statuses.sort_by_key(|repo| repo.relation.sort_rank());
}

fn collect_repo_statuses(repos: Vec<RepoRecord>) -> Result<Vec<RepoStatusDto>, String> {
    let mut statuses = Vec::with_capacity(repos.len());
    for batch in repos.chunks(STATUS_REFRESH_BATCH_SIZE) {
        let handles = batch
            .iter()
            .cloned()
            .map(|repo| {
                std::thread::spawn(move || {
                    let state = branch_state(&repo.path);
                    repo_status_from_branch_result(repo, state)
                })
            })
            .collect::<Vec<_>>();

        for handle in handles {
            statuses.push(
                handle
                    .join()
                    .map_err(|_| "刷新仓库状态线程异常退出".to_string())?,
            );
        }
    }
    sort_repo_statuses(&mut statuses);
    Ok(statuses)
}

#[tauri::command]
pub async fn get_settings(app: tauri::AppHandle) -> Result<AppSettings, String> {
    load_app_settings(&app)
}

#[tauri::command]
pub async fn save_settings(
    app: tauri::AppHandle,
    settings: AppSettings,
) -> Result<AppSettings, String> {
    let saved = save_app_settings(&app, &settings)?;
    Ok(saved)
}

#[tauri::command]
pub async fn scan_directory(path: String) -> Result<Vec<String>, String> {
    let root = std::path::PathBuf::from(&path);
    if !root.is_dir() {
        return Err("请选择有效的目录".to_string());
    }
    let repos = tauri::async_runtime::spawn_blocking(move || {
        crate::git::discovery::scan_repositories(&root)
    })
    .await
    .map_err(|err| err.to_string())?;
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
    if !crate::git::discovery::is_git_repo(&repo_path) {
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
    tauri::async_runtime::spawn_blocking(move || collect_repo_statuses(settings.repos))
        .await
        .map_err(|err| err.to_string())?
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
    let settings = load_app_settings(&app)?;
    if settings.safety.confirm_pull && !confirmed {
        return Err("Pull 需要确认".to_string());
    }
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
    let settings = load_app_settings(&app)?;
    if settings.safety.confirm_push && !confirmed {
        return Err("Push 需要确认".to_string());
    }
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
    let remote = crate::git::commands::normalize_remote_url(&remote)
        .ok_or_else(|| "当前仓库没有可打开的 HTTP/HTTPS 远端地址".to_string())?;
    open_http_url(&remote)?;
    Ok(())
}

#[tauri::command]
pub async fn exit_app(app: tauri::AppHandle) {
    app.exit(0);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::repo::RepoRecord;
    use crate::git::commands::GitBranchState;
    use std::path::PathBuf;

    fn sample_repo(id: &str) -> RepoRecord {
        RepoRecord {
            id: id.to_string(),
            name: id.to_string(),
            path: PathBuf::from(format!("C:/{id}")),
            group: "全部分组".to_string(),
        }
    }

    #[test]
    fn fetch_allows_repositories_with_remote_even_without_upstream() {
        assert!(
            validate_repo_git_operation(RepoGitOperation::Fetch, RemoteRelation::Synced, true)
                .is_ok()
        );
        assert!(validate_repo_git_operation(
            RepoGitOperation::Fetch,
            RemoteRelation::LocalAhead,
            true
        )
        .is_ok());
        assert!(validate_repo_git_operation(
            RepoGitOperation::Fetch,
            RemoteRelation::RemoteAhead,
            true
        )
        .is_ok());
        assert!(validate_repo_git_operation(
            RepoGitOperation::Fetch,
            RemoteRelation::Diverged,
            true
        )
        .is_ok());
        assert!(validate_repo_git_operation(
            RepoGitOperation::Fetch,
            RemoteRelation::NoRemote,
            true
        )
        .is_ok());
        assert!(validate_repo_git_operation(
            RepoGitOperation::Fetch,
            RemoteRelation::NoRemote,
            false
        )
        .is_err());
        assert!(
            validate_repo_git_operation(RepoGitOperation::Fetch, RemoteRelation::Error, true)
                .is_err()
        );
    }

    #[test]
    fn pull_only_allows_remote_changes_that_need_user_action() {
        assert!(validate_repo_git_operation(
            RepoGitOperation::Pull,
            RemoteRelation::RemoteAhead,
            true
        )
        .is_ok());
        assert!(validate_repo_git_operation(
            RepoGitOperation::Pull,
            RemoteRelation::Diverged,
            true
        )
        .is_ok());
        assert!(
            validate_repo_git_operation(RepoGitOperation::Pull, RemoteRelation::Synced, true)
                .is_err()
        );
        assert!(validate_repo_git_operation(
            RepoGitOperation::Pull,
            RemoteRelation::LocalAhead,
            true
        )
        .is_err());
        assert!(validate_repo_git_operation(
            RepoGitOperation::Pull,
            RemoteRelation::NoRemote,
            true
        )
        .is_err());
        assert!(
            validate_repo_git_operation(RepoGitOperation::Pull, RemoteRelation::Error, true)
                .is_err()
        );
    }

    #[test]
    fn push_only_allows_local_changes_that_need_user_action() {
        assert!(validate_repo_git_operation(
            RepoGitOperation::Push,
            RemoteRelation::LocalAhead,
            true
        )
        .is_ok());
        assert!(validate_repo_git_operation(
            RepoGitOperation::Push,
            RemoteRelation::Diverged,
            true
        )
        .is_ok());
        assert!(
            validate_repo_git_operation(RepoGitOperation::Push, RemoteRelation::Synced, true)
                .is_err()
        );
        assert!(validate_repo_git_operation(
            RepoGitOperation::Push,
            RemoteRelation::RemoteAhead,
            true
        )
        .is_err());
        assert!(validate_repo_git_operation(
            RepoGitOperation::Push,
            RemoteRelation::NoRemote,
            true
        )
        .is_err());
        assert!(
            validate_repo_git_operation(RepoGitOperation::Push, RemoteRelation::Error, true)
                .is_err()
        );
    }

    #[test]
    fn repo_status_from_branch_error_marks_repository_as_error() {
        let status =
            repo_status_from_branch_result(sample_repo("broken"), Err("missing".to_string()));

        assert_eq!(status.id, "broken");
        assert_eq!(status.branch, "未知");
        assert_eq!(status.relation, RemoteRelation::Error);
        assert_eq!(status.change_label, "!");
        assert!(status.hint.contains("missing"));
    }

    #[test]
    fn repo_status_from_branch_state_preserves_remote_url_and_labels() {
        let status = repo_status_from_branch_result(
            sample_repo("ready"),
            Ok(GitBranchState {
                branch: "main".to_string(),
                relation: RemoteRelation::RemoteAhead,
                ahead: 0,
                behind: 2,
                has_remote: true,
                remote_url: Some("https://github.com/owner/repo".to_string()),
            }),
        );

        assert_eq!(status.branch, "main");
        assert_eq!(status.relation, RemoteRelation::RemoteAhead);
        assert_eq!(status.change_label, "↓ 2");
        assert!(status.has_remote);
        assert_eq!(
            status.remote_url.as_deref(),
            Some("https://github.com/owner/repo")
        );
    }

    #[test]
    fn sort_repo_statuses_keeps_no_remote_last() {
        let mut statuses = vec![
            repo_status_from_branch_result(
                sample_repo("no-remote"),
                Ok(GitBranchState {
                    branch: "main".to_string(),
                    relation: RemoteRelation::NoRemote,
                    ahead: 0,
                    behind: 0,
                    has_remote: false,
                    remote_url: None,
                }),
            ),
            repo_status_from_branch_result(sample_repo("broken"), Err("missing".to_string())),
            repo_status_from_branch_result(
                sample_repo("synced"),
                Ok(GitBranchState {
                    branch: "main".to_string(),
                    relation: RemoteRelation::Synced,
                    ahead: 0,
                    behind: 0,
                    has_remote: true,
                    remote_url: Some("https://github.com/owner/repo".to_string()),
                }),
            ),
        ];

        sort_repo_statuses(&mut statuses);

        assert_eq!(
            statuses
                .iter()
                .map(|status| status.id.as_str())
                .collect::<Vec<_>>(),
            vec!["broken", "synced", "no-remote"],
        );
    }
}
