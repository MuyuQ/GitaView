use crate::domain::repo::RepoStatusDto;
use crate::domain::status::RemoteRelation;
use serde::Serialize;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use time::macros::format_description;
use time::OffsetDateTime;

// FFI 桥接：通知 WidgetKit 刷新时间线（仅 macOS）
#[cfg(target_os = "macos")]
extern "C" {
    fn reload_widget_timelines();
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

/// 写入 widget 数据（仅 macOS 生效；其他平台为空操作，避免产生无人读取的文件）
pub fn write_widget_data(statuses: &[RepoStatusDto]) -> Result<(), String> {
    #[cfg(not(target_os = "macos"))]
    {
        let _ = statuses;
        Ok(())
    }

    #[cfg(target_os = "macos")]
    enqueue_widget_write(widget_data_path(), statuses.to_vec())
}

/// 获取 widget-data.json 的路径（macOS 专用，与 WidgetKit 扩展共享）
#[cfg(target_os = "macos")]
fn widget_data_path() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
    home.join("Library")
        .join("Application Support")
        .join("GitaView")
        .join("widget-data.json")
}

const DEBOUNCE_DURATION: Duration = Duration::from_secs(5);

/// 写入状态机：pending 始终保存最新数据，防抖改为 trailing-edge，
/// 保证防抖窗口内到达的新数据不会被静默丢弃。
struct WidgetWriteState {
    last_write: Option<Instant>,
    pending: Option<Vec<RepoStatusDto>>,
    flush_scheduled: bool,
}

static WRITE_STATE: Mutex<WidgetWriteState> = Mutex::new(WidgetWriteState {
    last_write: None,
    pending: None,
    flush_scheduled: false,
});

/// 落盘互斥：同一时刻只允许一个写入者，且写入者在自己回合开始时
/// 取走当前最新的 pending —— 结构上保证文件永远不会被更旧的数据回写。
/// WRITE_STATE 锁只保护元数据读写，绝不跨文件 I/O。
static WRITE_IO_LOCK: Mutex<()> = Mutex::new(());

fn lock_write_io() -> std::sync::MutexGuard<'static, ()> {
    WRITE_IO_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

enum FlushDecision {
    Now,
    Defer(Duration),
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn flush_decision(last_write: Option<Instant>, now: Instant) -> FlushDecision {
    match last_write {
        Some(last) => {
            let elapsed = now.saturating_duration_since(last);
            if elapsed >= DEBOUNCE_DURATION {
                FlushDecision::Now
            } else {
                FlushDecision::Defer(DEBOUNCE_DURATION - elapsed)
            }
        }
        None => FlushDecision::Now,
    }
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn enqueue_widget_write(path: PathBuf, statuses: Vec<RepoStatusDto>) -> Result<(), String> {
    let now = Instant::now();
    {
        let mut state = WRITE_STATE.lock().map_err(|e| e.to_string())?;
        state.pending = Some(statuses);
        match flush_decision(state.last_write, now) {
            FlushDecision::Defer(remaining) => {
                // trailing-edge：窗口结束后由延迟写入者补写最新数据
                if !state.flush_scheduled {
                    state.flush_scheduled = true;
                    std::thread::spawn(move || {
                        std::thread::sleep(remaining);
                        if let Ok(mut state) = WRITE_STATE.lock() {
                            state.flush_scheduled = false;
                        }
                        if let Err(err) = perform_widget_write(path) {
                            crate::diagnostics::log("widget_data.deferred_write_error", &err);
                        }
                    });
                }
                return Ok(());
            }
            FlushDecision::Now => {}
        }
    }
    // WRITE_STATE 已释放；实际落盘在 I/O 锁内取走最新 pending
    perform_widget_write(path)
}

/// 串行化落盘：取走最新 pending 写入；失败时若期间没有更新的数据入队，
/// 则保留待写数据等待下次重试。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn perform_widget_write(path: PathBuf) -> Result<(), String> {
    let _io = lock_write_io();
    let pending = {
        let mut state = WRITE_STATE.lock().map_err(|e| e.to_string())?;
        state.pending.take()
    };
    let Some(statuses) = pending else {
        return Ok(());
    };
    let result = write_widget_data_to(&path, &statuses);
    match &result {
        Ok(()) => {
            if let Ok(mut state) = WRITE_STATE.lock() {
                state.last_write = Some(Instant::now());
            }
            notify_widget_refresh();
        }
        Err(_) => {
            if let Ok(mut state) = WRITE_STATE.lock() {
                if state.pending.is_none() {
                    state.pending = Some(statuses);
                }
            }
        }
    }
    result
}

/// 原子写入：临时文件 + fsync + rename，失败时清理临时文件
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn write_widget_data_to(path: &Path, statuses: &[RepoStatusDto]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {e}"))?;
    }

    let payload = build_payload(statuses);
    let json =
        serde_json::to_string_pretty(&payload).map_err(|e| format!("JSON 序列化失败: {e}"))?;

