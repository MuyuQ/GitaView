//! GitaView 正式 C ABI（N09 冻结版）。
//!
//! - 固定函数集合，无通用 execute(command)；JSON 提供 DTO，C 签名提供操作边界。
//! - int32 状态：OK=0 ERROR=1 TIMEOUT=2 STOPPING=3 BUSY=4 INVALID_ARGUMENT=5
//!   PANIC=6。TIMEOUT 只表示等待期限；Git 命令超时是业务错误 GIT_TIMEOUT。
//! - 成功 / 业务错误返回 UTF-8 JSON envelope：
//!   `{"schemaVersion":1,"ok":true,"data":…,"error":null}`；错误时 ok=false。
//!   TIMEOUT / STOPPING 允许空缓冲区；其余非 OK 状态若有缓冲区必为错误 envelope。
//! - 缓冲区由 `gv_buffer_free` 释放并清零；Box 字节切片，唯一释放责任。
//! - 每个导出入口阻止 panic 穿越 ABI，捕获返回 PANIC。
//! - 生命周期：request_stop / wait_stopped 不计入在途业务调用计数；
//!   destroy 仅在 Stopped 且无在途调用时成功，否则 BUSY。
//! - N02 原型探针导出已移除（探针保留在 gitaview-core 内部供参考与示例）。

pub mod buffer;
pub mod handle;

pub use buffer::GvBuffer;
pub use handle::GvEngine;

use buffer::allocate;
use gitaview_core::engine::{
    Engine, EngineConfig, EngineCreateData, EngineCreateInput, LifecycleData, LifecycleDto,
    RefreshRequestData, RepoAddData, RepoAddInput, RepoOperationData, RepoRemoveData,
    RepoTargetInput, ScanDirectoryInput, SnapshotWait, SnapshotWaitInput,
};
use gitaview_core::error::CoreError;
use gitaview_core::model::SCHEMA_VERSION;
use gitaview_core::settings::SettingsSaveInput;
use gitaview_core::view::ViewQuery;
use gitaview_core::widget::project_widget;
use serde::Serialize;
use serde_json::{json, Value};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;
use std::time::Duration;

pub const GV_ABI_VERSION: u32 = 1;

pub const GV_OK: i32 = 0;
pub const GV_ERROR: i32 = 1;
pub const GV_TIMEOUT: i32 = 2;
pub const GV_STOPPING: i32 = 3;
pub const GV_BUSY: i32 = 4;
pub const GV_INVALID_ARGUMENT: i32 = 5;
pub const GV_PANIC: i32 = 6;

// —— envelope 与输入工具 ——

fn envelope_ok(data: Value) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "schemaVersion": SCHEMA_VERSION,
        "ok": true,
        "data": data,
        "error": null,
    }))
    .expect("envelope serialization cannot fail")
}

fn envelope_error(code: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "schemaVersion": SCHEMA_VERSION,
        "ok": false,
        "data": null,
        "error": { "code": code },
    }))
    .expect("envelope serialization cannot fail")
}

fn data_of<T: Serialize>(payload: &T) -> Value {
    serde_json::to_value(payload).expect("DTO serialization cannot fail")
}

/// 写入 reply 并返回指定状态；null reply 返回 INVALID_ARGUMENT。
fn write_status(reply: *mut GvBuffer, status: i32, payload: Vec<u8>) -> i32 {
    if reply.is_null() {
        return GV_INVALID_ARGUMENT;
    }
    unsafe { *reply = allocate(payload) };
    status
}

/// 业务错误 → 状态码 + envelope（固定映射：REPO_BUSY→BUSY、参数非法→
/// INVALID_ARGUMENT、ENGINE_STOPPING→STOPPING、其余预期失败→ERROR）。
fn write_core_error(reply: *mut GvBuffer, error: CoreError) -> i32 {
    let status = match &error {
        CoreError::RepoBusy => GV_BUSY,
        CoreError::InvalidArgument => GV_INVALID_ARGUMENT,
        CoreError::EngineStopping => GV_STOPPING,
        _ => GV_ERROR,
    };
    write_status(reply, status, envelope_error(error.code()))
}

