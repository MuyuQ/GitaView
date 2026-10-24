# GitaView 原生应用验收清单（N18）

状态：**进行中**。本文是 N18 的验收清单与证据汇总；状态列取值：已验证 / 待验证（执行者或用户操作） / 阻塞 / 不适用。每次验收后更新本文件并在 [进度记录](native-app-progress.md) 留检查记录。

## 1. 环境与安装

| 项 | 声明 | 状态 | 证据 |
| --- | --- | --- | --- |
| Windows 11 x64 安装 | MSIX 测试签名安装（0.1.1.8） | 已验证 | `Add-AppxPackage` 成功；Status=Ok；冷启动到窗口 3.4s（0.1.1.5 实测）/ 1.8s（0.1.1.5 后续实测） |
| Windows ARM64 | 后续范围 | 不适用（本轮） | 平台矩阵 |
| macOS 安装 | DMG 签名分发 | 阻塞 | 无 Mac / Xcode / Apple 签名身份 |
| 干净机器无 SDK 运行 | 运行时自包含布局 | 待验证 | Provider 为自包含单文件；主应用自包含布局；需一台无 .NET SDK 的机器复测 |

## 2. 共享核心（gitaview-core / gitaview-ffi）

| 项 | 状态 | 证据 |
| --- | --- | --- |
| 关系 / pending / 计数 / 动作规则 | 已验证 | 60+ 项核心测试（含 8 仓库 fixture 断言与四桶不变式） |
| 设置事务 / 冲突 / 损坏恢复 | 已验证 | 16 项配置集成测试（连续 8 次稳定） |
| 引擎调度（合并 / 丢弃 / 停止 / 唤醒） | 已验证 | 17 项状态机测试（假采集器 + 假时钟，连跑 3 次稳定） |
| 视图查询（筛选 / 搜索 / 排序 / 协调） | 已验证 | 9 项 fixture 驱动测试 |
| 正式 C ABI（20 函数 / 状态码 / buffer / 生命周期） | 已验证 | 13 项 ABI 测试（串行 + 并行）；dumpbin 20 符号与头文件一致 |
| git 命令生成性能 | 已验证（本轮修复） | spawn_bench：挂起 149ms → 直接 49ms / 普通 22ms；Direct 超时终止测试锁定 |
| 诊断日志（脱敏 / 轮转 / 三路） | 已验证 | 5 项 diagnostics 测试；0.1.1.7 安装实测三文件产生且内容正确 |
| macOS 上核心编译与测试 | 部分验证（本轮修复） | 审查发现 Apple 目标 15 个编译错误 → R06 修复后本机 `cargo check --target x86_64-apple-darwin` 通过（2026-10-24 实测）；native-ci.yml macos-core 已加入双 Apple 目标 check（待下次 CI 运行确认）；Mac 实机测试仍阻塞 |
| 并发配置提交不回退镜像（R03） | 已验证（测试） | 引擎层 8 线程屏障压测 + 镜像 revision 单调断言；审查探针结果（revision 24→23）作为反例保留 |
| 删除分组归属完整性（R04） | 已验证（测试） | 空 repoGroups 删组 → 全部仓库归属清空，内存 / 磁盘 / 引擎投影一致 |
| 首刷前 pending 行投影（R08） | 已验证（测试） | 阻塞首扫 + 入组仓库 → 行带组、组计数包含 pending |
| 设置启动完整校验（R09） | 已验证（测试） | 七类非法业务值（未知 schema / revision 0 / 超范围刷新 / 重复 ID / 失效引用）按损坏隔离恢复 |
| 状态读取链统一 Direct（R13） | 已验证（结构） | comparison_ref 并入 branch_state Direct 闭包链，无法单独退回 Suspended；网络操作保留 Suspended |
| 宿主退出终止 Git 子进程树 | 已验证（代码） | Job Object KILL_ON_JOB_CLOSE（R02 配套加固）；实机强杀待复测 |
| 移除后同名仓库身份隔离（S02） | 已验证（测试） | 移除清 collected 缓存 + 复用前核对路径身份；同名不同路径新仓库 pending、无继承关系/分支、网络动作不可用 |
| FFI 输出参数前置校验（S03） | 已验证（测试） | create（null reply / null engine_out）不启动引擎不建目录；save null reply revision 不变；生命周期 null-reply 特例明确 |
| 删除与网络操作交错安全（S05） | 已验证（测试） | 屏障暂停"已定位未拿槽"→移除→恢复：REPO_NOT_FOUND，Git 不启动 |

