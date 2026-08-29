# GitaView 改进计划书

**基线**: `main` @ `fd06861` / v0.3.2
**日期**: 2026-08-30
**方法**: 基于当前代码的全量分析（后端 Rust / 前端 React / CI 与发布流程），所有问题均在本次分析中实际复现或逐行核对过源码。本文不重复 `CODE_REVIEW_REPORT.md`（2026-05、2026-06 两轮）中已修复并验证的项目，仅收录仍然成立的新问题与历史遗留。

---

## 0. 执行摘要

当前代码分层清晰、安全边界（固定参数 git、二次确认、URL 白名单）执行到位，但**质量门禁已经失守**：CI 自 2026-07-28 起连续失败，本地前端测试也有 4 个用例红着，一个月内合入的 3 次 "更新 readme" PR 全部带着失败 CI 进入 main。同时发现一个疑似泄漏凭据的文件已被提交进 git 历史。

最高优先级不是新功能，而是：**恢复门禁（P0）→ 修复用户可见的错误路径与 macOS widget 数据链路（P1）→ 健壮性与无障碍（P2）→ 卫生项（P3）**。

| 级别 | 数量 | 主题 |
|------|------|------|
| P0 紧急 | 3 | CI 全红、前端测试红、疑似凭据文件入库 |
| P1 高 | 7 | 错误显示为成功、折叠态静默失败、widget 数据不更新、Windows 垃圾目录、网络操作超时、settings 损坏变砖、macOS git 解析失败 |
| P2 中 | 12 | 持久化竞态、刷新性能、无障碍、文档漂移、流程自动化 |
| P3 低 | 10 | 死代码、依赖卫生、i18n、发布收尾 |

---

## 1. 现状快照

- 版本 0.3.2，`package.json` / `Cargo.toml` / `tauri.conf.json` 三处版本一致，tag 校验脚本齐备。
- 前端测试 118 例（**4 失败**），Rust 测试 54 个通过。
- CI（三平台矩阵）**自 2026-07-28 全红**；Release 流程上次成功为 v0.3.1-unsigned。
- macOS WidgetKit 原生 widget 已实现（`widget-extension/`、`widget_data.rs`、`beforeBundleCommand`），但 `AGENTS.md` 仍标注"implementation pending"。

---

## 2. P0 紧急项（本周内，恢复质量门禁）

### P0-1 CI 全红已持续一个月，两个独立根因

**位置**: `.github/workflows/ci.yml:44-49`、`release.yml:46`、`package.json` / `package-lock.json`

**现象与根因**（已从 2026-08-19 失败日志确认）:
1. `npm ci` 失败：`package.json` 与 `package-lock.json` 不同步（如 `vitest ^4.1.8` 未反映到 lock），EUSAGE 直接退出。
2. `swatinem/rust-cache@v2` 失败：action 在仓库根目录执行 `cargo metadata`，而 `Cargo.toml` 在 `src-tauri/` 下，报 `could not find Cargo.toml`（exit 101）。

**建议**:
- 本地 `npm install` 更新 lockfile 并提交。
- 为 `rust-cache` 增加 `workspaces: src-tauri`（ci.yml 与 release.yml 两处）。
- 增加 workflow `concurrency` 组（ci: `cancel-in-progress: true`；release: `false`），避免连续 push 堆叠三平台矩阵。

**工作量**: ~0.5 天（含验证三平台绿）。

### P0-2 前端测试 4 个用例失败（文本契约测试随源码重构失效）

**位置**: `src/lib/settingsSaveFreshnessContract.test.ts`（3 例）、`src/lib/desktopWidgetContract.test.ts`（1 例）

**现象**: 已本地复现。`settingsSaveFreshnessContract` 用正则匹配各设置组件 `handleSave` 函数体中的 `const latestSettings = await getSettings();`，但 `RefreshSettings`/`AppearanceSettings` 已重写为 `queueSettingsUpdate` 模式，`SafetySettings` 甚至没有 `handleSave`，正则匹配到空串。`desktopWidgetContract` 断言 README 措辞，同样过期。

