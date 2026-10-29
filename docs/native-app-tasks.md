# GitaView 原生改造任务清单

配套要求见 [实施说明](native-app-implementation.md)。这里列出的路径均为目标路径，执行前核对实际仓库。所有任务初始为 todo，实际状态只在 [进度记录](native-app-progress.md) 中维护。

## 执行顺序与通用验收

```text
N00 → N01.shared → N02
N02 + N01.Windows → N03 Windows 原型
N02 + N01.macOS   → N04 macOS 原型
N02 → N05 → N06 → N07 → N08 → N09 → N10
N04 + N10.macOS   → N11 → N12 → N13
N03 + N10.Windows → N14 → N15 → N16
已验证的平台构建步骤 → N17
N05–N17 的适用项 → N18
```

N01 / N10 / N17 按平台分别记录子项状态；shared / Windows / macOS 是同一任务内的检查项，不增加任务 ID。N03 / N04 缺环境时可继续 N05–N10；相应平台的正式实现不得绕过原型验证。N17 可以在每个平台有实际构建命令后逐步完善，不必等另一个平台。图示描述依赖，不要求创建多 agent 或新聊天。

每项任务都要满足：实际改动符合范围、相关检查运行通过、失败和未运行项记录清楚、没有覆盖原有工作区改动、进度文件能让下次执行恢复。任务表中的 verified 不接受“预计通过”。

N12 / N15 共同使用 [统一界面设计](native-app-ui-design.md)：macOS 与 Windows 在旧版基础上采用同一套产品布局与视觉。先提取旧版组件对应和共同 token，再分别用原生框架实现；不新增任务 ID，也不因缺少 Mac 阻塞共同设计与 Windows 实现。当前 Windows 原型不能直接当作两端最终 UI 标准。

## N00：工作区与执行环境盘点

**输入：** AGENTS.md、本套文档、当前工作区。

**操作：**

1. 记录 git 分支、HEAD、git status 和目标模块 diff，识别未跟踪 process / refresh 模块。
2. 查看本机 OS / 架构、Git、Rust、Cargo；Windows 检查 dotnet、MSBuild / Visual Studio、Windows SDK，macOS 检查 Xcode、xcodebuild、Swift、XcodeGen。
3. 只检查签名身份是否可用，不输出证书私钥、密码或 CI secret。
4. 在进度文件中记录哪些现有业务测试已运行；如果测试原 Tauri crate 会触发完整原生构建，可明确记录成本和结果，不为此更改旧工程。

**产物：** 进度文件的环境、工作区基线、平台限制和下一步。

**验收：** 能明确说出本机可实现 / 可验证哪些部分，不能只写“环境正常”。缺 Mac 不阻止继续 Windows 和纯 Rust 任务。

## N01：固定原生范围与工具链矩阵

**目标文件：** 新增 `docs/native-platform-matrix.md`，必要时更新原生产品说明。

**操作：**

1. 选定稳定 Rust / Xcode / .NET / Windows App SDK / IDE 版本，查对应官方支持范围。
2. 记录每个目标的最低 OS、构建宿主、架构、测试设备、SDK、签名身份、安装包类型。
3. 固定原生 bundle / 包身份、新数据目录、新协议 gitaview-native 和实际 App Group 方案；真实 Team ID / Publisher 不可猜填。
4. 给旧规格标明适用范围，避免后续任务把 WorkerW、Tauri、旧数据迁移当作需求。不要重写与新任务无关的历史文档。

**验收：** 有“已验证 / 待验证 / 环境阻塞”字段，不能把“无设备”写成“已取消支持”。软件版本精确到可复现版本，配置与记录一致。

shared 子项固定 Rust 工具链、共享范围和数据 / 协议规则即可供 N02 使用。Windows 与 macOS 子项分别冻结本平台工具链、身份、目录与签名方案；一个平台未定不阻塞另一个平台。全部子项验证前，N01 的汇总状态不能标为 verified。

## N02：新 workspace、最小契约与可复用探针

**依赖：** N00、N01.shared。**目标：** 根 Cargo.toml / Cargo.lock，两个 crate，contracts/native/v1。

**操作：**

