# GitaView 原生契约 v1

状态：**N09 冻结版**。`schema.json` 覆盖全部 C ABI 函数的输入与成功 data；首版字段已冻结，之后的变更必须同时更新 schema、三个语言模型（Rust / Swift / C#）、样例与测试。反例（缺字段、未知枚举、非法组合）由各端 `deny_unknown_fields` / 显式枚举解码测试保证拒绝。

三方共同消费本目录：Rust（gitaview-core 序列化）、Swift（macOS Shared 模型）、C#（Windows Contracts）。fixture 是跨语言解码测试的共享样例，不是各自手写的三份副本。

Schema 验证：安装 `scripts/native/requirements-test.txt` 后运行 `python scripts/native/test-contract-schema.py`，原生 CI 的 Windows/macOS 两个任务均执行该检查。顶层 `anyOf` 允许多个调用入口具有相同形状；验证指定入口时选用对应的 `definitions`。Envelope 内部的互斥成功/失败分支仍使用 `oneOf`。draft-07 中的可空引用用 `anyOf` 显式包含 null 分支，不使用会被忽略的 `$ref` 同级类型约束。

## 基础规则

- JSON 键 camelCase；枚举小写 snake_case；文本 UTF-8。
- `schemaVersion = 1`（数据契约）、`abiVersion = 1`（C ABI），各自独立于产品版本。
- 时间为 Unix 毫秒 `u64`；未知时间显式 `null`。不混用秒、ISO 字符串与默认 Date 解码。
- `sessionId`：引擎每次启动生成新值；跨会话不比较 generation 大小。
- `generation`：会话内单调递增，仅计被接受并发布的扫描完成结果（含全局失败）；初始 0。配置变化丢弃的结果不占代数。
- `stateRevision`：会话内公开状态通知序号，初始 0；扫描开始 / 完成、请求入队、配置提交都递增。订阅去重用它，不用 generation。
- `requestId` / `settingsRevision` / `generation` / `stateRevision` 各司其职，禁止互相替代。
- 未知字段策略：反例必须拒绝（`additionalProperties: false`），不允许解码默认值隐藏不匹配。
- 错误对象 `{code, message?}`：code 为稳定字符串（见错误码表），message 可选且安全（无用户路径、凭据、原始 stderr）。

## 冻结的结构（N09）

| 结构 | 用途 |
| --- | --- |
| `Envelope` | 所有 FFI 成功 / 业务错误返回的统一外层 |
| `ProbeRequest` / `ProbeResult` | 原型探针（导出已移除；DTO 保留供示例与参考） |
| `Relation` / `ReadState` / `RefreshState` / `ErrorObject` | 基础枚举与错误 |
| `ViewQuery` / `ViewResult`（RelationFilter / GroupCount） | 共享视图查询（N08）：组计数基于全部仓库、状态计数只基于所选组、排序与筛选协调、行携带 actions |
| `AppSnapshot` 及其子结构（RepoStatus / RepoActions / RefreshPolicy / RefreshProgress） | 应用快照（宿主窗口 / 菜单） |
| `WidgetSnapshot`（WidgetCounts / CompactCounts / WidgetRepo） | Widget 安全投影 |
| `SettingsData`（RepoSettings / GroupSettings）/ `SettingsSaveInput`（SettingsEditable / RepoGroupAssignment） | 设置读写（N06） |
| `EngineCreateInput` / `EngineCreateData`（startupWarningCode） | 引擎创建 |
| `ScanDirectoryInput` / `ScanDirectoryData` | 目录扫描候选（truncated） |
| `RepoAddInput` / `RepoAddData` / `RepoRemoveData` | 仓库增删 |
| `RepoOperationData`（repoOperation） | Fetch / Pull / Push 执行结果 |
| `RefreshRequestData` / `SnapshotWaitInput` | 刷新请求与等待 cursor |
| `RepoTargetInput` / `RepoTargetData`（repoTargetKind） | 打开目录 / 远端 |
| `LifecycleData` | request_stop / wait_stopped 响应 |

