# 原生应用代码复查与统一 UI 决策

日期：2026-10-24。对象：GLM 完成首轮审查修改后的当前工作区，Windows 包清单版本 0.1.3.0；HEAD 仍为 `06f84da`，实现大部分尚未提交。本文补充 [首轮代码审查](native-app-code-review.md)，不删除其历史证据。

本轮先复查 N00–N14，再核对已落地的 N15 / N16 / N17.Windows 相关代码。没有改动产品实现、安装包或用户正在运行的应用。产品源码哈希在审查期间单独记录并核对，审查探针位于被忽略的 target 目录。

结论：上一轮多项修复有效，不能继续把那 13 项全部当作未修复问题。本轮确认仍需处理 6 项：2 项 P1、4 项 P2；其中 S06 是已提前实现的 N15 功能缺口。用户同时确认新的共同 UI 约束，见 [统一界面设计](native-app-ui-design.md)。

## 1. 本轮实际检查

| 检查 | 结果 |
| --- | --- |
| `cargo test --workspace --locked -q` | 129 项通过，0 失败 |
| `cargo fmt --all -- --check` | 通过 |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Windows 目标通过 |
| `cargo check --workspace --locked --target x86_64-apple-darwin` | 通过，原 15 个编译错误已修复 |
| `cargo check --workspace --locked --target aarch64-apple-darwin` | 本机缺少该 target 的 std/core，未能完成；属于环境限制，不认定为源代码编译缺陷 |
| `dotnet test apps/windows/GitaView.Native.Tests/GitaView.Native.Tests.csproj --no-restore --verbosity quiet` | 36 项通过，0 失败 |
| Windows 主程序 Release build（x64、无安装） | 通过，0 错误、8 个 Provider 单文件部署相关警告 |
| Release DLL 来源 SHA-256 比较 | 主程序输出 DLL 与 target/release 相同，与 target/debug 不同，原 R11 已得到实际产物验证 |
| 单独重复 R03 并发回归测试 30 次 | 全部通过；测试断言本身的排序问题见后文建议 |
| 真实 Interop 包装 + 可控 native ABI 替身 | 复现两个 Dispose 并发时，destroy 后仍有一个 stop 在执行，见 S01 |
| 真实 Rust DLL 的 null reply 探针 | 复现返回 INVALID_ARGUMENT 后引擎已创建、配置已保存，见 S03 |
| 真实 Engine + 可控采集器 | 复现移除后添加同名不同路径仓库继承历史状态，见 S02 |

未在本轮重装 MSIX、重启 Explorer、执行真实网络写操作或操作用户仓库。GUI、长时间退出、首次添加系统组件和 macOS 实机验收继续单独记录。

## 2. 优先修改

### S01 [P1] 并发 Dispose 仍能使用已释放的 native 句柄

位置：`apps/windows/GitaView.Native.Interop/GitaViewEngine.cs:234`、`:248`、`:267`。

普通调用的 AcquireLease / ReleaseLease 已解决原 R01 的主要窗口，但 Dispose 自身在 `_destroyLock` 之外先复制 `_handle` 并调用 gv_engine_request_stop。该生命周期调用不持租约，也不计入 Rust 的普通调用计数。线程 A 可以在复制指针后或 stop 内暂停，线程 B 完成停止和 destroy；A 随后继续使用旧指针。只将 DestroyCore 串行化不足以覆盖整个关闭过程。

本轮使用原封不动的真实 Interop 程序集，配一个不释放真实内存的 ABI 替身：暂停第一次 stop，让第二个 Dispose 走到 destroy，再恢复第一次 stop，得到：

```text
CONCURRENT_DISPOSE: calls_after_destroy=1 (expected 0)
```

该探针确认关闭调用的顺序缺陷，没有故意对真实 Rust 堆执行访问已释放内存，也没有触发用户应用崩溃。现有生命周期测试只有一个线程 Dispose，或顺序重复 Dispose，未覆盖此路径。

建议：从读取句柄、首次 stop、排空租约、等待、destroy 到清空句柄，都由同一个关闭 owner 串行化；其他 Dispose 等待同一关闭结果或幂等返回。不要让无租约的控制调用越过销毁边界，也不能重新把业务查询和 SnapshotWait 全部串行化。

验证：增加带屏障的两个 Dispose 并发测试，确保任何 native 调用都不会发生在 destroy 之后；覆盖关闭失败可重试。探针可在 `target/review-followup-probes/CloseRaceHarness` 重跑；它是审查辅助程序，不属于产品工程。

### S02 [P1] 移除后添加同名仓库，会把另一仓库的旧状态套到新路径

位置：`crates/gitaview-core/src/engine.rs:402`、`:820`；ID 生成见 `crates/gitaview-core/src/registry.rs:21`。