/// 读取调用方传入的字节切片。null / len 组合非法时返回 None。
fn input_bytes<'a>(ptr: *const u8, len: usize) -> Option<&'a [u8]> {
    match (ptr.is_null(), len) {
        (true, 0) => Some(&[]),
        (false, 0) => None,
        (true, _) => None,
        (false, _) => Some(unsafe { std::slice::from_raw_parts(ptr, len) }),
    }
}

/// JSON 输入解码失败统一映射 INVALID_ARGUMENT（结构 / 枚举 / 编码非法）。
fn parse_json<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, i32> {
    serde_json::from_slice(bytes).map_err(|_| GV_INVALID_ARGUMENT)
}

/// UTF-8 原始 ID 字节（非 JSON 字符串）。
fn parse_raw_id(bytes: &[u8]) -> Result<String, i32> {
    std::str::from_utf8(bytes)
        .map(str::to_string)
        .map_err(|_| GV_INVALID_ARGUMENT)
}

/// confirmed 只允许 0 / 1，不传 Rust bool 跨边界。
fn parse_confirmed(value: u8) -> Result<bool, i32> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(GV_INVALID_ARGUMENT),
    }
}

/// 取有效引擎并计入在途调用：null / 销毁中 → 状态码；业务体结果原样返回。
fn with_engine<T>(
    engine: *mut GvEngine,
    body: impl FnOnce(&Engine) -> Result<T, CoreError>,
) -> Result<Result<T, CoreError>, i32> {
    if engine.is_null() {
        return Err(GV_INVALID_ARGUMENT);
    }
    let handle = unsafe { &*engine };
    let _call = handle.acquire_call().map_err(|_| GV_BUSY)?;
    Ok(body(handle.engine()))
}

fn status_code(status: i32) -> &'static str {
    match status {
        GV_BUSY => "REPO_BUSY",
        GV_STOPPING => "ENGINE_STOPPING",
        _ => "INVALID_ARGUMENT",
    }
}

/// 业务调用统一出口：成功 → envelope；核心错误 → 固定映射；null / 销毁 → 状态码
///（有 reply 时写错误 envelope）。
/// 输出参数在业务体执行前校验（S03）：null reply 直接返回 INVALID_ARGUMENT，
/// 不执行任何有副作用的业务体。
fn run_call<T: Serialize>(
    engine: *mut GvEngine,
    reply: *mut GvBuffer,
    body: impl FnOnce(&Engine) -> Result<T, CoreError>,
) -> i32 {
    if reply.is_null() {
        return GV_INVALID_ARGUMENT;
    }
    match with_engine(engine, body) {
        Ok(Ok(payload)) => write_status(reply, GV_OK, envelope_ok(data_of(&payload))),
        Ok(Err(error)) => write_core_error(reply, error),
        Err(status) => write_status(reply, status, envelope_error(status_code(status))),
    }
}

// —— ABI 版本 ——

/// ABI 版本；宿主加载后先核对再调用其他函数。
#[no_mangle]
pub extern "C" fn gv_abi_version() -> u32 {
    GV_ABI_VERSION
}

// —— 引擎生命周期 ——

