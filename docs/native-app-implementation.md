# GitaView 原生应用实施说明（GLM 执行版）

日期：2026-10-24。本文定义后续实现要求；文中的目录、接口和脚本除明确标为现有内容者外，均为待实现目标。

## 1. 如何使用这套材料

先阅读仓库 `AGENTS.md`，再按以下顺序阅读：

1. 本文件：产品边界、架构、数据与接口、平台实现和验收原则。
2. [任务清单](native-app-tasks.md)：按依赖执行的 N00–N18 任务。
3. [进度记录](native-app-progress.md)：当前任务、平台阻塞和验证证据；每轮开始先读，结束前更新。
4. [GLM 启动提示词](native-app-glm-prompt.md)：用户可以直接复制给执行模型。
5. [统一界面设计](native-app-ui-design.md)：两端在旧版基础上升级的共同布局、视觉与验收要求。

[已审查规划](native-app-roadmap.md)用于解释总体决策。本文件补充实施细节；若执行中发现两份文档不一致，先核对用户本轮约束，再将实际取舍同步到两份文档并记录原因。不得为了跟随旧规格恢复已排除的功能。

[执行方案审查记录](native-app-review.md)记录本轮补充及回归场景。它帮助检查遗漏，不替代本文的最终接口约定。

原项目 `DESIGN_AND_BUILD_SPEC.md`、`NATIVE_WIDGET_IMPLEMENTATION_PLAN.md`、`docs/native-desktop-widget-layer-plan.md` 和旧签名说明只供识别现有行为。它们中的 Tauri 技术栈、桌面贴附、旧版本兼容和旧发布步骤，不是原生版本要求。

本文最初用于编写交接方案，当前已有部分实现；实际状态以进度记录和验证证据为准。不能把文档、伪代码、模板或 mock 画面报告为已实现产品。

## 2. 已确定的范围

| 项目 | 实施要求 |
| --- | --- |
| 产品 | 多 Git 仓库状态总览，支持仓库管理、分组、筛选、Fetch / Pull / Push |
| macOS | SwiftUI 内容、原生菜单栏和正常窗口；必要时使用 AppKit；WidgetKit 系统小组件 |
| Windows | C# + WinUI 3、标准 Win32 托盘、正常窗口；Windows Widget Provider + Adaptive Cards |
| 共享实现 | Rust Git 执行、状态、设置、刷新调度和应用服务；C ABI 供两个宿主调用 |
| 界面设计 | macOS / Windows 使用统一产品设计，在当前旧版布局与视觉基础上升级；见统一界面设计文档 |
| 原生组件 | 两端小 / 中 / 大尺寸；全部仓库摘要、需要关注的仓库、更新时间、打开应用 |
| 运行方式 | 一个主应用实例持有扫描引擎；关闭窗口继续驻留；显式退出停止扫描 |
| 数据 | 使用全新设置目录和新 schema，首次启动为空仓库列表 |
| 分发 | macOS Developer ID 签名 / 公证后的 DMG；Windows 签名 MSIX |
| 系统范围 | 由选定稳定 SDK、工具链和实际使用 API 的正式支持范围确定；记录准确 OS 下限 |

明确排除：旧数据导入、旧 JSON 兼容、数据回退、旧安装升级、旧客户端再适配、WorkerW / Progman、跨进程 SetParent、特殊桌面层级、跨 Space 常驻复刻、WebView / Electron UI、libgit2 替换、自建 shell 面板、自动 Fetch、自选分组的小组件、独立后台扫描服务、自动更新和商店上架。

不需要维持旧设置字段、旧 Tauri IPC 或旧 Widget 文件。UI 保留旧版的信息架构、紧凑密度与视觉识别，在两端使用共同设计升级；原生字体度量和系统窗口装饰允许有差异。仍需保留 Git 行为、安全确认、设置一致性和平台原生交互的正确性。

菜单栏 / 托盘可以显示汇总并提供“打开仓库列表、刷新、设置、退出”。两端不用复制同一套菜单样式。小组件由系统负责添加、移除和放置，不实现一个虚假的跨平台组件开关。

## 3. 仓库现实与工作方式

当前代码有大量未提交变更，包括 `git/process.rs`、`status_refresh.rs` 和相关测试。开始执行时重新运行 `git status --short` 和针对目标文件的 diff；不要按旧提交覆盖工作区，不做 reset / clean / stash，不删除已有未跟踪文件。

现有代码的重点入口：

| 读取位置 | 要提取或核对的内容 |
| --- | --- |
| `src-tauri/src/git/commands.rs` | Git 定位、状态计算、origin 参数、30 秒本地 / 600 秒网络命令超时 |
| `src-tauri/src/git/process.rs` | Windows Job Object、无控制台启动、macOS 进程组、管道读取与超时 |
| `src-tauri/src/git/discovery.rs` | `.git` 文件 / 目录、扫描边界、跳过目录、符号链接和路径处理 |
| `src-tauri/src/git/remote.rs`、`operation_lock.rs` | URL 安全转换；同仓库网络操作立即拒绝重复请求 |
| `repo_operation.rs`、`app_commands.rs` | 前置条件、强制确认、执行时重新读取状态、配置读改写 |
| `repo_status.rs`、`status_refresh.rs` | 8 个状态 worker、一次扫描、后续请求合并、版本化快照 |
| `app_settings.rs`、`storage/store.rs` | 设置冲突、原子写入、损坏隔离；剥离 Tauri 路径 |
| `diagnostics.rs` | 日志脱敏和轮转；含 `log_window`，不能整个复制进 core |
| `src/lib/statusModel.ts` 及测试 | 排序、分组计数、搜索、动作可用性和筛选协调 |
| `src-tauri/widget-extension/` | SwiftUI 视图、Provider、模型、fixture 与测试经验 |
| `.github/workflows/` | 现有架构矩阵、签名检查经验；不能直接视为原生构建流程 |

先以复制并整理模块的方式提取新核心，保留旧参考实现；新应用只有一份生产用 Rust 核心。不要为了消除参考代码的重复而顺便重构旧客户端。

每轮完成一个可验证的任务或任务子项。实现前列明目标文件；实现后运行相关检查，再更新进度。不要一次铺开两端全部 UI 后再补核心与测试。

## 4. 目标结构与构建隔离