1. 新 workspace 排除 src-tauri，创建 core 与 ffi 的最小可编译项目，明确 lib 输出类型。
2. 建立契约 README、schema.json 和 fixture，先覆盖原型所用数据类型、五种关系、pending / 读取错误、毫秒时间、sessionId / stateRevision / generation、settingsRevision 和 Widget 安全投影；标明 N09 实现正式导出前必须补齐的剩余 DTO。
3. 为 8 仓库混合样例写期望摘要与排序；空、失败、未知 schema 等样例按实施说明准备。
4. 提供仅供平台原型调用的最小固定 Git 读取探针，复用已有执行 / 超时方法，读取真实指定仓库并返回结构化 JSON。
5. 原型可以临时导出 `gv_probe_repository` 与 ABI / buffer 函数，明确标记为 probe；不提前伪造完整引擎。N09 移除临时导出或移入非发布示例，不能将探针当最终核心。

**验证：** 两个 crate 的 cargo check / test；JSON 语法与摘要断言；依赖树没有 Tauri。新生成的根 lockfile 纳入版本控制。

**验收：** 原型数据来自真实 Git，不用硬编码成功数据代替。旧 Cargo.lock 和旧 build.rs 不被误改。

## N03：Windows 原生与系统小组件纵向原型

**依赖：** N02、N01.Windows、可用 Windows 工具链。**目标：** apps/windows 最小实际工程。

**操作：**

1. 建立 x64 WinUI 主程序、Contracts / Interop 和 Provider，主界面显示探针返回的真实仓库名 / 分支 / 关系。
2. 同一 MSIX 包中注册 Provider，明确包身份、CLSID、可执行文件路径和组件定义。
3. 主应用将安全投影写入包数据目录，安装后的 Provider 读取，显示在 Board 中；记录实际路径。接线时可先用 fixture，但验收必须改为真实探针产生的数据。
4. 从组件点击新协议，证明能冷启动主程序；先验证最小尺寸，三种尺寸在 N16 完成。
5. 记录原始构建 / 打包 / 安装命令及错误；签名身份不足时只把已执行部分记为通过。

**验证：** 安装 MSIX → Board 可发现 → 添加 → 主程序写数据 → Provider 显示实际字段 → 冷启动深链。dotnet build 只覆盖其中一部分。

**验收：** 真实安装路径验证全部成功才是 verified；缺签名、Board 或安装权限则保留准确子项状态。

## N04：macOS 原生与 WidgetKit 纵向原型

**依赖：** N02、N01.macOS、可用 Mac / Xcode。**目标：** apps/macos 最小实际工程。

**操作：**

1. 建立主应用 / Widget Extension / Shared 模型，Swift 主应用链接目标架构 Rust 静态库。
2. 主应用显示探针读取的真实仓库字段。
3. 配置同一团队下正式可授权的 App Group；通过系统容器 URL 写安全快照，Extension 实际读取。
4. 构建并安装带有效签名的应用，Widget Gallery 可发现与添加，点击组件冷启动新协议。
5. 检查 entitlements 和签名，记录工具链与实际命令，不复用旧固定 home 路径或旧 Widget bridge。

**验收：** 脱离 Xcode 调试器完成“可发现 → 可添加 → 可读取主程序数据 → 可打开应用”。没有 Mac 时保留待执行项，不通过 Windows 静态审查宣称验证成功。

## N05：提取 Git 执行、状态与仓库基础

**依赖：** N02。**目标：** core/src/git、model、error、registry、核心测试。

**操作：**

1. 提取固定参数执行、Git 定位、远端规范化、扫描、进程处理和网络操作锁；扫描结果保留预算中止的 truncated 标志。
2. 分离读取错误与五种关系，保留已有 ahead / behind / 目标 origin 分支和 hasCommit 信息。
3. 移除对 Tauri、旧 Widget bridge 与窗口日志的依赖；日志只保留普通诊断能力。
4. 引入内部可测试执行器和短超时，不开放生产任意程序执行参数；提供 stop 检查位置。
5. 沿用当前比较规则，遇到现有行为缺陷先写回归，不在提取中自行改变 Pull / Push 语义。

**验证：** 本地 bare origin + 两 clone 的关系测试；无 origin、无提交、有 origin 无提交、detached、worktree、不同 upstream 名称；SSH / HTTPS URL 转换；超时和管道继承；同仓库 REPO_BUSY。

**验收：** 无 Tauri 依赖；Windows 命令不闪控制台；新核心保留子进程树与管道 deadline，未通过放弃这些能力换取编译。

## N06：新设置、仓库与分组应用服务

**依赖：** N05。**目标：** settings.rs、registry.rs、storage/store.rs、配置集成测试。

**操作：**

