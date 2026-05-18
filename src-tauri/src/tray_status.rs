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
    replace_tray_menu(app, loading_tray_menu)
}

pub fn set_status_menu(app: &AppHandle, statuses: &[RepoStatusDto]) -> Result<(), String> {
    next_tray_menu_generation();
    set_status_menu_inner(app, statuses)
}

fn set_status_menu_inner(app: &AppHandle, statuses: &[RepoStatusDto]) -> Result<(), String> {
    replace_tray_menu(app, |app| status_tray_menu(app, statuses))
}

fn set_error_menu_inner(app: &AppHandle, message: &str) -> Result<(), String> {
    replace_tray_menu(app, |app| error_tray_menu(app, message))
}

fn replace_tray_menu(
    app: &AppHandle,
    build_menu: impl FnOnce(&AppHandle) -> tauri::Result<Menu<tauri::Wry>>,
) -> Result<(), String> {
    let tray = app
        .tray_by_id(MAIN_TRAY_ID)
        .ok_or_else(|| "未找到主托盘图标".to_string())?;
    let menu = build_menu(app).map_err(|err| err.to_string())?;
    tray.set_menu(Some(menu)).map_err(|err| err.to_string())
}

pub fn refresh_tray_menu_async(app: AppHandle) {
    let generation = next_tray_menu_generation();
    if let Err(err) = set_loading_menu(&app) {
        eprintln!("更新托盘读取状态失败: {err}");
    }

    tauri::async_runtime::spawn(async move {
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
                    return;
                }
                if let Err(err) = set_status_menu_inner(&app, &statuses) {
                    eprintln!("更新托盘状态菜单失败: {err}");
                }
            }
            Err(err) => {
                eprintln!("读取托盘仓库状态失败: {err}");
                if !is_current_tray_menu_generation(generation) {
                    return;
                }
                if let Err(menu_err) = set_error_menu_inner(&app, &err) {
                    eprintln!("更新托盘错误菜单失败: {menu_err}");
                }
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