**建议**: 短期按当前实现修正断言；中期按 §P2-11 将这类"源码文本断言"迁移为行为测试（`renderSmoke.test.tsx` 已有可复用的模式）。文本契约目前占全部用例的约 47%（14/29 个文件、~55 例），已有一例实证失效，是本仓库测试体系最大的结构性风险。

**工作量**: 短期 ~0.5 天。

### P0-3 疑似泄漏凭据的文件已被提交进 git

**位置**: `docs/reviews/wahsingtonawad685@gmail.com----hkvpwnwkd`（138 字节，内容为邮箱 + 口令样式字符串 + 地区/年份）

**影响**: 无论凭据是否真实有效，这类文件会被 secret 扫描器标记，若仓库公开即视为泄漏。它显然是误粘贴的产物（旁边只有一份正经的评审文档）。

**建议**:
1. `git rm` 该文件并提交；
2. 因已进入历史，若仓库公开或将公开，用 `git filter-repo` / BFG 清除历史；
3. 在 `.gitignore` 中无需特殊处理，但建议给 `docs/reviews/` 定一个命名规范避免再犯。

**工作量**: ~0.5 小时（不含历史清除沟通成本）。

---

## 3. P1 高优先级（用户可见正确性与数据安全，1–2 周）

### P1-1 Fetch/Pull/Push 失败被渲染成绿色成功样式

**位置**: `src/components/RepoActions.tsx:20-22`（catch 写入与成功共用的 `result` state）、`RepoActions.tsx:99`、`src/styles/widget.css:503-507`（`.action-result` 固定 `--gv-green`）

**问题**: Pull/Push 是应用内唯一修改工作树/远端的破坏性操作，其失败提示与成功提示同色（绿）、无 `role="alert"`、行折叠即消失。快速扫一眼时**一次失败的 push 和一次成功的 push 无法区分**——这与产品"可信状态一目了然"的核心承诺直接冲突。

**建议**: 将 `result` 拆为 success/error 两态；错误用红色类 + `role="alert"`，成功用 `role="status"`；错误信息保留至该行下一次操作前不消失。后端已返回 `Result<String, String>`，消息文本是现成的，只差呈现层。

### P1-2 折叠态下后台刷新失败完全不可见

**位置**: `src/lib/useWidgetView.ts:199-207`（设置 `refreshError`）、`src/components/WidgetExpanded.tsx:109`（唯一渲染点）、`src/components/WidgetCollapsed.tsx:7-21`（未接收任何错误 prop）

**问题**: 折叠态下定时刷新照常运行；一旦仓库扫描开始失败（磁盘移动、git 缺失、权限变化），widget 会**持续展示停更的旧数据且没有任何提示**。初始加载的错误路径处理良好（`App.tsx:27-37`，有重试 + `role="alert"`），但周期性/后台路径是死胡同。

**建议**: 向 `WidgetCollapsed` 传递 `refreshError` 与 `lastRefreshAt`，在折叠面显示一行"数据停更于 HH:MM"或总数旁的小警示标记（文字而非仅颜色，符合 AGENTS.md 约束）。

### P1-3 macOS widget 数据只在托盘刷新路径写入，定时刷新不更新

**位置**: `src-tauri/src/tray_status.rs:166`（`write_widget_data` 全仓唯一调用点）；`src-tauri/src/app_commands.rs` 的 `list_repo_statuses` 不写 widget 数据

**问题**: 前端轻量自动刷新（默认 5 分钟）走 `list_repo_statuses`，只更新托盘与窗口，**从不更新 widget-data.json**。widget 只在应用启动和用户点托盘"刷新状态"时才有新数据——用户盯着桌面上几小时前的旧数据，而窗口里是新的。这是 macOS widget 这个核心新功能的链路断点。

**建议**: 将 widget 写入下沉为 `collect_repo_statuses` 成功路径后的共享步骤（现有 5 秒 debounce 已限制频率），托盘与 IPC 两条路径统一收敛到一处。更进一步见 §6 架构建议。

### P1-4 Windows 上每次托盘刷新都创建垃圾 `~/Library/` 目录

**位置**: `src-tauri/src/widget_data.rs:52-58`（`widget_data_path()` 无 `#[cfg(target_os = "macos")]` gate）、`tray_status.rs:166`（无条件调用）