1. 实现新 schema 默认值与 revision：空仓库、空分组、默认全部、定时启用 5 分钟。
2. 路径由宿主注入；新目录绝不查询或导入旧 Tauri 设置。
3. 添加仓库验证、规范化、去重和稳定 ID；移除仓库清理分组归属；网络忙时移除返回 REPO_BUSY。
4. 分组增删改、默认分组与单一归属规则；settings_save 只允许约定的可编辑字段。
5. 按 expectedRevision 保存，冲突返回 SETTINGS_CONFLICT；串行构造候选、原子落盘，再提交内存与 revision；失败不改变任何已提交状态。
6. 新配置损坏时隔离并返回默认值，通过启动结果的 startupWarnings 将 SETTINGS_RECOVERED 传给 UI；无法读取 / 隔离时返回错误，保留现场。

**验证：** 并发添加与设置保存、过期 revision、写入失败后内存 / revision / 重启读回一致、损坏 / 不可读文件与可见恢复提示、重复路径、无效组、移除忙仓库、无旧数据访问。

**验收：** 改动不能丢失另一项已经完成的设置写入；不出现旧字段解析或 migration marker。

## N07：引擎、刷新与停止

**依赖：** N06。**目标：** engine.rs、refresh.rs、进程 stop 检查与状态机测试。

**操作：**

1. 实现 sessionId、stateRevision、generation、settingsRevision、请求 / 完成序号、快照缓存与可唤醒等待；订阅 cursor 使用 stateRevision。
2. 首次 / 定时 / 手动 / 设置变化 / Git 操作结束均经过统一调度；一次全量扫描、合并一次后续请求。
3. 采集时抓取配置 revision；配置变化时拒绝发布旧配置结果，不完成对应请求，合并新扫描。设置提交立即重投影，移除项消失、新增项 pending。
4. 区分尝试时间、最近完整收集成功时间、全局失败和单仓库失败。
5. 实现 Running / Stopping / Stopped、拒绝新工作、停止派发、唤醒等待和线程回收。
6. Git 应用服务在执行前校验 confirmed 与实时状态；结束后用 finally 请求刷新，Stopping 时不补扫。
7. 单调时钟调度；原生恢复通知合并请求；手动模式不因唤醒变成自动扫描；时间回拨有明确显示规则。

**验证：** 用可控时钟 / 假采集器验证：阻塞当前扫描时连续 50 次请求最多合并一次后续扫描、旧扫描不提前完成新请求、丢弃结果后请求仍可完成、失败也结束请求忙状态、配置变更单独唤醒订阅、删除后扫描失败不复活、休眠后不补扫 N 次、时间回拨、关闭定时刷新后手动仍有效、等待者停止唤醒、重复 stop、停止后不启动下条 Git、操作成功 / 失败都触发刷新。

**验收：** 独立无 GUI 测试宿主可以启动、读取、操作和停止；没有永远无法唤醒的 Condvar 等待，也没有持锁 join。

## N08：共享视图与安全 Widget 投影

**依赖：** N07。**目标：** view.rs、widget.rs、共享 fixture 测试。

**操作：**

1. 实现组计数、状态计数、筛选、搜索、排序与 actions；两端只消费结果。
2. 实现读取失败 / pending / 五种关系的规定顺序、no_remote 最后、同档稳定排序、组 / 状态筛选协调。
3. 生成全部仓库摘要和最多 10 项关注列表，保留错误 / 过期判断所需安全字段。
4. 验证失败历史数据的时间，不将尝试完成时间替代成功时间。
5. 待读取数独立计数；新增、删除、分组变更后的全部摘要立即与设置一致，时间仍保留原成功值。

**验证：** 8 仓库 fixture 的摘要为 2 / 1 / 1 / 1 / 2 / 1；紧凑四桶为 2 / 2 / 2 / 2；筛选不改变全部摘要；搜索零结果不清空条件；Widget 不包含 path、原始 stderr 或远端凭据。

**验收：** UI / Widget 无需各写一份关系计数和操作规则；四桶总和加 pendingCount 等于 total。过期边界的等于 / 大于及时间回拨测试明确。

## N09：正式 C ABI 与内存生命周期

**依赖：** N08。**目标：** ffi 源码、include/gitaview.h、ABI 测试。

**操作：**

