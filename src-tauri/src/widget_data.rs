use crate::domain::repo::RepoStatusDto;
use crate::domain::status::RemoteRelation;
use serde::Serialize;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use time::macros::format_description;
use time::OffsetDateTime;

// FFI 桥接：通知 WidgetKit 刷新时间线（仅 macOS）
#[cfg(target_os = "macos")]
extern "C" {
    fn reload_widget_timelines();
}

/// 防抖：上次刷新时间
static LAST_REFRESH: Mutex<Option<Instant>> = Mutex::new(None);
const DEBOUNCE_DURATION: Duration = Duration::from_secs(5);

/// Widget 数据 JSON 格式
#[derive(Debug, Serialize)]
struct WidgetPayload {
    version: u32,
    last_updated: String,
    repos: Vec<WidgetRepo>,
    summary: WidgetSummary,
}

#[derive(Debug, Serialize)]
struct WidgetRepo {
    id: String,
    name: String,
    group: String,
    branch: String,
    relation: String,
    change_label: String,
    hint: String,
}

#[derive(Debug, Serialize)]
struct WidgetSummary {
    synced: u32,
    local_ahead: u32,
    remote_ahead: u32,
    diverged: u32,
    no_remote: u32,
    total: u32,
}

/// 获取 widget-data.json 的路径
fn widget_data_path() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
    home.join("Library")
        .join("Application Support")
        .join("GitaView")
        .join("widget-data.json")
}

/// 通知 WidgetKit 刷新（仅 macOS）
#[cfg(target_os = "macos")]
fn notify_widget_refresh() {
    unsafe {
        reload_widget_timelines();
    }
}

#[cfg(not(target_os = "macos"))]
fn notify_widget_refresh() {
    // Windows/Linux 不支持 WidgetKit，静默跳过
}

/// 写入 widget 数据（带防抖）
pub fn write_widget_data(statuses: &[RepoStatusDto]) -> Result<(), String> {
    // 先检查防抖，尽快释放锁
    {
        let now = Instant::now();
        let mut last = LAST_REFRESH.lock().map_err(|e| e.to_string())?;

        if let Some(last_time) = *last {
            if now.duration_since(last_time) < DEBOUNCE_DURATION {
                return Ok(());
            }
        }
        *last = Some(now);
    }
    // 锁已释放，执行 I/O 操作

    write_widget_data_impl(statuses)
}

fn write_widget_data_impl(statuses: &[RepoStatusDto]) -> Result<(), String> {
    let path = widget_data_path();

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {e}"))?;
    }

    let payload = build_payload(statuses);
    let json = serde_json::to_string_pretty(&payload)
        .map_err(|e| format!("JSON 序列化失败: {e}"))?;

    // 原子写入
    let temp_path = path.with_extension("json.tmp");
    fs::write(&temp_path, &json).map_err(|e| format!("写入临时文件失败: {e}"))?;
    fs::rename(&temp_path, &path).map_err(|e| format!("重命名文件失败: {e}"))?;

    notify_widget_refresh();

    Ok(())
}