**问题**: Windows 用户每次刷新都会在 `C:\Users\<user>\Library\Application Support\GitaView\` 下生成一份没人读取的 widget-data.json——既困惑用户，也干扰备份工具，还浪费 I/O。

**建议**: 将 widget 写入整体 gate 到 macOS（其他平台 no-op）；路径改用与 `app_settings.rs` 相同的 `app_data_dir()` 来源，避免两处口径分叉。

### P1-5 30 秒硬超时作用于 fetch/pull/push，且无按仓库操作锁

**位置**: `src-tauri/src/git/commands.rs:178`（`GIT_OPERATION_TIMEOUT` 供 `run_git` 全部调用方使用）、`app_commands.rs:200-247`

**问题**:
- 慢网络上的大仓库 fetch/pull 合理耗时超过 30s，子进程树被强杀：被杀的 `git pull` 可能留下 `MERGE_HEAD`（仓库卡在合并中态），fetch 可能留下 `refs` 锁文件使后续操作报 "cannot lock ref"。
- 双击/重复触发没有互斥，两次并发 `pull` 争抢 `index.lock` 直接报错。

**建议**: 状态读取保留 30s（甚至降到 10s），网络操作用独立的长超时或无超时；用 per-repo-path 的 in-flight map 做互斥，重复触发返回"操作进行中"；超时错误信息中检测遗留的 `*.lock` / `MERGE_HEAD` 并给出恢复指引。

### P1-6 settings.json 一旦损坏，应用永久不可用

**位置**: `src-tauri/src/storage/store.rs:6-14`（解析失败直接 `Err`）、`app_settings.rs:12-28`（向上传播）

**问题**: 一次崩溃中的半写入（当前写入流程无 fsync，见 P2-3）或一次手滑编辑，会让 `get_settings`、`list_repo_statuses`、所有仓库操作、托盘刷新**全部**永久失败，直到用户自己找到并删除数据文件。没有任何自愈路径。

**建议**: 解析失败时把损坏文件改名为 `settings.json.corrupt-<timestamp>` 留档，返回 `AppSettings::default()` 让应用自愈并记录日志。顺带为该路径补上单测（当前 store.rs 没有 corrupt-JSON 测试）。

### P1-7 macOS 图形界面启动找不到 Homebrew git

**位置**: `src-tauri/src/git/commands.rs:115`（`Command::new("git")` 裸 PATH 解析）

**问题**: 从 Finder/Dock 启动的 GUI 应用只继承最小 PATH（`/usr/bin:/bin:...`）。只装了 Homebrew git 的 macOS 用户会**所有仓库全部"读取失败"**，且诊断日志难以看出原因。Windows 因 Git for Windows 安装器写 PATH 基本不受影响。

**建议**: 应用启动时解析一次 git 绝对路径：先 `$SHELL -lc 'command -v git'`，再探测 `/opt/homebrew/bin`、`/usr/local/bin` 等常见位置，缓存进状态并在诊断日志输出（可脱敏）。

---

## 4. P2 中优先级（健壮性、无障碍、流程，2–4 周）

### P2-1 设置读-改-写竞态 + async 命令中的阻塞 I/O
`app_commands.rs:95-140`（add）、`:143-156`（remove）、`:44-60`（save）都是"load → 修改 → save"，两个并发写命令交错时后写覆盖先写（静默丢失仓库/分组变更）；且同步 `fs` I/O 直接跑在 tokio worker 上。建议：所有设置变更经 `tauri::State<Mutex<()>>` 或单一写任务串行化；I/O 包 `spawn_blocking`。

### P2-2 状态收集的伸缩性：批处理 join + 每仓库 4–5 次 git spawn
`repo_status.rs:7,45-96`（批大小 4，整批 join 后才开下一批）、`git/commands.rs:204-266`（`branch_state` 串行 4–5 个子进程）。30 个仓库 ≈ 120–150 次 spawn；一个挂死的网络驱动器仓库可拖满所在批次的 30s。建议：改为有界并发（信号量，N≈2×核数）替代批 join；`branch_state` 用 `git status --porcelain=v2 --branch` + 一次 `rev-list` 折叠为 1–2 次 spawn。

### P2-3 写入无 fsync + widget debounce 丢数据
`widget_data.rs:74-90`、`storage/store.rs:22-26`：临时文件写后未 `sync_all()` 即 rename，断电可能留下零字节文件（联动 P1-6）；`write_widget_data` 的 5s debounce 是 leading-edge，刷新完成早于窗口时**新数据被静默丢弃却返回 Ok**。建议：写-刷-改名的完整原子序列；debounce 改 trailing-edge 或直接移除（刷新节奏本就是分钟级）；失败时清理 `.tmp`。

### P2-4 托盘 generation guard 的 check-then-act 窗口
`tray_status.rs:75-85,148-155`：原子检查 generation 与 `set_menu` 之间，另一线程可插入更新导致过期菜单覆盖新菜单。窗口极小但 guard 的存在意义正是消除它。建议：用 `Mutex<u64>` 覆盖"检查+应用"全程。

### P2-5 目录扫描无时间/条目上限，路径 IPC 往返有损
`app_commands.rs:62-92`、`git/discovery.rs:28-60,88-91`：指向 `C:\` 或用户主目录时无预算限制（skip 列表不含 `Library`/`AppData`/`.cargo`/`.rustup`），UI 全程等待无法取消；路径经 `to_string_lossy` 字符串化再 `PathBuf::from` 还原，Windows 上含未配对代罪的路径会被 U+FFFD 损坏。建议：加 10s 死线与条目上限；扩充 skip 列表或跳过全部隐藏目录；IPC 边界避免 lossy 转换。

### P2-6 无障碍缺口（四项）
- 过滤器/设置导航激活态仅靠 CSS class：`GroupFilters.tsx:9-14`、`StatusFilters.tsx:27-46`、`SettingsShell.tsx:51-59` 缺 `aria-pressed` / `aria-current`。
- 表格列宽调整仅鼠标可操作：`RepoTable.tsx:85-115` 的 handle 无键盘路径（可加 `role="separator"` + 方向键，或放弃 resize）；`<th>` 缺 `scope="col"`；状态点（`:143`）对读屏是空单元格——`.sr-only` 工具类（`widget.css:110-120`）已定义却从未使用。
- 折叠态右键菜单无键盘触发：`WidgetCollapsed.tsx:59-67`，键盘用户可展开但无法从折叠态刷新/退出（补 `ContextMenu` 键 / Shift+F10）。
- 对比度：amber 文字按钮 `#b57412` ≈3.8:1（`widget.css:493-496`，11px 需 4.5:1），`--gv-muted` 在多处 10–11px 小字上处于 4.5:1 边缘（`tokens.css:4`）。建议为文字用途加深色阶（如 `#8a5a0a`、`#5a6a80`），色点保持亮色。