/// 创建引擎。输入 `{dataDirectory, logDirectory}`（绝对私有路径）；
/// 成功 data `{sessionId, abiVersion, startupWarnings}`；失败保证 `*engine` 为 null。
///
/// # Safety
/// `json` 借用期间有效；`engine_out` 为 null 或指向可写指针；`reply` 已清零。
#[no_mangle]
pub unsafe extern "C" fn gv_engine_create(
    json: *const u8,
    len: usize,
    engine_out: *mut *mut GvEngine,
    reply: *mut GvBuffer,
) -> i32 {
    catch_unwind(AssertUnwindSafe(|| {
        // 输出参数在创建引擎之前校验（S03）：非法输入不启动引擎、不建数据目录；
        // 失败保证 `*engine_out` 为 null。engine_out 缺失时若有 reply 仍写错误
        // envelope（调用方能拿到稳定状态）；reply 也缺失时只返回状态码。
        if engine_out.is_null() {
            if reply.is_null() {
                return GV_INVALID_ARGUMENT;
            }
            return write_status(
                reply,
                GV_INVALID_ARGUMENT,
                envelope_error("INVALID_ARGUMENT"),
            );
        }
        unsafe { *engine_out = std::ptr::null_mut() };
        if reply.is_null() {
            return GV_INVALID_ARGUMENT;
        }
        let Some(bytes) = input_bytes(json, len) else {
            return write_status(
                reply,
                GV_INVALID_ARGUMENT,
                envelope_error("INVALID_ARGUMENT"),
            );
        };
        let input: EngineCreateInput = match parse_json(bytes) {
            Ok(input) => input,
            Err(_) => {
                return write_status(
                    reply,
                    GV_INVALID_ARGUMENT,
                    envelope_error("INVALID_ARGUMENT"),
                );
            }
        };
        let data_dir = std::path::PathBuf::from(&input.data_directory);
        let created = if data_dir.is_absolute() {
            gitaview_core::engine::engine_create(EngineConfig {
                data_dir,
                log_dir: PathBuf::from(&input.log_directory),
                ..EngineConfig::default()
            })
        } else {
            Err(CoreError::InvalidArgument)
        };
        match created {
            Ok(startup) => {
                let data = EngineCreateData {
                    session_id: startup.engine.session_id().to_string(),
                    abi_version: GV_ABI_VERSION,
                    startup_warnings: startup
                        .warnings
                        .iter()
                        .map(|warning| warning.as_str().to_string())
                        .collect(),
                };
                let handle = Box::new(GvEngine::new(startup.engine));
                unsafe { *engine_out = Box::into_raw(handle) };
                write_status(reply, GV_OK, envelope_ok(data_of(&data)))
            }
            Err(error) => {
                if !engine_out.is_null() {
                    unsafe { *engine_out = std::ptr::null_mut() };
                }
                write_core_error(reply, error)
            }
        }
    }))
    .unwrap_or(GV_PANIC)
}

/// 请求停止：幂等；唤醒全部等待者。不计入在途业务调用计数。
/// 生命周期特例（S03）：允许 null reply——停止是幂等操作且结果不依赖输出，
/// null reply 时执行停止并返回 OK；有 reply 时正常写 envelope。
///
/// # Safety
/// `engine` 为有效句柄；`reply` 为 null 或已清零。
#[no_mangle]
pub unsafe extern "C" fn gv_engine_request_stop(
    engine: *mut GvEngine,
    reply: *mut GvBuffer,
) -> i32 {
    catch_unwind(AssertUnwindSafe(|| {
        if engine.is_null() {
            return GV_INVALID_ARGUMENT;
        }
        let handle = unsafe { &*engine };
        handle.engine().request_stop();
        if reply.is_null() {
            return GV_OK;
        }
        let lifecycle = LifecycleData {
            lifecycle: handle.engine().lifecycle().into(),
        };
        write_status(reply, GV_OK, envelope_ok(data_of(&lifecycle)))
    }))
    .unwrap_or(GV_PANIC)
}