## 3. Windows 主程序（0.1.4.0）

| 项 | 状态 | 证据 / 待办 |
| --- | --- | --- |
| 并发 Dispose 关闭 owner（S01） | 已验证（测试） | 屏障双 Dispose 串行（关闭序列整体在 _closeLock）、排空失败重试完成关闭、并发调用+Dispose 无已释放句柄 |
| 快照循环后台调度 + 写入器关闭（S04） | 已验证（测试） | 循环整体 Task.Run（续体不回 UI 线程）；WidgetDataWriter 唯一后台写入器、StopAsync flush、失败有界重试、首刷缓存保护（4 项测试）；慢写入 / 退出竞争的 GUI 级验证待实机 |
| 统一设计状态页（S06/N15） | 已实现，待界面验收 | 标题行 + 紧凑摘要 + 组/状态筛选条 + 固定列宽表格 + 选中行展开操作（两段式确认）；按 native-app-ui-design.md §7 对应表实现；**待用户截图与交互核对（同组 fixture）** |
| 扫描目录 / 候选 / 移除（S06/N15） | 已实现，待界面验收 | 设置页：FolderPicker 添加、扫描候选勾选批量添加、已管理仓库路径+分组+移除（只删管理记录，平台确认框）；**待用户全流程操作** |
| 冷启动 | 已验证（旧数据作废） | 1.8–3.4s 出自 Debug DLL 混合构建（审查 R11 确认），不能作为 Release 基线；正确产物待重测 |
| 单实例 | 已验证 | 重复启动 1 进程同 PID；自定义 Main 重定向在 XAML 初始化之前 |
| 调用租约 / 句柄生命周期（R01） | 已验证（测试） | HandleLifecycleFacts 3 项：Dispose 后拒绝新调用且幂等、并发调用 + Dispose 无已释放句柄、SnapshotWait 租约归还后销毁 |
| 退出流程等待真停止（R02） | 已验证（代码） | RequestExit 分段等待至 Stopped（期限 660s > network_timeout 600s）→ 快照循环结束 → Dispose；超时不伪装成功；**GUI 复测待用户**（含网络操作进行中退出） |
| 托盘（注册 / 菜单 / 退出流） | 已验证 | WM_CLOSE 驻留、托盘退出干净结束（0.1.2.8 用户确认）；Explorer 重启后**图标恢复**经 R07 修复（清标记重 NIM_ADD + 有界重试），**待用户重启 Explorer 复测** |
| Widget 写入故障隔离（R05） | 已验证（代码） | 写入失败不终止快照循环 + 有界重试 + 首刷前有效缓存保护；文件级故障注入待复测 |
| 关闭驻留 + 刷新持续 | 已验证 | WM_CLOSE 后进程存活 |
| 诊断日志三路 | 已验证 | core / host / provider 三文件产生且内容正确（脱敏） |
| 启动失败恢复路径 | 已验证（代码） | MessageBox + 退出；需故障注入复测（N18 遗留） |
| 仓库列表 / 筛选 / 搜索 / 行操作 / 设置 UI | 待验证 | 0.1.4.0 统一设计重建后按设计文档 §6 验收（布局 / 筛选顺序 / 表格列 / 选中行操作 / 设置分区 / 主题）——**待用户** |
| Pull / Push 确认对话框 | 待验证 | **待用户**（取消不触发 FFI；执行成功 / 失败反馈） |
| 设置冲突重载 | 待验证 | **待用户**（手工改 settings.json 制造冲突） |
| Narrator / 键盘 / 缩放 / 多显示器 | 待验证 | **待用户** |
| 开机启动状态 | 待验证 | StartupTask 四态显示已实现；待用户点按钮验证 |

