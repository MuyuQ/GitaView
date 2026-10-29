# native-app 分支审查（2026-10-25）

审查基线：`main` / `origin/main` 为 `06f84da`，`native-app` 为 `600cb259c997a56e74e44efee5c9f0fe1f8fb06d`。本报告是该提交的检查记录，不表示报告之后的修改已验证。

结论：分支整理已完成，共享核心的几项旧问题已修复；当前 Windows 应用仍有设置页运行时错误和交互状态问题，暂不适合合并为已验收版本。继续沿用当前原生技术路线与统一 UI 设计，无须重新规划架构。

## 分支与范围

- 审查开始时工作区干净，`main...HEAD` 为 `0 / 5`，当前分支包含 main 的全部历史。
- 五个提交依次保存旧 Rust/Tauri 参考改动、旧前端行为测试、原生方案文档、Rust core/FFI、Windows 宿主及构建链。
- 差异共 137 个文件，21,076 行新增、338 行删除。该范围包含迁移前积累的参考改动，并非全部都是新原生应用代码。
- 本地未配置 `native-app` 的 upstream，现有远端引用中也没有该分支；本轮未执行 fetch 或 push，因此不判断远端实际是否已存在同名分支。
- 本轮不要求数据迁移、旧安装兼容或恢复旧桌面层。

## 需要修复的问题

### B01 / P1：已管理仓库模板的 x:Bind 数据类型错误

位置：`apps/windows/GitaView.Native/SettingsPage.xaml:168`，关联模板从第 145 行附近开始。

该模板使用经典 Binding 绑定 `ManagedRepoVm` 的名称、路径和下拉选项，却在 ComboBox 的 Tag 上使用没有声明 `x:DataType` 的 `Tag="{x:Bind}"`。本轮 Debug 构建生成的 `SettingsPage.g.cs` 中，`SettingsPage_obj22_Bindings.dataRoot` 类型是 `Microsoft.UI.Xaml.DataTemplate`，`SetDataRoot(newDataRoot)` 将实际行对象转换为 DataTemplate。实际 ItemsSource 是 `List<ManagedRepoVm>`；有仓库的行被创建时会进入错误的类型转换路径，设置页无法正常完成该行绑定。

修复：该 Tag 改成与其他字段一致的 `Tag="{Binding}"`，或为整个 DataTemplate 明确声明正确的 `x:DataType` 并一致使用编译绑定。不要修改 obj 中的生成文件。检查初始 SelectedIndex 绑定是否会触发保存事件，避免打开设置页就写配置。

验收：先增加有至少一个已管理仓库的 UI 回归场景；清理对应构建缓存后重建，检查生成类型；打开设置、切换归属、返回再进入、移除仓库均正常。只测空列表不能覆盖此问题。

### B02 / P1：保存刷新设置在后台线程访问 WinUI 控件

位置：`apps/windows/GitaView.Native/SettingsPage.xaml.cs:466`，具体控件读取在第 475 行。

`OnSaveRefreshClick` 在 `Task.Run` 的委托内部读取 `RefreshEnabled.IsOn`。ToggleSwitch 是 UI 线程对象，在线程池读取其属性违反线程访问要求；这条保存路径会进入错误处理，刷新策略无法按预期保存。当前 interval 已在 Task.Run 前读取，enabled 却没有。

修复：在 UI 线程读取所有控件值，并捕获 expectedRevision、groups、defaultGroupId 等组成不可变的完整 SettingsSaveInput；后台委托只接收请求并调用 FFI。返回 UI 线程后更新提示与加载状态。同类保存处理也应避免在线程池读取可变页面字段，以免快速连续操作构造出混合版本的请求。

验收：在原生 UI 中分别保存自动刷新开启、关闭和不同间隔；重启后核对持久化结果。检查失败信息和冲突重载仍可用。当前 C# 单元测试项目没有引用 WinUI 主应用，42 项测试无法验证该控件线程路径。

### B03 / P2：Widget 写入器仍以磁盘 I/O 阻塞生产者

位置：`apps/windows/GitaView.Native.Contracts/WidgetDataWriter.cs:168`。

`WriteSnapshot` 在共用的 Gate 锁内执行 JSON 编码、缓存文件读取、File.WriteAllText 和 File.Move；`Write`、`RewriteIfStale` 和 `StopAsync` 也需要同一把锁。移动到后台任务后，慢磁盘写仍会阻塞调用者。App 快照循环在调用 Write 之后才发送 SnapshotArrived，因此后续 UI 更新也会被拖住。

本轮独立探针使用现有 WriteHookOverride 阻塞一次写入，再从另一个线程调用 Write；300ms 内第二次 Write 未返回，释放模拟磁盘后才继续。输出：`Write returned while disk write held: False (expected True)`。探针不修改产品代码，位于忽略目录 `target/native-branch-review-probes/`。

修复：Gate 只保护 pending/dirty/stopping 和任务生命周期；唯一 writer 在锁外编码、读缓存及写盘。写入成功状态由 writer 单独维护或短暂加锁提交。保证写盘期间到来的新投影仍保留 dirty，StopAsync 在当前写入完成后继续 flush 最新投影。不要增加第二个并行写入器。

验收：先增加屏障测试，确定 writer 已进入慢写，再验证第二次 Write 与停止请求的发起不等待磁盘锁；释放屏障后确认最后一代落盘、停止可等待、失败重试不会终止快照消费。现有合并与 flush 测试没有覆盖生产者与实际写入重叠的场景。上一轮 S04 应保持部分完成。