    let temp_path = path.with_extension("json.tmp");
    let write_result = (|| -> std::io::Result<()> {
        let mut file = fs::File::create(&temp_path)?;
        file.write_all(json.as_bytes())?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temp_path, path)
    })();
    if let Err(err) = write_result {
        let _ = fs::remove_file(&temp_path);
        return Err(format!("写入 widget 数据失败: {err}"));
    }

    Ok(())
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
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
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// 涉及全局 WRITE_STATE 的测试用它串行执行
    static WIDGET_TEST_SERIAL: Mutex<()> = Mutex::new(());

    fn unique_temp_dir(name: &str) -> std::path::PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("{name}_{suffix}"))
    }

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

    #[cfg(target_os = "macos")]
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

    #[test]
    fn flush_decision_writes_immediately_without_history() {
        assert!(matches!(
            flush_decision(None, Instant::now()),
            FlushDecision::Now
        ));
    }

    #[test]
    fn flush_decision_defers_within_debounce_window() {
        let now = Instant::now();
        assert!(matches!(
            flush_decision(Some(now - Duration::from_secs(2)), now),
            FlushDecision::Defer(d) if d <= DEBOUNCE_DURATION && d > Duration::from_secs(2)
        ));
    }

    #[test]
    fn flush_decision_flushes_after_debounce_window() {
        let now = Instant::now();
        assert!(matches!(
            flush_decision(Some(now - DEBOUNCE_DURATION), now),
            FlushDecision::Now
        ));
    }

    #[test]
    fn write_widget_data_to_writes_atomic_json_and_leaves_no_temp_file() {
        let temp = unique_temp_dir("gitaview_widget_write_test");
        let path = temp.join("widget-data.json");
        let statuses = vec![make_status("a", RemoteRelation::Synced)];

        write_widget_data_to(&path, &statuses).expect("widget write should succeed");

        let text = fs::read_to_string(&path).expect("widget data file should exist");
        let value: serde_json::Value = serde_json::from_str(&text).expect("valid JSON");
        assert_eq!(value["version"], 1);
        assert_eq!(value["summary"]["total"], 1);
        assert_eq!(value["summary"]["synced"], 1);
        assert!(!temp.join("widget-data.json.tmp").exists());

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn enqueue_widget_write_defers_within_window_then_trailing_flush_writes_latest() {
        let _serial = WIDGET_TEST_SERIAL.lock().unwrap();
        let temp = unique_temp_dir("gitaview_widget_defer_test");
        let path = temp.join("nested").join("widget-data.json");

        // 模拟刚写过：进入防抖窗口，数据只入队，不落盘
        reset_widget_write_state_for_tests(Some(Instant::now()));
        enqueue_widget_write(path.clone(), vec![make_status("a", RemoteRelation::Synced)])
            .expect("enqueue should succeed");
        assert!(!path.exists(), "deferred write must not land immediately");

        // 手动触发 trailing flush：写入的是入队的最新数据
        perform_widget_write(path.clone()).expect("trailing flush should succeed");
        let text = fs::read_to_string(&path).expect("widget data file should exist");
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["summary"]["total"], 1);

        // 恢复全局状态，避免影响其他测试
        reset_widget_write_state_for_tests(None);
        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn enqueue_widget_write_flushes_immediately_without_history() {
        let _serial = WIDGET_TEST_SERIAL.lock().unwrap();
        let temp = unique_temp_dir("gitaview_widget_immediate_test");
        let path = temp.join("widget-data.json");

        reset_widget_write_state_for_tests(None);
        enqueue_widget_write(path.clone(), vec![make_status("a", RemoteRelation::Synced)])
            .expect("enqueue should succeed");

        assert!(path.exists(), "immediate write must land right away");
        let text = fs::read_to_string(&path).expect("widget data file should exist");
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["summary"]["total"], 1);

        {
            let state = WRITE_STATE.lock().unwrap();
            assert!(state.pending.is_none(), "pending must be consumed");
            assert!(state.last_write.is_some(), "last_write must be recorded");
        }

        reset_widget_write_state_for_tests(None);
        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn perform_widget_write_without_pending_is_a_noop() {
        let _serial = WIDGET_TEST_SERIAL.lock().unwrap();
        let temp = unique_temp_dir("gitaview_widget_noop_flush_test");
        let path = temp.join("widget-data.json");

        reset_widget_write_state_for_tests(None);
        perform_widget_write(path.clone()).expect("no pending should be a no-op");

        assert!(!path.exists());
        reset_widget_write_state_for_tests(None);
        let _ = fs::remove_dir_all(&temp);
    }

    /// 测试间隔离全局状态：记录 last_write 供防抖判定，其余清零。
    /// WRITE_STATE 是进程级全局状态，涉及它的测试必须先取 WIDGET_TEST_SERIAL。
    fn reset_widget_write_state_for_tests(last_write: Option<Instant>) {
        let mut state = WRITE_STATE.lock().unwrap();
        state.last_write = last_write;
        state.pending = None;
        state.flush_scheduled = false;
    }

    #[test]
    fn write_widget_data_is_a_no_op_off_macos() {
        // 非 macOS 平台不得创建任何文件或目录
        let temp = unique_temp_dir("gitaview_widget_noop_test");
        write_widget_data(&[]).expect("write_widget_data should not fail off macOS");
        #[cfg(not(target_os = "macos"))]
        assert!(!temp.exists(), "no files should be created off macOS");
        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn widget_payload_relation_strings_stay_lowercase_serializable() {
        let statuses = vec![make_status("a", RemoteRelation::Diverged)];
        let payload = build_payload(&statuses);
        assert_eq!(payload.repos[0].relation, "diverged");
    }
}