1. 先补齐并冻结 schema.json 的全部输入 / 输出及必填 / null / 反例，再按实施说明实现固定函数、数字状态、JSON envelope、错误码、字节长度与 buffer_free。
2. 明确 create 失败、null / len、confirmed 取值、wait 超时 / stopping、destroy BUSY 的行为。
3. 保护 Rust panic 边界；测试异常路径释放与宿主错误恢复。
4. 在途业务调用计数与引擎停止协调，生命周期控制函数不计入它们正在等待清零的业务调用数。
5. 通过真实 staticlib / DLL 导出验证 ABI；移除临时生产 probe 导出，不留下成功 stub。

**验证：** 符号表、C 头布局、真实 Rust 输出符合 schema、Unicode buffer、多次分配释放、返回码与 envelope 一致、GIT_TIMEOUT 与等待 TIMEOUT 可区分、停止时唤醒 wait、在途调用结束前 destroy 拒绝。无效任意地址不能作为“应当安全返回错误”的测试要求。

**验收：** 所有缓冲区有唯一释放责任；没有 Rust Vec / String / bool 直接跨边界；新 ABI 不调用旧 Tauri 命令。

## N10：Swift / C# 包装与真实跨语言测试

**依赖：** N09，分别需要对应语言工具链。**目标：** Shared / Interop / Contracts / 两端测试。

**操作：**

1. 实现 DTO、错误映射、句柄 owner、缓冲区 try/finally 释放和后台调用。
2. 单独快照循环与停止路径，UI 线程只提交状态；SafeHandle / Swift owner 活到调用结束。
3. 同时处理 sessionId / stateRevision、刷新 requestId 与 view query 序号；同代配置变化可见，同状态的新筛选不被忽略，旧查询不覆盖新状态。
4. 三种语言读取同一 fixture；至少在两个原生宿主各做一次真实 FFI 操作和关闭测试。

**验收：** UTF-8 / null / uint64 时间正确；没有按 ISO 默认解码毫秒；没有每次 UI 重绘创建引擎。缺语言环境的子项保留未验证。

分别记录 N10.Windows 与 N10.macOS；各自通过后可进入相应宿主任务，不等待另一端。汇总 N10 仍须两端真实调用都通过才能 verified。

## N11：macOS 主程序生命周期与菜单栏

**依赖：** N04、N10 的 macOS 部分。**目标：** macOS App / Scene / 原生生命周期。

**操作：** 新身份与路径、菜单栏、正常仓库 / 设置窗口、窗口关闭继续驻留、显式退出状态、协议冷 / 热启动、单实例、开机启动的系统状态展示、启动失败 / 恢复警告、原生唤醒通知。

**验证：** 双开仅激活现有应用；未知协议 / 已删 repo 回总列表；窗口关闭不停止刷新；退出时不提前销毁 FFI；应用完全退出后无扫描子进程。

**验收：** 正常原生行为，无旧桌面 level / collection behavior / watchdog。

## N12：macOS 仓库与设置完整 UI

**依赖：** N11。**目标：** SwiftUI 仓库视图、筛选、动作、设置与字符串资源。

**操作：** 按共同界面设计实现旧版升级后的状态面板、筛选条、表格 / 选中行操作和设置分区；完整接入真实 View / Settings 服务；实现添加、扫描候选选择、移除、分组、搜索、默认筛选、刷新与强制确认。错误状态保留已有内容，配置冲突给重载入口。

**验证：** 用与 Windows 相同 fixture、内容尺寸、主题和缩放条件核对共同布局与 token；原生交互覆盖确认取消 / 成功 / 失败、组与状态计数、空态、设置冲突；键盘、VoiceOver、深浅色和正常窗口缩放。缺少 Mac 时不伪造实机截图。

**验收：** 无 preview fixture 作为默认运行数据；显示“实时远端”之类不符合本地跟踪信息的文案视为未通过。

## N13：macOS WidgetKit 完整实现

**依赖：** N12、N08。**目标：** Widget Extension、安全快照写入与共享模型。

**操作：** 三种尺寸、平台背景 / 边距 / 着色、原子写入与最新待发布投影保留、按 stateRevision 排序、冷启动有效缓存保护、时间线、失败 / 过期 / 未初始化、深链与无关字段裁剪。Extension 不链接引擎，不扫描仓库。

**验证：** 签名应用在调试器外添加组件；真实 Git 状态变化后主程序写文件 / 请求 reload；记录实际组件更新；主程序退出后显示历史时间；重启首刷未完成不清空有效缓存，配置删除则及时更新；未知 schema 提示明确。

**验收：** 分别记录应用内发布成功和系统显示更新，不承诺固定秒数；正式签名下容器权限与组件发现通过。

## N14：Windows 主程序生命周期与托盘