/// 等待停止完成：成功 data `{lifecycle:"stopped"}`；期限到仍清理完 → TIMEOUT
///（空缓冲区，不代表可以 destroy）。不计入在途业务调用计数。
/// null reply 在等待开始前拒绝（S03）。
///
/// # Safety
/// `engine` 为有效句柄；`reply` 已清零。
#[no_mangle]
pub unsafe extern "C" fn gv_engine_wait_stopped(
    engine: *mut GvEngine,
    timeout_ms: u32,
    reply: *mut GvBuffer,
) -> i32 {
    catch_unwind(AssertUnwindSafe(|| {
        if engine.is_null() || reply.is_null() {
            return GV_INVALID_ARGUMENT;
        }
        let handle = unsafe { &*engine };
        if handle
            .engine()
            .wait_stopped(Duration::from_millis(u64::from(timeout_ms)))
        {
            let lifecycle = LifecycleData {
                lifecycle: LifecycleDto::Stopped,
            };
            write_status(reply, GV_OK, envelope_ok(data_of(&lifecycle)))
        } else {
            // 等待期限已到：空缓冲区（§9.2 允许），宿主凭状态码分支。
            GV_TIMEOUT
        }
    }))
    .unwrap_or(GV_PANIC)
}

/// 销毁引擎：仅在 Stopped 且无在途调用时成功（成功把 `*engine` 置 null）；
/// 否则 BUSY 且不销毁。无 reply。
///
/// # Safety
/// `engine` 为 null 或有效句柄指针。
#[no_mangle]
pub unsafe extern "C" fn gv_engine_destroy(engine: *mut *mut GvEngine) -> i32 {
    catch_unwind(AssertUnwindSafe(|| {
        if engine.is_null() {
            return GV_INVALID_ARGUMENT;
        }
        let current = unsafe { *engine };
        if current.is_null() {
            return GV_INVALID_ARGUMENT;
        }
        let handle = unsafe { &*current };
        if handle.engine().lifecycle() != gitaview_core::engine::Lifecycle::Stopped {
            return GV_BUSY;
        }
        if handle.begin_destroy().is_err() {
            return GV_BUSY;
        }
        drop(unsafe { Box::from_raw(current) });
        unsafe { *engine = std::ptr::null_mut() };
        GV_OK
    }))
    .unwrap_or(GV_PANIC)
}

// —— 设置 ——

/// 当前完整设置（含 revision 与仓库记录）。
///
/// # Safety
/// `engine` 有效；`reply` 已清零。
#[no_mangle]
pub unsafe extern "C" fn gv_settings_get(engine: *mut GvEngine, reply: *mut GvBuffer) -> i32 {
    catch_unwind(AssertUnwindSafe(|| {
        run_call(engine, reply, |engine| Ok(engine.get_settings()))
    }))
    .unwrap_or(GV_PANIC)
}

/// 保存可编辑字段：`{expectedRevision, settings}`；冲突 → SETTINGS_CONFLICT。
///
/// # Safety
/// `engine` 有效；`json` 借用期间有效；`reply` 已清零。
#[no_mangle]
pub unsafe extern "C" fn gv_settings_save(
    engine: *mut GvEngine,
    json: *const u8,
    len: usize,
    reply: *mut GvBuffer,
) -> i32 {
    catch_unwind(AssertUnwindSafe(|| {
        let Some(bytes) = input_bytes(json, len) else {
            return write_status(
                reply,
                GV_INVALID_ARGUMENT,
                envelope_error("INVALID_ARGUMENT"),
            );
        };
        let input: SettingsSaveInput = match parse_json(bytes) {
            Ok(input) => input,
            Err(_) => {
                return write_status(
                    reply,
                    GV_INVALID_ARGUMENT,
                    envelope_error("INVALID_ARGUMENT"),
                )
            }
        };
        run_call(engine, reply, |engine| engine.save_settings(input))
    }))
    .unwrap_or(GV_PANIC)
}

// —— 仓库管理 ——

