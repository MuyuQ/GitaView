# GitaView 原生应用验收清单（N18）

> 2026-10-30 清理说明：旧客户端已从当前源码树移除。本文保留原生改造的历史方案或检查记录；旧源码路径、“保留旧工程”和旧发布命令不再适用。当前目录、命令与平台状态以 [原生架构说明](native-architecture.md) 和仓库 README 为准。

状态：**进行中**。本文是 N18 的验收清单与证据汇总；状态列取值：已验证 / 待验证（执行者或用户操作） / 阻塞 / 不适用。每次验收后更新本文件并在 [进度记录](native-app-progress.md) 留检查记录。

## 2026-10-30 当前基线

仓库只保留原生应用，入口与签名步骤见[原生架构说明](native-architecture.md)。旧客户端及其 macOS Widget 扩展已移除；macOS 主程序尚未实现。2026-11-07 起产品移除 Windows 小组件子系统（见第 4 节屏障说明）。

清理前提交 64462c5 的 [native-ci](https://github.com/MuyuQ/GitaView/actions/runs/40995753149) 已全部通过：Windows 的 Rust / C# 测试、布局检查、应用构建、无签名 MSIX 打包与上传；macOS 共享核心测试、Clippy 和 arm64 / x86_64 静态检查。这些证据不替代签名安装、Windows GUI 或 macOS 产品验收。下方有日期的条目保留对应版本的历史事实。

## 1. 环境与安装

| 项 | 声明 | 状态 | 证据 |
| --- | --- | --- | --- |
| Windows 11 x64 安装 | MSIX 测试签名安装（0.1.5.11） | 已验证 | 2026-10-25：Release 构建、签名打包及 `Add-AppxPackage` 原位更新成功，版本 0.1.5.11、Status=Ok；安装 DLL 与 Release 产物 SHA-256 一致。已启动，进程路径为 0.1.5.11，日志确认 primary=True 及 ui.rendering。实际画面与冷启动性能尚未复测 |
| Windows 0.1.5.15 构建与配置隔离打包 | x64 Debug / Release 分布局，Release -SkipBuild 测试签名包 | 已验证（本地） | 2026-10-27：Release → Debug → Release -SkipBuild 实际执行；Release 布局哈希保持不变，最终 MSIX 内 Rust DLL 和主程序 DLL 匹配 Release、不同于 Debug。9 项临时布局回归通过；尚未安装 |
| Windows 0.1.5.16 构建与打包 | x64 Release 测试签名包 | 已验证（本地） | 2026-10-28：修复编码损坏恢复、分组归属与 Provider 退出竞态后构建和签名通过；Rust 141/141、C# 111/111、布局回归 9/9；尚未安装 |
| Windows 0.1.5.17 构建与打包 | x64 Release 测试签名包 | 已验证（本地） | 2026-10-28：设置请求捕获、浮窗关闭、Widget 四桶修复后编译和签名通过，C# 118/118；尚未安装 |
| Windows 0.1.5.18 构建与打包 | 默认解决方案 Debug、脚本 Release、测试签名包 | 已验证（本地） | 2026-10-28：默认解决方案产物 apphost 与 Rust DLL 均为 x64；Release 构建与签名成功，C# 127/127；尚未安装 |
| Windows 0.1.5.19 构建与打包 | x64 Release 测试签名包 | 已验证（本地） | 2026-10-28：浮窗版本门控、并发测试观察顺序修复后，Rust 141/141、C# 129/129、fmt / Clippy、Release 与签名通过；尚未安装 |
| Windows 0.1.5.20 构建与打包 | x64 Release 测试签名包 | 已验证（本地） | 2026-10-28：启动恢复提示、schema 修复后，C# 129/129、Rust 契约 5/5、schema 6/6、Release 与签名通过；尚未安装 |
| Windows 0.1.5.21 构建与打包 | 禁用本机发布配置的 x64 自包含包 | 已验证（本地） | 2026-10-28：跟踪项目明确 SelfContained=true，脚本禁用 PublishProfile 后 Release 与签名通过；包内三份 .NET 运行时文件及 runtimeconfig 哈希匹配布局，Rust 141/141、C# 129/129、布局 12/12；未安装 |
| Windows 0.1.5.22 构建与打包 | x64 Release 测试签名包 | 已验证（本地） | 2026-10-29：进程入组竞态和日志阻塞修复后 Rust 147 项、C# 129 项、布局 12 项及 fmt / Clippy 通过；1 项手工性能用例已单独运行；包内关键文件哈希匹配 Release 布局；未安装 |
| Windows ARM64 | 后续范围 | 不适用（本轮） | 平台矩阵 |
| macOS 安装 | DMG 签名分发 | 阻塞 | 无 Mac / Xcode / Apple 签名身份 |
| 干净机器无 SDK 运行 | 运行时自包含布局 | 待验证 | 主应用自包含布局；需一台无 .NET SDK 的机器复测 |

## 2. 共享核心（gitaview-core / gitaview-ffi）

2026-10-29 / 0.1.5.22：Windows 读进程挂起入 Job 后用目标进程线程快照恢复，避免快速退出被误判为 Git 缺失，新增可控交错和错误分类回归。核心日志通过有界非阻塞队列后台写入，状态发布和停止标志先于日志；慢日志锁下的保存、查询、停止及 flush 期限测试通过。Rust 147 项通过，1 项手工性能用例单独运行；fmt / Clippy 通过，不替代 macOS 编译/实机验证。

2026-10-28 / 0.1.5.16：非法 UTF-8、UTF-16 设置文件进入隔离恢复流程，两个新增回归验证原始字节保留、默认设置、启动警告及恢复后保存和重启；正常 I/O 错误仍拒绝初始化。配置集成 20/20，Rust workspace 141/141，Clippy / fmt 通过。

2026-10-27 / 0.1.5.15：全局失败后配置提交保留错误对象，直到新扫描结果被接受；三个回归场景覆盖保存设置、添加及移除仓库，验证中间快照的 refreshError 序列化字段与 Widget 错误码均保留，恢复成功后清除。三项旧实现失败、修复后通过。Rust workspace 139/139、Clippy / fmt 通过，C# 契约与真实 FFI 回归 100/100。

2026-10-27：业务调用的允许检查与计数登记已合并到同一生命周期临界区。新增屏障测试在旧实现中复现停止提前完成，修复后验证登记期间不能提前停止、在途调用结束后才能报告 Stopped、停止后拒绝新调用。Rust workspace 总计 136/136，Clippy --workspace --all-targets -- -D warnings 及 fmt --all --check 通过。

| 项 | 状态 | 证据 |
| --- | --- | --- |
| 关系 / pending / 计数 / 动作规则 | 已验证 | 60+ 项核心测试（含 8 仓库 fixture 断言与四桶不变式） |
| 设置事务 / 冲突 / 损坏恢复 | 已验证 | 16 项配置集成测试（连续 8 次稳定） |
| 引擎调度（合并 / 丢弃 / 停止 / 唤醒） | 已验证 | 17 项状态机测试（假采集器 + 假时钟，连跑 3 次稳定） |
| 视图查询（筛选 / 搜索 / 排序 / 协调） | 已验证 | 9 项 fixture 驱动测试 |
| 正式 C ABI（20 函数 / 状态码 / buffer / 生命周期） | 已验证 | 13 项 ABI 测试（串行 + 并行）；dumpbin 20 符号与头文件一致 |
| git 命令生成性能 | 已验证（本轮修复） | spawn_bench：挂起 149ms → 直接 49ms / 普通 22ms；Direct 超时终止测试锁定 |
| 诊断日志（脱敏 / 轮转 / 三路） | 已验证 | 5 项 diagnostics 测试；0.1.1.7 安装实测三文件产生且内容正确 |
| macOS 上核心编译与测试 | 部分验证（本轮修复） | 审查发现 Apple 目标 15 个编译错误 → R06 修复后本机 `cargo check --target x86_64-apple-darwin` 通过（2026-10-24 实测）；2026-10-30 native-ci 已通过 macOS 核心测试及双 Apple 目标 check；原生 macOS 宿主和实机验收仍未完成 |
| 并发配置提交不回退镜像（R03） | 已验证（测试） | 引擎层 8 线程屏障压测 + 镜像 revision 单调断言；审查探针结果（revision 24→23）作为反例保留 |
| 删除分组归属完整性（R04） | 已验证（测试） | 空 repoGroups 删组 → 全部仓库归属清空，内存 / 磁盘 / 引擎投影一致 |
| 首刷前 pending 行投影（R08） | 已验证（测试） | 阻塞首扫 + 入组仓库 → 行带组、组计数包含 pending |
| 设置启动完整校验（R09） | 已验证（测试） | 七类非法业务值（未知 schema / revision 0 / 超范围刷新 / 重复 ID / 失效引用）按损坏隔离恢复 |
| 状态读取链统一 Direct（R13） | 已验证（结构） | comparison_ref 并入 branch_state Direct 闭包链，无法单独退回 Suspended；网络操作保留 Suspended |
| 宿主退出终止 Git 子进程树 | 已验证（代码） | Job Object KILL_ON_JOB_CLOSE（R02 配套加固）；实机强杀待复测 |
| 移除后同名仓库身份隔离（S02） | 已验证（测试） | 移除清 collected 缓存 + 复用前核对路径身份；同名不同路径新仓库 pending、无继承关系/分支、网络动作不可用 |
| FFI 输出参数前置校验（S03） | 已验证（测试） | create（null reply / null engine_out）不启动引擎不建目录；save null reply revision 不变；生命周期 null-reply 特例明确 |
| 删除与网络操作交错安全（S05） | 已验证（测试） | 屏障暂停"已定位未拿槽"→移除→恢复：REPO_NOT_FOUND，Git 不启动 |

## 2.1 Flutter 宿主（0.2.0.0，第 62 轮新增）

> 2026-11-07 启动迁移：Rust 核心 + Flutter 宿主（apps/flutter）。
> 2026-11-10 第 64 轮完成 Windows 实机首轮验收（本机 zh-CN Windows 11 26H2，
> RTX 5060 Ti；方法 = computer use AX 树驱动 + 消息注入 + 引擎日志，像素级
> 截取受本机 MPO/串流环境限制，见 §2.4 环境备注）。各条状态见下表；
> 标注"待用户复核"的项为注入通道无法覆盖的部分，不是已通过。

| 项 | 范围 | 状态 |
| --- | --- | --- |
| Dart 层 | flutter analyze 零问题；63 项测试全绿（契约 fixture 与 Rust 同源、真实 dylib FFI 全链路、双 isolate 引擎服务、8 个纯逻辑类、11 项 widget/控制器测试） | 已验证（macOS 开发机 + CI macos-core，2026-11-07） |
| CI 全链 | native-ci 三 job 全绿（run 6，commit 149f8f9）：windows C# 全链、windows-flutter（Dart 真实 DLL 测试 → Release 构建 → makeappx 无签名 MSIX + 清单校验 → 双产物上传）、macos Rust + Dart 层；第 62-63 轮经 6 轮迭代修复（registrar API、shellscalingapi、窄化转换、wstring 编码、包装库链接、托盘窗口类型、makeappx 基名资产、两处测试竞态） | 已验证（云端构建链，2026-11-07） |
| Windows 构建 | runner CMake 接 cargo 产出 gitaview_ffi.dll 随包安装；gv_platform C++ 插件编译（托盘 / LocalState / StartupTask 镜像 / openTarget） | 已验证（CI + zh-CN 实机：2026-11-10 修复 CMake 缺 `/utf-8`——中文区域下 MSVC 按 ANSI 误读 UTF-8 中文注释触发 C4819→/WX 必炸，commit 5923254；本机 Release + Debug 双配置构建通过） |
| MSIX 打包 | package-flutter-windows.ps1 无签名产物 13.3MB 上传（makeappx 清单校验通过；基名资产由 scale-200 生成，多尺度分辨率待评估） | 已验证（2026-11-10 实机签名打包 + 安装：脚本补 `-Encoding UTF8` 修复 PS 5.1 中文区域读坏清单，commit 5923254；测试证书 C909CB45 签名，0.1.5.27 → 0.2.0.0 原位升级与卸载重装均通过，LocalState 用户数据保留/恢复验证） |
| GUI 验收 | 主页面（筛选 / 表格 / 两段式确认）/ 设置页 / mica 背景 / 沉浸式标题栏 | 已验证（2026-11-10 实机，AX 树驱动，computer use）：筛选 chip 生效（本地领先 → 仅 GitaView 行，全部恢复）、行展开操作面板（Push/Pull/Fetch/远端/打开目录，本地领先正确提供 Push）、两段式确认态与「取消确认」按钮回退、搜索键入过滤（"gita" → 仅 GitaView 行）、设置页完整渲染（分组管理 + 11 仓库 + 别名 + 两分区）。像素级视觉（mica 观感 / 颜色 / 字体渲染）待用户肉眼复核——本机截取通道（GDI / PrintWindow / 窗口级 API）对 D3D 内容盲区（MPO/串流环境，Electron 应用同现象），非应用缺陷 |
| 托盘与浮窗 | Windows：Shell_NotifyIcon 注册 / TaskbarCreated 重注册（1s×5 重试）/ 左键浮窗定位 / 失焦收起 / 右键菜单 / 子窗口数据推送；macOS：菜单栏状态项 + 左键浮窗（fork 第二窗口）+ 右键系统菜单（同通道契约，见 §2.3） | Windows 部分已验证（2026-11-10 实机，消息注入驱动）：托盘回调（WM_APP+1）→ 左键浮窗创建并按托盘图标定位（任务栏右下角）、摘要实时推送正确（1 待推送 / 0 待拉取 / 0 需关注 / 0 未比较，与仓库真实状态一致）、「打开 GitaView」前置主窗口并收起浮窗、左键多次开合正常。右键菜单与失焦收起待用户实机复核（注入通道会污染模态菜单状态）；TaskbarCreated / Explorer 重启恢复待实机 |
| 单实例与深链 | windows_single_instance 管道；gitaview-native:// 冷 / 热激活路由（未知 / 已删 ID 回总列表） | 已验证（2026-11-10 实机）：第二实例启动后参数经管道转交首实例并自行退出（进程数恒为 1）；冷启动深链 `gitaview-native://repo/<id>` 正确清筛选并展开目标仓库（操作面板出现）；热深链（运行中第二实例）切换展开目标（GitaView → AegisOTA）。URL 格式注意：host 必须为 `repo`（`open/repo/<id>` 形式会被拒回总列表，与 C# 解析一致） |
| 开机启动 | StartupTask 注册表镜像读数与文案映射（best-effort，需与系统开关实测对齐） | 待验证（实机） |
| ⚠️ 界面重设计双端分裂（2026-11-08） | Flutter 宿主已按 `design/preview/index.html` 落地重设计：状态色映射变更（本地领先 teal、远端领先 blue、分叉 red→amber、red 收窄为错误）、表格列序 7 列 → 6 列（状态点与关系合并为胶囊）、表面色去 alpha、工具栏 66 / 侧栏 196 / 浮窗 2×2 统计格。同日用户反馈追加（Flutter 已落地，契约新增 `alias` 字段 + settingsEditable 新增 `repoAliases`）：仓库别名（主界面列表显示"目录名（别名）"、浮窗显示名 = 别名优先、搜索命中别名）、路径收纳进展开面板、macOS 沉浸式标题栏（TitleBarStyle.hidden + 红绿灯内缩 + 工具栏拖拽）、主题切换（设置-外观三档分段控件，宿主本地 `ui-preferences.json` 持久化，不入契约；C# 宿主对应 LocalSettings 方向）。**WinUI 宿主（0.1.5.x）未同步**以上任何一项。`docs/native-app-ui-design.md` §3 / §4 / §8.2 已记录为共同设计的新基线 | 待办：WinUI 宿主同步改造（含 alias 契约字段）+ 双端一致性验收（迁移期间以 Flutter 宿主为准；Flutter 侧证据 `target/ui-redesign/`，macOS 开发构建 2026-11-08：浅 / 深 × 主界面 / 设置 / 展开确认 / 别名显示 / 窄 640 / 空库 / 无匹配 / 骨架屏，尺寸为窗口逻辑尺寸，非设计稿 §8.3 全矩阵） |

### 2.1.1 2026-11-10 Windows 实机验收环境备注（第 64 轮）

- **截取通道盲区**：本机（挂 GameViewer / MuMu / Honor 三个虚拟显示适配器 + 串流会话）下，GDI `CopyFromScreen`、`PrintWindow(PW_RENDERFULLCONTENT)` 与 computer use 窗口级截取对 **D3D 交换链内容一律取不到**（Win32 标题栏正常），表现为 Flutter 窗口"全白"、Electron 窗口"全黑"。经桌面复制 API 独占冲突、引擎适配器查询（确认为 RTX 5060 Ti 物理卡）、dxdiag、事件日志（无 TDR）等多路排查定性为环境假象，非应用缺陷。像素级视觉验收只能由用户肉眼完成。
- **中文 IME 吞键**：zh-CN 拼音输入法在合成态吞掉字母与 Esc。自动化键入需绕过 IME（直投 `WM_CHAR`）；系统级 SendKeys / SendInput 对合成态不可靠。应用键盘链路本身正常（WM_CHAR 键入过滤生效）。Esc 取消确认 / Ctrl+K 聚焦搜索两条快捷键的实机复核待用户完成（widget 逻辑有单测覆盖的仅限控制器状态机，键事件分发无自动化覆盖）。
- **退出路径告警**：浮窗「退出」后进程干净退出（无残留窗口 / 进程），Debug 运行时日志可见一次 `Dart_EnterIsolate expects there to be no current isolate` 告警（watcher isolate 的原生回调与引擎销毁竞态，JIT 运行时输出）。不阻断退出，留作后续收敛项。
- **验收方法学**：托盘回调 = `PostMessage(GitaViewTrayWindow, WM_APP+1 /*0x8001*/, 0, WM_LBUTTONUP)`；自动化键入 = 直投 `WM_CHAR` 到 FLUTTERVIEW；多实例并存时必须按 pid 精确锁定目标窗口（曾因此产生"二次打开失效"的假象，单进程下多次开合复验正常）。

### 2.2 Flutter 宿主 macOS 开发构建前台实测（2026-11-07，computer use）

> 范围与局限（如实标注）：这是 **macOS 开发构建**（`flutter run -d macos --debug
> --no-enable-impeller`，Skia 软件路径）的界面实测——不是 Windows 实机验收，
> 也不构成 macOS 应用打包/签名验收。浮窗第二窗口、单实例、深链、开机启动为
> Windows 独占能力，不在本轮范围；macOS 托盘（菜单栏状态项）已于 2026-11-09
> 落地并在 §2.3 单独实测。Impeller 在本机 Intel Iris Pro
> 上挂起（`PlatformView::NotifyCreated` 死等），Skia（`--no-enable-impeller`）
> 正常——已确认为宿主环境限制。

前置：脚手架 macos runner；main.dart 平台接线（窗口初始化/尺寸、macOS 用
`open` 打开目标、file_selector 目录选择）；`GvNative.load` 候选路径解析
（env → 可执行目录 → 仓库 target 逐级上溯，GUI 进程 cwd 不固定）；
DebugProfile.entitlements 去除 App Sandbox（开发构建需加载容器外 Rust dylib）。
测试数据：4 个真实 git 仓库（backend-api 同步 / docs-site 远端领先 /
diverged-app 制造分叉 / no-remote-app 无远端，origin 为本地 bare 仓库），
预置 settings.json（含"后端"分组）。

computer use 前台实测结果（AX 驱动 + 截图，证据 target/ui-macos/）：

| 项 | 结果 |
| --- | --- |
| 启动 / 空态 | 空态文案、添加仓库入口、底部摘要（0 仓库）正确渲染 |
| 目录选择器添加仓库 | 原生 NSOpenPanel 打开、导航到 ~/Desktop/gv-e2e/backend-api、Open 后绿色横幅"已添加仓库"+ 仓库列表出现该仓库（分组下拉"未分组"+ 移除按钮） |
| 分组种子 | backend-api 分组列显示"后端"（settings 预置生效） |
| 4 仓库主页面 | 真实 git 扫描：docs-site 远端领先 ↓1（琥珀）、backend-api 已同步、diverged-app 未比较、no-remote-app 无远端；排序按核心规则；chips 计数全部正确（全部分组 4 / 后端 1；全部 4 / 远端领先 1 / 同步 1 / 未比较 2） |
| 行展开操作面板 | backend-api（已同步）：打开目录/远端/Fetch 可用、Pull/Push 正确禁用；点击 Fetch → 绿色"Fetch 完成" |
| Fetch 关系翻转 | diverged-app Fetch 后：未比较 → 分叉 ↑1 ↓1，chips 出现"分叉 1" |
| Pull 两段式确认 | docs-site（远端领先）：Pull → 琥珀"确认 Pull"+"Pull 会修改当前仓库工作区，是否继续？"+ 取消确认 → 确认后真实 pull 执行 →"Pull 完成"+ 关系翻转已同步 |
| Push 两段式确认 | shell 制造本地领先提交 → 刷新按钮检测（本地领先 ↑1）→ Push → 琥珀"确认 Push"+"Push 会更新远端分支，是否继续？" → 确认后真实 push →"Push 完成"+ 关系翻转已同步、Pull/Push 转禁用 |
| 状态 chip 筛选 | 点击"同步"→ 表格只剩 2 个已同步仓库 |
| 组合筛选 | 后端分组 + 全部状态 → 1 行；状态计数重算（全部 1/同步 1）；结果计数"1 / 4"出现 |
| 筛选重置 | 全部分组/全部 → 4 行恢复 |
| 设置页 | 4 仓库列表 + 分组下拉（后端）正确；返回主页面上下文保留 |
| 自动刷新 | 刷新时间随 interval 自动更新（17:05:44） |
| 搜索框键入 | 未验证：本机 helper 键盘注入受限 + AX setValue 不触发 Flutter onChanged（记录为待验） |

未在本轮覆盖：分组下拉改归属（界面存在）、移除仓库确认流、深色/浅色切换、
错误路径横幅（无真实失败源）。Windows 独占能力仍以 §2.1 实机验收为准。

### 2.3 Flutter 宿主 macOS 托盘（菜单栏状态项）开发构建实测（2026-11-09）

> 范围与局限（如实标注）：macOS 开发构建（xcodebuild Debug，x86_64，Skia）的
> 托盘 + 浮窗链路实测；不构成 macOS 打包/签名验收。点击用 CGEvent 在系统
> 窗口列表报告的状态项 / 菜单项坐标合成（CUA 坐标绑定窗口栅格够不到菜单栏；
> ZCode 辅助功能权限由用户授权后注入）——菜单弹出、菜单动作经
> `gitaview/platform` 通道回传 Dart 的整条链路为真实代码路径。

实现（第一层：托盘 + 系统菜单；第二层：浮窗。设计稿 §平台分工：macOS
菜单栏与 Windows 托盘命令含义相同、菜单外观由系统负责）：

- `apps/flutter/macos/Runner/GvPlatformPlugin.swift`：`gitaview/platform`
  通道 macOS 侧实现——ensureTray（NSStatusItem，幂等）/ removeTray /
  getTrayContext（图标矩形 + 所在屏幕工作区，左上原点物理像素 + backing
  scale，契约同 Windows；混合 DPI 多屏跨屏换算待浮窗接入时精确化）；
  点击分流同 Windows：左键 → onTrayLeftClick（浮窗开合），右键 → 临时挂
  NSMenu + performClick（系统定位弹出；打开 GitaView / 刷新 / 分隔线 /
  退出，动作整数 1/2/3 经 onTrayMenuAction 回传，与 Windows kMenu* 一致）；
  styleFlyoutWindow（浮窗形态收尾，见下）；getSystemPaths /
  getStartupTaskState / openTarget 回 NotImplemented。
- 菜单栏图标：`assets/brand/gitaview-menubar.svg`（品牌分支字形 ÷8 单色模板
  变体）→ 16px@1x / 32px@2x PNG 入 `MenuBarIcon.imageset`
  （template-rendering-intent=template）。
- main.dart：bootstrap 放宽到 macOS（引擎启动 + ensureTray + setHandlers）；
  关闭拦截同 Windows（preventClose → MainWindowClosePolicy）；_showMainWindow
  含 macOS（show + focus）；_requestExit macOS 分支（removeTray → 引擎优雅
  停止 → exit(0)）。
- macOS Runner 注册：MainFlutterWindow.awakeFromNib 手动注册（同 Windows
  gv_platform 模式）；AppDelegate 增加 applicationShouldHandleReopen
  （Dock 点击恢复主窗口）。

实测结果（证据 target/ui-macos-tray/）：

| 项 | 结果 |
| --- | --- |
| 注册与图标 | ensureTray 成功；状态项窗口（layer 25 "Item-0" 38×24）出现；菜单栏截图可见分支字形模板渲染正确（00-menubar-icon.png）；Hidden Bar 会将图标收纳到隐藏区（负 X），属该工具的正常行为 |
| 点击弹菜单 | 点击状态项弹出系统菜单：打开 GitaView / 刷新 / 分隔线 / 退出，外观由系统渲染（01-menu-open.png） |
| 打开 GitaView | 菜单动作回传 Dart 后窗口激活：lsappinfo front 显示 gitaview "(in front)"（02-open-main-window.png） |
| 刷新 | 刷新时间 01:10:13 → 01:12:33，引擎真实重扫（03-after-refresh.png） |
| 退出 | 菜单退出后进程干净退出、托盘图标同步消失；引擎日志 stop_request Running → Stopping（04-engine-exit.log.txt） |

Dart 侧检查：flutter analyze 0 issue、73 项测试全绿。构建：xcodebuild
Debug BUILD SUCCEEDED（本机验证）；`flutter build macos` / `flutter run`
官方路径当前被宿主环境 CocoaPods 状态阻塞（Podfile.lock 由 1.17.0 生成，
系统 Ruby 2.6 可装的最高 1.11.3；1.17 的 nkf 原生扩展在系统 Ruby +
Xcode 15.2 SDK（缺 universal-darwin23 Ruby 头）下编译失败）——环境问题，
与本次改动无关，改动前 pod 二进制已不可用。另注：失败的 pod install
（1.11.3）会删掉 `Pods/Manifest.lock` 并把 Pods xcconfig 降级写入
`DT_TOOLCHAIN_DIR`（Xcode 15 硬错误），导致 xcodebuild 跟着失效；现场
恢复法：`cp Podfile.lock Pods/Manifest.lock` + 将 Pods xcconfig 中
`DT_TOOLCHAIN_DIR` 替换为 `TOOLCHAIN_DIR`。

待验项：

- Cmd-Q / 应用菜单退出绕过优雅退出序列：applicationShouldTerminate 未拦截，
  直接 terminate 跳过 Dart `_requestExit`（引擎 stop_request、removeTray）；
  存储层原子写兜底，后果有限。待实现 Swift → Dart 终止桥（含 Dart 无响应
  超时回退）后实测——第一层跟进项（review P1）。
- 关闭 → 隐藏驻留（MainWindowClosePolicy macOS 分支）：本机开发构建无法
  触发关闭请求——上游 nativeapi（window_manager 0.6 后端）对
  TitleBarStyle.hidden 直接隐藏窗口控制按钮（红绿灯缺席，预先存在行为，
  与托盘改动无关）；待窗口样式提供关闭入口或绑定关闭快捷键后实测。
- Dock 点击恢复（applicationShouldHandleReopen）：已实现，未实测。
- getTrayContext 定位上下文：已实现，无消费者（浮窗第二层接入时连同混合
  DPI 多屏一起验证）。
- 深浅色菜单栏外观：本机深色菜单栏已验证（模板渲染）；浅色菜单栏待验。

与 Windows 的已接受差异：Windows 菜单设 SetMenuDefaultItem（默认项粗体 /
回车触发），macOS 系统菜单无默认项概念——设计稿约定菜单外观由系统负责，
不做等价实现。

### 2.4 Flutter 宿主 macOS 托盘浮窗（第二窗口）开发构建实测（2026-11-09）

> 范围与局限（如实标注）：同 §2.3 的开发构建与 CGEvent 合成点击方式；
> 不构成打包/签名验收。浮窗复用 Windows 侧同一套 Dart 链路
> （flyout_host / flyout_app / FlyoutWindowContent），macOS 仅适配窗口
> 形态与坐标单位；**Windows 侧行为未回退**（平台分支外代码原样）。

实现（方案 A：复用 desktop_multi_window fork 的 macOS 第二窗口，零依赖
变化；fork 钉在 Devolutions@cbc8cf58）：

- 坐标单位：定位纯逻辑输出物理像素（TrayFlyoutPresentation.position，
  其"上方放不下翻转到下方"分支天然支持 macOS 顶部菜单栏图标）；
  fork 的 macOS setFrame/getFrame 以左上原点逻辑点为单位——新增
  `TrayFlyoutPresentation.nativeFrame` 按缩放除回（presentation_test
  锁定），flyout_host / flyout_app._autoSize 按平台取单位。
- 浮窗形态（`styleFlyoutWindow`）：fork 的 NSWindow 子类
  desktop_multi_window.FlutterWindowInner 按运行时类名匹配，隐藏标题栏
  按钮簇（保留窗口自带圆角与投影）、level = .floating；resizable(false)
  / setMovable(false) 走 fork macOS 专属 Dart API（Windows 侧回
  NotImplemented，故平台守护）。
- 左键语义从"点击即菜单"改为与 Windows 一致（左键浮窗 / 右键菜单）。
- **fork 行为适配（两处，均已实机定位）**：
  1. `FlutterWindowInner.order` 覆写里的 hiddenSubWindowAtLaunch 会在
     进程内首个子窗口首次 order 时强制隐藏（全局一次性防启动闪烁旗标）——
     首 show 被吞，host 检测 isHidden 后补一次 show（旗标已消耗，幂等）；
  2. 标题栏按钮簇在窗口首次上屏后才实例化，创建时隐藏无效——样式收尾
     移到首次上屏后执行（仅一次）。
- **双端状态同步修复**：子窗口失焦自藏（blur → hide）后 host 的 _visible
  缓存过时，下一次左键 toggle 会走 hide 分支导致"打不开"——Windows 上
  同样存在（此前未实机验证到）。修复：toggle 以 `isHidden()` 真实状态
  为准，不再信任缓存。

实测结果（证据 target/ui-macos-tray/）：

| 项 | 结果 |
| --- | --- |
| 左键浮窗 | 弹出在图标右下（x=269 = 583+38−352 右对齐，y=32 = 菜单栏下 8px 间隙），2×2 统计 / 关注行（别名「文档站」优先）/ fly-note / 三按钮齐全，浮于普通窗口之上（layer 3 floating）、圆角与投影保留（10/11/12-*.png） |
| 刷新按钮 | 浮窗时间 07:51:44 → 07:53:02，引擎真实重扫并推送（13-after-flyout-refresh.png） |
| 打开按钮 | 浮窗收起 + 主窗口转前台（lsappinfo in front） |
| 失焦收起 | 点击主窗口后浮窗隐藏（blur → hide） |
| toggle 全循环 | 开 → 失焦收起 → 再开（修复路径）→ toggle 关，全部通过 |
| Esc 收起 | Escape 键合成后浮窗隐藏 |
| 右键菜单 | 左右键分流正确，系统菜单三项照旧（14-rightclick-menu.png） |
| 退出按钮 | 浮窗退出 → 进程干净退出 + 引擎 stop_request 日志 |

已知差异与待验项：

- 激活式浮窗（方案 A）：打开浮窗会激活应用（fork show() 内
  NSApp.activate，Spotlight 式）；不抢焦点的 NSPanel 形态留作后续打磨
  （需 vendor/patch fork）。
- 性能实测（2026-11-09，同机）：热开 114–147ms（含 ~60ms 合成点击开销）；
  debug 冷首开 ~4.9s（子引擎懒启动 + JIT，AOT release 待实机复测）；
  浮窗开 / 关空闲 CPU 均 0%；内存 debug ~514MB / release ~76MB（双引擎
  debug 开销为主）。已做微优化：托盘菜单构建一次复用、首 show 的
  isHidden 补偿检查门控到仅首开。
- 全屏 Space 表现：fork 子窗口钉死 collectionBehavior
  [.moveToActiveSpace, .fullScreenPrimary]，浮窗（level=.floating）在
  原生全屏应用之上的呈现未实测；必要时补 .fullScreenAuxiliary。
- fork setFrame 带 `animate: true`：浮窗每次定位有短动画，观感可接受；
  fork 无关闭动画的 API。
- 首次推送的 "FlutterPlatformMessageCreateResponseHandle" 警告（子引擎
  懒启动竞态）：浮窗内容由 createWindow 参数携带的初始快照兜底，后续
  推送正常；子引擎启动后未再复现。
- 浅色模式浮窗、混合 DPI 多屏定位、浮窗在 Hidden Bar 收纳状态下的定位
  回退，待后续验证。

## 3. Windows 主程序（源码 0.1.5.27，已安装 0.1.5.27）

2026-11-05 / 0.1.5.23→0.1.5.27：computer use 实机界面验收大半完成（主页面 / 行操作 / 设置 / 托盘浮窗 / 退出流），并修复分组归属回写 P1（容器再生回声事件以空归属覆盖刚保存的归属；新增 7 项回归，C# 151 项通过）；归属选择修复后复验持久。呈现停摆根因确认并修复：DesktopAcrylicBackdrop 破坏呈现管线（可见前台取证：最大化后帧冻结且不随窗口缩放、最小化-还原/真实点击/移动均不恢复；对照实验去除背景材质即全部正常）——主窗口与浮窗改 MicaBackdrop Kind=BaseAlt，同批升级 WASDK 2.5.1→2.6.5；0.1.5.27 实机复核：后台启动即最大化全屏呈现、内容随窗口缩放、浮窗真实托盘点击后屏幕完整渲染。原位更新后首次固定组件需预热 Provider（脚本 -Install 自带；手动 Add-AppxPackage 需手动 COM 激活一次，实测解压约 3.2s）。

2026-10-28 / 0.1.5.21：禁用忽略的本机发布配置后，主程序仍明确自包含 .NET；构建和签名包的 coreclr、hostfxr、CoreLib、runtimeconfig 验证通过。修正三项测试隔离和时序假设后 Rust 141/141、C# 129/129、布局 12/12、fmt / Clippy 通过。尚未替代干净机器无 SDK 运行验收。

2026-10-28 / 0.1.5.20：配置恢复提示接入首次主页面；真实损坏配置启动后的 UI 呈现尚待实机复测。draft-07 schema 接受重叠入口及显式可空引用，6 项标准校验器检查通过并接入原生 CI；云端任务未运行。C# 129/129、Rust 契约 5/5、Release 构建与签名通过。

2026-10-28 / 0.1.5.19：浮窗在回写前按会话与 StateRevision 拒绝旧查询结果；两个测试覆盖旧结果晚完成及新旧会话交错。并发 Rust 回归使用单一快照观察线程，保留并发添加、镜像单调和最终一致性检查。Rust 141/141、C# 129/129、fmt / Clippy、Release 构建与签名通过；UI 动态回写及 Widgets Board 仍待实机复测。

2026-10-28 / 0.1.5.18：默认解决方案映射 x64，并读取 PE Machine 验证主程序和 Rust DLL 一致。9 项新增回归验证必需缓存字段及条目 null 被拒绝、损坏缓存首刷替换、跨写入生命周期的相同快照仍能落盘；旧实现全失败，修复后 C# 127/127。Debug 解决方案、Release 构建与签名通过，不替代新包的安装与 GUI 验收。

2026-10-28 / 0.1.5.17：所有设置保存请求在后台调度前固定候选与 revision；三个真实 FFI 延迟提交回归确认旧请求收到 SETTINGS_CONFLICT，保留刚提交的分组。Widget 三种尺寸的四桶、pending、占位与末尾未比较绑定回归通过。浮窗的系统关闭已接线为取消关闭并隐藏，Alt+F4 后再打开待实机复测。C# 118/118，Release 构建与测试签名包通过。

2026-10-28：分组归属按 ID 映射，重名与重排回归通过。Provider 退出与回调接收共用生命周期状态，6 项协调测试通过；测试不替代真实 COM / Widgets Board 激活、退出和重启实测。全部 C# 回归 111/111，Release 编译与测试签名包通过。

| 项 | 状态 | 证据 / 待办 |
| --- | --- | --- |
| 2026-11-05 / 0.1.5.23 实机界面验收（computer use 真实输入） | 部分已验证（本轮完成大半） | 主页面（列表 / 排序 / 计数 / 汇总 / 搜索 / 状态筛选 / 行展开操作面板）、行操作执行（对临时分叉仓库 Fetch / Pull 确认+取消+执行 / Push 确认+执行，关系随操作刷新：分叉→本地领先→已同步，反馈与忙态正确）、FolderPicker 添加（PickerHost 独立进程对话框）、移除确认与磁盘保留、分组创建 / 归属 / 删除与参照完整性、托盘真实图标点击浮窗（内容 / 刷新 / Esc / 重开 / 打开主程序）、退出流完整序列与干净退出、小组件固定 / 真实数据 / 深链 / 退出后历史显示。截图证据 target/ui-*.png。测试仓库与分组已清理，设置恢复原状 |
| 行操作执行成功 / 失败与忙态 | 已验证（成功路径 + 确认取消） | 2026-11-05 / 0.1.5.23：临时分叉仓库上 Fetch / Pull / Push 实际执行成功（"Fetch 完成" / "Pull 完成" / "Push 完成"），确认文案、取消确认、执行后关系自动刷新（分叉→本地领先→已同步）全部正确；失败路径（对无效仓库操作）仍待注入验证 |
| 单仓库添加（FolderPicker） | 已验证（主包复验） | 2026-11-05：主包内通过原生对话框添加 target/ui-temp/work，提示"已添加仓库"，列表即时出现，触发扫描（revision 16） |
| 扫描目录 / 候选 / 移除（S06/N15） | 部分已验证 | 移除全流程（确认框 → 列表移除 → 磁盘保留）已验证；扫描目录批量候选勾选仍待界面验收 |
| 分组创建 / 归属 / 删除与参照完整性 | 已验证（0.1.5.23） | 2026-11-05：分组创建、归属下拉选择持久（修复回写 bug 后复验：单次 config.commit、磁盘 groupId 持久）、仓库移除后分组计数归 0、分组删除；0.1.5.22 上实测归属保存 ~230ms 后被回声事件抹除（P1，已修复 + 7 项回归） |
| 设置冲突重载 | 架构修正 | 引擎常驻内存设置：手改 settings.json 不经过引擎内存，无法产生真实 UI 冲突；SETTINGS_CONFLICT → ConflictBar 映射与 expectedRevision 事务语义由 C# / FFI 测试覆盖（144+ 项），真实双写冲突的 UI 呈现留待并发场景 |
| 托盘（注册 / 菜单 / 退出流） | 已验证（浮窗链路本轮补验） | 2026-11-05 / 0.1.5.23：真实托盘图标左键打开浮窗、内容正确（计数 / 时间 / 空态 / 三按钮）、浮窗刷新生效、Esc 收起、重开、"打开 GitaView"聚焦主窗口；右键菜单自动化无法触发（a11y 限制，菜单链路 0.1.2.8 已由用户确认）；退出流经浮窗"退出"完整验证（exit.begin→writer_flushed→exit.done 约 25ms、进程干净退出、Provider 存活） |
| 托盘浮窗实际托盘触发 | 已验证（本轮） | 2026-11-05：真实鼠标点击托盘图标（溢出区内"GitaView 仓库状态"）打开浮窗，锚定图标附近；内容 / 按钮行为全部正确 |
| 主应用完全退出后组件显示历史 | 已随小组件移除（2026-11） | 2026-11-05 的 Board 历史显示验证随子系统删除失效 |
| 三种尺寸模板 | 已随小组件移除（2026-11） | 模板与 WidgetCardContentFacts 单测随子系统删除 |
| Narrator / 完整键盘 / 缩放 / 多显示器 | 部分验证 | 键盘路径本轮实测：Tab 焦点穿越、Enter 触发行展开、Escape 关浮窗；Narrator / 100%-200% 矩阵 / 多屏仍待验证 |
| 启用 / 禁用开机启动 | 待验证 | 本轮未更改系统启动设置 |
| 退出流程等待真停止（R02） | 已验证（GUI 复测完成） | 2026-11-05：浮窗退出触发完整退出序列（约 25ms），进程干净结束；网络操作进行中退出的极端场景仍留待注入 |
| 呈现管线间歇停摆（黑窗 / 帧冻结） | 已解决（根因+修复+实机复核） | 2026-11-05：根因 = DesktopAcrylicBackdrop（其存在下 swapchain 不随窗口缩放、新帧不提交 DWM；最大化冻结、后台启动首帧不呈现、最小化-还原/真实点击/移动均不恢复；AX 与 PrintWindow 证明 XAML 照常渲染）。取证链：可见前台 GDI 截屏 vs PrintWindow vs UIA 三方对照 + 最大化/还原实测；对照实验去除背景材质即全部正常。修复 = 主窗口与浮窗改 MicaBackdrop Kind=BaseAlt（同批 WASDK 2.6.5）；0.1.5.27 实机复核最大化全屏呈现、随窗口缩放、浮窗完整渲染。平台背景 microsoft-ui-xaml #5148（2.6.5 未覆盖本症状，官方修复随 2026 夏季 2.6 期）。Mica 观感比 DesktopAcrylic 含蓄，用户如偏好亚克力可在平台修复后重评 |
| 协议路由与初始化前后的导航调度 | 已验证（逻辑测试），实机待验证 | 2026-10-27 / 0.1.5.14：新增 22 项解析与调度用例，冷请求等待就绪、热请求立即处理，open / 未知 / 已删 ID 回总列表，repo 路由使用当前管理记录。总 C# 100/100、Release 构建通过；WinUI 已接线到状态页、清除筛选与选中滚动，未通过安装后的真实协议 / Widget 点击验证 |
| 托盘不可用时关闭与退出清理 | 已验证（测试），故障注入待验证 | 2026-10-27 / 0.1.5.12：以实际图标注册状态判断驻留，关闭事件取消立即销毁；无托盘时走 RequestExit，退出过程中重复关闭不隐藏窗口、不重复请求、不绕过清理。5 项路由及重复关闭回归用例通过，总 C# 73/73，Release 构建通过；未在真实 Explorer 故障下操作关闭 |
| 无远端与无法比较的文案区分 | 已验证（含实机画面） | 0.1.5.11 的 HasOrigin / HasCommit 投影与标签原因映射覆盖 5 种输入；2026-11-05 / 0.1.5.23 实机核对：无 origin 行显示"未配置 origin 远端"+"无远端"，有 origin 行显示"未比较"，汇总栏与筛选文案一致 |
| 并发 Dispose 关闭 owner（S01） | 已验证（测试） | 屏障双 Dispose 串行（关闭序列整体在 _closeLock）、排空失败重试完成关闭、并发调用+Dispose 无已释放句柄 |
| 快照循环后台调度（S04） | 已验证（测试 + GUI 退出复测） | 循环整体 Task.Run（续体不回 UI 线程）；widget-data.json 写入器随小组件子系统移除（2026-11-07）；2026-11-05 退出序列实测干净退出 |
| 统一设计状态页（S06/N15） | 已验证（主要交互，0.1.5.23 实机） | 标题栏 / 筛选 / 表格 / 行展开 / 汇总栏 / 搜索 / 状态筛选全部实测；完整 DPI / 辅助技术矩阵仍待验证 |
| 冷启动 | 已验证（旧数据作废） | 1.8–3.4s 出自 Debug DLL 混合构建（审查 R11 确认），不能作为 Release 基线；正确产物待重测 |
| 单实例 | 已验证 | 重复启动 1 进程同 PID；自定义 Main 重定向在 XAML 初始化之前 |
| 调用租约 / 句柄生命周期（R01） | 已验证（测试） | HandleLifecycleFacts 3 项：Dispose 后拒绝新调用且幂等、并发调用 + Dispose 无已释放句柄、SnapshotWait 租约归还后销毁 |
| 托盘图标注册 / Explorer 重启恢复（R07） | 已验证（注册与点击，本轮） | 2026-11-05：托盘图标注册并在溢出区出现（Shell_NotifyIconGetRect 可查询、真实点击到达应用）；Explorer 重启恢复仍待复测 |
| Widget 写入故障隔离（R05） | 已随小组件移除（2026-11） | 写入器删除，隔离逻辑不再存在 |
| 关闭驻留 + 刷新持续 | 已验证 | WM_CLOSE 后进程存活 |
| 诊断日志三路 | 已验证 | core / host / provider 三文件产生且内容正确（脱敏）；本轮排查全程依赖三路日志定位 |
| 启动失败恢复路径 | 已验证（代码） | MessageBox + 退出；需故障注入复测（N18 遗留） |
| 搜索 / 分组 / 状态筛选 | 已验证（0.1.5.23 实机复验） | 2026-11-05：搜索 "gita" 命中名称与路径（2/15，摘要显示 2/15）、未比较筛选 3 行、全部 15 行切换正确 |
| 筛选按钮焦点与零分组 | 已验证 | 修复后独立包实测 Tab→空格选分叉→Tab→空格选远端领先，焦点及顺序保留；原生包零仓库显示“全部分组 0”。5 项新增回归测试覆盖筛选内容，总 C# 55/55 |
| 设置分区 / 主题 / 窄窗口 | 已验证（当前环境） | 仓库设置列表及归属下拉、常规设置、浅色→深色、返回后保留筛选；125% 缩放，通过原生窗口菜单缩至约 634 DIP 宽，列表、行按钮及常规设置仍可用；其他 DPI 未验证 |
| Pull / Push 首次确认与取消 | 已验证（含执行） | 2026-11-05：分叉仓库上 Pull 确认文案 + 取消 + 执行全链、Push 确认 + 执行全链；远端不可达等失败反馈仍待注入 |
| 开机启动状态读取 / 不可用反馈 | 已验证（当前环境） | 原生包显示真实“未启用”；独立包不声明任务，复现并修复 ArgumentException 未捕获后一直加载的问题，现显示失败提示并禁用按钮；5 秒等待上限已实现，未注入超时故障 |

## 4. Windows 系统小组件

> 2026-11-07：产品移除 Windows 小组件子系统（Widget Provider、widget-data.json 缓存、Widgets Board 集成与 WidgetCatalogProbe 诊断工具整体删除；Summary 投影改由托盘浮窗经 FFI 实时消费）。本节保留为历史验收记录，不再适用于当前代码。

2026-10-27 / 构建版本 0.1.5.13：Provider 的所有卡片提交统一进入单一队列，执行时读取当前活跃状态、尺寸与最新投影，磁盘读取后再次复核；文件事件合并与过期计时共用此路径，更新 I/O 和 RPC 不持有实例状态锁。新增 5 项屏障测试验证慢旧提交与新数据/尺寸的顺序、待处理重复通知合并、已删除/失活项跳过、读取期间的删除/尺寸变化，以及更新和诊断异常后继续处理、空闲后恢复处理；总 C# 78/78。Release 构建通过，真实 Widget Board COM 回调 / 渲染未实机验证。本机仍安装 0.1.5.11。

2026-10-27 / 构建版本 0.1.5.12：三尺寸模板已修正刷新失败、首刷等待、时间异常提示的可见条件；刷新失败与时间异常同时存在时均展示，历史计数保留。名称仅由外层 JSON 编码一次。6 项新测试验证名称往返与三尺寸提示，修复前全部失败、修复后通过；测试解析真实模板与数据，解析状态块的简单可见性绑定，未运行 Widget Board 渲染。本机安装仍为 0.1.5.11。

| 项 | 状态 | 证据 / 待办 |
| --- | --- | --- |
| 2026-11-05 / 0.1.5.23 小组件实机验收 | 大部分已验证 | 修复 + 预热后：Board 固定"仓库状态"卡片成功；卡片显示真实数据（共 14 个仓库 / 已同步 11 · 待同步 0 / 需关注 0 · 待读 0 · 未比较 3 / 刷新时间 01:53）与"打开 GitaView"深链按钮；卡片数据随应用刷新自动更新（01:48→01:53）；主应用完全退出后卡片保留最后缓存继续显示（历史时间不刷新，符合设计）；Provider 进程在主应用退出后存活。截图 target/ui-widget-*.png |
| Board 发现 | 已验证 | 用户截图（0.1.1.3 起，选择器出现 GitaView）；2026-11-05 选择器复验出现"仓库状态" |
| 添加 + 真实数据显示 | 已验证（0.1.5.23 复验） | 2026-11-05：经 Board 选择器固定成功，卡片真实数据显示。注意：原位更新后首次添加需先预热 Provider（解压约 3.2s 超过添加超时报"当前无法添加所选小组件"，预热后成功）——脚本 -Install 已内置预热，手动 Add-AppxPackage 需自行预热 |
| 生命周期（restore / create / activate / deactivate） | 已验证 | widgetprovider.log：restored=1 → create → activate → deactivate（面板关闭）→ activate（重开） |
| 三种尺寸模板 | 部分验证 | 三模板代码 + 索引测试；Board 菜单切换尺寸的自动化点击未提交（Board 侧菜单回声，自动化受限）；默认尺寸卡片已实测 |
| 深链冷 / 热启动 | 已验证 | gitaview-native://open 热激活存活 + 冷启动拉起；卡片"打开 GitaView"按钮本轮实机点击有效 |
| 过期边界卡片（无新文件跨边界） | 已实现，待验证 | Provider 计时器（last+2×interval+1）；主应用退出后卡片持续显示本轮已观察，跨 2×间隔的过期重查仍待验证 |
| 损坏 / 未知 schema | 已实现，待验证 | noData 占位；Contracts 解码测试锁定 |
| Provider 不加载引擎 / 不写设置 | 已验证（结构） | 工程不引用 Interop / 引擎；仅读 widget-data.json |
| Provider 进程寿命（R12） | 已验证（代码 + 本轮实机观察） | 主应用退出后 Provider 存活（保留历史）；预热激活后进程存活；空闲退出条件在无固定组件时仍由代码保证 |
| 主应用完全退出后组件显示历史 | 已验证（本轮） | 2026-11-05：浮窗退出 → 进程结束 → Board 重开显示最后缓存（01:53 时间戳） |

## 5. 性能与资源（旧 Release 数据作废，待重测）

> 审查 R11 确认：0.1.1.4/0.1.1.5 的"Release"包携带 Debug 核心 DLL（混合构建），
> 下表数值只描述混合构建，不能作为 Release 基线；0.1.3.0 起构建链按
> Configuration 映射 Cargo profile，重测后再更新本表。

| 项 | 数值 | 条件 |
| --- | --- | --- |
| 冷启动到主窗口 | 1.8–3.4s（混合构建，作废） | 需以 0.1.3.0 正确 Release 产物重测 |
| 引擎操作延迟 | duration_ms=0（日志精度） | 0 仓库 |
| git 进程生成 | 49ms/次（Direct，含 Defender 扫描 git.exe 固定成本） | 修复前 149ms |
| 主应用内存 | ~150MB | WinUI + 自包含 .NET |
| Provider 内存 | 已随小组件移除（2026-11） | 进程已删除，无测量对象 |
| 1 / 20 / 100 仓库扫描 | 待测量 | N18 遗留：需用户配置批量测试仓库 |

## 6. 旧数据不触碰证据

- 新应用数据目录 = 包 LocalState（`%LOCALAPPDATA%\Packages\GitaView.Native_paf40868c0wtc\LocalState`）；旧 Tauri 目录（`%APPDATA%\GitaView` 等）从未被读写（核心无任何旧目录引用；EngineCreateInput 由宿主注入新路径）。
- 旧 Tauri 工程已跟踪文件零改动（每轮 `git status` 记录）。

## 7. 已知问题与遗留

| 级别 | 项 | 说明 |
| --- | --- | --- |
| 中 | 窗口背景材质由 DesktopAcrylic 改为 Mica（BaseAlt） | 根因修复呈现停摆（见第 3 节）；观感变化：Mica 为壁纸色调，弱于原亚克力模糊质感；用户如偏好亚克力可在 microsoft-ui-xaml #5148 平台修复落地后重评 |
| 中 | 原位更新后首次固定组件超时 | 已随小组件移除（2026-11），不再适用 |
| 低 | 组件选择器预览图损坏外观 | 已随小组件移除（2026-11），不再适用 |
| 低 | R2R 未启用 | 与 WinUI 投影崩溃（0xc000027b），已关闭；需排查后开启 |
| 低 | PublishTrimmed 未启用 | 反射 JSON 序列化需 source-gen 后才能安全裁剪 |
| 低 | UI 文案内联中文 | resw 迁移待 N18 收尾 |
| 信息（历史） | 重装包后 Board 不自动刷新 | WidgetService 缓存；2026-11 起无小组件集成，部署脚本已不再重启宿主 |

## 8. 完成判定对照（任务清单 N18）

- [x] 按声明 OS / 架构（Windows x64）安装验证核心流程 —— 大部分完成（2026-11-05 / 0.1.5.23 实机验收：主页面 / 行操作 / 设置 / 托盘浮窗 / 退出流；剩余：失败注入、扫描候选批量、完整 DPI / 辅助技术 / 多屏、开机启动切换）
- [ ] 两端组件（macOS 侧阻塞）
- [ ] 性能记录 —— 部分（1/20/100 仓库待测；Release 冷启动重测待做）
- [ ] 规格对齐与审查记录逐项核对 —— 本文件即清单；呈现停摆已解决，余项为验收范围补充（失败注入 / 扫描候选批量 / DPI 矩阵 / 开机启动 / Explorer 重启恢复 / 冷启动重测）
- [x] 旧数据不触碰证据
- [ ] 无未验证平台被标记完成 —— 持续遵守（macOS 全部 blocked；呈现停摆已解决并留证）
