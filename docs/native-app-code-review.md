# 原生应用代码审查：N00–N14 及当前 Windows 增量

日期：2026-10-24。审查基线：HEAD `06f84dacfc7bc6ae2d339c3595d0e42434b0ec69` 加当前未提交工作区。原生代码主要为未跟踪文件，不能只用 `git diff HEAD` 判断审查范围。

本文保留首轮审查事实。GLM 已据此修改，修复后的当前结论见 [后续复查](native-app-code-review-followup.md)；不要继续将本文的旧编译失败、旧索引和 Debug DLL 结论当作当前缺陷。UI 按用户新确认的 [统一界面设计](native-app-ui-design.md) 继续实现。

本轮以 N00–N14 为主。工作区已包含 N15、N16 和 N17.Windows 的代码，因此也检查了与核心、宿主和部署直接相连的部分。macOS 应用尚未实现；这里的 macOS 问题是共享 Rust 核心编译失败，不能归因于缺少 Swift 工程或签名身份。

结论：现有分层和固定 Git 操作边界可以继续使用，但当前实现仍有需修复的问题。记录 13 项：6 项 P1、7 项 P2。不要依据已有测试通过就将 N05–N10.Windows、N14 的全部行为视为验收完成。

本轮只新增此审查报告，没有修改应用实现、旧 Tauri 工程或 GLM 的进度记录；没有重装应用、重启 Explorer、结束用户进程或改用户 Git 配置。

## 实际执行的检查

| 检查 | 结果 | 适用范围 |
| --- | --- | --- |
| `cargo test --workspace --locked` | 124 项通过，0 失败 | 本机 Windows；含 core / FFI 单元及集成测试 |
| `cargo fmt --all -- --check` | 通过 | 根原生 workspace |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | 通过 | 本机 Windows 目标 |
| `dotnet test apps/windows/GitaView.Native.Tests/GitaView.Native.Tests.csproj --no-restore` | 33 项通过，0 失败 | C# Contracts / Interop / 现有 Provider 测试 |
| `dotnet build apps/windows/GitaView.Native/GitaView.Native.csproj -c Release -p:Platform=x64 -p:AppxPackage=false -p:AppxPackageSigningEnabled=false --no-restore` | 通过，0 错误、10 警告 | Windows 主程序与其构建依赖；未安装 |
| `cargo check --workspace --locked --target x86_64-apple-darwin` | 失败，15 个编译错误 | 共享 Rust 的 macOS 目标静态检查，不涉及 Swift / 签名 |
| 独立 Rust 审查探针：调用公开 SettingsService / Engine API | 复现分组删除、pending 归属和非法配置被接受 | 使用唯一临时目录，未改实现 |
| 独立并发探针：8 线程同步开始，50 轮添加共 400 个候选仓库，观察配置 revision | 捕获 revision `24 → 23` 回退 | 注入无 I/O 持久化器和阻塞采集器，隔离配置发布竞争 |

Windows GUI、真实 Explorer 重启、长网络操作退出、首次添加 Board 组件没有在本轮实机执行；对应问题明确按代码路径分析，不伪称已复现 GUI 结果。Release 构建警告主要涉及 Provider 单文件部署设置、未使用字段和宿主可空引用，本轮没有把“有警告”单独算成缺陷。

## P1：先修复生命周期、数据一致性与跨平台编译

### R01：C# 裸句柄存在读取后被销毁的竞争

位置：`apps/windows/GitaView.Native.Interop/GitaViewEngine.cs:255` 的 CurrentHandle、`:269` 等调用入口和 `:200` 的 Dispose；关联 `crates/gitaview-ffi/src/lib.rs:321` 的实际释放。

CurrentHandle 在锁内取出 IntPtr 后立即释放锁，调用方随后才进入 FFI。期间另一个线程可以 Dispose，停止并释放 Rust 的 Box；第一个线程恢复后仍把已释放地址传入 FFI。Rust 的 active 计数要进入 acquire_call 后才增加，无法保护“C# 已取出指针、尚未进入 Rust”的窗口；destroy 返回 BUSY 的重试也无法补这个窗口。结果可能是 native 访问已释放内存及进程崩溃。