/// 目录扫描：`{path}` → `{paths, truncated}`；候选不自动添加。
///
/// # Safety
/// `engine` 有效；`json` 借用期间有效；`reply` 已清零。
#[no_mangle]
pub unsafe extern "C" fn gv_scan_directory(
    engine: *mut GvEngine,
    json: *const u8,
    len: usize,
    reply: *mut GvBuffer,
) -> i32 {
    catch_unwind(AssertUnwindSafe(|| {
        let Some(bytes) = input_bytes(json, len) else {
            return write_status(
                reply,
                GV_INVALID_ARGUMENT,
                envelope_error("INVALID_ARGUMENT"),
            );
        };
        let input: ScanDirectoryInput = match parse_json(bytes) {
            Ok(input) => input,
            Err(_) => {
                return write_status(
                    reply,
                    GV_INVALID_ARGUMENT,
                    envelope_error("INVALID_ARGUMENT"),
                )
            }
        };
        run_call(engine, reply, |engine| {
            engine.scan_directory(&std::path::PathBuf::from(&input.path))
        })
    }))
    .unwrap_or(GV_PANIC)
}

/// 添加仓库：`{path, groupId?}`；重复规范路径返回既有记录。
///
/// # Safety
/// `engine` 有效；`json` 借用期间有效；`reply` 已清零。
#[no_mangle]
pub unsafe extern "C" fn gv_repo_add(
    engine: *mut GvEngine,
    json: *const u8,
    len: usize,
    reply: *mut GvBuffer,
) -> i32 {
    catch_unwind(AssertUnwindSafe(|| {
        let Some(bytes) = input_bytes(json, len) else {
            return write_status(
                reply,
                GV_INVALID_ARGUMENT,
                envelope_error("INVALID_ARGUMENT"),
            );
        };
        let input: RepoAddInput = match parse_json(bytes) {
            Ok(input) => input,
            Err(_) => {
                return write_status(
                    reply,
                    GV_INVALID_ARGUMENT,
                    envelope_error("INVALID_ARGUMENT"),
                )
            }
        };
        run_call(engine, reply, |engine| {
            engine
                .repo_add(
                    &std::path::PathBuf::from(&input.path),
                    input.group_id.as_deref(),
                )
                .map(|(repo, settings)| RepoAddData {
                    repo,
                    settings_revision: settings.revision,
                })
        })
    }))
    .unwrap_or(GV_PANIC)
}

/// 移除仓库：输入 UTF-8 原始 ID 字节（非 JSON 字符串）→ `{settingsRevision}`。
///
/// # Safety
/// `engine` 有效；`id` 借用期间有效；`reply` 已清零。
#[no_mangle]
pub unsafe extern "C" fn gv_repo_remove(
    engine: *mut GvEngine,
    id: *const u8,
    len: usize,
    reply: *mut GvBuffer,
) -> i32 {
    catch_unwind(AssertUnwindSafe(|| {
        let Some(bytes) = input_bytes(id, len) else {
            return write_status(
                reply,
                GV_INVALID_ARGUMENT,
                envelope_error("INVALID_ARGUMENT"),
            );
        };
        let repo_id = match parse_raw_id(bytes) {
            Ok(repo_id) => repo_id,
            Err(_) => {
                return write_status(
                    reply,
                    GV_INVALID_ARGUMENT,
                    envelope_error("INVALID_ARGUMENT"),
                )
            }
        };
        run_call(engine, reply, |engine| {
            engine.repo_remove(&repo_id).map(|settings| RepoRemoveData {
                settings_revision: settings.revision,
            })
        })
    }))
    .unwrap_or(GV_PANIC)
}

// —— Git 操作 ——