### P2-7 视图切换时窗口帧同步执行两次
`useWidgetView.ts:158-164`（`showView` 直调 `syncNativeWindowFrame`）+ `:241-248`（effect 在 state 变化后再调一次）。每次折叠/展开都做两轮背景色翻转 + IPC，加重 resize 保护窗（140ms）附近的闪烁。删直调即可（effect 已覆盖）。

### P2-8 文档与实现漂移
- `AGENTS.md:87` 仍写 "Planning complete, implementation pending"，但 macOS WidgetKit widget 已实现（`widget-extension/` 存在、`beforeBundleCommand` 已配置、CI 已在构建扩展）。README 状态分类表也已与 AGENTS.md 口径一致（`error` 说明已补），唯此处未跟上。
- `README.md:81-88` 叠了三个"最后更新"脚注，是追加式编辑的痕迹；建议删除日期脚注，让 git 历史承担版本信息。
- `NATIVE_WIDGET_IMPLEMENTATION_PLAN.md` 状态行同步更新为"已实现（需 Apple Developer 签名才能实际加载）"。

### P2-9 CI 缺类型检查；无依赖更新自动化
- CI 只跑 `npm test`（vitest 不做类型检查），`npm run build`（即 tsc，README 明言 "this IS the typecheck"）只在打 tag 后的 release 流程执行——TypeScript 错误会在发布日而非 PR 日暴露。ci.yml 增加 `npm run build` 即可。
- 无 dependabot/renovate：npm、Cargo、GitHub Actions 三个依赖面（含 sharp、vite、tauri、windows crate）都没有安全补丁自动化。加 `dependabot.yml`（三个 ecosystem，weekly）。
- 仓库无任何 ESLint/Prettier 配置；是否引入见 P3-8 一起决策。