collected 历史索引只按 ID 查找，配置移除没有清除对应缓存；ID 又由当前仓库名称及现有 ID 集合生成，移除后可以复用。于是删除 `a/shared` 后添加 `b/shared`，两者都得到 `shared`，新记录立即匹配旧行。build_snapshot 会改写 path / name，却保留旧 branch、relation、ahead 和 actions，使新仓库显示为旧仓库的状态。

在第三轮采集阻塞期间，公开 API 探针得到：

```text
REMOVE_READD: oldId=shared newId=shared readState=Ok
branch=Some("FROM-OLD-REPOSITORY") ahead=Some(7) canPush=true
```

预期为 pending，未知 Git 字段为空，网络动作不可用。此次没有执行 Push；核心执行前校验仍然存在，但不能因此接受界面和 Widget 展示另一仓库的状态。

建议：配置提交时剔除已经删除的缓存；复用历史结果至少同时核对仓库 ID 与规范路径。可进一步采用不复用的稳定仓库标识，避免历史深链指向另一仓库。过时扫描仍按配置 revision 拒绝。

验证：A 成功采集 → 阻塞下一轮 → 移除 A → 添加另一目录下同名 B；断言 B 为 pending、摘要不含 A 的关系、Push 不可用；再覆盖随后全局采集失败。审查探针在 `target/review-followup-probes/IdentityProbe`。

### S03 [P2] FFI 输出参数非法时，仍先执行创建或配置写入

位置：`crates/gitaview-ffi/src/lib.rs:75`、`:154`、`:177`、`:229`。

write_status 到最后才检查 reply 是否为空，run_call 在此之前已执行 body。create 也是先启动引擎和分配句柄，再发现 reply 为空并返回 INVALID_ARGUMENT。这违反“create 失败保证输出句柄为 null”，也让调用方收到错误时实际配置已经改变。

真实 DLL 探针结果：

```text
CREATE_WITH_NULL_REPLY: {"status":5,"handleNonNull":true}
SAVE_WITH_NULL_REPLY: {"status":5,"beforeRevision":1,"afterRevision":2,
                      "refresh":{"enabled":false,"intervalMinutes":7}}
```

engine_out 为空的有效创建请求还会启动后再丢弃 Engine；单纯 drop 不会请求停止持有 Arc 的扫描线程。本轮没有刻意创建这种无法从输出回收的引擎。

建议：在所有有副作用的业务体执行前验证必需的输出参数，create 先初始化 out engine 为 null，失败时清理已创建资源。生命周期接口若希望允许空 reply，应明确作为特例并返回一致状态；当前 Dispose 使用该特例不能成为业务函数忽略输出校验的理由。

验证：有效 JSON + null reply / engine_out，断言返回参数错误、没有新线程和设置目录副作用、没有配置 revision 变化。对网络写入口使用假执行器，保证参数错误发生在 Git 启动前。

### S04 [P2] Widget 写入已捕获异常，但磁盘 I/O 仍在 UI 线程

位置：`apps/windows/GitaView.Native/App.xaml.cs:142`、`:149`、`:158`、`:371`；`WidgetDataWriter.cs:31`、`:104`。

RunSnapshotLoopAsync 从 OnLaunched 的 UI 上下文启动；await Task.Run 默认恢复该上下文，后续 WidgetSnapshot / WidgetDataWriter.Write 又同步执行。catch 解决了原来的“一次写入错误终止订阅”，没有解决写入延迟卡住界面的问题。文件写入器仍无独立 worker、停止入口和最终 flush；退出末尾 Dispose 同样直接在恢复后的 UI 上下文执行，异常排空路径可长时间阻塞主线程。

建议：明确后台快照消费与唯一最新投影写入器的调度；UI 只提交展示状态。写入器提供可等待停止 / flush，退出时完成这些任务再销毁引擎；可能阻塞的 Dispose 排空也应留在后台。继续保留已加入的异常隔离与有效缓存保护。

验证：注入慢写入、临时替换失败、退出与重试并发，断言 UI 调度持续响应、任务数量有界、最后有效投影按策略完成发布。不要仅凭 try/catch 或注释写着“后台循环”判定这一项完成。

### S05 [P2] 获取仓库操作锁后没有复核记录，删除与 Git 操作可能交错

位置：`crates/gitaview-core/src/engine.rs:878` 至 execute_repo_operation；仓库锁位于 `crates/gitaview-core/src/git/commands.rs` 的 execute_repo_operation。

引擎先读出 ID 对应路径，再进入 execute_repo_operation 获取仓库槽。如果期间另一线程取得槽、删除记录并释放，Git 操作仍会在已经移除的旧路径上执行。当前逻辑只复核 Git 状态，没有重新确认该 ID / 路径仍在受管理配置中，与实施说明 §7 的明确锁顺序不一致。

