use crate::app_settings::load_app_settings;
use crate::domain::repo::RepoStatusDto;
use crate::domain::settings::AppSettings;
use crate::git::commands::{
    branch_state, origin_fetch_args, origin_pull_args, origin_push_args, run_git,
    run_git_args_with_timeout, GIT_NETWORK_TIMEOUT,
};
use crate::git::operation_lock::try_acquire_repo_operation;
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
    // 文件 I/O 移到阻塞线程池，避免占用 async 运行时
    let result = tauri::async_runtime::spawn_blocking(move || load_app_settings(&app))
        .await
        .map_err(|err| err.to_string())?;
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
    // 与 add/remove 共用互斥，避免并发读改写互相覆盖
    let saved = tauri::async_runtime::spawn_blocking(move || {
        crate::app_settings::mutate_app_settings(&app, |current| {
            *current = settings;
            Ok(())
        })
    })
    .await
    .map_err(|err| err.to_string())??;
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
    if crate::git::discovery::contains_lossy_replacement(&repo_path) {
        crate::diagnostics::log("command.add_repository.error", "lossy path characters");
        return Err("仓库路径包含无法正确表示的字符，请检查路径名称".to_string());
    }
    let record = tauri::async_runtime::spawn_blocking(move || -> Result<RepoRecord, String> {
        let mut existing: Option<RepoRecord> = None;
        let saved = crate::app_settings::mutate_app_settings(&app, |settings| {
            if let Some(found) = settings
                .repos
                .iter()
                .find(|repo| repo.path.as_path() == repo_path.as_path())
            {
                existing = Some(found.clone());
                return Ok(());
            }
            let name = repo_path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("unknown")
                .to_string();
            let record = RepoRecord {
                id: repo_id_from_path(&repo_path, settings),
                name,
                path: repo_path.clone(),
                group: "全部分组".to_string(),
            };
            settings.repos.push(record.clone());
            Ok(())
        })?;
        let record = existing.unwrap_or_else(|| {
            saved
                .repos
                .iter()
                .find(|repo| repo.path.as_path() == repo_path.as_path())
                .cloned()
                .expect("added repository must be present in saved settings")
        });
        crate::diagnostics::log(
            "command.add_repository.ok",
            format!(
                "id={} path={}",
                record.id,
                crate::diagnostics::redact_path(&record.path)
            ),
        );
        Ok(record)
    })
    .await
    .map_err(|err| err.to_string())??;
    Ok(record)
}

#[tauri::command]
pub async fn remove_repository(app: tauri::AppHandle, repo_id: String) -> Result<(), String> {
    crate::diagnostics::log(
        "command.remove_repository.start",
        format!("repo_id={repo_id}"),
    );
    tauri::async_runtime::spawn_blocking(move || {
        crate::app_settings::mutate_app_settings(&app, |settings| {
            settings.repos.retain(|repo| repo.id != repo_id);
            for group in &mut settings.groups {
                group.repo_ids.retain(|id| id != &repo_id);
            }
            Ok(())
        })
    })
    .await
    .map_err(|err| err.to_string())??;
    crate::diagnostics::log("command.remove_repository.ok", "");
    Ok(())
}

#[tauri::command]
pub async fn list_repo_statuses(app: tauri::AppHandle) -> Result<Vec<RepoStatusDto>, String> {
    let started = Instant::now();
    let tray_generation = crate::tray_status::begin_tray_menu_update();
    crate::diagnostics::log("command.list_repo_statuses.start", "");
    // 设置加载与状态收集同在阻塞线程池执行
    let app_for_task = app.clone();
    let statuses = tauri::async_runtime::spawn_blocking(move || {
        let settings = load_app_settings(&app_for_task)?;
        crate::diagnostics::log(
            "command.list_repo_statuses.settings",
            format!("repos={}", settings.repos.len()),
        );
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
    // 前端刷新路径同样更新 widget 数据，保证桌面 widget 不落后于窗口/托盘
    let widget_statuses = statuses.clone();
    tauri::async_runtime::spawn_blocking(move || {
        if let Err(err) = crate::widget_data::write_widget_data(&widget_statuses) {
            crate::diagnostics::log("widget_data.write_error", &err);
        }
    });
    crate::diagnostics::log_duration(
        "command.list_repo_statuses.ok",
        started.elapsed(),
        format!("statuses={}", statuses.len()),
    );
    Ok(statuses)
}

#[tauri::command]
pub async fn fetch_repo(app: tauri::AppHandle, repo_id: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let settings = load_app_settings(&app)?;
        let repo = find_repo(&settings, &repo_id)?;
        let repo_path = repo.path.clone();
        let _operation = try_acquire_repo_operation(&repo_path)?;
        let state = branch_state(&repo_path)?;
        validate_repo_git_operation(RepoGitOperation::Fetch, state.relation, state.has_remote)?;
        run_git_args_with_timeout(&repo_path, origin_fetch_args(), GIT_NETWORK_TIMEOUT)
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
    tauri::async_runtime::spawn_blocking(move || {
        let settings = load_app_settings(&app)?;
        let repo = find_repo(&settings, &repo_id)?;
        let repo_path = repo.path.clone();
        let _operation = try_acquire_repo_operation(&repo_path)?;
        let state = branch_state(&repo_path)?;
        validate_repo_git_operation(RepoGitOperation::Pull, state.relation, state.has_remote)?;
        run_git_args_with_timeout(
            &repo_path,
            origin_pull_args(&state.branch),
            GIT_NETWORK_TIMEOUT,
        )
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
    tauri::async_runtime::spawn_blocking(move || {
        let settings = load_app_settings(&app)?;
        let repo = find_repo(&settings, &repo_id)?;
        let repo_path = repo.path.clone();
        let _operation = try_acquire_repo_operation(&repo_path)?;
        let state = branch_state(&repo_path)?;
        validate_repo_git_operation(RepoGitOperation::Push, state.relation, state.has_remote)?;
        run_git_args_with_timeout(
            &repo_path,
            origin_push_args(&state.branch),
            GIT_NETWORK_TIMEOUT,
        )
    })
    .await
    .map_err(|err| err.to_string())??;
    Ok("Push 已完成".to_string())
}

#[tauri::command]
pub async fn open_repo_directory(app: tauri::AppHandle, repo_id: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let settings = load_app_settings(&app)?;
        let repo = find_repo(&settings, &repo_id)?;
        // 校验路径存在
        if !repo.path.exists() {
            return Err("仓库目录不存在".to_string());
        }
        open_directory(&repo.path)
    })
    .await
    .map_err(|err| err.to_string())??;
    Ok(())
}

#[tauri::command]
pub async fn open_repo_remote(app: tauri::AppHandle, repo_id: String) -> Result<(), String> {
    let remote = tauri::async_runtime::spawn_blocking(move || {
        let settings = load_app_settings(&app)?;
        let repo = find_repo(&settings, &repo_id)?;
        run_git(&repo.path, &["config", "--get", "remote.origin.url"])
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
pub async fn save_window_state(app: tauri::AppHandle, x: i32, y: i32) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let path = crate::window_state::window_state_path(&app)?;
        crate::window_state::save_window_position(
            &path,
            &crate::window_state::WindowPosition { x, y },
        )
    })
    .await
    .map_err(|err| err.to_string())??;
    Ok(())
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