### P2-10 发布通道缺口：无更新器、签名收尾未完成
- `tauri.conf.json` 无 `plugins.updater` / `createUpdaterArtifacts`：常驻型 widget 应用只能靠手动重装升级，摩擦显著。建议启用 tauri-plugin-updater + 签名密钥 + latest.json 发布（若有意推迟，在 RELEASE_SIGNING.md 写明）。
- `docs/RELEASE_SIGNING.md` 自述的 v0.2.2 草稿清理仍无执行记录；`docs/platform-acceptance-checklist.md` 对 v0.3.2-unsigned 三平台仍全部 `_TBD_`——这是发布前自身设定的门禁。
- `configure-windows-signing.ps1` 会在 runner 上原地改写 tauri.conf.json 的证书指纹，本地构建静默产出未签名包，文档应加一句说明。

### P2-11 测试体系转型：文本契约 → 行为契约
14/29 个测试文件把源码当文本做正则断言（含对一个 `.md` 和测试文件自身的断言），P0-2 是第一个实证牺牲品，且部分断言重复编译器保证。建议：UI 结构类断言迁移到渲染测试（扩展 `renderSmoke.test.tsx` 模式：Pull/Push 二次确认流、筛选联动、设置页导航三个优先场景）；仅保留无法用行为表达的跨语言边界检查。同时补 P2-2（并发收集）、store.rs 损坏路径、`repo_status` 整批 join 行为等 Rust 行为测试。这也是旧报告遗留项（Playwright/Testing Library 层交互测试）的落地方式。

### P2-12 错误展示一致性（设置页）
`RepositorySettings.tsx:222`、`GroupSettings.tsx:83`、`RefreshSettings.tsx:69`、`AppearanceSettings.tsx:56` 把成功与失败写进同一个灰色 `.settings-message`（`settings.css:376-381`）。失败看起来像确认。加 `isError` 标志 + 红色变体（与 P1-1 同一 CSS 基建）。

---

## 5. P3 低优先级（卫生与打磨，择机）

| # | 项目 | 位置 | 建议 |
|---|------|------|------|
| P3-1 | 死代码 `CollapsedBucket` | `domain/status.rs:26-43`（仅同文件测试引用，`lib.rs` 全 pub 导致死代码 lint 失效） | 删除或接线到 `tray_menu_rows` |
| P3-2 | 依赖卫生 | `Cargo.toml:11,14`（dialog 精确 pin 2.7.1 与整体 `^2` 风格不一；`dirs = "5"` 与 lock 中 dirs 6 并存）；`package.json:24`（`png-to-ico` 无引用）；根目录 `create-icon.cjs` 游离 | dirs 升 6；统一 pin 风格；删 png-to-ico；脚本移入 `scripts/` 并挂 `npm run icon` 或删除 |
| P3-3 | 无 `[profile.release]` | `Cargo.toml` | 常驻应用加 `lto="thin"`、`strip=true`、`codegen-units=1` 可显著缩包 |
| P3-4 | 权限最小化 | `capabilities/default.json` | `deep-link:allow-register`/`get-current` 前端从未调用（注册在 `lib.rs:76` 原生完成），移除 |
| P3-5 | 时间戳 URL 占位 | `tauri.conf.json:44`（`timestampUrl: ""`） | 启用签名时设默认 RFC 3161 地址，避免无时间戳签名 |
| P3-6 | release.yml 重复块 | `release.yml:105-155` | 签名/未签名两个 tauri-action 步骤仅差 env 与 prerelease，用条件合并 |
| P3-7 | i18n 基础 | 全部组件 + `"全部分组"` sentinel | 抽 `strings.ts`；用显式 `"all"` 过滤值或 `settings.defaultGroup` 替代中文哨兵串做逻辑比较（`statusModel.ts:37-38`、`commands.ts:37,43`、`domain/settings.rs:79,82`） |
| P3-8 | lint 工具链 | 仓库级 | 与 P2-9 合并决策：引入 ESLint + Prettier + `npm run lint` 进 CI |
| P3-9 | 类型契约死字段 | `types.ts:49-52` | `safety.confirmPull/confirmPush` UI 不可编辑且 Rust 端强制归 true，从 TS 契约移除或改为只读展示 |
| P3-10 | 渲染打磨 | `RepoTable.tsx:157`（Fragment 内冗余 key）、`RepoTable.tsx:121-167`（可 memo）、`RepositorySettings.tsx:226`（scanResults 以路径字符串为 key 可碰撞） | 低成本随手修 |

