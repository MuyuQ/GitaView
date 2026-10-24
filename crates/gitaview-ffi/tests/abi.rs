//! N09 正式 ABI 测试：通过 C 边界调用全部导出函数，验证状态码、envelope、
//! 缓冲区生命周期、destroy 语义与 Unicode。真实 Git 临时仓库，不访问公网。

use gitaview_ffi::{
    gv_abi_version, gv_buffer_free, gv_engine_create, gv_engine_destroy, gv_engine_request_stop,
    gv_engine_wait_stopped, gv_refresh_request, gv_repo_add, gv_repo_pull, gv_repo_remove,
    gv_repo_target, gv_scan_directory, gv_settings_get, gv_settings_save, gv_snapshot_get,
    gv_snapshot_wait, gv_view_query, gv_widget_snapshot, GvBuffer, GvEngine, GV_BUSY, GV_ERROR,
    GV_INVALID_ARGUMENT, GV_OK, GV_STOPPING, GV_TIMEOUT,
};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

fn unique_temp_dir(name: &str) -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("{name}_{suffix}"))
}

fn run_git(repo_path: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn create_input(temp: &Path) -> String {
    serde_json::json!({
        "dataDirectory": temp.to_string_lossy(),
        "logDirectory": temp.join("logs").to_string_lossy(),
    })
    .to_string()
}

/// 创建引擎并返回 (句柄, create data)。
fn create_engine(temp: &Path) -> (*mut GvEngine, Value) {
    let input = create_input(temp);
    let mut engine: *mut GvEngine = std::ptr::null_mut();
    let mut reply = GvBuffer::empty();
    let status = unsafe { gv_engine_create(input.as_ptr(), input.len(), &mut engine, &mut reply) };
    let payload = take_payload(&mut reply);
    assert_eq!(status, GV_OK, "create failed: {:?}", payload);
    let envelope: Value = serde_json::from_slice(&payload.expect("create writes reply")).unwrap();
    (engine, envelope["data"].clone())
}

/// 跨测试引擎仍向首个日志目录写入（diagnostics 首引擎生效）：删除需重试。
fn remove_dir_retry(path: &Path) {
    for _ in 0..20 {
        if fs::remove_dir_all(path).is_ok() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    panic!("remove_dir_all failed: {}", path.display());
}

fn take_payload(reply: &mut GvBuffer) -> Option<Vec<u8>> {
    if reply.data.is_null() {
        None
    } else {
        Some(unsafe { std::slice::from_raw_parts(reply.data, reply.len) }.to_vec())
    }
}

/// 调用返回 (状态, envelope JSON 或 None)。缓冲区总是释放。
fn call_result(status: i32, reply: &mut GvBuffer) -> (i32, Option<Value>) {
    let payload = take_payload(reply);
    unsafe { gv_buffer_free(reply) };
    let envelope =
        payload.map(|bytes| serde_json::from_slice::<Value>(&bytes).expect("valid JSON envelope"));
    (status, envelope)
}

fn stop_and_destroy(engine: *mut GvEngine) {
    let mut reply = GvBuffer::empty();
    unsafe { gv_engine_request_stop(engine, &mut reply) };
    let _ = take_payload(&mut reply);
    unsafe { gv_buffer_free(&mut reply) };
    let mut reply = GvBuffer::empty();
    let status = unsafe { gv_engine_wait_stopped(engine, 30_000, &mut reply) };
    let _ = call_result(status, &mut reply);
    assert_eq!(status, GV_OK);
    let mut handle = engine;
    assert_eq!(unsafe { gv_engine_destroy(&mut handle) }, GV_OK);
    assert!(handle.is_null());
}

fn poll_until(deadline_ms: u64, check: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_millis(deadline_ms);
    while !check() {
        assert!(Instant::now() < deadline, "condition not reached in time");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn snapshot_of(engine: *mut GvEngine) -> Value {
    let mut reply = GvBuffer::empty();
    let (status, envelope) =
        unsafe { call_result(gv_snapshot_get(engine, &mut reply), &mut reply) };
    assert_eq!(status, GV_OK);
    envelope.unwrap()["data"].clone()
}

#[test]
fn abi_version_is_one() {
    assert_eq!(gv_abi_version(), 1);
}

#[test]
fn engine_create_returns_session_and_defaults() {
    let temp = unique_temp_dir("gitaview_n09_create");
    let (engine, data) = create_engine(&temp);

    assert_eq!(data["abiVersion"], 1);
    assert!(data["sessionId"].as_str().is_some_and(|s| !s.is_empty()));
    assert_eq!(data["startupWarnings"], serde_json::json!([]));
    // 首次启动为内存默认值；settings.json 在第一次变更时才落盘（N06 语义）。
    stop_and_destroy(engine);
    remove_dir_retry(&temp);
}

#[test]
fn engine_create_failures_keep_engine_null() {
    let temp = unique_temp_dir("gitaview_n09_create_fail");
    fs::create_dir_all(&temp).unwrap();

    for bad in ["{", "null", "{\"dataDirectory\": 42}"] {
        let mut engine: *mut GvEngine = std::ptr::null_mut();
        let mut reply = GvBuffer::empty();
        let (status, envelope) = unsafe {
            let status = gv_engine_create(bad.as_ptr(), bad.len(), &mut engine, &mut reply);
            call_result(status, &mut reply)
        };
        assert_eq!(status, GV_INVALID_ARGUMENT, "input: {bad}");
        assert!(engine.is_null(), "failed create must leave engine null");
        assert_eq!(envelope.unwrap()["error"]["code"], "INVALID_ARGUMENT");
    }

    // 相对路径 → INVALID_ARGUMENT。
    let relative = r#"{"dataDirectory":"relative/dir","logDirectory":"logs"}"#;
    let mut engine: *mut GvEngine = std::ptr::null_mut();
    let mut reply = GvBuffer::empty();
    let (status, envelope) = unsafe {
        let status = gv_engine_create(relative.as_ptr(), relative.len(), &mut engine, &mut reply);
        call_result(status, &mut reply)
    };
    assert_eq!(status, GV_INVALID_ARGUMENT);
    assert!(engine.is_null());
    assert_eq!(envelope.unwrap()["error"]["code"], "INVALID_ARGUMENT");

    remove_dir_retry(&temp);
}

#[test]
fn settings_get_and_save_with_conflict_mapping() {
    let temp = unique_temp_dir("gitaview_n09_settings");
    let (engine, _) = create_engine(&temp);

    let mut reply = GvBuffer::empty();
    let (status, envelope) =
        unsafe { call_result(gv_settings_get(engine, &mut reply), &mut reply) };
    assert_eq!(status, GV_OK);
    let data = envelope.unwrap()["data"].clone();
    assert_eq!(data["revision"], 1);
    assert_eq!(data["repos"], serde_json::json!([]));

    // 保存：建组。
    let save = serde_json::json!({
        "expectedRevision": 1,
        "settings": {
            "groups": [{"id": "g1", "name": "后端"}],
            "defaultGroupId": "g1",
            "refresh": {"enabled": true, "intervalMinutes": 10},
            "repoGroups": []
        }
    })
    .to_string();
    let mut reply = GvBuffer::empty();
    let (status, envelope) = unsafe {
        call_result(
            gv_settings_save(engine, save.as_ptr(), save.len(), &mut reply),
            &mut reply,
        )
    };
    assert_eq!(status, GV_OK);
    assert_eq!(envelope.unwrap()["data"]["revision"], 2);

    // 过期 revision → SETTINGS_CONFLICT（业务错误 → ERROR 状态 + envelope code）。
    let stale = serde_json::json!({
        "expectedRevision": 1,
        "settings": {"groups": [], "defaultGroupId": null,
                     "refresh": {"enabled": true, "intervalMinutes": 5}, "repoGroups": []}
    })
    .to_string();
    let mut reply = GvBuffer::empty();
    let (status, envelope) = unsafe {
        call_result(
            gv_settings_save(engine, stale.as_ptr(), stale.len(), &mut reply),
            &mut reply,
        )
    };
    assert_eq!(status, GV_ERROR, "expected business error maps to ERROR");
    assert_eq!(envelope.unwrap()["error"]["code"], "SETTINGS_CONFLICT");

    stop_and_destroy(engine);
    remove_dir_retry(&temp);
}

#[test]
fn repo_add_remove_scan_and_targets() {
    let temp = unique_temp_dir("gitaview_n09_repos");
    let (engine, _) = create_engine(&temp);

    // 真实 Git 仓库（中文路径），带 https origin。
    let repo = temp.join("仓库-示例");
    fs::create_dir_all(&repo).unwrap();
    run_git(&repo, &["init", "-b", "main"]);
    run_git(&repo, &["config", "user.email", "test@example.test"]);
    run_git(&repo, &["config", "user.name", "Test"]);
    run_git(&repo, &["commit", "--allow-empty", "-m", "初始提交"]);
    run_git(
        &repo,
        &[
            "remote",
            "add",
            "origin",
            "https://github.com/example/x.git",
        ],
    );

    let add = serde_json::json!({ "path": repo.to_string_lossy(), "groupId": null }).to_string();
    let mut reply = GvBuffer::empty();
    let (status, envelope) = unsafe {
        call_result(
            gv_repo_add(engine, add.as_ptr(), add.len(), &mut reply),
            &mut reply,
        )
    };
    assert_eq!(status, GV_OK);
    let data = envelope.unwrap()["data"].clone();
    // ID 是稳定机器标识：非 ASCII 字母数字折叠为 '-'，中文名目录折叠为 "repo"。
    let repo_id = data["repo"]["id"].as_str().unwrap().to_string();
    assert_eq!(repo_id, "repo");
    assert_eq!(data["settingsRevision"], 2);

    // 重复添加：同一记录，revision 不变。
    let mut reply = GvBuffer::empty();
    let (status, envelope) = unsafe {
        call_result(
            gv_repo_add(engine, add.as_ptr(), add.len(), &mut reply),
            &mut reply,
        )
    };
    assert_eq!(status, GV_OK);
    assert_eq!(envelope.unwrap()["data"]["settingsRevision"], 2);

    // 快照最终读到该仓库（异步首扫 / 重扫）：行存在且读取完成。
    poll_until(10_000, || {
        snapshot_of(engine)["repos"]
            .as_array()
            .is_some_and(|repos| repos.len() == 1 && repos[0]["readState"] == "ok")
    });
    let snapshot = snapshot_of(engine);
    assert_eq!(snapshot["repos"][0]["readState"], "ok");
    assert_eq!(snapshot["repos"][0]["relation"], "no_remote");
    assert_eq!(snapshot["repos"][0]["hasOrigin"], true);

    // 目录扫描：发现候选，不自动添加。
    let scan = serde_json::json!({ "path": temp.to_string_lossy() }).to_string();
    let mut reply = GvBuffer::empty();
    let (status, envelope) = unsafe {
        call_result(
            gv_scan_directory(engine, scan.as_ptr(), scan.len(), &mut reply),
            &mut reply,
        )
    };
    assert_eq!(status, GV_OK);
    let scan_data = envelope.unwrap()["data"].clone();
    assert_eq!(scan_data["truncated"], false);
    assert_eq!(scan_data["paths"].as_array().unwrap().len(), 1);

    // 打开目标：directory 复核存在；remote 返回安全 URL。
    let target_dir = serde_json::json!({"repoId": repo_id, "kind": "directory"}).to_string();
    let mut reply = GvBuffer::empty();
    let (status, envelope) = unsafe {
        call_result(
            gv_repo_target(engine, target_dir.as_ptr(), target_dir.len(), &mut reply),
            &mut reply,
        )
    };
    assert_eq!(status, GV_OK);
    assert_eq!(envelope.unwrap()["data"]["kind"], "directory");

    let target_remote = serde_json::json!({"repoId": repo_id, "kind": "remote"}).to_string();
    let mut reply = GvBuffer::empty();
    let (status, envelope) = unsafe {
        call_result(
            gv_repo_target(
                engine,
                target_remote.as_ptr(),
                target_remote.len(),
                &mut reply,
            ),
            &mut reply,
        )
    };
    assert_eq!(status, GV_OK);
    assert_eq!(
        envelope.unwrap()["data"]["value"],
        "https://github.com/example/x"
    );

    // 未知 kind → INVALID_ARGUMENT。
    let target_bad = r#"{"repoId":"x","kind":"shell"}"#;
    let mut reply = GvBuffer::empty();
    let (status, _) = unsafe {
        call_result(
            gv_repo_target(engine, target_bad.as_ptr(), target_bad.len(), &mut reply),
            &mut reply,
        )
    };
    assert_eq!(status, GV_INVALID_ARGUMENT);

    // 移除：原始 UTF-8 ID 字节（非 JSON）。
    let id_bytes = repo_id.as_bytes();
    let mut reply = GvBuffer::empty();
    let (status, envelope) = unsafe {
        call_result(
            gv_repo_remove(engine, id_bytes.as_ptr(), id_bytes.len(), &mut reply),
            &mut reply,
        )
    };
    assert_eq!(status, GV_OK);
    assert_eq!(envelope.unwrap()["data"]["settingsRevision"], 3);

    // 再移除 → REPO_NOT_FOUND。
    let mut reply = GvBuffer::empty();
    let (status, envelope) = unsafe {
        call_result(
            gv_repo_remove(engine, id_bytes.as_ptr(), id_bytes.len(), &mut reply),
            &mut reply,
        )
    };
    assert_eq!(status, GV_ERROR);
    assert_eq!(envelope.unwrap()["error"]["code"], "REPO_NOT_FOUND");

    stop_and_destroy(engine);
    remove_dir_retry(&temp);
}

#[test]
fn pull_requires_confirmation_and_validates_state() {
    let temp = unique_temp_dir("gitaview_n09_pull");
    let (engine, _) = create_engine(&temp);
    let repo = temp.join("repo");
    fs::create_dir_all(&repo).unwrap();
    run_git(&repo, &["init", "-b", "main"]);
    run_git(&repo, &["config", "user.email", "test@example.test"]);
    run_git(&repo, &["config", "user.name", "Test"]);
    run_git(&repo, &["commit", "--allow-empty", "-m", "initial"]);
    let add = serde_json::json!({ "path": repo.to_string_lossy() }).to_string();
    let mut reply = GvBuffer::empty();
    let (status, _) = unsafe {
        call_result(
            gv_repo_add(engine, add.as_ptr(), add.len(), &mut reply),
            &mut reply,
        )
    };
    assert_eq!(status, GV_OK);

    let id = b"repo";
    // confirmed=0 → CONFIRMATION_REQUIRED（启动 Git 前拒绝）。
    let mut reply = GvBuffer::empty();
    let (status, envelope) = unsafe {
        call_result(
            gv_repo_pull(engine, id.as_ptr(), id.len(), 0, &mut reply),
            &mut reply,
        )
    };
    assert_eq!(status, GV_ERROR);
    assert_eq!(envelope.unwrap()["error"]["code"], "CONFIRMATION_REQUIRED");

    // confirmed 非法取值 → INVALID_ARGUMENT。
    let mut reply = GvBuffer::empty();
    let (status, _) = unsafe {
        call_result(
            gv_repo_pull(engine, id.as_ptr(), id.len(), 2, &mut reply),
            &mut reply,
        )
    };
    assert_eq!(status, GV_INVALID_ARGUMENT);

    // confirmed=1 但无 origin → ORIGIN_REQUIRED。
    let mut reply = GvBuffer::empty();
    let (status, envelope) = unsafe {
        call_result(
            gv_repo_pull(engine, id.as_ptr(), id.len(), 1, &mut reply),
            &mut reply,
        )
    };
    assert_eq!(status, GV_ERROR);
    assert_eq!(envelope.unwrap()["error"]["code"], "ORIGIN_REQUIRED");

    stop_and_destroy(engine);
    remove_dir_retry(&temp);
}

#[test]
fn refresh_snapshot_view_and_widget_roundtrip() {
    let temp = unique_temp_dir("gitaview_n09_roundtrip");
    let (engine, create_data) = create_engine(&temp);
    let session_id = create_data["sessionId"].as_str().unwrap().to_string();

    // 刷新请求：{sessionId, requestId}。
    let mut reply = GvBuffer::empty();
    let (status, envelope) =
        unsafe { call_result(gv_refresh_request(engine, &mut reply), &mut reply) };
    assert_eq!(status, GV_OK);
    let refresh_data = envelope.unwrap()["data"].clone();
    assert_eq!(refresh_data["sessionId"], session_id.as_str());
    assert!(refresh_data["requestId"].as_u64().is_some());

    // 视图查询。
    let query = r#"{"relationFilter":"all","query":""}"#;
    let mut reply = GvBuffer::empty();
    let (status, envelope) = unsafe {
        call_result(
            gv_view_query(engine, query.as_ptr(), query.len(), &mut reply),
            &mut reply,
        )
    };
    assert_eq!(status, GV_OK);
    let view = envelope.unwrap()["data"].clone();
    assert_eq!(view["sessionId"], session_id.as_str());
    assert_eq!(view["summaryCounts"]["total"], 0);
    assert_eq!(view["rows"], serde_json::json!([]));

    // Widget 投影：空仓库，total 0；不含 path 字段。
    let mut reply = GvBuffer::empty();
    let (status, envelope) =
        unsafe { call_result(gv_widget_snapshot(engine, &mut reply), &mut reply) };
    assert_eq!(status, GV_OK);
    let widget = envelope.unwrap()["data"].clone();
    assert_eq!(widget["counts"]["total"], 0);
    assert!(widget.get("path").is_none());

    // 未知 relationFilter → INVALID_ARGUMENT。
    let bad_query = r#"{"relationFilter":"pending"}"#;
    let mut reply = GvBuffer::empty();
    let (status, _) = unsafe {
        call_result(
            gv_view_query(engine, bad_query.as_ptr(), bad_query.len(), &mut reply),
            &mut reply,
        )
    };
    assert_eq!(status, GV_INVALID_ARGUMENT);

    stop_and_destroy(engine);
    remove_dir_retry(&temp);
}

#[test]
fn snapshot_wait_timeout_and_stopping_have_empty_replies() {
    let temp = unique_temp_dir("gitaview_n09_wait");
    let (engine, create_data) = create_engine(&temp);
    let session_id = create_data["sessionId"].as_str().unwrap().to_string();

    // 极高 cursor：无变化可等 → TIMEOUT，缓冲区保持空（data null）。
    let cursor =
        serde_json::json!({"sessionId": session_id, "stateRevision": u64::MAX}).to_string();
    let mut reply = GvBuffer::empty();
    let status =
        unsafe { gv_snapshot_wait(engine, cursor.as_ptr(), cursor.len(), 150, &mut reply) };
    assert_eq!(status, GV_TIMEOUT);
    assert!(
        reply.data.is_null(),
        "TIMEOUT must be allowed an empty buffer"
    );
    unsafe { gv_buffer_free(&mut reply) };

    // 停止 → STOPPING（空缓冲区），与等待 TIMEOUT 状态码区分。
    let mut reply = GvBuffer::empty();
    unsafe { gv_engine_request_stop(engine, &mut reply) };
    let _ = take_payload(&mut reply);
    unsafe { gv_buffer_free(&mut reply) };

    let mut reply = GvBuffer::empty();
    let status =
        unsafe { gv_snapshot_wait(engine, cursor.as_ptr(), cursor.len(), 5_000, &mut reply) };
    assert_eq!(status, GV_STOPPING);
    assert!(reply.data.is_null());
    unsafe { gv_buffer_free(&mut reply) };

    // 停止后业务调用 → STOPPING + envelope ENGINE_STOPPING。
    let mut reply = GvBuffer::empty();
    let (status, envelope) =
        unsafe { call_result(gv_refresh_request(engine, &mut reply), &mut reply) };
    assert_eq!(status, GV_STOPPING);
    assert_eq!(envelope.unwrap()["error"]["code"], "ENGINE_STOPPING");

    // wait_stopped 成功；destroy 成功并置 null；重复 destroy → INVALID_ARGUMENT。
    let mut reply = GvBuffer::empty();
    let (status, envelope) = unsafe {
        call_result(
            gv_engine_wait_stopped(engine, 30_000, &mut reply),
            &mut reply,
        )
    };
    assert_eq!(status, GV_OK);
    assert_eq!(envelope.unwrap()["data"]["lifecycle"], "stopped");

    let mut handle = engine;
    assert_eq!(unsafe { gv_engine_destroy(&mut handle) }, GV_OK);
    assert!(handle.is_null());
    let mut handle2 = std::ptr::null_mut();
    assert_eq!(
        unsafe { gv_engine_destroy(&mut handle2) },
        GV_INVALID_ARGUMENT
    );

    remove_dir_retry(&temp);
}

#[test]
fn wait_stopped_without_stop_times_out_and_destroy_rejects_running_engine() {
    let temp = unique_temp_dir("gitaview_n09_destroy_busy");
    let (engine, _) = create_engine(&temp);

    // 未停止 → destroy BUSY，引擎仍可用。
    let mut handle = engine;
    assert_eq!(unsafe { gv_engine_destroy(&mut handle) }, GV_BUSY);
    assert_eq!(handle, engine);
    let mut reply = GvBuffer::empty();
    let (status, _) = unsafe { call_result(gv_snapshot_get(engine, &mut reply), &mut reply) };
    assert_eq!(status, GV_OK, "engine survives rejected destroy");

    // wait_stopped 在未 request_stop 时限期到 → TIMEOUT。
    let mut reply = GvBuffer::empty();
    let status = unsafe { gv_engine_wait_stopped(engine, 150, &mut reply) };
    assert_eq!(status, GV_TIMEOUT);
    assert!(reply.data.is_null());
    unsafe { gv_buffer_free(&mut reply) };

    stop_and_destroy(engine);
    remove_dir_retry(&temp);
}

#[test]
fn destroy_rejects_while_snapshot_wait_in_flight() {
    let temp = unique_temp_dir("gitaview_n09_destroy_inflight");
    let (engine, create_data) = create_engine(&temp);
    let session_id = create_data["sessionId"].as_str().unwrap().to_string();
    let cursor =
        serde_json::json!({"sessionId": session_id, "stateRevision": u64::MAX}).to_string();

    // 句柄地址以 usize 传入线程（裸指针本身 !Send；FFI 句柄按定义为跨线程）。
    let engine_addr = engine as usize;
    let cursor_clone = cursor.clone();
    let waiter = std::thread::spawn(move || {
        let engine = engine_addr as *mut GvEngine;
        let mut reply = GvBuffer::empty();
        let status = unsafe {
            gv_snapshot_wait(
                engine,
                cursor_clone.as_ptr(),
                cursor_clone.len(),
                2_000,
                &mut reply,
            )
        };
        unsafe { gv_buffer_free(&mut reply) };
        status
    });
    std::thread::sleep(Duration::from_millis(200));

    // 在途调用存在 → destroy BUSY。
    let mut handle = engine;
    assert_eq!(unsafe { gv_engine_destroy(&mut handle) }, GV_BUSY);

    // request_stop 可与在途调用并发：唤醒等待者。
    let mut reply = GvBuffer::empty();
    unsafe { gv_engine_request_stop(engine, &mut reply) };
    let _ = take_payload(&mut reply);
    unsafe { gv_buffer_free(&mut reply) };

    assert_eq!(waiter.join().unwrap(), GV_STOPPING);
    stop_and_destroy(engine);
    remove_dir_retry(&temp);
}

#[test]
fn null_arguments_map_to_invalid_argument_without_crash() {
    let temp = unique_temp_dir("gitaview_n09_null_args");
    let (engine, _) = create_engine(&temp);

    let mut reply = GvBuffer::empty();
    let (status, envelope) = unsafe {
        let status = gv_snapshot_get(std::ptr::null_mut(), &mut reply);
        call_result(status, &mut reply)
    };
    assert_eq!(status, GV_INVALID_ARGUMENT);
    // 非 TIMEOUT / STOPPING 的错误状态：有缓冲区则必为错误 envelope（§9.2）。
    assert_eq!(envelope.unwrap()["error"]["code"], "INVALID_ARGUMENT");

    assert_eq!(
        unsafe { gv_snapshot_get(engine, std::ptr::null_mut()) },
        GV_INVALID_ARGUMENT
    );

    let id = b"repo";
    assert_eq!(
        unsafe { gv_repo_pull(engine, id.as_ptr(), 3, 1, std::ptr::null_mut()) },
        GV_INVALID_ARGUMENT
    );

    // 非 UTF-8 原始 ID → INVALID_ARGUMENT + envelope。
    let bad_id = [0xff, 0xfe];
    let mut reply = GvBuffer::empty();
    let (status, envelope) = unsafe {
        call_result(
            gv_repo_remove(engine, bad_id.as_ptr(), bad_id.len(), &mut reply),
            &mut reply,
        )
    };
    assert_eq!(status, GV_INVALID_ARGUMENT);
    assert_eq!(envelope.unwrap()["error"]["code"], "INVALID_ARGUMENT");

    stop_and_destroy(engine);
    remove_dir_retry(&temp);
}

#[test]
fn two_engines_coexist_with_independent_sessions() {
    let temp_a = unique_temp_dir("gitaview_n09_engine_a");
    let temp_b = unique_temp_dir("gitaview_n09_engine_b");
    let (engine_a, data_a) = create_engine(&temp_a);
    let (engine_b, data_b) = create_engine(&temp_b);

    assert_ne!(
        data_a["sessionId"].as_str().unwrap(),
        data_b["sessionId"].as_str().unwrap()
    );

    stop_and_destroy(engine_a);
    // 引擎 A 销毁不影响引擎 B。
    let mut reply = GvBuffer::empty();
    let (status, _) = unsafe { call_result(gv_snapshot_get(engine_b, &mut reply), &mut reply) };
    assert_eq!(status, GV_OK);
    stop_and_destroy(engine_b);
    remove_dir_retry(&temp_a);
    remove_dir_retry(&temp_b);
}

#[test]
fn buffer_alloc_free_cycles_and_null_safety() {
    let temp = unique_temp_dir("gitaview_n09_cycles");
    let (engine, _) = create_engine(&temp);
    // 多轮分配 / 释放：每轮新建缓冲区并释放，验证无泄漏路径可重复执行。
    for round in 0..50 {
        let mut reply = GvBuffer::empty();
        let status = unsafe { gv_settings_get(engine, &mut reply) };
        assert_eq!(status, GV_OK, "round {round}");
        let payload = take_payload(&mut reply).expect("settings_get writes reply");
        assert!(payload.len() > 2);
        unsafe { gv_buffer_free(&mut reply) };
        assert!(reply.data.is_null() && reply.len == 0);
    }
    stop_and_destroy(engine);
    unsafe { gv_buffer_free(std::ptr::null_mut()) };
    remove_dir_retry(&temp);
}

// —— 复查回归（S03）——

/// S03：null reply 时 create 不执行业务体——不启动引擎、不创建数据目录，
/// 输出句柄保持 null。
#[test]
fn create_with_null_reply_has_no_side_effects() {
    let temp = unique_temp_dir("gitaview_s03_create_null_reply");
    let input = create_input(&temp);
    let mut engine: *mut GvEngine = std::ptr::null_mut();
    let status = unsafe {
        gv_engine_create(
            input.as_ptr(),
            input.len(),
            &mut engine,
            std::ptr::null_mut(),
        )
    };
    assert_eq!(status, GV_INVALID_ARGUMENT);
    assert!(engine.is_null(), "failed create must leave engine null");
    assert!(
        !temp.join("settings.json").exists(),
        "engine body must not run: settings.json must not be created"
    );
}

/// S03：null engine_out 时 create 拒绝执行，不创建引擎、不丢弃已启动线程。
#[test]
fn create_with_null_engine_out_has_no_side_effects() {
    let temp = unique_temp_dir("gitaview_s03_create_null_out");
    let input = create_input(&temp);
    let mut reply = GvBuffer::empty();
    let status = unsafe {
        gv_engine_create(
            input.as_ptr(),
            input.len(),
            std::ptr::null_mut(),
            &mut reply,
        )
    };
    let (status, envelope) = call_result(status, &mut reply);
    assert_eq!(status, GV_INVALID_ARGUMENT);
    assert_eq!(envelope.unwrap()["error"]["code"], "INVALID_ARGUMENT");
    assert!(
        !temp.join("settings.json").exists(),
        "engine body must not run: settings.json must not be created"
    );
}

/// S03：null reply 时 settings_save 不执行业务体——revision 与刷新偏好保持不变。
#[test]
fn save_with_null_reply_does_not_change_settings() {
    let temp = unique_temp_dir("gitaview_s03_save_null_reply");
    let (engine, _) = create_engine(&temp);

    let save_input = serde_json::json!({
        "expectedRevision": 1,
        "settings": {
            "groups": [],
            "defaultGroupId": null,
            "refresh": { "enabled": false, "intervalMinutes": 7 },
            "repoGroups": []
        }
    });
    let status = unsafe {
        gv_settings_save(
            engine,
            save_input.to_string().as_ptr(),
            save_input.to_string().len(),
            std::ptr::null_mut(),
        )
    };
    assert_eq!(status, GV_INVALID_ARGUMENT);

    // 副作用检查：revision 仍为 1，刷新偏好保持默认。
    let mut reply = GvBuffer::empty();
    let (status, envelope) =
        unsafe { call_result(gv_settings_get(engine, &mut reply), &mut reply) };
    assert_eq!(status, GV_OK);
    let data = envelope.unwrap()["data"].clone();
    assert_eq!(data["revision"], 1);
    assert_eq!(data["refresh"]["enabled"], true);
    assert_eq!(data["refresh"]["intervalMinutes"], 5);

    stop_and_destroy(engine);
    remove_dir_retry(&temp);
}

/// S03：生命周期接口的明确特例——request_stop 允许 null reply（返回 OK、
/// 停止已执行）；wait_stopped 的 null reply 在等待开始前拒绝。
#[test]
fn lifecycle_null_reply_follows_documented_special_case() {
    let temp = unique_temp_dir("gitaview_s03_lifecycle_reply");
    let (engine, _) = create_engine(&temp);

    // request_stop + null reply：OK，且停止确实生效——随后的 wait_stopped
    // 必须快速成功（若 null reply 的调用没有执行停止，这里会等满超时）。
    assert_eq!(
        unsafe { gv_engine_request_stop(engine, std::ptr::null_mut()) },
        GV_OK
    );
    let mut reply = GvBuffer::empty();
    let started = Instant::now();
    let status = unsafe { gv_engine_wait_stopped(engine, 5_000, &mut reply) };
    let _ = call_result(status, &mut reply);
    assert_eq!(
        status, GV_OK,
        "null-reply request_stop must have stopped the engine"
    );
    assert!(started.elapsed() < Duration::from_secs(3));

    // wait_stopped + null reply：立即 INVALID_ARGUMENT，不等待。
    let started = Instant::now();
    assert_eq!(
        unsafe { gv_engine_wait_stopped(engine, 30_000, std::ptr::null_mut()) },
        GV_INVALID_ARGUMENT
    );
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "must reject before waiting"
    );

    // 收尾：等停止完成后销毁。
    let mut reply = GvBuffer::empty();
    let status = unsafe { gv_engine_wait_stopped(engine, 30_000, &mut reply) };
    assert_eq!(status, GV_OK);
    let _ = call_result(status, &mut reply);
    let mut handle = engine;
    assert_eq!(unsafe { gv_engine_destroy(&mut handle) }, GV_OK);
    remove_dir_retry(&temp);
}