fn build_payload(statuses: &[RepoStatusDto]) -> WidgetPayload {
    let now = OffsetDateTime::now_utc();
    let format = format_description!("[year]-[month]-[day]T[hour]:[minute]:[second]Z");
    let last_updated = now.format(&format).unwrap_or_default();

    // 单次遍历计算所有统计
    let mut synced = 0u32;
    let mut local_ahead = 0u32;
    let mut remote_ahead = 0u32;
    let mut diverged = 0u32;
    let mut no_remote = 0u32;

    let repos: Vec<WidgetRepo> = statuses
        .iter()
        .map(|s| {
            match s.relation {
                RemoteRelation::Synced => synced += 1,
                RemoteRelation::LocalAhead => local_ahead += 1,
                RemoteRelation::RemoteAhead => remote_ahead += 1,
                RemoteRelation::Diverged => diverged += 1,
                RemoteRelation::NoRemote => no_remote += 1,
                RemoteRelation::Error => {} // 不计入统计
            }

            WidgetRepo {
                id: s.id.clone(),
                name: s.name.clone(),
                group: s.group.clone(),
                branch: s.branch.clone(),
                relation: format!("{:?}", s.relation).to_lowercase(),
                change_label: s.change_label.clone(),
                hint: s.hint.clone(),
            }
        })
        .collect();

    WidgetPayload {
        version: 1,
        last_updated,
        repos,
        summary: WidgetSummary {
            synced,
            local_ahead,
            remote_ahead,
            diverged,
            no_remote,
            total: statuses.len() as u32,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::repo::RepoStatusDto;
    use crate::domain::status::RemoteRelation;

    fn make_status(id: &str, relation: RemoteRelation) -> RepoStatusDto {
        RepoStatusDto {
            id: id.to_string(),
            name: id.to_string(),
            path: "/tmp/test".to_string(),
            group: "默认分组".to_string(),
            branch: "main".to_string(),
            relation,
            change_label: "clean".to_string(),
            hint: String::new(),
            has_remote: true,
            remote_url: None,
        }
    }

    #[test]
    fn test_build_payload_summary_counts() {
        let statuses = vec![
            make_status("a", RemoteRelation::Synced),
            make_status("b", RemoteRelation::Synced),
            make_status("c", RemoteRelation::LocalAhead),
            make_status("d", RemoteRelation::Diverged),
            make_status("e", RemoteRelation::NoRemote),
        ];
        let payload = build_payload(&statuses);
        assert_eq!(payload.version, 1);
        assert_eq!(payload.summary.total, 5);
        assert_eq!(payload.summary.synced, 2);
        assert_eq!(payload.summary.local_ahead, 1);
        assert_eq!(payload.summary.diverged, 1);
        assert_eq!(payload.summary.no_remote, 1);
        assert_eq!(payload.repos.len(), 5);
    }

    #[test]
    fn test_widget_data_path_format() {
        let path = widget_data_path();
        assert!(path.ends_with("GitaView/widget-data.json"));
    }

    #[test]
    fn test_build_payload_with_error_relation() {
        let statuses = vec![
            make_status("a", RemoteRelation::Synced),
            make_status("b", RemoteRelation::Error),
        ];
        let payload = build_payload(&statuses);
        assert_eq!(payload.summary.total, 2);
        assert_eq!(payload.summary.synced, 1);
        assert_eq!(payload.summary.local_ahead, 0);
        assert_eq!(payload.summary.diverged, 0);
    }

    #[test]
    fn test_build_payload_empty_statuses() {
        let statuses: Vec<RepoStatusDto> = vec![];
        let payload = build_payload(&statuses);
        assert_eq!(payload.version, 1);
        assert_eq!(payload.summary.total, 0);
        assert_eq!(payload.summary.synced, 0);
        assert!(payload.repos.is_empty());
    }

    #[test]
    fn test_build_payload_preserves_repo_fields() {
        let statuses = vec![RepoStatusDto {
            id: "test-id".to_string(),
            name: "test-repo".to_string(),
            path: "/path/to/repo".to_string(),
            group: "业务".to_string(),
            branch: "feature/test".to_string(),
            relation: RemoteRelation::LocalAhead,
            change_label: "3 ahead".to_string(),
            hint: "Push 3 commits".to_string(),
            has_remote: true,
            remote_url: Some("https://github.com/test/repo".to_string()),
        }];
        let payload = build_payload(&statuses);
        let repo = &payload.repos[0];
        assert_eq!(repo.id, "test-id");
        assert_eq!(repo.name, "test-repo");
        assert_eq!(repo.group, "业务");
        assert_eq!(repo.branch, "feature/test");
        assert_eq!(repo.relation, "localahead");
        assert_eq!(repo.change_label, "3 ahead");
        assert_eq!(repo.hint, "Push 3 commits");
    }
}