---

## 6. 架构演进建议（贯穿 P1/P2 的结构性主题）

**单一状态所有者**：当前"最新仓库状态"没有唯一 owner——托盘刷新（`tray_status.rs`）、IPC 刷新（`list_repo_statuses`）、widget 写入（`widget_data.rs`）是 `collect_repo_statuses` 的三个平行消费者，行为分叉（P1-3 即其症状），且互不共享缓存，同一时刻可能重复拉起几十个 git 进程。建议引入一个后台状态服务（`tauri::State` 持有 `Arc<Mutex<RepoSnapshot>>` + 版本号）：刷新入口统一、消费方（托盘/IPC/widget）从快照读取并订阅变更。这一步能同时化解 P1-3、P2-1（竞态）、P2-4（guard 竞态）的根因。

**错误模型**：后端全线 `Result<_, String>` 且中文文案在各层拼接，IPC 边界无法枚举、无法测试。中期可改为带错误码的枚举 serde DTO，前端据码决定呈现（info/warn/error + 是否可重试），P1-1/P1-2/P2-12 的呈现规则就有了统一挂靠点。

---

## 7. 实施路线图

| 阶段 | 内容 | 出口标准 |
|------|------|----------|
| **一（本周）** | P0-1、P0-2、P0-3 | CI 三平台全绿且含 `npm run build`；`npm test` 全绿；可疑文件出库 |
| **二（1–2 周）** | P1-1 ~ P1-7 | 失败可见（操作错误红显 + 折叠态停更提示）；widget 数据随任意刷新更新；Windows 无 `Library/` 目录；网络操作不受 30s 限制且有互斥；损坏 settings 自愈；macOS Homebrew git 可用 |
| **三（2–4 周）** | P2-1 ~ P2-12 | 设置写入串行化 + fsync；扫描有上限；无障碍四项关闭；文档口径一致；dependabot + concurrency 运转；文本契约迁移过半 |
| **四（择机）** | P3 全部 + 更新器 + 状态服务重构 | P3 清零；updater 通道上线（或文档写明推迟）；单一状态所有者落地 |

顺序依据：门禁先于功能（门禁不红才能保证后续每个 PR 的质量）；用户可见正确性先于内部健壮性；结构性重构放在行为修复稳定之后，避免在移动的地基上施工。

---

## 8. 验证与验收

每项完成后按 AGENTS.md 全量验证：

```bash
npm test
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
npm run build
```

另需专项验收：
- P1-3/P1-4：macOS 实机确认 widget 数据随前端刷新更新；Windows 实机确认 `~/Library` 不再出现。
- P1-5：用 `git config http.lowSpeedLimit` 或限速代理模拟慢网络，验证 pull 不被 30s 杀死、重复点击返回"操作进行中"。
- P1-6：手工写入坏 JSON 启动应用，确认自愈 + 留档。
- 行为修复一律先补回归测试（AGENTS.md 既有约定）。

---

## 附录：本次分析实际执行的验证

| 验证 | 结果 |
|------|------|
| `npm test`（本地） | **4 失败**（settingsSaveFreshnessContract ×3、desktopWidgetContract ×1），114 通过 |
| `gh run list` | CI 自 2026-07-28 连续 4 次失败，日志确认 npm ci EUSAGE + rust-cache "could not find Cargo.toml" |
| `grep write_widget_data` | 唯一生产调用点 `tray_status.rs:166` |
| `widget_data.rs` / `git/commands.rs` / `storage/store.rs` 源码抽查 | P1-4、P1-5、P1-6 所述代码逐行核实 |
| `docs/reviews/` 目录 | 存在疑似凭据文件（内容已脱敏查看） |
| `swatinem/rust-cache` | ci.yml:44 与 release.yml:46 均未配置 `workspaces` |
