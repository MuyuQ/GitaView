use crate::domain::repo::RepoStatusDto;
use crate::tray_menu_rows::{
    error_tray_rows, loading_tray_rows, tray_status_rows, TrayMenuRow, TrayMenuRowKind,
};
use std::sync::atomic::{AtomicU64, Ordering};
use tauri::menu::{Menu, MenuBuilder, MenuItem};
use tauri::{AppHandle, Manager, Runtime};

pub const MAIN_TRAY_ID: &str = "main-tray";
pub use crate::tray_menu_rows::{TRAY_QUIT_ID, TRAY_REFRESH_ID, TRAY_SHOW_ID};

static TRAY_MENU_GENERATION: AtomicU64 = AtomicU64::new(0);

fn build_menu_from_rows<R: Runtime, M: Manager<R>>(
    manager: &M,
    rows: &[TrayMenuRow],
) -> tauri::Result<Menu<R>> {
    let mut builder = MenuBuilder::new(manager);
    for (index, row) in rows.iter().enumerate() {
        match &row.kind {
            TrayMenuRowKind::Display => {
                let item = MenuItem::with_id(
                    manager,
                    format!("tray-display-{index}"),
                    &row.text,
                    false,
                    None::<&str>,
                )?;
                builder = builder.item(&item);
            }
            TrayMenuRowKind::Separator => {
                builder = builder.separator();
            }
            TrayMenuRowKind::Action { id } => {
                let item = MenuItem::with_id(manager, *id, &row.text, true, None::<&str>)?;
                builder = builder.item(&item);
            }
        }
    }
    builder.build()
}

pub fn loading_tray_menu<R: Runtime, M: Manager<R>>(manager: &M) -> tauri::Result<Menu<R>> {
    build_menu_from_rows(manager, &loading_tray_rows())
}

pub fn status_tray_menu<R: Runtime, M: Manager<R>>(
    manager: &M,
    statuses: &[RepoStatusDto],
) -> tauri::Result<Menu<R>> {
    build_menu_from_rows(manager, &tray_status_rows(statuses))
}

fn error_tray_menu<R: Runtime, M: Manager<R>>(
    manager: &M,
    message: &str,
) -> tauri::Result<Menu<R>> {
    build_menu_from_rows(manager, &error_tray_rows(message))
}

pub fn set_loading_menu(app: &AppHandle) -> Result<(), String> {
    crate::diagnostics::log("tray.set_loading.start", "");
    replace_tray_menu(app, loading_tray_menu)
}

pub fn set_status_menu(app: &AppHandle, statuses: &[RepoStatusDto]) -> Result<(), String> {
    begin_tray_menu_update();
    set_status_menu_inner(app, statuses)
}

pub fn begin_tray_menu_update() -> u64 {
    next_tray_menu_generation()
}

pub fn set_status_menu_if_current(
    app: &AppHandle,
    generation: u64,
    statuses: &[RepoStatusDto],
) -> Result<bool, String> {
    if !is_current_tray_menu_generation(generation) {
        return Ok(false);
    }
    set_status_menu_inner(app, statuses)?;
    Ok(true)
}

fn set_status_menu_inner(app: &AppHandle, statuses: &[RepoStatusDto]) -> Result<(), String> {
    crate::diagnostics::log(
        "tray.set_status.start",
        format!("statuses={}", statuses.len()),
    );
    replace_tray_menu(app, |app| status_tray_menu(app, statuses))
}

fn set_error_menu_inner(app: &AppHandle, message: &str) -> Result<(), String> {
    crate::diagnostics::log(
        "tray.set_error.start",
        format!("error_len={}", message.len()),
    );
    replace_tray_menu(app, |app| error_tray_menu(app, message))
}