/// Fetch：输入 UTF-8 原始 ID 字节；成功表示命令执行完成，新状态由后续刷新产生。
///
/// # Safety
/// `engine` 有效；`id` 借用期间有效；`reply` 已清零。
#[no_mangle]
pub unsafe extern "C" fn gv_repo_fetch(
    engine: *mut GvEngine,
    id: *const u8,
    len: usize,
    reply: *mut GvBuffer,
) -> i32 {
    catch_unwind(AssertUnwindSafe(|| {
        let Some(bytes) = input_bytes(id, len) else {
            return write_status(
                reply,
                GV_INVALID_ARGUMENT,
                envelope_error("INVALID_ARGUMENT"),
            );
        };
        let repo_id = match parse_raw_id(bytes) {
            Ok(repo_id) => repo_id,
            Err(_) => {
                return write_status(
                    reply,
                    GV_INVALID_ARGUMENT,
                    envelope_error("INVALID_ARGUMENT"),
                )
            }
        };
        run_call(engine, reply, |engine| {
            engine
                .repo_fetch(&repo_id)
                .map(|outcome| RepoOperationData {
                    repo_id: repo_id.clone(),
                    operation: outcome.operation.as_str().to_string(),
                })
        })
    }))
    .unwrap_or(GV_PANIC)
}

/// Pull：需要用户确认（confirmed 仅接受 0 / 1）；核心执行前再次校验状态。
///
/// # Safety
/// `engine` 有效；`id` 借用期间有效；`reply` 已清零。
#[no_mangle]
pub unsafe extern "C" fn gv_repo_pull(
    engine: *mut GvEngine,
    id: *const u8,
    len: usize,
    confirmed: u8,
    reply: *mut GvBuffer,
) -> i32 {
    confirmed_operation(
        engine,
        id,
        len,
        confirmed,
        reply,
        gitaview_core::git::RepoGitOperation::Pull,
    )
}

/// Push：需要用户确认（confirmed 仅接受 0 / 1）；核心执行前再次校验状态。
///
/// # Safety
/// `engine` 有效；`id` 借用期间有效；`reply` 已清零。
#[no_mangle]
pub unsafe extern "C" fn gv_repo_push(
    engine: *mut GvEngine,
    id: *const u8,
    len: usize,
    confirmed: u8,
    reply: *mut GvBuffer,
) -> i32 {
    confirmed_operation(
        engine,
        id,
        len,
        confirmed,
        reply,
        gitaview_core::git::RepoGitOperation::Push,
    )
}

#[allow(clippy::too_many_arguments)]
fn confirmed_operation(
    engine: *mut GvEngine,
    id: *const u8,
    len: usize,
    confirmed: u8,
    reply: *mut GvBuffer,
    operation: gitaview_core::git::RepoGitOperation,
) -> i32 {
    catch_unwind(AssertUnwindSafe(|| {
        let Some(confirmed) = parse_confirmed(confirmed).ok() else {
            return write_status(
                reply,
                GV_INVALID_ARGUMENT,
                envelope_error("INVALID_ARGUMENT"),
            );
        };
        let Some(bytes) = input_bytes(id, len) else {
            return write_status(
                reply,
                GV_INVALID_ARGUMENT,
                envelope_error("INVALID_ARGUMENT"),
            );
        };
        let repo_id = match parse_raw_id(bytes) {
            Ok(repo_id) => repo_id,
            Err(_) => {
                return write_status(
                    reply,
                    GV_INVALID_ARGUMENT,
                    envelope_error("INVALID_ARGUMENT"),
                )
            }
        };
        run_call(engine, reply, |engine| {
            let outcome = match operation {
                gitaview_core::git::RepoGitOperation::Pull => engine.repo_pull(&repo_id, confirmed),
                gitaview_core::git::RepoGitOperation::Push => engine.repo_push(&repo_id, confirmed),
                gitaview_core::git::RepoGitOperation::Fetch => engine.repo_fetch(&repo_id),
            };
            outcome.map(|outcome| RepoOperationData {
                repo_id: repo_id.clone(),
                operation: outcome.operation.as_str().to_string(),
            })
        })
    }))
    .unwrap_or(GV_PANIC)
}

// —— 刷新与快照 ——