### B04 / P2：每次查询重建行对象，丢失操作忙态、确认态与结果

位置：`apps/windows/GitaView.Native/MainPage.xaml.cs:364`，关联快照回调在第 198 行，操作处理在第 589 行之后。

BuildRowsAsync 每次 Rows.Clear 后创建新 RepoRowVm；异步 Fetch/Pull/Push 的处理函数仍持有旧对象。网络操作期间到来的快照、搜索输入或筛选变化会替换当前行，使新行显示空闲，旧对象上的结果提示也无法显示到新行。成功处理主动请求刷新后，同样会重新清空结果。当前忙态守卫只检查旧/新行各自的 bool，不能覆盖对象被替换的过程；核心操作锁仍可防止同时执行 Git，但 UI 会错误地允许再次提交并显示 BUSY。

修复：按稳定的仓库身份增量更新行 VM，或把每仓库的操作状态放到独立状态容器；投影刷新不重置进行中的操作，完成结果回写当前可见状态。仓库删除、ID 被新路径复用、筛选隐藏后再显示时要明确状态清理规则。绑定按钮可用性应包含忙态，并协调同仓库的互斥操作；确认态可在投影变化时主动取消，但应有明确行为。

验收：用可控的慢 Fetch/Pull/Push，在执行中触发快照、搜索、切组以及进出设置页；忙态持续正确、不重复提交、完成或失败结果出现在当前行。不要只验证静态 RepoRowVm 的通知属性。

### B05 / P2：默认分组被保存，但状态页不使用

位置：`apps/windows/GitaView.Native/MainPage.xaml.cs:173` 与 OnNavigatedTo。

设置页持久化 DefaultGroupId，并明确提示它控制默认过滤；状态页始终从 null（全部分组）开始，没有读取 DefaultGroupId。Rust view_query 只协调传入 GroupId，不会自动代入设置默认值。因此保存默认分组后冷启动仍显示全部仓库，该偏好没有产生界面效果。

修复：状态页首次初始化时读取有效 DefaultGroupId，再查询和渲染；后续返回页面保留用户临时选择，不能每次快照到达都覆盖用户筛选。删除默认分组后按核心协调结果回到全部。若要支持立即应用新的默认分组，应在统一设计里写清触发时机。

验收：保存一个含仓库的默认分组后冷启动，确认该组已选中；手动切换后进出设置仍保留临时选择；删除默认组后回到全部，不留下无效筛选。

## 已取得的修复证据

- S01：关闭 owner 包住整个 Dispose。复用上一轮并发关闭探针，第二个 Dispose 在首个 stop 屏障释放前等待，最终 `calls_after_destroy=0`。
- S02：删除旧路径后添加同名新路径，独立探针结果为 `Pending / branch=None / ahead=None / canPush=false`，未复用旧仓库成功状态。
- S03：代码已在创建引擎和业务调用前校验必需输出参数；当前 Rust 回归套件通过。request_stop 的无 reply 例外已在实现中明确。
- S05：操作槽内重新检查配置身份的实现和对应 Rust 测试已提交，本轮工作区测试通过。
- S06：扫描候选、批量添加、已管理仓库与移除入口已写入设置页，但 B01 阻碍非空列表验收，不能据此标记整项完成。

## 本轮验证与边界

实际执行并通过：

1. `cargo test --workspace --locked -q`：135 项通过。
2. `cargo fmt --all -- --check`。
3. `cargo clippy --workspace --all-targets --locked -- -D warnings`。
4. `dotnet test apps/windows/GitaView.Native.Tests/GitaView.Native.Tests.csproj --no-restore --verbosity quiet`：42 项通过。
5. `dotnet build apps/windows/GitaView.Native/GitaView.Native.csproj -c Debug -p:Platform=x64 -p:SkipRustBuild=true --no-restore --verbosity quiet`：0 错误、10 警告，含 Provider 单文件发布提示和未使用字段提示。Rust Debug DLL 已由前面的 C# 测试构建。
6. 上述两个旧问题探针及 Widget 写入阻塞探针。

本轮没有安装 0.1.4.0、没有执行 GUI 交互或截图验收、没有重新验证 Release 安装包和 macOS 实机；B01/B02/B04/B05 为源码及构建生成物检查，B03 有独立运行复现。编译通过不能替代这些交互验收。

UI 方向已有实质调整：去掉顶层 NavigationView，恢复分组/状态筛选条、对齐仓库表、选中行下方操作、仓库/常规两区设置，并加入共享主题角色。`native-app-ui-design.md:130` 仍明确记录紧凑/明细窗口切换未实现；不能把摘要条存在视为该需求已完成。macOS 原生界面尚无可对照的实现，当前只能确认 Windows 正在按共同设计推进。

## 给 GLM 的执行顺序

先修 B01/B02，恢复非空设置页与偏好保存；然后修 B03/B04/B05，并补各项针对性回归。对 B01/B02 至少运行一次带真实非空数据的原生 UI 流程。完成后更新进度表与验收表，将“源码实现”“自动测试通过”“GUI 已验证”分别记录，纠正文档中刷新策略“变更即保存”与实际保存按钮路径不一致的描述。现有 5 个提交保留，在 native-app 上新增修复提交，不重新混入 main，不自动推送或合并。