原始 ID 输入（repo_remove / repo_fetch / repo_pull / repo_push）不是 JSON：直接传 UTF-8 ID 字节。confirmed 为 0 / 1 字节。

关键语义（详见 docs/native-app-implementation.md §6）：

- 五种关系 `synced / local_ahead / remote_ahead / diverged / no_remote` 只在 `readState = ok` 时出现；读取失败是独立 `readState = error`，不是第六种关系；`pending` 不是关系。
- pending / error 时未知 Git 字段显式 null；`ok + no_remote` 的 ahead / behind / comparisonBranch 可为 null。
- Widget 投影不包含 `path`、完整远端 URL、凭据或原始 Git 输出；错误只保留 code。
- 紧凑四桶 synced / syncable / needs_attention / no_remote：error 计入 needs_attention；pendingCount 单独展示；四桶之和 + pendingCount = total。
- 视图查询：组计数基于全部仓库；状态计数只基于所选组；行依次应用分组 → 状态筛选 → 搜索；`relationFilter = pending` 非法（pending 仅出现在 `all`）；所选组消失或所选状态在组内不存在时有效筛选切回全部；搜索零结果不清空搜索。

## 错误码表（稳定 code）

`INVALID_ARGUMENT`、`INVALID_PATH`、`GIT_NOT_FOUND`、`REPO_NOT_FOUND`、`REPO_BUSY`、`ORIGIN_REQUIRED`、`COMPARISON_REQUIRED`、`CONFIRMATION_REQUIRED`、`OPERATION_UNAVAILABLE`、`SETTINGS_CONFLICT`、`SETTINGS_IO`、`GIT_FAILED`、`GIT_TIMEOUT`、`ENGINE_STOPPING`。

C ABI int32 状态映射：`REPO_BUSY → BUSY(4)`；参数结构 / 枚举非法 → `INVALID_ARGUMENT(5)`；`ENGINE_STOPPING → STOPPING(3)`；其他预期业务失败 → `ERROR(1)`；等待期限已到 → `TIMEOUT(2)`（不代表 Git 超时）；PANIC(6) 单独进入停止 / 恢复路径。

## 后续语言侧义务（N10）

- Swift Shared 模型与 C# Contracts 按本 schema 镜像解码（拒绝未知字段与未知枚举）；
- 三种语言读取同一 fixtures；错误码 → 数字状态映射测试；
- schema 变更规则：先改本文件与 schema.json，再同步三个语言模型与测试。

## fixtures

| 文件 | 场景 | 断言来源 |
| --- | --- | --- |
| `snapshot-mixed-8.json` | 8 仓库混合：synced 2、local_ahead 1、remote_ahead 1、diverged 1、no_remote 2、读取失败 1 | `expectations.json`（计数 + 排序 + 紧凑桶 2/2/2/2） |
| `snapshot-empty.json` | 空仓库列表，尚未首刷（refreshState=pending、时间为 null、generation 0） | 计数全 0 |
| `snapshot-pending-first.json` | 已配置 2 仓库、首轮扫描进行中（pending 行 + isRefreshing） | pending 计数独立 |
| `snapshot-global-failure-keeps-history.json` | 全局失败保留历史 repos；含新增 pending 仓库（配置变更后首刷失败） | 历史时间保留、错误码、删除不复活 |
| `snapshot-unborn-origin.json` | 有 origin、无初始提交 → ok + no_remote + hasOrigin=true | ahead/behind null |
| `snapshot-unicode.json` | 中文仓库 / 分支 / 分组 | 计数与排序 |
| `snapshot-manual.json` | 定时刷新关闭（手动模式） | staleness=Manual |
| `snapshot-stale-boundary.json` | 过期边界：恰好两倍间隔 vs 超过一分钟 | staleness 边界值 |
| `widget-unknown-schema.json` | 未知 schemaVersion 的 Widget 文件 | 两端解码必须拒绝 |

`expectations.json` 汇总每个 fixture 的期望计数、关注顺序与过期边界时间点；Rust 测试（`gitaview-core` / `gitaview-ffi`）以此为断言，Swift / C# 解码测试（N10）复用同一文件。