```text
Cargo.toml                       # 新增原生 workspace
Cargo.lock                       # 新 workspace 自己的锁文件
crates/
  gitaview-core/
    src/{lib,model,error,engine,refresh,settings,registry,view,widget}.rs
    src/git/{commands,process,discovery,remote,operation_lock}.rs
    src/storage/store.rs
    tests/
  gitaview-ffi/
    src/{lib,buffer,handle}.rs
    include/gitaview.h
    tests/
apps/
  macos/
    project.yml                  # 默认沿用现有 XcodeGen 工程管理方式
    GitaViewNative/
    GitaViewNativeWidget/
    Shared/
    Tests/
  windows/
    GitaView.Native.sln
    GitaView.Native/              # WinUI 3 主程序
    GitaView.Native.Interop/      # C ABI 包装
    GitaView.Native.Contracts/    # DTO 与 JSON 解码，供 Provider 复用
    GitaView.Native.WidgetProvider/
    GitaView.Native.Package/      # 或 SDK 正式支持的等价打包结构
    GitaView.Native.Tests/
contracts/native/v1/
  README.md
  schema.json                    # 完整 DTO 与请求 / 响应结构，正式导出前冻结
  fixtures/
scripts/native/
  build-macos.sh
  test-macos.sh
  package-macos.sh
  build-windows.ps1
  test-windows.ps1
  package-windows.ps1
docs/native-platform-matrix.md
docs/native-platform-acceptance.md
.github/workflows/native-ci.yml
```

目录允许随正式 SDK 模板调整，但须在进度中记录最终入口和命令，不能留下一个文档路径、另一个实际路径的状态。

根 workspace 初始形态：

```toml
[workspace]
members = ["crates/gitaview-core", "crates/gitaview-ffi"]
default-members = ["crates/gitaview-core", "crates/gitaview-ffi"]
exclude = ["src-tauri"]
resolver = "2"
```