fn replace_tray_menu(
    app: &AppHandle,
    build_menu: impl FnOnce(&AppHandle) -> tauri::Result<Menu<tauri::Wry>>,
) -> Result<(), String> {
    crate::diagnostics::log("tray.replace.start", "");
    let tray = app
        .tray_by_id(MAIN_TRAY_ID)
        .ok_or_else(|| "未找到主托盘图标".to_string())?;
    let menu = build_menu(app).map_err(|err| err.to_string())?;
    let result = tray.set_menu(Some(menu)).map_err(|err| err.to_string());
    match &result {
        Ok(()) => crate::diagnostics::log("tray.replace.ok", ""),
        Err(err) => {
            crate::diagnostics::log("tray.replace.error", format!("error_len={}", err.len()))
        }
    }
    result
}

pub fn refresh_tray_menu_async(app: AppHandle) {
    let generation = begin_tray_menu_update();
    crate::diagnostics::log("tray.refresh.schedule", format!("generation={generation}"));
    if let Err(err) = set_loading_menu(&app) {
        eprintln!("更新托盘读取状态失败: {err}");
        crate::diagnostics::log(
            "tray.refresh.loading_error",
            format!("error_len={}", err.len()),
        );
    }

    tauri::async_runtime::spawn(async move {
        let started = std::time::Instant::now();
        crate::diagnostics::log("tray.refresh.start", format!("generation={generation}"));
        let result = match crate::app_settings::load_app_settings(&app) {
            Ok(settings) => tauri::async_runtime::spawn_blocking(move || {
                crate::repo_status::collect_repo_statuses(settings.repos)
            })
            .await
            .map_err(|err| err.to_string())
            .and_then(|inner| inner),
            Err(err) => Err(err),
        };

        match result {
            Ok(statuses) => {
                if !is_current_tray_menu_generation(generation) {
                    crate::diagnostics::log(
                        "tray.refresh.stale",
                        format!("generation={generation} statuses={}", statuses.len()),
                    );
                    return;
                }
                if let Err(err) = set_status_menu_inner(&app, &statuses) {
                    eprintln!("更新托盘状态菜单失败: {err}");
                    crate::diagnostics::log(
                        "tray.refresh.status_error",
                        format!("error_len={}", err.len()),
                    );
                }

                // 异步写入 widget 数据，不阻塞 tray 刷新
                let statuses_clone = statuses.clone();
                tauri::async_runtime::spawn_blocking(move || {
                    if let Err(err) = crate::widget_data::write_widget_data(&statuses_clone) {
                        crate::diagnostics::log("widget_data.write_error", &err);
                    }
                });

                crate::diagnostics::log_duration(
                    "tray.refresh.ok",
                    started.elapsed(),
                    format!("generation={generation} statuses={}", statuses.len()),
                );
            }
            Err(err) => {
                eprintln!("读取托盘仓库状态失败: {err}");
                if !is_current_tray_menu_generation(generation) {
                    crate::diagnostics::log(
                        "tray.refresh.error_stale",
                        format!("generation={generation} error_len={}", err.len()),
                    );
                    return;
                }
                if let Err(menu_err) = set_error_menu_inner(&app, &err) {
                    eprintln!("更新托盘错误菜单失败: {menu_err}");
                    crate::diagnostics::log(
                        "tray.refresh.error_menu_error",
                        format!("error_len={}", menu_err.len()),
                    );
                }
                crate::diagnostics::log_duration(
                    "tray.refresh.error",
                    started.elapsed(),
                    format!("generation={generation} error_len={}", err.len()),
                );
            }
        }
    });
}

fn next_tray_menu_generation() -> u64 {
    TRAY_MENU_GENERATION.fetch_add(1, Ordering::AcqRel) + 1
}

fn is_current_tray_menu_generation(generation: u64) -> bool {
    TRAY_MENU_GENERATION.load(Ordering::Acquire) == generation
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_generation_marks_older_refreshes_as_stale() {
        let older = next_tray_menu_generation();
        let newer = next_tray_menu_generation();

        assert!(!is_current_tray_menu_generation(older));
        assert!(is_current_tray_menu_generation(newer));
    }
}