/// 手动 / 通用刷新请求：`{sessionId, requestId}`；完成看 completedRequestId。
///
/// # Safety
/// `engine` 有效；`reply` 已清零。
#[no_mangle]
pub unsafe extern "C" fn gv_refresh_request(engine: *mut GvEngine, reply: *mut GvBuffer) -> i32 {
    catch_unwind(AssertUnwindSafe(|| {
        run_call(engine, reply, |engine| {
            engine
                .refresh_request()
                .map(|(session_id, request_id)| RefreshRequestData {
                    session_id,
                    request_id,
                })
        })
    }))
    .unwrap_or(GV_PANIC)
}

/// 当前快照；初始 generation 0 带未就绪状态，不伪造成功。
///
/// # Safety
/// `engine` 有效；`reply` 已清零。
#[no_mangle]
pub unsafe extern "C" fn gv_snapshot_get(engine: *mut GvEngine, reply: *mut GvBuffer) -> i32 {
    catch_unwind(AssertUnwindSafe(|| {
        run_call(engine, reply, |engine| Ok(engine.snapshot_get()))
    }))
    .unwrap_or(GV_PANIC)
}

/// 等待公开状态变化。cursor `{sessionId, stateRevision}`；停止 → STOPPING
///（空缓冲区）；期限到 → TIMEOUT（空缓冲区，与 Git 超时 GIT_TIMEOUT 区分）。
///
/// # Safety
/// `engine` 有效；`cursor_json` 借用期间有效；`reply` 已清零。
#[no_mangle]
pub unsafe extern "C" fn gv_snapshot_wait(
    engine: *mut GvEngine,
    cursor_json: *const u8,
    len: usize,
    timeout_ms: u32,
    reply: *mut GvBuffer,
) -> i32 {
    catch_unwind(AssertUnwindSafe(|| {
        // 输出参数在进入等待前校验（S03）：null reply 不占用在途调用名额。
        if reply.is_null() {
            return GV_INVALID_ARGUMENT;
        }
        let Some(bytes) = input_bytes(cursor_json, len) else {
            return write_status(
                reply,
                GV_INVALID_ARGUMENT,
                envelope_error("INVALID_ARGUMENT"),
            );
        };
        let cursor: SnapshotWaitInput = match parse_json(bytes) {
            Ok(cursor) => cursor,
            Err(_) => {
                return write_status(
                    reply,
                    GV_INVALID_ARGUMENT,
                    envelope_error("INVALID_ARGUMENT"),
                )
            }
        };
        if engine.is_null() {
            return GV_INVALID_ARGUMENT;
        }
        let handle = unsafe { &*engine };
        let _call = match handle.acquire_call() {
            Ok(call) => call,
            Err(_) => return GV_BUSY,
        };
        match handle.engine().snapshot_wait(
            &cursor.session_id,
            cursor.state_revision,
            Duration::from_millis(u64::from(timeout_ms)),
        ) {
            SnapshotWait::Ready(snapshot) => {
                write_status(reply, GV_OK, envelope_ok(data_of(&snapshot)))
            }
            // 期限 / 停止：空缓冲区（§9.2 允许）。
            SnapshotWait::Timeout => GV_TIMEOUT,
            SnapshotWait::Stopping => GV_STOPPING,
        }
    }))
    .unwrap_or(GV_PANIC)
}

// —— 视图与 Widget ——

/// 视图查询：`{groupId?, relationFilter?, query?}` → ViewResult。
///
/// # Safety
/// `engine` 有效；`json` 借用期间有效；`reply` 已清零。
#[no_mangle]
pub unsafe extern "C" fn gv_view_query(
    engine: *mut GvEngine,
    json: *const u8,
    len: usize,
    reply: *mut GvBuffer,
) -> i32 {
    catch_unwind(AssertUnwindSafe(|| {
        let Some(bytes) = input_bytes(json, len) else {
            return write_status(
                reply,
                GV_INVALID_ARGUMENT,
                envelope_error("INVALID_ARGUMENT"),
            );
        };
        let query: ViewQuery = match parse_json(bytes) {
            Ok(query) => query,
            Err(_) => {
                return write_status(
                    reply,
                    GV_INVALID_ARGUMENT,
                    envelope_error("INVALID_ARGUMENT"),
                )
            }
        };
        run_call(engine, reply, |engine| Ok(engine.view_query(query)))
    }))
    .unwrap_or(GV_PANIC)
}

