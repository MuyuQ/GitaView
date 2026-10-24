/*
 * GitaView C ABI — 正式版（N09 冻结）。
 *
 * 状态码：OK=0 ERROR=1 TIMEOUT=2 STOPPING=3 BUSY=4
 * INVALID_ARGUMENT=5 PANIC=6。TIMEOUT 只表示等待期限（snapshot_wait /
 * wait_stopped）；Git 命令超时是业务错误 GIT_TIMEOUT（ERROR + envelope）。
 *
 * 成功 / 业务错误返回 UTF-8 JSON envelope：
 *   {"schemaVersion":1,"ok":true,"data":…,"error":null}
 * 错误时 ok=false、data=null、error 含稳定 code。
 * TIMEOUT / STOPPING 允许空缓冲区；其余非 OK 状态若有缓冲区必为错误 envelope。
 *
 * buffer 为 Rust 分配的字节序列（无 NUL 终止），只能由 gv_buffer_free
 * 释放一次；释放后结构体被清零。宿主先看返回状态，最后统一释放非空缓冲区。
 *
 * 生命周期：request_stop / wait_stopped 不计入在途业务调用计数；
 * destroy 仅在 Stopped 且无在途调用时成功（成功把 *engine 置 null），否则 BUSY。
 * engine_create 失败保证 *engine 为 null。
 */

#ifndef GITAVIEW_H
#define GITAVIEW_H

#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct GvEngine GvEngine;
typedef struct { uint8_t *data; size_t len; } GvBuffer;

/* ABI 版本；当前为 1。宿主加载库后先核对再调用其他函数。 */
uint32_t gv_abi_version(void);

/* 创建引擎。输入 {"dataDirectory":"…","logDirectory":"…"}（绝对私有路径）；
 * 成功 data {"sessionId","abiVersion","startupWarnings"}；
 * 失败保证 *engine 为 null 并返回错误 envelope。 */
int32_t gv_engine_create(const uint8_t *json, size_t len,
                         GvEngine **engine, GvBuffer *reply);

/* 请求停止：幂等，唤醒全部等待者；data {"lifecycle":"stopping"|"stopped"}。 */
int32_t gv_engine_request_stop(GvEngine *engine, GvBuffer *reply);

/* 等待停止完成：成功 data {"lifecycle":"stopped"}；
 * 期限到仍清理完 → TIMEOUT（空缓冲区，不代表可以 destroy）。 */
int32_t gv_engine_wait_stopped(GvEngine *engine, uint32_t timeout_ms,
                               GvBuffer *reply);

/* 销毁引擎：仅在 Stopped 且无在途调用时成功并把 *engine 置 null；否则 BUSY。
 * 无 reply。 */
int32_t gv_engine_destroy(GvEngine **engine);

/* 当前完整设置（含 revision 与仓库记录）。 */
int32_t gv_settings_get(GvEngine *engine, GvBuffer *reply);

/* 保存可编辑字段 {"expectedRevision","settings":{groups,defaultGroupId,
 * refresh,repoGroups}}；冲突 → SETTINGS_CONFLICT。 */
int32_t gv_settings_save(GvEngine *engine, const uint8_t *json,
                         size_t len, GvBuffer *reply);

/* 目录扫描 {"path"} → {"paths":[…],"truncated":bool}；候选不自动添加。 */
int32_t gv_scan_directory(GvEngine *engine, const uint8_t *json,
                          size_t len, GvBuffer *reply);

/* 添加仓库 {"path","groupId":null|string} → {"repo","settingsRevision"}；
 * 重复规范路径返回既有记录。 */
int32_t gv_repo_add(GvEngine *engine, const uint8_t *json,
                    size_t len, GvBuffer *reply);

/* 移除仓库：输入 UTF-8 原始 ID 字节（非 JSON 字符串）→ {"settingsRevision"}。 */
int32_t gv_repo_remove(GvEngine *engine, const uint8_t *id,
                       size_t len, GvBuffer *reply);

/* Fetch：输入 UTF-8 原始 ID 字节 → {"repoId","operation":"fetch"}；
 * 成功仅表示命令执行完成，新状态由后续刷新产生。 */
int32_t gv_repo_fetch(GvEngine *engine, const uint8_t *id,
                      size_t len, GvBuffer *reply);

/* Pull / Push：confirmed 仅接受 0 / 1；核心执行前再次校验状态与确认。 */
int32_t gv_repo_pull(GvEngine *engine, const uint8_t *id, size_t len,
                     uint8_t confirmed, GvBuffer *reply);
int32_t gv_repo_push(GvEngine *engine, const uint8_t *id, size_t len,
                     uint8_t confirmed, GvBuffer *reply);

/* 刷新请求 → {"sessionId","requestId"}；完成与否看快照的 completedRequestId。 */
int32_t gv_refresh_request(GvEngine *engine, GvBuffer *reply);

/* 当前快照；初始 generation 0 带未就绪状态，不伪造成功。 */
int32_t gv_snapshot_get(GvEngine *engine, GvBuffer *reply);

/* 等待公开状态变化。cursor {"sessionId","stateRevision"}；
 * 停止 → STOPPING（空缓冲区）；期限到 → TIMEOUT（空缓冲区）。 */
int32_t gv_snapshot_wait(GvEngine *engine, const uint8_t *cursor_json,
                         size_t len, uint32_t timeout_ms, GvBuffer *reply);

/* 视图查询 {"groupId":null|string,"relationFilter","query"} → ViewResult。 */
int32_t gv_view_query(GvEngine *engine, const uint8_t *json,
                      size_t len, GvBuffer *reply);

/* 当前安全 Widget 投影：不写磁盘、不调用系统组件 API、不含 path / 凭据。 */
int32_t gv_widget_snapshot(GvEngine *engine, GvBuffer *reply);

/* 打开目标 {"repoId","kind":"directory"|"remote"} → {"kind","value"}；
 * 复核目录存在或重读并验证当前 origin URL，宿主调用系统打开 API。 */
int32_t gv_repo_target(GvEngine *engine, const uint8_t *json,
                       size_t len, GvBuffer *reply);

/* 释放本库返回的缓冲区并把结构体清零；null 指针为安全 no-op。 */
void gv_buffer_free(GvBuffer *buffer);

#ifdef __cplusplus
}
#endif

#endif /* GITAVIEW_H */