建议：在宿主侧建立调用租约。取句柄与增加宿主调用计数必须在同一个短临界区完成，调用结束用 finally 归还；关闭先阻止新租约，再等待已有租约及 Rust 引擎结束，最后 destroy。也可采用与此语义等价的 SafeHandle 引用保护。不能恢复“整个 SnapshotWait 持同一个串行锁”的旧做法，也不能在 finalizer 中等待网络操作。

验证：增加可控屏障，使业务线程完成租约获取后暂停；同时执行关闭，确认 destroy 在该调用归还前不会发生。覆盖 SnapshotWait、普通查询、生命周期调用与重复 Dispose。现有并发测试只覆盖多个读调用及等待不阻塞查询，没有覆盖销毁窗口。[Microsoft：原生互操作生命周期建议](https://learn.microsoft.com/en-us/dotnet/standard/native-interop/best-practices)

### R02：正常退出会在 Git 尚未完成时结束进程，失败后还丢失句柄

位置：`apps/windows/GitaView.Native/App.xaml.cs:302`、`:305`、`:313`；`apps/windows/GitaView.Native.Interop/GitaViewEngine.cs:210`、`:219`、`:225`。

宿主忽略 WaitStopped(30000) 的 false 结果，继续 Dispose；Dispose 最多再等待 13 次 × 5 秒，之后抛异常。宿主吞掉异常，仍记录“engine stopped and disposed”并 Environment.Exit(0)。因此一个仍在合法 600 秒期限内的 Pull / Push，可能在约 95 秒后失去宿主。当前 Windows Job 未配置主进程退出即清理整个 Job，这也不能被描述为已安全取消所有 Git 进程。

Dispose 在确认释放成功前已经把 `_handle` 清零；等待超时或 destroy 失败后，再调用 Dispose 会直接返回，不能按注释“稍后重试”完成回收。

建议：建立可等待的关闭任务，进入 Stopping 后后台分段等待并保持 UI 可响应；只有真正 Stopped、宿主调用租约清零、快照循环和写入器结束后才 destroy / Exit。超时表示继续退出中或显示诊断，不能当成功；失败前保留可恢复的 owner。日志只记录实际完成的阶段。

验证：用可控阻塞业务和注入短等待期限验证，不必真实等 600 秒；超过单轮等待期限后进程及句柄仍有效，业务完成后仅销毁一次。增加 Dispose 失败后可恢复的测试。[Microsoft：Job Objects 的关联与终止规则](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects)

### R03：并发配置提交会把引擎镜像回退到旧 revision，已复现

位置：`crates/gitaview-core/src/engine.rs:760`、`:767`、`:778`、`:791`，尤其 `:802` 无条件覆盖 state.settings。

SettingsService 的持久化事务串行，但引擎的 commit_config_change 在其外执行。线程 A 保存 revision 23 后被挂起，线程 B 保存并发布 24，A 恢复后仍能发布 23。get_settings、快照、分组和 Widget 随之回到旧配置；磁盘与服务内部却保留新配置。被移除记录也可能在这个路径重新出现。

审查探针在 8 线程屏障下观察到 `CONFIG_REVISION_REGRESSIONS: count=1 examples=[(24, 23)]`。第一轮没有屏障的探针未复现，增加屏障后复现；并非把一次未复现当成通过。

建议：把“设置服务事务 + 引擎镜像提交”置于同一个配置写入串行范围，保持磁盘 I/O 不占用刷新状态锁；同时拒绝旧 revision 覆盖新镜像。扫描发布还要核对采集 revision 与实际用于投影的配置版本。不能只在 SettingsService 的测试里断言写入不丢失。

验证：测试 Engine 层的并发 add / remove / save，不只测试 SettingsService；用屏障控制两个事务发布顺序。断言订阅到的 settingsRevision 永不回退，最终内存 / 磁盘一致，已删记录不复活。

### R04：删除含仓库的分组会保留悬挂归属，当前设置 UI 随后可能抛异常

位置：`crates/gitaview-core/src/settings.rs:217`、`:223`；实际调用路径为 `apps/windows/GitaView.Native/SettingsPage.xaml.cs:216`，读取路径为 `:70`。

save 只遍历本次 repoGroups 中的仓库来清除不存在的 groupId，其他仓库保持原样。但设置页删除分组时明确传 RepoGroups = []。结果是 groups 已为空，仓库仍属于旧组；随后 LoadFromEngine 使用 Groups.First(g => g.Id == groupId)，找不到记录便抛 InvalidOperationException，未被只处理 GvException 的分支捕获。

公开 API 探针结果：`DELETE_GROUP: groups=0 repo.groupId=Some("g") (expected None)`。现有“删除分组”测试把该仓库也放进 repoGroups，遗漏了真实 UI 使用的空数组路径。

建议：在构造候选设置时，对全部仓库归属执行参照完整性检查，删除组后统一归未分组；再应用约定的显式归属覆盖。UI 读取也用 TryGet / FirstOrDefault 安全处理异常数据，但核心必须首先修复，不能只让界面绕过。

验证：复用真实删除组 payload（repoGroups 空），断言内存、磁盘、引擎投影归属均为空；重开设置页不报错。

### R05：一次 Widget 文件写入失败会终止主应用的唯一快照循环

位置：`apps/windows/GitaView.Native/App.xaml.cs:172`、`:184`；`apps/windows/GitaView.Native/WidgetDataWriter.cs:53`、`:54`。

WriteAllText / File.Move 可能因共享文件占用、权限或磁盘错误抛异常。调用位于 RunSnapshotLoopAsync 的 try/catch 之外，该 Task 又以丢弃返回值的方式启动。一旦写入失败，循环结束，本次 SnapshotArrived 也不会触发，之后核心继续扫描但 UI 与组件都不再收到更新。平台发布失败因此反过来破坏了主应用状态消费。

建议：把写入器与订阅解耦为单一、有界的最新待发布投影处理器；失败记录诊断、保留最新数据并有界重试，不能终止核心消费循环。宿主持有并观察快照任务，区分正常停止、可恢复发布失败和引擎故障。写入与阻塞 FFI 放到后台，关闭时明确等待任务与写入器；补上方案规定的首刷前有效缓存保护。

验证：对写入器注入一次替换失败，之后连续推进两代快照；UI 均收到状态，写入可恢复，任务数量有界。再测试停机与写入并发。此项为代码路径确认，本轮未锁定用户实际共享文件。

### R06：共享核心当前无法编译 macOS 目标

位置：`crates/gitaview-core/src/git/process.rs:24`、`:45`；`crates/gitaview-core/src/git/commands.rs:119`。

本机运行 `cargo check --workspace --locked --target x86_64-apple-darwin` 得到 15 个错误。Windows 的 impl ProcessTree 缺少 cfg，进入 Unix 编译并导入 std::os::windows / windows crate，与 Unix 方法重复；Unix spawn 调用 spawn_with_mode 缺一个 SpawnMode 参数；登录 shell 探测调用 run_command_with_timeout 也缺该参数。

建议：给 Windows impl 增加平台条件，修复全部模式参数调用，检查两端公共签名。把两个 Apple target 的 cargo check 加入可执行的核心验证流程。无需等待 Mac / Xcode / 签名才能先修复这些静态错误；Windows 上的检查仍不能替代 Mac 上的测试和链接验证。

验证：本机可执行的 Apple target check 先通过；取得 Mac 后再运行核心测试 / clippy 与实际 Swift 链接。当前 acceptance 文件中“macOS 上核心编译与测试已验证（CI）”不能作为当前工作区的通过证据，需关联确切提交及 CI run。

## P2：修复已落地行为和后续验收缺口

### R07：Explorer 重启后托盘不会重新注册

位置：`apps/windows/GitaView.Native/TrayWindow.cs:73`、`:205`、`:208`。

TaskbarCreated 分支调用 AddIcon，但 `_iconAdded` 仍为 true，AddIcon 立即返回。接到 Shell 重建通知后应认为图标已不存在，清掉本地标记再 NIM_ADD；必要时有界重试。否则隐藏窗口后的主要入口可能消失。“Explorer 重启后应用进程存活”不能证明图标恢复。

验证：先用替身记录 Shell_NotifyIcon 的调用，断言收到 TaskbarCreated 必须再 NIM_ADD；实际验收再由用户确认 Explorer 重启后图标可见且菜单可用。本轮没有重启用户 Explorer。[Microsoft：Taskbar Creation Notification](https://learn.microsoft.com/en-us/windows/win32/shell/taskbar#taskbar-creation-notification)

### R08：pending 仓库丢失分组，组计数和组内列表在首刷前不正确

位置：`crates/gitaview-core/src/engine.rs:405`。

build_snapshot 对尚未收集的仓库直接调用 RepoStatus::pending，构造结果的 group_id 固定为 None，没有复制配置记录的 group_id。探针结果为 `PENDING_GROUP: settings=Some("g") snapshot=None`。新仓库已经配置进某组，却暂时不计入该组；若扫描全局失败会持续如此。

建议：不论历史行还是 pending 行，都由当前配置统一覆盖 id / name / path / groupId。测试在阻塞首扫时向指定组添加仓库，立刻断言组计数、组内行与 pending 数；再覆盖首扫失败。

### R09：启动时只做 JSON 解码，未知 schema 和非法值直接进入设置

位置：`crates/gitaview-core/src/settings.rs:175`、`:176`。

serde 解码不能代替业务校验。探针写入结构合法但 schemaVersion=999、intervalMinutes=0 的新原生配置，open 正常返回，warnings=0。代码注释所说“旧 schema 按损坏恢复”实际没有实现；重复组 ID、失效引用等也没有统一启动校验。

建议：建立完整 SettingsData 校验，在读入文件和构造写入候选时共用。未知 schema / 不满足约束按约定隔离并提示恢复，隔离失败则初始化失败。这里只处理新原生应用自身的文件，不引入旧 Tauri 数据迁移。

验证：除坏 JSON 外，增加未知 schema、零 / 超范围刷新间隔、重复 ID、无效分组引用等有效 JSON 反例，并断言警告和原文件保留。

### R10：N15 两处下拉索引偏移会筛错组或改错归属

位置：`apps/windows/GitaView.Native/MainPage.xaml.cs:190`、`:195`；`apps/windows/GitaView.Native/SettingsPage.xaml.cs:70`。

MainPage 的 `_groupOptions` 已包含第 0 项“全部分组”，读取却使用 index-1，反向选择又额外 +1。选择第一个真实组会读成全部，选择第二组会读成第一组。SettingsPage 的 options 已含“未分组”，IndexOf 已给正确位置，初始化再 +1 会显示下一组或无选中；随后保存可能把归属改错。

建议：两处各自保留唯一的选项数组，直接用选项绑定稳定 ID，避免同时维护 UI index 与偏移规则。最低限度修复三个多余偏移，增加索引与 ID 双向测试。

验证：至少两个组和未分组仓库；分别选择全部 / 第一组 / 第二组，断言提交给核心的 ID；打开设置不修改直接保存，所有归属必须不变。这是超出 N14 的已实现 UI 增量问题。

### R11：Windows Release 构建始终携带 Rust Debug DLL

位置：`scripts/native/build-windows.ps1:25`；`apps/windows/GitaView.Native/GitaView.Native.csproj:86`、`:103`。

脚本与 MSBuild Target 都只执行 cargo build，复制路径固定 target/debug。即便主应用选择 Release，核心仍为 Debug。本轮 Release 构建输出也明确显示 Rust `dev` profile；进一步比较 SHA-256，Release 主程序输出中的 DLL 与 target/debug/gitaview_ffi.dll 完全相同。因此已有“Release 性能”数据只能描述混合构建，不能代表完整 Release 核心。

建议：统一 Configuration 到 Cargo profile 的映射，Release 使用 --release --locked 并复制 target/release 的 DLL；Debug 对应 debug。只保留一处构建责任，避免脚本与项目各编译一次。打包记录实际 Rust profile 与 DLL 来源。

验证：分别构建两种配置，核对实际构建命令、输出路径与包内 DLL；性能基线在正确产物上重测。这是 N17.Windows 增量问题，本轮未打包或重装。

### R12：Provider 的空列表退出事件可能抢在首次 Create 前生效

位置：`apps/windows/GitaView.Native.WidgetProvider/GitaViewWidgetProvider.cs:72`、`:78`；`apps/windows/GitaView.Native.WidgetProvider/Program.cs:86`。

构造时 GetWidgetInfos 返回空集合即 Set EmptyWidgetListEvent，主循环随即撤销 COM 服务并退出；CreateWidget 不 Reset 事件。若主机正处于“创建 Provider → 随后 CreateWidget”的首次添加路径，或与最后一次 Delete / 新 Create 交错，进程可能在接单时退出。安装预热只解决解压耗时，不能修复这个事件竞争。

建议：用有界空闲退出与活动 COM / 回调计数管理进程寿命，创建 / 激活时取消待退出，最后 Delete 后确认稳定空闲再退出。同步空闲事件与实例表。不要简单在构造函数看到空集合就断言以后不会 Create。

验证：无任何固定组件时第一次激活，延迟后 Create；最后 Delete 与新 Create 并发；安装预热完成后首次从 Board 添加。此项为静态生命周期竞争，本轮没有实机复现 Board 失败；N16 尚不能据现有 staleness 测试判完成。[Microsoft：Provider 创建、删除与启动恢复](https://learn.microsoft.com/en-us/windows/apps/develop/widgets/implement-widget-provider-cs)

### R13：状态读取仍有命令走挂起路径，性能修复没有覆盖全链

位置：`crates/gitaview-core/src/git/commands.rs:317` 的 comparison_ref，函数内两次 run_git_monitored；关联 `run_git_monitored:215` 与 `branch_state_monitored:371`。

branch_state 主体用 Direct，但 comparison_ref 仍调用固定 Suspended 的 run_git_monitored，正常带 upstream 的仓库至少还有一次全系统线程快照，后备路径还可能再做一次。ExecPolicy.suspend_spawn / for_status_reads 没有被执行器消费。当前“全部本地读命令已切 Direct”的注释与实际不一致。

建议：把 comparison_ref 等完整本地状态链统一传入明确的生成模式，删除无效策略字段或让它真正生效；网络写操作继续保留所需进程树约束。Direct 也要测试立即派生子进程与 Job 关联之间的竞争，不能用单一父进程超时测试宣称完全等价。

验证：用可观察执行器记录完整状态扫描的每次模式；带正常 upstream、无 upstream 后备等路径都符合约定。测量 1 / 20 / 100 仓库的完整扫描，不能只引用单次 spawn 基准。

## 交给 GLM 的修改顺序与验收要求

1. 修 R01 / R02，先恢复可靠的调用租约和异步关闭；不得重新引入等待持全局锁。将停止任务、快照任务、写入任务纳入可观察生命周期。
2. 修 R03 / R04 / R08 / R09，补 Engine 层配置发布回归与完整设置约束。修复前保留本轮探针结果作为反例，不能只增加恰好适配实现的断言。
3. 修 R05 / R07，分别故障注入共享文件写入和托盘 Shell 重建；验证主应用持续消费状态。
4. 修 R06 并恢复 Apple 目标静态检查，再处理 R13 的完整状态执行链。Mac 实机测试仍单独标记待验证。
5. 处理已落地的 R10–R12，再做 N15 / N16 / N17.Windows 的交互与部署验收。不要通过继续扩大功能绕过基础缺陷。
6. 重跑本报告中适用检查，更新进度与 acceptance 的具体版本 / 提交 / 命令 / 证据。当前有“代码已验证但待故障注入”、N17.Windows 子项 todo 而汇总称 verified、旧包版本和新修复混写等记录，需按实际证据整理；不删除历史失败记录。

建议重新打开的验证项：N05（跨平台与进程路径）、N06（设置完整性）、N07（引擎配置发布）、N09 / N10.Windows（句柄全生命周期）、N14（退出 / 托盘 / 快照循环）；N15 / N16 继续保持 implemented_unverified。某项单元测试通过的事实可以保留，但不能代表未覆盖的整项行为已验收。

每项修改至少附：触发条件、修复位置、先失败后通过的回归、实际检查结果，以及未运行的目标平台或 GUI 验收。这里没有要求旧数据迁移、旧桌面层兼容或额外产品功能。