/// 当前安全 Widget 投影：不写磁盘、不调用系统组件 API、不含 path / 凭据。
///
/// # Safety
/// `engine` 有效；`reply` 已清零。
#[no_mangle]
pub unsafe extern "C" fn gv_widget_snapshot(engine: *mut GvEngine, reply: *mut GvBuffer) -> i32 {
    catch_unwind(AssertUnwindSafe(|| {
        run_call(engine, reply, |engine| {
            Ok(project_widget(&engine.snapshot_get()))
        })
    }))
    .unwrap_or(GV_PANIC)
}

// —— 打开目标 ——

/// `{repoId, kind}`（directory / remote）→ `{kind, value}`；
/// 复核目录存在或重读并验证当前 origin URL，宿主调用系统打开 API。
///
/// # Safety
/// `engine` 有效；`json` 借用期间有效；`reply` 已清零。
#[no_mangle]
pub unsafe extern "C" fn gv_repo_target(
    engine: *mut GvEngine,
    json: *const u8,
    len: usize,
    reply: *mut GvBuffer,
) -> i32 {
    catch_unwind(AssertUnwindSafe(|| {
        let Some(bytes) = input_bytes(json, len) else {
            return write_status(
                reply,
                GV_INVALID_ARGUMENT,
                envelope_error("INVALID_ARGUMENT"),
            );
        };
        let input: RepoTargetInput = match parse_json(bytes) {
            Ok(input) => input,
            Err(_) => {
                return write_status(
                    reply,
                    GV_INVALID_ARGUMENT,
                    envelope_error("INVALID_ARGUMENT"),
                )
            }
        };
        run_call(engine, reply, |engine| {
            engine.repo_target(&input.repo_id, input.kind)
        })
    }))
    .unwrap_or(GV_PANIC)
}

// —— 缓冲区 ——

/// 释放本库返回的缓冲区并把结构体清零；null 指针为安全 no-op。
///
/// # Safety
/// 见 `buffer::free`。
#[no_mangle]
pub unsafe extern "C" fn gv_buffer_free(buffer: *mut GvBuffer) {
    let _ = catch_unwind(AssertUnwindSafe(|| buffer::free(buffer)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelopes_have_fixed_shape() {
        let ok: Value = serde_json::from_slice(&envelope_ok(json!({"x": 1}))).unwrap();
        assert_eq!(ok["schemaVersion"], 1);
        assert_eq!(ok["ok"], true);
        assert_eq!(ok["data"]["x"], 1);
        assert!(ok["error"].is_null());

        let error: Value = serde_json::from_slice(&envelope_error("INVALID_ARGUMENT")).unwrap();
        assert_eq!(error["ok"], false);
        assert!(error["data"].is_null());
        assert_eq!(error["error"]["code"], "INVALID_ARGUMENT");
    }

    #[test]
    fn input_validation_rejects_bad_pointer_length_combinations() {
        assert!(input_bytes(std::ptr::null(), 3).is_none());
        assert!(input_bytes(std::ptr::NonNull::<u8>::dangling().as_ptr(), 0).is_none());
        assert_eq!(input_bytes(std::ptr::null(), 0), Some(&[][..]));
        assert_eq!(parse_confirmed(2), Err(GV_INVALID_ARGUMENT));
        assert_eq!(parse_confirmed(1), Ok(true));
        assert!(parse_raw_id(&[0xff, 0xfe]).is_err());
    }
}