## 4. Windows 系统小组件

| 项 | 状态 | 证据 / 待办 |
| --- | --- | --- |
| Board 发现 | 已验证 | 用户截图（0.1.1.3 起，选择器出现 GitaView） |
| 添加 + 真实数据显示 | 已验证 | 用户截图：共 1 个仓库 · 已同步 1（真实 Git 数据） |
| 生命周期（restore / create / activate / deactivate） | 已验证 | widgetprovider.log：restored=1 → create → activate → deactivate（面板关闭）→ activate（重开） |
| 三种尺寸模板 | 已实现，待验证 | 三模板代码 + 索引测试；**待用户切换尺寸观察** |
| 深链冷 / 热启动 | 已验证 | gitaview-native://open 热激活存活 + 冷启动拉起 |
| 过期边界卡片（无新文件跨边界） | 已实现，待验证 | Provider 计时器（last+2×interval+1）；**待用户**：完全退出主应用后跨 2×间隔观察 |
| 损坏 / 未知 schema | 已实现，待验证 | noData 占位；Contracts 解码测试锁定 |
| Provider 不加载引擎 / 不写设置 | 已验证（结构） | 工程不引用 Interop / 引擎；仅读 widget-data.json |
| Provider 进程寿命（R12） | 已验证（代码） | 有界空闲确认退出（构造空集合 60s / 最后 Delete 后 10s；Create/Activate 取消并 Reset；主循环退出前 ConfirmIdleExit 复核）——消除"构造见空集合立即退出"与首次添加的竞争；**首次添加 / 预热后添加复测待用户** |
| 主应用完全退出后组件显示历史 | 待验证 | **待用户** |

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
| Provider 内存 | ~33–68MB | 自包含单文件 |
| 1 / 20 / 100 仓库扫描 | 待测量 | N18 遗留：需用户配置批量测试仓库 |

## 6. 旧数据不触碰证据

- 新应用数据目录 = 包 LocalState（`%LOCALAPPDATA%\Packages\GitaView.Native_paf40868c0wtc\LocalState`）；旧 Tauri 目录（`%APPDATA%\GitaView` 等）从未被读写（核心无任何旧目录引用；EngineCreateInput 由宿主注入新路径）。
- 旧 Tauri 工程已跟踪文件零改动（每轮 `git status` 记录）。

## 7. 已知问题与遗留

| 级别 | 项 | 说明 |
| --- | --- | --- |
| 低 | 组件选择器预览图损坏外观 | Screenshots 资源规格待对齐（发现不受影响） |
| 低 | R2R 未启用 | 与 WinUI 投影崩溃（0xc000027b），已关闭；需排查后开启 |
| 低 | PublishTrimmed 未启用 | 反射 JSON 序列化需 source-gen 后才能安全裁剪 |
| 低 | UI 文案内联中文 | resw 迁移待 N18 收尾 |
| 信息 | 重装包后 Board 不自动刷新 | WidgetService 缓存；部署脚本已自动终止宿主 |

## 8. 完成判定对照（任务清单 N18）

- [x] 按声明 OS / 架构（Windows x64）安装验证核心流程 —— 部分（交互清单待用户）
- [ ] 两端组件（macOS 侧阻塞）
- [ ] 性能记录 —— 部分（1/20/100 仓库待测）
- [ ] 规格对齐与审查记录逐项核对 —— 本文件即清单；N15/N16 交互项完成后勾选
- [x] 旧数据不触碰证据
- [ ] 无未验证平台被标记完成 —— 持续遵守（macOS 全部 blocked；N15/N16 implemented_unverified）
