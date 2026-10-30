# 交给 GLM 的启动提示词

> 2026-10-30 清理说明：旧客户端已从当前源码树移除。本文保留原生改造的历史方案或检查记录；旧源码路径、“保留旧工程”和旧发布命令不再适用。当前目录、命令与平台状态以 [原生架构说明](native-architecture.md) 和仓库 README 为准。

将下面整段复制给负责后续实现的 GLM。方案文件都在本仓库中；不要只发送旧的总体规划。

```text
请在当前 GitaView 仓库实现 macOS / Windows 原生应用与系统小组件。

先读 AGENTS.md，再依次阅读：
1. docs/native-app-implementation.md
2. docs/native-app-tasks.md
3. docs/native-app-progress.md
4. docs/native-app-roadmap.md
5. docs/native-app-review.md（本轮发现的边界与必测场景）
6. docs/native-app-ui-design.md（用户已确认的双平台统一 UI，替换此前自由重做布局的约定）
7. docs/native-app-code-review-followup.md（当前修复后复查；先处理 S01–S06，再据实际证据关闭审查）

已经确认的决策：
- 共用 Rust 核心。macOS 用 SwiftUI / 必要的 AppKit，Windows 用 C# + WinUI 3。
- 系统小组件分别使用 WidgetKit、Windows Widget Provider + Adaptive Cards。
- 不做旧版到新版的数据迁移、旧 JSON / IPC 兼容、旧安装升级或数据回退。
- macOS / Windows 主应用必须统一界面设计，在当前旧版设计上升级，保留其信息层级、紧凑密度、筛选条、表格 / 选中行操作与设置结构。先提取旧版组件和共同主题，再分别用原生控件实现，不把 WinUI / SwiftUI 默认模板当另一套最终设计。
- 窗口和系统小组件行为以平台原生为准；不实现 WorkerW / Progman / 特殊桌面层级。系统标题栏、菜单栏 / 托盘、选择器和 Widget 宿主允许平台差异；这不等于两端产品布局可以不同。
- 复用现有 gitaview-core / gitaview-ffi 和 Windows 原生宿主。旧客户端已移除，不恢复旧工程，也不重新提取已完成的核心。
- Git 继续使用固定参数调用用户安装的 Git；仅管理 origin；自动刷新不 Fetch。
- Pull / Push 必须由用户确认，核心再次校验；不能为了测试通过削弱这一要求。
- 主应用退出后不继续扫描，小组件显示最近一次数据与时间。

请执行实现，不要只复述方案或输出另一个计划。按 N00–N18 的依赖逐项推进；每项任务拆成能验证的改动，不要一次生成整套未验证骨架后宣称完成。

当前工作区已有未提交变更和未跟踪源码。先运行 git status 并查看相关 diff，不得 reset / clean / stash 或覆盖这些改动。不得启动旧版本的数据读取或清理。

每轮开始读进度文件，确认上次的任务与证据。每轮结束更新任务状态、实际命令、结果、平台限制和下一步。恢复上下文后以仓库文件和实际状态为准。

先完成 N00 的工作区与环境盘点，再建立新契约和最小原型。在 Windows 环境中可以先做 Rust 与 Windows，缺少 Mac / Xcode 时将 macOS 验证明确标记为阻塞，并继续不依赖它的任务。签名不足也按相同原则处理。不要把未执行的平台测试标成通过，也不要因此中止全部独立工作。

文档中的新接口、脚本和目录是待实现目标，不是当前已存在的 API。实现后必须更新最终路径、命令、fixture 和绑定。外部 SDK 版本与 API 用实际稳定工具链及官方文档核实。

执行时特别核对：snapshot_wait 使用 sessionId + stateRevision；刷新完成看 completedRequestId，不能用 FFI 返回或扫描 generation 代替；设置提交后立即重投影，新增仓库为 pending、删除仓库不得复活；小组件保留有效冷启动缓存，并在无新文件时处理过期。N09 实现正式导出前固定完整 schema 和错误映射。N01 / N10 / N17 的平台子项独立推进，另一平台阻塞不妨碍已满足依赖的工作。

只有实际验证通过的任务标为 verified。只写了代码、mock 画面、通过 Rust 编译或没有签名安装证据，均不足以判定原生小组件完成。

可以做本任务范围内的代码、测试、构建和本地安装产物准备。不擅自推送发布标签、发布安装包、上传证书或向其他聊天发送消息。

现在从 docs/native-app-tasks.md 的 N00 开始，完成可执行工作，并简要报告已验证的结果和下一项任务。
```

续做时可使用下面的短提示词：

```text
继续 GitaView 原生应用实现。先读取 docs/native-app-progress.md 和 docs/native-app-tasks.md，核对 git status，恢复当前任务及其依赖。遵循 docs/native-app-implementation.md 的范围和验收规则，从下一项未完成且可执行的任务继续；不要重复已验证任务，不要重写总体方案。执行后更新进度文件并附实际检查结果。
界面方向以 docs/native-app-ui-design.md 为准：双平台统一设计，在旧版基础上升级。不能继续把当前 Windows 原型的布局当作最终设计。
先核对 docs/native-app-code-review-followup.md 中 S01–S06 的状态，不将“上一轮 13 项已改”当作全部通过；已修复的旧问题不用重复实现。
```