建议：由应用服务获取仓库操作槽，在槽内重新读取并确认同一 ID / 路径仍存在，再执行 Git 前置检查与命令。不要把设置锁持有到网络调用，也不要重复获取同一非重入仓库槽。

验证：用屏障暂停“已定位路径、尚未取得槽”的操作，完成 remove 后恢复；必须返回 REPO_NOT_FOUND，假执行器不得收到网络命令。本项为静态可行交错，本轮未实际运行网络操作。

### S06 [P2] N15 尚缺仓库扫描和移除入口，不能称代码完整

位置：`apps/windows/GitaView.Native/MainPage.xaml`、`MainPage.xaml.cs:316`、`SettingsPage.xaml`、`SettingsPage.xaml.cs`。

当前主程序只有单仓库 FolderPicker 添加入口。检索所有宿主 C# / XAML，未发现 ScanDirectory / RepoRemove 的接线或对应扫描候选、移除控件。后端和 Interop 有接口，用户界面没有，因此不是“等待用户操作验收”就能关闭的项目。设置页也尚未落实共同设计规定的仓库管理区域。

建议：保留 N15 为未完成，补扫描候选选择、托管仓库路径列表、移除和反馈；移除只删除管理记录。随后按统一 UI 要求调整页面，不能只给当前模板换颜色。

## 3. 首轮意见的复核结果

| 首轮项目 | 本轮结论 |
| --- | --- |
| R03 / R04 / R08 / R09 | 配置串行范围、全仓库归属清理、pending 分组与完整设置校验已实现，新增回归通过；S02 是另外的身份 / 缓存缺陷 |
| R06 | x86_64 Apple 目标已通过；arm64 本机缺 target，Mac 链接与实机仍待验证 |
| R10 | 先前索引偏移已修，不能重复报告旧问题；GUI 完整性见 S06 |
| R11 | Release 编译映射与实际 DLL 来源已确认修复，不再是混合 Debug DLL |
| R13 | comparison_ref 已复用本地 Direct 执行闭包，旧的未生效策略字段已移除 |
| R07 / R12 | 托盘重注册和 Provider 空闲宽限已有代码；Explorer 重启、首次添加及生命周期实机证据仍需补齐 |
| R01 / R02 / R05 | 普通调用租约、长退出等待、写入错误隔离已改进；S01 / S04 的完整关闭和线程约束仍需处理 |

新增并发测试 `concurrent_config_commits_keep_mirror_monotonic` 使用多个线程返回的 saved.revision 与共享 fetch_max 比较，存在把正常返回调度误判为回退的窗口。例如 A 的 revision 2 已返回但线程暂停，B 先记录 revision 3，A 恢复后记录 2，不代表镜像回退。跨线程采样后的比较也有相同问题。本轮单独运行 30 次均通过，未声称复现偶发失败；建议改为单观察者顺序读取公开镜像，或在有定义的发布事件序列上断言，另行核对最终磁盘一致性。

## 4. 用户新确认的 UI 方向

用户明确要求“2 个平台应该统一界面设计，在原有设计的基础上升级”。此前方案将“平台原生”扩大解释为允许重做布局，促成当前通用模板页面。这个偏差来自交接要求，应先修正文档，不能仅将旧方案允许的差异归责为 GLM 违反设计。

现已新增 [统一界面设计](native-app-ui-design.md)，并同步实施说明、任务、roadmap 与启动提示词：

- 以旧版紧凑摘要、展开面板、分组 / 状态筛选条、对齐表格、选中行展开动作及设置分区为基础升级。
- 两端采用相同布局、视觉角色、密度、组件和业务交互；分别用 SwiftUI / WinUI 实现共同设计。
- 系统标题栏、菜单栏 / 托盘、对话框、字体度量与 Widget 宿主允许原生差异，不因此另做一套主应用外观。
- 当前 Windows 模板需调整；保留已有 Rust、FFI、生命周期和服务接线。视觉修正与底层重写分开处理。

本轮没有制作新的像素稿或修改产品界面。后续 GLM 必须以这份共同设计和同一 fixture 的截图 / 交互验收推进 N12 / N15。

## 5. GLM 的续做顺序

1. 先修 S01 / S02，加入可控交错的生命周期与仓库身份回归，再修 S03 的 FFI 输入 / 输出校验。
2. 完成 S04 的后台调度、写入器关闭和 UI 响应验证；修 S05 的仓库槽内配置复核。
3. 读取共同 UI 文档，建立旧版组件与原生组件对应表和主题；在同一设计下补齐 S06 的功能，再做 N15 / N16 的实机验收。
4. 更新进度中的实际完成条件：不要把“代码改过”写成全部验证完成，也不要在修复后的报告中继续沿用旧编译失败或旧 Debug DLL 结论。