两个新 crate 的 edition 首先使用 2021，减少与现有模块无关的改动。根 workspace 记录 release profile 和 Rust 工具链版本。`src-tauri/Cargo.lock` 保持独立，不把 Tauri 隐式并入新 workspace。Workspace 共用根锁文件和输出目录，需显式处理成员与 exclude。[Cargo：Workspaces](https://doc.rust-lang.org/cargo/reference/workspaces.html)

`gitaview-core` 与 `gitaview-ffi` 不依赖 Tauri、AppKit、WidgetKit 或 WinUI。core 允许用于 Git 子进程和文件替换的 OS API。旧 `build.rs` 会编译 Swift Widget bridge，不能复制到新 crate；原生 Swift 宿主直接调用 WidgetCenter。

FFI crate 输出 `staticlib` 与 `cdylib`，根据目标平台消费正确产物。根 `target/` 中 `.a` / `.dll` 与宿主架构必须匹配。Swift 链接所需系统库用实际目标链接验证确定，不通过不断忽略未定义符号掩盖问题。

## 5. 工具链、身份和平台记录

N00–N04 在 `docs/native-platform-matrix.md` 记录 OS 构建号、架构、Rust / SDK / IDE 的准确版本、最低系统版本、签名方式和是否有真机。使用当前稳定工具链并锁定版本，不照抄本文件中的某个历史 SDK 示例版本。

首批目标为 macOS arm64 / x86_64、Windows x64。若工具链正式不再支持某目标，记录证据并调整矩阵；若只是缺少设备，则标记未验收，不能写成平台不支持。Windows ARM64 为后续范围。

新身份的默认建议：macOS 主 bundle `com.gitaview.native`，Widget bundle `com.gitaview.native.widget`；Windows 包名 `GitaView.Native`。真实 Publisher、Team ID 与 App Group ID 来自可用签名身份，不填伪造值。App Group 的标识形式与权限按 Apple 对 macOS 的正式规则配置，主程序与扩展一致。

原生版本使用新协议 `gitaview-native://`，避免复用旧客户端的协议注册。允许的首版路由只有：

```text
gitaview-native://open
gitaview-native://repo/<percent-encoded-repo-id>
```

协议仅导航，不执行 Git 操作；忽略未知路由与参数，不接受本地路径、命令或 `confirmed=true`。未知 / 已删除的仓库 ID 打开总列表。协议在引擎初始化前到达时先存为待处理导航，初始化后再解析仓库。

## 6. 共享数据契约

N02 先建立 `contracts/native/v1/README.md`、`schema.json` 和可解析的 JSON fixture，再实现三个语言模型。原型期间可调整命名，但 N09 实现正式导出前必须固定首版字段；之后的变更同时更新 schema、三个语言模型、样例和测试。不能靠解码默认值隐藏不匹配。

schema 必须覆盖完整 Settings / SettingsSaveInput、RepoStatus、Snapshot、ViewResult、WidgetSnapshot、Error / Envelope，以及每个 FFI 函数的输入与成功 data。逐字段写明类型、必填、可空、枚举、数值范围和未知字段策略；本文示例不是字段全集。缺字段、错误类型、未知枚举 / schema 和非法组合都要有反例。测试需验证真实 Rust 序列化结果及两端解码，不能只测试三个各自手写的样例。原型只冻结实际使用的子集，N09 的退出条件才是全部闭合。

### 6.1 基础规则

- JSON 键使用 camelCase；枚举使用文中指定的小写值；文本 UTF-8。
- `schemaVersion = 1`，`abiVersion = 1`，各自独立于产品版本。
- 时间为 Unix 毫秒 `u64` / Swift `UInt64` / C# `ulong`，显式转换；不混用秒、ISO 字符串与默认 Date 解码。
- 引擎每次启动生成新的 `sessionId`，会话内 generation 单调递增；跨会话不比较 generation 大小。
- 初始 generation 为 0；每次被接受并发布的扫描完成结果递增一代，包括全局失败。因配置已变化而丢弃的结果不占代数。
- `stateRevision` 是会话内公开状态的通知序号，初始为 0；扫描开始 / 完成、请求入队、配置提交等可见变化都递增。订阅和 Widget 写入去重使用它，不能只看 generation。
- `requestId` 标识刷新请求，`settingsRevision` 标识配置，`generation` 标识扫描结果，`stateRevision` 标识可见状态；均使用无符号 64 位，禁止互相替代。
- 仓库 ID 与分组 ID 为稳定机器标识，不用界面文字作标识。每仓库最多属于一个分组；`groupId = null` 表示未分组，筛选的 `groupId = null` 表示全部仓库。
- 两种 null 发生在不同字段对象中；UI 的“全部分组”是虚拟筛选，不持久化为真实分组。首版不提供单独“仅未分组”筛选，避免复用 null 造成歧义。
- 配置包含独立 revision；修改设置、添加 / 移除仓库时递增。revision 用于保存冲突，不用扫描 generation 代替。

### 6.2 仓库状态

成功读取结果必须包含五种关系之一：`synced`、`local_ahead`、`remote_ahead`、`diverged`、`no_remote`。失败结果是独立 `readState = error`，`relation = null`，不能伪装成 no_remote。

尚未读取的已配置仓库使用 `readState = pending`、`relation = null`、`error = null`，不是第六种 Git 关系。pending / error 时未知 Git 字段显式为 null，不虚构分支、领先数或 origin 状态；网络 actions 全为 false。目录入口仍按已验证路径处理。已读成功的 no_remote 可以有未知的比较计数，字段可空规则须在 schema 固定。

示例结构（单仓库）：

```json
{
  "id": "repo-api",
  "name": "api",
  "path": "/sample/projects/api",
  "groupId": "group-backend",
  "readState": "ok",
  "relation": "remote_ahead",
  "branch": "main",
  "hasCommit": true,
  "hasOrigin": true,
  "comparisonBranch": "main",
  "ahead": 0,
  "behind": 2,
  "remoteUrl": "https://github.com/example/api",
  "error": null,
  "actions": {
    "canOpenDirectory": true,
    "canOpenRemote": true,
    "canFetch": true,
    "canPull": true,
    "canPush": false
  }
}
```

已有 GitBranchState 提供 ahead / behind / remote_branch 等信息，`hasCommit` 在已有状态算法中已经判断；将必要结果保留在结构中即可。不新增工作区修改统计，也不把 ahead / behind 称为未提交文件数量。

错误对象使用稳定 code 与可选安全 message；保留内部诊断上下文，但 Widget 不包含原始 stderr、用户路径或带凭据 URL。

### 6.3 应用快照与时间语义

```json
{
  "schemaVersion": 1,
  "sessionId": "sample-session",
  "generation": 4,
  "stateRevision": 12,
  "settingsRevision": 2,
  "attemptedAtMs": 1792800000000,
  "lastSuccessAtMs": 1792799700000,
  "refreshState": "failed",
  "refreshError": { "code": "SETTINGS_IO", "message": "无法读取设置" },
  "refreshPolicy": { "enabled": true, "intervalMinutes": 5 },
  "refresh": {
    "isRefreshing": false,
    "requestedRequestId": 7,
    "completedRequestId": 7
  },
  "repos": []
}
```

`repos` 在全局失败时保留最近一次完整收集结果，并按最新配置移除已删项、更新分组、补入 pending 项；示例空列表也可能表示此前没有成功结果且未配置仓库。`lastSuccessAtMs` 此时保持原值，没有历史结果则为 null。单仓库读取失败属于一次已完成的收集，结果中标记该仓库错误，不让其拖垮所有仓库。

refreshState 只描述最近完成尝试：pending 表示尚未完成首次尝试，ready 表示收集完成，failed 表示全局失败。pending 时 attemptedAtMs / lastSuccessAtMs 均可为 null；ready / failed 时 attemptedAtMs 必须有值。attemptedAtMs 记录最近一次被接受的尝试的完成时间，成功时 lastSuccessAtMs 取该值。正在刷新用 `refresh.isRefreshing` 表示，有待处理请求用 requestedRequestId > completedRequestId 表示；UI 不从 FFI 请求已返回推断刷新已完成。

设置读改写、仓库删除和分组变更与扫描结果不能交错复活旧记录：扫描捕获 settingsRevision，发布前检查。若 revision 已变化，丢弃过时结果并合并一次新扫描；停止阶段不再补扫。

配置保存成功返回前，核心提交与新配置一致的可见状态并唤醒订阅：移除仓库立即从列表、计数、托盘和 Widget 投影删除；分组变更立即重投影；新仓库立即出现为 pending。保留仍存在仓库的历史结果和原成功时间；配置提交本身不增加 generation、不表示 Git 扫描成功。首次启动同样从新设置建立 pending 行。平台显示可异步更新，不能因下一轮扫描失败而重新出现已删记录。

开启定时刷新时，距离最后完整收集成功超过两个配置间隔即为过期；等于阈值尚不标记过期。关闭时显示手动刷新及时间。单仓库错误始终单独提示；有成功收集时间不表示每个仓库都读取成功。

### 6.4 设置

新设置包含 schemaVersion、revision、仓库记录、分组、defaultGroupId、refresh 和原生产品所需偏好。默认仓库 / 分组均为空，defaultGroupId 为 null，定时刷新启用，间隔 5 分钟，可设范围 1–60 分钟。

不存储可关闭的 Pull / Push 确认字段。新设置不包含 allowWidgetDrag、compactMode、alwaysOnTop。开机启动是平台宿主能力；UI 显示系统实际状态，不以一个 JSON 布尔值宣称已经配置成功。

通用 settings_save 修改偏好、分组和已有仓库的分组归属，必须提交 expectedRevision；添加、移除和路径变更不通过完整 JSON 偷渡。首版路径变更使用移除后重新添加。分组名称唯一且非空，删除分组时成员归为未分组，默认分组不存在时归回全部。

配置写入按“校验候选 → 原子落盘 → 提交内存与 revision → 发布配置变化”执行。写入失败不得改变内存、revision 或对外投影。设置写入串行器可覆盖落盘步骤，不能占住刷新状态锁或阻塞 UI 线程。损坏配置隔离成功后加载默认值，并在 engine_create 的 `startupWarnings` 返回 `SETTINGS_RECOVERED` 供 UI 提示；无法读取 / 隔离 / 建立数据目录时初始化失败，不静默当成首次空配置。startupWarnings 是本次启动的结果，不作为永久设置。

### 6.5 Widget 快照

Widget 快照由 Rust 投影生成，两端消费同一种 JSON：包含 schemaVersion、sessionId、generation、stateRevision、settingsRevision、尝试 / 成功时间、刷新策略、安全错误码、全部仓库计数及有界关注列表。默认关注列表最多 10 项，按共享排序选取读取失败、分叉、远程领先、本地领先，UI 按尺寸显示适合的前几项。pending 计入 total 和独立 pendingCount，不挤入五种关系或 error；组件显示“部分仓库尚未读取”，不能显示全部同步。

不包含 path、完整远端 URL、凭据或原始 Git 输出。错误展示由稳定错误码映射为原生文案。摘要始终覆盖全部仓库，不能用截断后的关注列表计算总数。fixture 必须证明摘要计数正确、裁剪不泄漏路径。

初版 fixture 至少包括：8 仓库混合结果、空仓库、尚未首刷、配置新增 pending / 删除后首刷失败、全局失败保留历史、单仓库失败、过期、手动刷新、无初始提交且有 origin、未知 schema、Unicode。混合结果固定为 synced 2、local_ahead 1、remote_ahead 1、diverged 1、no_remote 2、读取失败 1，总数 8；另设带 pending 的样例验证计数，不改动这个基准样例。

## 7. Git 与仓库操作契约

| 场景 | 允许的操作 |
| --- | --- |
| 成功读取，存在 origin，但无比较分支或尚无提交 | Fetch；不允许 Pull / Push |
| synced | 有 origin 时可 Fetch；不显示可执行的 Pull / Push |
| remote_ahead | Fetch、确认后 Pull |
| local_ahead | Fetch、确认后 Push |
| diverged | Fetch、确认后 Pull 或 Push；保留 Git 本身的拒绝结果 |
| 没有 origin | 不允许 Fetch / Pull / Push |
| 读取失败 | 保留打开目录入口；网络操作禁用，重新读取后再判断 |

Pull / Push 参数中 confirmed 缺失或为 false 时，在启动 Git 前拒绝。UI 原生对话框必须指出仓库与操作，取消时不得调用写操作。Git 执行前在仓库网络锁内重新获取状态和目标分支，不能只用旧快照 actions。

origin 是唯一管理的远端。沿用现有 upstream 选择和 origin 同名分支后备规则；本地分支与 origin 目标分支名称不同时，使用已验证的远端目标。参数构造沿用 `origin_fetch_args` / `origin_pull_args` / `origin_push_args`，不拼接 shell 字符串。

一个仓库同时只执行一个网络操作，重复请求立即返回 REPO_BUSY，不自动排队。不同仓库可并行。读取与写入不能持有一个全局锁贯穿网络调用；仓库锁与设置锁固定顺序，网络调用期间不持有设置锁。移除正在执行网络操作的仓库返回 REPO_BUSY。

建议统一顺序：先短暂读取并释放设置锁以定位路径 → 获取仓库操作槽 → 再短暂读取 / 修改设置并复核仓库仍存在 → 释放设置锁 → 执行 Git。移除也使用同一仓库槽再修改设置；不得在持有设置锁时等待仓库槽。已开始操作的路径由不可变副本持有，首版不支持原地修改仓库路径。

操作成功、非零退出和超时都可能需要刷新。使用 guard / finally 保证锁释放和刷新请求；若引擎进入 Stopping，则下次启动重新扫描。Push 超时不能报告远端一定没变更，也不能自动再 Push。

保留用户现有 Git / SSH / credential helper 环境，不在新版本中实现登录系统或收集口令。保持非交互 Git 环境与无控制台启动；认证失败给出可执行的检查提示。Git 不存在时不偷偷安装或弹终端。

目录扫描沿用当前深度 8、条目检查预算 200000、15 秒 deadline 和跳过规则，停止时在遍历点检查 stop 标志。15 秒不是单个不可中断文件系统调用的硬保证；不得靠一个 UI 超时假装底层扫描已停止。扫描只发现，UI 让用户选择要添加的仓库；添加在核心中验证并规范化路径。

## 8. 刷新、投影与关闭

### 8.1 刷新状态机

初始化后请求首次扫描。定时器只读本地 Git，不 Fetch。一次最多一个全量收集；扫描期间反复刷新合并成最多一次后续请求。配置变更通知调度器重新计算期限，关闭定时刷新仍允许首次和手动刷新。

所有刷新来源使用同一个请求序号：每个已接受请求获得递增 requestId；调度器只保存最新请求号，最多一个扫描和一个待扫标记，不建立请求队列。扫描开始时捕获当时最新请求号；完成并接受结果后，将 completedRequestId 推进到捕获值。扫描期间的新请求由下一轮覆盖。因配置变化丢弃结果时不推进 completedRequestId，下轮继续覆盖未完成请求。全局失败也完成所覆盖的请求，但 UI 必须显示失败。宿主等待自身请求时检查同一 session 下 completedRequestId >= requestId；进入 Stopping 后退出等待，不伪造完成。

定时期限使用单调时钟；展示时间使用 Unix 毫秒。宿主收到原生恢复 / 唤醒通知时，定时开启则合并请求一次刷新，不按休眠期间遗漏的间隔补 N 次；手动模式仍保持手动。时间回拨导致成功时间在未来时显示时间异常 / 等待刷新，避免无符号减法下溢或把数据永久当新。两个 Widget 用相同时间 fixture 验证边界；系统实际重绘时间仍由平台决定。

复用现有最多 8 worker 的策略，停止时不再从队列取新仓库。单次 Git 命令的超时覆盖子进程退出和 stdout / stderr 读取，不能重新退化为阻塞 read_to_end。

刷新状态锁内只更新状态与克隆快照，不执行 Git、磁盘 I/O、主线程同步或系统 Widget API。配置提交与扫描发布采用固定且无反向获取的锁顺序，确保 revision 检查与结果接受属于同一个同步步骤。先提交最新快照并通知等待者，再由宿主异步写 Widget 数据与更新菜单；平台失败不阻塞核心。

每个宿主只保留一个 Widget 写入器和一份最新待发布投影，顺序写入并按 sessionId / stateRevision 忽略过时数据。不能每来一次快照就启动独立写线程，让旧写入最后覆盖新文件。每个写入器只接受所属引擎会话；重建引擎前先停止并等待旧写入器结束，不能用 sessionId 的字典序判断新旧。写入失败记录诊断并等待下一次通知或有界退避重试，不无界堆积任务或忙循环。

冷启动尚未首刷时，若已有同 schema 且 settingsRevision 与当前配置一致的有效 Widget 文件，保留它，避免用 generation 0 的 pending 内容清空上次结果。该缓存只供 Widget 展示，不回灌核心为成功状态。首轮完成或配置实际变化后再用当前投影替换；没有有效缓存则正常发布 pending。删除仓库或配置恢复为空必须及时发布新投影，不能以保留历史为由继续展示已删内容。此规则只读取新原生应用自己的共享文件，不读取旧版数据。

正常退出在停止引擎与等待在途调用完成后，按上述缓存规则尝试写入宿主已收到的最后有效待发布投影，再结束写入器；无需等待系统 Widget 重绘。共享容器不可用时主应用仍可管理仓库，明确记录组件发布失败，不退回旧 home 路径或临时目录假装共享成功。

### 8.2 共享视图投影

由 Rust 查询函数接收 groupId、relationFilter、query，返回当前快照版本、有效筛选、组计数、状态计数、已排序行、摘要和 actions。两个 UI 不重复实现业务计数与动作规则。

组计数基于全部仓库；状态计数只基于所选组，不随状态筛选或搜索进一步缩小。行列表依次应用分组、状态 / 读取失败筛选、搜索。搜索 name / path / branch / group name，Unicode 按明确的统一规则处理；首版使用 Unicode lowercase 后匹配，不承诺拼音检索。

排序为：读取失败、pending、diverged、remote_ahead、local_ahead、synced、no_remote；同档按统一的名称比较再按 ID 打破并列。首版使用 Rust 名称 lowercase 字典序，避免两端 localeCompare 的结果不同；这不承诺中文拼音排序。紧凑摘要为 synced、syncable、needs_attention、no_remote，error 计入 needs_attention；pendingCount 单独展示，四桶之和加 pendingCount 等于 total。pending 仅在“全部状态”中展示，不新增关系筛选。

所选组消失时切回全部；所选状态在新组不存在时切回全部；搜索零结果不清空搜索。新原生窗口可在无结果时显示空态，不改变核心计数含义。

公开状态版本与筛选查询版本分别管理：同一 stateRevision 的两个查询可能有不同筛选。原生包装增加 query request 序号，仅应用当前会话的最新请求；若返回 stateRevision 已落后于本地已知状态则丢弃并查询最新状态。不能只用 generation 丢弃同代的配置变化，也不能只用 stateRevision 丢弃同版的新筛选。

### 8.3 生命周期

`Running → Stopping → Stopped`。request_stop 幂等，停止接收业务操作、启动新扫描及后续 Git 命令，唤醒条件变量等待者。已开始的单次 Git 命令按既有 30 / 600 秒超时结束；网络操作尚在预检查阶段时停止，不再继续启动写命令。

wait_stopped 在后台等待全部 worker 与在途业务调用结束。宿主显示“正在退出”，UI 主线程保持可响应；句柄和 DLL 不提前释放。等待超时仅表示仍在退出，不代表可以 destroy 或将任务标记取消成功。

destroy 仅在 Stopped 且没有其他 FFI 调用时允许，提前调用返回 BUSY。正常退出不新增强制结束 Pull / Push 按钮。异常崩溃和系统强制终止不承诺正常清理；下一次启动重新获取真实状态。

## 9. C ABI 设计

以下是目标头文件的接口草案，N09 必须实现头文件、Rust 导出和 Swift / C# 声明的一致性测试。JSON 提供 DTO，C 签名提供固定操作边界，不暴露通用 execute(command) 或 Git 参数数组。

```c
#include <stdint.h>
#include <stddef.h>

typedef struct GvEngine GvEngine;
typedef struct { uint8_t *data; size_t len; } GvBuffer;

uint32_t gv_abi_version(void);
int32_t gv_engine_create(const uint8_t *json, size_t len,
                         GvEngine **engine, GvBuffer *reply);
int32_t gv_engine_request_stop(GvEngine *engine, GvBuffer *reply);
int32_t gv_engine_wait_stopped(GvEngine *engine, uint32_t timeout_ms,
                              GvBuffer *reply);
int32_t gv_engine_destroy(GvEngine **engine);

int32_t gv_settings_get(GvEngine *engine, GvBuffer *reply);
int32_t gv_settings_save(GvEngine *engine, const uint8_t *json,
                        size_t len, GvBuffer *reply);
int32_t gv_scan_directory(GvEngine *engine, const uint8_t *json,
                         size_t len, GvBuffer *reply);
int32_t gv_repo_add(GvEngine *engine, const uint8_t *json,
                   size_t len, GvBuffer *reply);
int32_t gv_repo_remove(GvEngine *engine, const uint8_t *id,
                      size_t len, GvBuffer *reply);
int32_t gv_repo_fetch(GvEngine *engine, const uint8_t *id,
                     size_t len, GvBuffer *reply);
int32_t gv_repo_pull(GvEngine *engine, const uint8_t *id, size_t len,
                    uint8_t confirmed, GvBuffer *reply);
int32_t gv_repo_push(GvEngine *engine, const uint8_t *id, size_t len,
                    uint8_t confirmed, GvBuffer *reply);
int32_t gv_refresh_request(GvEngine *engine, GvBuffer *reply);
int32_t gv_snapshot_get(GvEngine *engine, GvBuffer *reply);
int32_t gv_snapshot_wait(GvEngine *engine, const uint8_t *cursor_json,
                        size_t len, uint32_t timeout_ms, GvBuffer *reply);
int32_t gv_view_query(GvEngine *engine, const uint8_t *json,
                     size_t len, GvBuffer *reply);
int32_t gv_widget_snapshot(GvEngine *engine, GvBuffer *reply);
int32_t gv_repo_target(GvEngine *engine, const uint8_t *json,
                      size_t len, GvBuffer *reply);
void gv_buffer_free(GvBuffer *buffer);
```

JSON init 只接受主应用私有数据目录、日志配置等初始化参数；不接受任意脚本。Widget 输出路径由平台宿主掌握，核心只产生安全投影。repo_target 请求固定 kind（directory / remote），核心按 repo ID 返回验证后的路径或 HTTP(S) URL，宿主调用系统打开 API。

各函数 payload 与 data 约定如下；表中 JSON 都是 envelope 内部的数据，不省略外层统一错误协议：

| 函数 | 输入 | 成功 data / 行为 |
| --- | --- | --- |
| engine_create | `{dataDirectory, logDirectory}`，绝对私有路径 | `{sessionId, abiVersion, startupWarnings}`；警告为空数组或稳定 code；创建一次主引擎，安排首次读取 |
| settings_get | 无 | 当前完整设置，包含 revision 与仓库记录 |
| settings_save | `{expectedRevision, settings}`，settings 仅含可编辑的分组、归属、默认组、刷新偏好 | 保存后完整设置；不允许替换仓库 ID / 路径或增删仓库 |
| scan_directory | `{path}` | `{paths, truncated}`；路径候选，不自动添加；预算中止时 truncated=true |
| repo_add | `{path, groupId}`，groupId 可为 null | `{repo, settingsRevision}`；重复规范路径返回既有记录，不重复插入 |
| repo_remove | UTF-8 原始 ID 字节，非 JSON 字符串 | `{settingsRevision}`；不存在则 REPO_NOT_FOUND |
| repo_fetch / pull / push | UTF-8 原始 ID 字节；后两者另有 confirmed | `{repoId, operation}`；表示命令执行成功，新的状态由后续刷新产生 |
| refresh_request | 无 | `{sessionId, requestId}`；只请求不等待；按 §8.1 的 completedRequestId 判断完成 |
| snapshot_get | 无 | 当前快照；初始 generation 0 带未就绪状态，不伪造成功 |
| snapshot_wait | `{sessionId,stateRevision}` + timeout_ms | 公开状态变化后的快照；不同会话立即返回；TIMEOUT / STOPPING 独立处理 |
| view_query | `{groupId, relationFilter, query}` | sessionId / stateRevision / generation / settingsRevision、有效筛选、摘要、计数与排序行；relationFilter 为 all / error / 五种关系之一 |
| widget_snapshot | 无 | 当前安全 Widget 投影，不写磁盘也不调用系统组件 API |
| repo_target | `{repoId, kind}`，kind 为 directory / remote | `{kind,value}`；复核目录存在或重新读取并验证当前 origin URL |
| request_stop / wait_stopped | 前者无，后者有 timeout_ms | 生命周期状态；等待超时不销毁引擎 |

路径候选 truncated 的信息需在现有预算扫描器中显式保留，不能在未遍历完整目录时返回“已找到全部”。逐目录权限跳过可记录诊断，不把它混同为全局扫描器 panic。所有输入以模型校验，未知 operation / kind / filter 返回 INVALID_ARGUMENT，不回落为任意命令。

ABI 细则：

1. int32 返回固定状态：OK=0、ERROR=1、TIMEOUT=2、STOPPING=3、BUSY=4、INVALID_ARGUMENT=5、PANIC=6。跨语言不用 C enum 默认大小，不传 Rust bool；confirmed 只允许 0 / 1。
2. 调用方传入已清零且不持有未释放分配的 out buffer，函数再将有效 out buffer 清零；成功 / 业务错误返回 UTF-8 JSON envelope：`{"schemaVersion":1,"ok":true,"data":...,"error":null}`，错误时 ok=false、data=null、error 含稳定 code。TIMEOUT / STOPPING 可为空 buffer；其他非 OK 状态若有 buffer 则必须是错误 envelope。包装层先看返回状态，最后统一释放非空 buffer。
3. create 失败保证 out engine 为 null。destroy 成功将调用方指针设为 null；非空任意无效指针不是可安全恢复的业务输入，包装层必须保证生命周期。
4. buffer 是 Rust 分配的字节序列，无 NUL 终止要求；由 gv_buffer_free 释放并清零。可使用 Box 字节切片，不能按未知 capacity 重建 Vec。只允许释放本次库返回的原始 buffer，不复制后重复释放。
5. 输入在调用期间借用，Rust 如需留存必须复制。size_t 对应 Swift Int / C# nuint，uint64_t 对应无符号 64 位。先检查长度与空指针组合，再建立切片；包装层不传任意内存地址。
6. 每个可能 panic 的导出入口阻止 unwind 穿越 ABI；release 不使用会让该承诺失效的 panic=abort。捕获的 panic 返回 PANIC，令宿主进入错误恢复 / 停止流程，不能无条件继续使用未知状态。
7. 同一引擎允许后台等待、读快照、设置和不同仓库业务调用并行；全局引擎锁不能覆盖等待或 Git 执行。request_stop 可与在途调用并发；destroy 不与任何调用并发。
8. Swift wrapper 显式管理 owner 与在途调用；C# 使用 SafeHandle / 受控 owner 管理，正常路径显式异步关闭后再释放。每个调用保证 SafeHandle 存活，不在 finalizer 中等待网络或卸载 DLL。[Microsoft：Native interoperability best practices](https://learn.microsoft.com/en-us/dotnet/standard/native-interop/best-practices)

稳定业务错误至少包含 INVALID_PATH、GIT_NOT_FOUND、REPO_NOT_FOUND、REPO_BUSY、ORIGIN_REQUIRED、COMPARISON_REQUIRED、CONFIRMATION_REQUIRED、OPERATION_UNAVAILABLE、SETTINGS_CONFLICT、SETTINGS_IO、GIT_FAILED、GIT_TIMEOUT、ENGINE_STOPPING。code 与 UI 文案分离，不能解析中文字符串决定控制流。

状态码映射固定：REPO_BUSY → BUSY；参数结构 / 枚举非法 → INVALID_ARGUMENT；ENGINE_STOPPING → STOPPING；其他预期业务失败（包括 GIT_TIMEOUT、SETTINGS_CONFLICT）→ ERROR。TIMEOUT 只表示 snapshot_wait / wait_stopped 的等待期限已到，不代表 Git 命令超时。PANIC 单独进入停止 / 恢复路径，不能被当普通业务错误重试；destroy 没有 reply，提前销毁仅返回 BUSY。

快照等待 cursor 为 `{sessionId,stateRevision}`。引擎正在停止则返回 STOPPING；否则不同会话或已有更新立即返回，相同版本后台等待。不能使用 `{sessionId,generation}`，因为配置变化和忙状态也要通知。正常等待超时、停止、业务错误三个分支必须区别处理。初始 generation 0 不是“全部仓库都已同步”。

request_stop / wait_stopped 等生命周期控制调用不计入它们正在等待清零的业务调用计数，避免 wait_stopped 等待自己。后台 snapshot_wait 由 stop 唤醒并退出，它持有的调用引用随后才释放。

## 10. 原生宿主通用要求

主程序按顺序完成单实例注册、路径解析、ABI 检查、引擎创建、快照订阅和 UI 更新。第二次启动只导航已有实例，不创建第二个引擎。配置冲突提示重载，不静默覆盖。

DLL / 静态库版本不匹配、路径失败或 engine_create 失败时，显示可恢复的启动错误与重试入口；不进入“空仓库且启动成功”的主界面，不发布空 Widget 覆盖历史。重试前释放失败调用的 buffer，并确认没有残留引擎 / 订阅循环。startupWarnings 非空时展示配置恢复提示，不能只写到日志。

使用一个后台快照消费循环；状态变动后查询最新视图，在 UI 线程提交状态。窗口关闭不停止循环。用户修改筛选和搜索时不启动 Git 扫描，只重新计算视图。刷新按钮与 Git 操作各有状态，不能因为单仓库 Fetch 把整张列表清空。

UI 必须包含仓库列表 / 详情操作、分组与状态筛选、搜索、最近读取时间、手动刷新、空态、初始加载、全局失败和单仓库错误。设置包含仓库添加 / 扫描 / 移除、分组管理、默认分组、刷新配置、开机启动和小组件使用说明。

主应用布局与视觉遵循 [统一界面设计](native-app-ui-design.md)，以当前 WidgetExpanded / RepoTable / SettingsShell 及主题 token 为基线。两端共享页面结构、筛选顺序、选中行展开操作、设置分区与语义主题；不得按 SwiftUI / WinUI 默认模板各自重做另一套产品导航。主应用内容的紧凑 / 明细模式与系统 Widget 分开，均不要求旧桌面层行为。

Pull / Push 取消不触发 FFI；成功和失败都有结果反馈。写操作禁用状态以 core 的 actions 为依据，执行时仍再次校验。未知 / 不安全远端 URL 不显示可打开入口。

本地化以中文为首版默认，使用平台字符串资源保留扩展能力，不在 Rust 里生成整段 UI 文案。原生图标和系统字体优先；状态始终有文字与数字。键盘、屏幕阅读器、深浅色、缩放和减少动画走平台机制。

## 11. macOS 工程要求

主应用与 Widget Extension 同一原生 Xcode 工程，默认使用 XcodeGen。新增 Shared 模型用于主应用与 Widget 解码；Swift wrapper 只链接到主应用，扩展不能链接完整 Rust 引擎。

主应用使用菜单栏常驻模式和正常状态 / 设置窗口；优先所选系统版本的原生 MenuBarExtra / Scene API，必要时 AppKit 接管生命周期。是否 accessory、窗口关闭与退出的处理需在所选 API 下测试，不能把旧 NSWindow level 代码带过来。

App Group URL 使用系统容器解析，不拼接 `~/Library/...`。主应用原子写 Widget JSON 后请求 WidgetCenter 重载；由 Swift 平台适配器处理写入节流并保留最新待发布数据，失败有诊断且下一次数据变化仍可重试。[Apple：Configuring app groups](https://developer.apple.com/documentation/xcode/configuring-app-groups)

Widget 只读取共享 JSON：三种尺寸使用同一个语义模型。适配系统 container background、内容边距和着色渲染；缺失数据提示打开应用，损坏 / 未知 schema 提示重新读取。对带成功时间的历史数据显示时间，不能始终退回装饰性 placeholder。

Timeline 在过期阈值安排状态变化，并设置合理兜底读取；时间线只是请求，实际更新服从系统调度。脱离调试器测试，不能用 Xcode 中及时更新推导正式环境固定时延。[Apple：Keeping a widget up to date](https://developer.apple.com/documentation/widgetkit/keeping-a-widget-up-to-date)

构建先验证单架构，再为矩阵分别构建 `.a` 和主应用 / 扩展。不要把某架构 `.a` 放入另一个架构；若采用 universal，需要显式验证所有 slice，而不是仅改文件名。

发布脚本检查 Team ID、bundle ID、App Group、主程序 / extension 的签名与 entitlement、嵌入 Rust 链接产物、Developer ID 与 hardened runtime；按原生工具链正确顺序签名、公证和 staple。不要使用 `codesign --deep` 作为修复错误 entitlement 的替代方法。

## 12. Windows 工程要求

使用当前稳定 WinUI 3 桌面模板，主程序默认 x64。Interop 和 Contracts 独立于 WinUI 控件；Contracts 可被 Provider 与测试使用，Provider 不引用 Interop 的引擎创建代码。SDK 模板与实际项目名写入最终构建脚本。

单实例在创建 XAML 窗口和引擎之前完成，协议激活重定向给主实例；按 Windows App SDK 正式 AppInstance 机制实现并测试安装后的行为。[Microsoft：Single-instanced WinUI app](https://learn.microsoft.com/en-us/windows/apps/windows-app-sdk/applifecycle/applifecycle-single-instance)

主程序按固定安装位置解析 Rust DLL，不依赖当前工作目录或用户 PATH 去寻找同名 DLL。WinUI DispatcherQueue 提交 UI；Task.Run 等后台执行阻塞 FFI。托盘使用正式 Shell_NotifyIcon 机制，Explorer 重启后重新注册标准图标，无桌面重父化。

MSIX 中包含主程序、Rust DLL 和 Provider COM server。Provider 清单、CLSID、可执行路径、图标和小 / 中 / 大模板定义必须一致。安装验证必须证明包身份、依赖、COM 激活以及主程序写入文件可被 Provider 读取，不能只验证双击 exe。[Microsoft：C# Widget Provider](https://learn.microsoft.com/en-us/windows/apps/develop/widgets/implement-widget-provider-cs)

Provider 生命周期：

1. Create / Activate / 尺寸上下文变化读取最近快照；持久化必要的 widget instance ID / 状态。
2. 至少一个实例活跃时监听共享目录；原子 rename、快速覆盖、丢失通知和重读竞争都需恢复。
3. 更新 JSON 使用 WidgetManager，按实例当前尺寸选择 Adaptive Card 模板；卡片的计数和关注列表只来自安全投影。
4. 所有实例 Deactivate 后停止主动监听；下一次 Activate 必须重读，不依赖此前一定收到通知。
5. Delete 清理实例，Provider 被系统回收后无需常驻；不能创建另一个引擎来“补刷新”。

Provider 重启用 WidgetManager.GetWidgetInfos 重建现有实例信息，不能等系统重新 Create 才认识已固定的小组件。回调中复制 ID、尺寸等普通值，不能跨异步任务保留只在回调期间有效的 WidgetContext / 参数对象；共享实例表和更新任务须同步，删除后取消该实例的待更新。以上生命周期限制以所选 SDK 文档核对。[Microsoft：Provider 回调与启动恢复](https://learn.microsoft.com/en-us/windows/apps/develop/widgets/implement-widget-provider-cs)

文件监听不能处理“文件没变，但时间已过期”。本项目要求：活跃期间按最后成功时间和当前策略安排一次过期边界更新，即使主应用退出、没有新文件，也重新计算并提交过期卡片；等于阈值仍有效，因此触发时间须越过边界。最新快照到达时重设期限。所有实例失活后取消监听和计时；重新 Activate / 系统恢复时重读并计算当前状态。时间回拨显示异常并在活跃期间有界重查，不能忙循环。Provider 不为这一计时创建 Git 引擎，也不保证系统立即重绘。

稳定场景测试包括主程序未启动、主程序完全退出、Provider 重启、快速连续快照、Board 关闭再打开、尺寸变化、空态和协议冷启动。Windows Widgets Board 由系统托管，UI 不显示贴桌面开关。

## 13. 验证方法与构建入口

原生 workspace 建立后先生成并提交自己的 lockfile，后续使用 --locked。以下为目标命令，脚本尚未实现时不能报告已运行：

```text
cargo test --workspace --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo tree -p gitaview-core
cargo tree -p gitaview-ffi
```

依赖树应证明没有 Tauri / GUI 框架。平台编译只在正确宿主工具链上运行；Windows 下 cargo check 不能证明 macOS Swift 工程工作。

新增脚本的目标入口约定：

```text
bash scripts/native/build-macos.sh --arch arm64 --configuration Debug
bash scripts/native/test-macos.sh --arch arm64
bash scripts/native/package-macos.sh --arch arm64
powershell -NoProfile -File scripts/native/build-windows.ps1 -Architecture x64 -Configuration Debug
powershell -NoProfile -File scripts/native/test-windows.ps1 -Architecture x64
powershell -NoProfile -File scripts/native/package-windows.ps1 -Architecture x64
```

脚本内部使用被验证的 xcodebuild scheme / dotnet project 和 MSIX 打包工具，测试失败与依赖缺失返回非零；Windows 后台辅助程序保持隐藏。签名 secret 从 CI / 安全环境注入，不写进文档或日志。未签名开发构建与正式安装包分开标记。

测试分层：

| 层 | 必须验证 |
| --- | --- |
| 核心单元 | 关系 / pending、计数、动作、组规则、配置事务、三种版本与请求完成、刷新合并与停止 |
| Git 集成 | 本地临时 bare origin + 两个 clone，不访问公网；领先 / 落后 / 分叉、不同跟踪分支、空仓库、detached |
| 假进程 | 超时、子孙进程持有管道、连续输出、非零退出、停止期间不启动后续命令；用短注入期限测试 |
| ABI | 真实导出符号、布局、UTF-8、错误、buffer 释放、关闭竞争和 Swift / C# 真实调用 |
| 契约 | schema 校验真实 Rust 输出与两端解码；必填 / null / 反例；错误返回映射；Widget 没有 path / 原始错误 |
| 原生 UI | 选择仓库、筛选、设置冲突、确认取消 / 执行、加载与失败恢复 |
| 安装与系统组件 | 真正签名安装后的组件发现、冷启动缓存、无新写入时过期、重启恢复、协议、单实例、运行时依赖 |

文件系统测试使用唯一临时目录并清理，Git 测试设置局部作者身份，不改用户全局 Git 配置。假 Git / 故障注入接口仅供测试，不作为生产任意程序执行配置。

新原生 CI 使用独立 workflow 与路径过滤；旧发布 workflow 保持原状，避免随意推 `v*` 标签触发旧 Tauri 发布。此方案授权生成代码、脚本与本地可审查安装产物，不授权向外发布、上传证书、推送发布标签或发送其他聊天消息。

## 14. 阶段、环境受限与完成定义

总估算沿用已审查范围：33–51 人日，单人约 7–11 工作周。任务清单是该范围的细化，不能将每个任务再重复相加。完成原型后重估签名、工具链和平台实际投入。

N03 / N04 的平台原型是各自正式 UI 与组件实现的前置验证。若缺少一个平台，可以继续 N05–N10 的独立核心和可运行平台工作；不能跳过原型就把另一平台正式实现判为通过。若原型结果要求改变共享契约，先记录并调整，再继续相关实现。

N01 分共享 / Windows / macOS 子项；N02 只要求共享子项已固定，N03 / N04 分别要求本平台子项。N10、N17 同样分别维护平台子项，Windows 的 N14 只依赖 N10.Windows，macOS 的 N11 只依赖 N10.macOS。另一个平台阻塞不传播为所有后续任务阻塞；整体任务仍不能提前标成 verified。

任务状态使用 todo / in_progress / implemented_unverified / verified / blocked。verified 必须有实际命令或手动证据；implemented_unverified 适用于已写代码但缺少目标平台或签名验证，不等同完成。blocked 写明具体缺少什么及可以继续的独立任务，不因一个平台受阻停止全部工作。

平台实机结果记录精确 OS、架构、包版本、签名状态、步骤与观察。构建成功不代表组件可发现，组件可添加不代表数据读取正常，mock / fixture 画面不代表真实仓库已接通。

整个项目只有在核心 / FFI / 两端原生应用 / 两端系统小组件 / 声明矩阵安装验收全部 verified，且已知问题没有阻断时才报告完成。无法完成的验收诚实列出，交付可运行的已验证部分和续做入口。

每轮报告：当前任务、实际修改、已运行检查及结果、未运行原因、下一条可执行任务。依据保存在 [进度记录](native-app-progress.md)，避免依赖聊天历史记忆。

## 15. 执行前的事实核对入口

本文给出实施约定，不冻结外部 SDK 的具体版本。GLM 应先读本地锁文件、已安装工具和现有工程，再按所选版本查官方文档。优先使用以下来源，不从非官方示例猜签名、生命周期或包清单：

- Apple：App Groups、WidgetKit 时间线、原生 SwiftUI / AppKit 生命周期与 Developer ID 分发。
- Microsoft：WinUI、Windows App SDK AppInstance、C# Widget Provider、MSIX 与 .NET native interop。
- Rust：Cargo workspace、FFI / panic 规则和标准库进程 API。

发生 API 不一致时记录 SDK 版本、实际错误和官方依据，修正文档与代码。不要通过关闭确认、删除测试、引入 WebView、恢复 WorkerW 或把未验证平台移出表格来获得表面上的全绿。