**依赖：** N03、N10 的 Windows 部分。**目标：** AppInstance、WinUI App、托盘与协议。

**操作：** 引擎初始化前完成单实例；新数据目录；固定 DLL 加载；托盘与正常窗口；关闭驻留、退出、冷 / 热协议重定向；开机启动系统状态；启动失败 / 恢复警告；原生恢复通知。

**验证：** 重复启动不产生第二引擎；Explorer 重启图标恢复；窗口关闭后刷新持续；停止期间不提前卸载 DLL；路径 / 未知路由不能执行命令。

**验收：** 无 WorkerW / SetParent；运行不依赖开发工作目录或全局 Rust DLL PATH。

## N15：Windows 仓库与设置完整 UI

**依赖：** N14。**目标：** WinUI 页面、ViewModel、原生对话框与字符串资源。

**操作：** 接入同一共享服务；功能与共同界面设计同 N12，用 WinUI 实现共同布局与主题，按旧版升级，替换当前通用模板式页面结构。保留已有生命周期和后端接线；补齐扫描候选、移除仓库等入口。单仓库网络操作使用独立忙状态，其他仓库仍可使用。

**验证：** 确认取消 / 成功 / 失败、组筛选、搜索、设置冲突、空态；Narrator、键盘、100% / 150% / 200% 缩放及多显示器。

**验收：** 两端共同布局、视觉角色、筛选层级与选中行操作符合统一设计；完成同 fixture 的真实截图与交互检查。不将 JSON 文本 / Rust 错误堆栈直接当产品界面；React / CSS 仅作为设计输入，运行界面仍由原生控件实现。

## N16：Windows Provider 完整生命周期

**依赖：** N15、N08。**目标：** Provider、Card 模板、包清单与组件测试。

**操作：** 三种尺寸；Create / Activate / Deactivate / Delete / ContextChanged；用 GetWidgetInfos 恢复实例，回调内复制普通值后再异步处理；串行更新与 Delete 清理；目录监听 / 原子替换 / 通知丢失恢复；安全快照、冷启动缓存与深链；活跃时过期计时、失活后停止监听与计时。

**验证：** Board 打开 / 关闭、Provider 重启且没有新 Create、快速写入、Delete 与更新并发、尺寸变化、不同实例、主程序未运行和完全退出、损坏 / 未知 schema、没有任何新文件写入时跨过过期边界、时间回拨 / 系统恢复、协议冷启动。

**验收：** Provider 不加载 Rust 引擎、不写仓库设置；实际安装包可激活 COM server，不能只通过 Provider 单元测试判定完成。

## N17：可复现脚本、CI 与签名安装产物

**依赖：** 各平台已有验证过的命令，可逐步执行。**目标：** scripts/native、native-ci.yml、原生发布说明。

**操作：**

1. 将实际验证的命令固化为实施说明中的 build / test / package 入口，校验参数和架构。
2. 原生 CI 独立运行 root workspace 与语言测试 / 构建，明确签名验证与无签名开发构建的区别。
3. macOS 做正确嵌套签名、公证、staple；Windows 做 MSIX 签名、Publisher / 依赖与签名验证。
4. 防止触发旧 v* 发布流程；生成本地或 CI 可审查产物，不擅自发布。
5. 清理 build 输出与忽略规则，保留必要工程配置；不把证书、私钥或绝对开发机路径提交。

**验收：** 从干净 checkout 按记录命令可构建，测试失败返回非零。干净目标机器没有开发 SDK 也能通过正常运行时部署安装。

## N18：最终验收、规格对齐与交付

**依赖：** 前面适用任务全部通过。**目标：** docs/native-platform-acceptance.md、README 原生入口、进度汇总。

**操作：**

1. 按声明的每个 OS / 架构安装验证真实仓库管理、所有关系、确认、失败恢复、设置和退出。
2. 两端组件发现、显示、生命周期与协议在正式安装环境验证。
3. 记录冷启动、空闲进程消耗、1 / 20 / 100 仓库扫描结果，明确机器、数据与测试条件。
4. 对齐产品规格和本套文档的实际结构 / 命令；逐项核对 [审查记录](native-app-review.md) 的验收场景，列出剩余缺陷及其阻断级别。
5. 保留旧数据不触碰的证据；交付新配置目录、构建命令、安装产物和实际已验证范围。

**验收：** 无未验证平台被标记完成；缺签名或真机则整体保持未完成，交付已验证部分与准确续做入口。最终声明不依赖聊天中的早期猜测或过期文件路径。
