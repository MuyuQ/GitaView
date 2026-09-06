# GitaView 改进计划书

**基线**: `main` @ `5ef22f7` / v0.3.2
**日期**: 2026-09-06（第二轮）
**方法**: 在第一轮全部执行完毕、复审修复合入之后，对项目做的新一轮全量分析。重点覆盖此前未深挖的区域（Windows/macOS 桌面层实现、Swift widget 扩展、system_open、diagnostics、发布脚本），并对照 `DESIGN_AND_BUILD_SPEC.md` / `PRODUCT.md` 逐条核对。关键结论均经人工复核源码证实。

---

## 0. 第一轮执行情况（2026-08-30 计划，已全部落地）

- **P0 全部**：CI 门禁恢复（lockfile、rust-cache workspaces、CI 补类型检查、concurrency）、失效契约测试修正、疑似凭据文件出库。
- **P1 全部 7 项**：操作失败红色告警、折叠态停更提示、widget 数据全路径更新、macOS 门控、网络超时 + per-repo 操作锁、settings 损坏自愈、macOS git 路径解析。
- **P2 六项**：设置读写串行化、状态收集工作池、托盘菜单锁（复审后重构为 apply 锁 + generation 锁双锁模型）、扫描预算、无障碍补齐、dependabot。
- **P3 七项**：死代码清理、依赖卫生、`[profile.release]`、capabilities 最小化、timestampUrl、类型契约清理、渲染打磨。
- **复审修复**（PR #22）：托盘锁序倒置、widget 写入乱序、扫描文件级预算。
- 当前验证基线：Rust 77 测试 / 前端 129 测试 / fmt / clippy(-D warnings) / build 全绿；CI 三平台稳定通过。

---

## 1. 第二轮执行摘要

第一轮的快赢完成后，剩余问题不再分散在各模块，而是集中在三个**边界**上：

1. **语言边界（Rust ↔ Swift）**：macOS widget 的 JSON 契约从未被两端同时执行过，旗舰功能**从未真正渲染过一条真实数据**。
2. **进程边界（单实例）**：应用可以双开，设置读写锁是进程内的，跨进程丢更新无防护。
3. **发布边界（签名/公证/验收）**：widget 扩展 Release 仍是 ad-hoc 签名，公证分发必然失败；发布脚本校验了前置条件却不真正使用。

| 级别 | 数量 | 主题 |
|------|------|------|
| P0 紧急 | 1 | macOS widget 数据契约断裂 |
| P1 高 | 3 | 单实例守卫、扩展签名、窗口位置持久化 |
| P2 中 | 7 | 兜底刷新、构建脚本断链、watchdog 线程纪律、日志脱敏、git 探测性能、刷新排队、deep link |
| P3 低 | 9 | 校验/卫生/脚本/文档漂移 |
| 体系 | 5 | 跨语言契约测试、文本契约迁移、交互测试、依赖消化、lint 工具链 |
| 演进 | 2 | 单一状态所有者、类型化错误模型 |

---

## 2. P0 紧急项

### P0-1 macOS widget 数据契约断裂——旗舰功能端到端失效

**位置**: `src-tauri/src/widget_data.rs:22-49`（Rust 序列化）、`src-tauri/widget-extension/GitaViewWidget/Models/WidgetData.swift`、`GitaViewWidget/Provider.swift:32`

**问题**（已逐行核实，双重必败）：
1. **键名不匹配**：Rust `WidgetPayload`/`WidgetRepo`/`WidgetSummary` 无 `#[serde(rename_all = "camelCase")]`，序列化为 `last_updated`、`change_label`、`local_ahead`、`remote_ahead`、`no_remote`；Swift `WidgetData` 声明的是 `lastUpdated`、`changeLabel`、`localAhead`……且无 `CodingKeys`，`JSONDecoder()` 使用默认 `.useDefaultKeys` 策略——**每个 payload 的解码都以 `keyNotFound` 抛错**。
2. **Date 解码不匹配**：Swift `lastUpdated: Date` 默认策略期望 Double（2001-01-01 起的秒数），Rust 写入的是 ISO-8601 字符串（`widget_data.rs` 的 `format_description!("[year]-[month]-[day]T…")`）——即使修好键名，第二次解码仍会失败。

**后果**: `Provider.loadEntry()` 永远返回 nil → widget 永久显示空态（"打开 GitaView"），且空态兜底把这个失败**静默掩盖**了。PR #12 修好的数据写入链路（写入时机、防抖、原子性）写出的数据，Swift 端一个字节都没读到过。

**根因**: 两端各自独立定义数据模型，没有任何跨语言契约测试。这正是本轮四个最高优先级发现的共同根因（见 §7-1）。

**建议**:
1. Swift 侧 `Provider.swift`：`decoder.keyDecodingStrategy = .convertFromSnakeCase`、`decoder.dateDecodingStrategy = .iso8601`（或 Rust 侧统一 `rename_all = "camelCase"`——二选一，**以共享 fixture 为准**）。
2. 建立跨语言契约测试：CI 中用 Rust `build_payload` 生成 fixture JSON 提交为测试资产，Swift 测试 target 解码断言（project.yml 目前没有 test target，需补）；同时在 Rust 侧锁定 fixture 的序列化形状（键名 + Date 格式），任何一端漂移即红。
3. 顺带统一数据文件路径常量（Swift `Provider.swift:23` 与 Rust `widget_data_path()` 各自硬编码，见 P3-9）。

**工作量**: 代码 ~0.5 天，契约测试基建 ~1 天。

---

## 3. P1 高优先级

### P1-1 无单实例守卫，双实例丢更新

**位置**: `src-tauri/Cargo.toml`（无 `tauri-plugin-single-instance`）、`src-tauri/src/app_settings.rs:8`

**问题**: 应用可以同时运行两个实例（双击双开；或应用运行中点击 `gitaview://` 链接再启一个——deep-link 插件官方就要求与 single-instance 插件配对）。`SETTINGS_MUTATION_LOCK` 是进程内的：实例 A 的 `add_repository` 会被实例 B 随后的 `save_settings` 静默覆盖（文件原子性防撕裂，不防读-改-写交错）。两个实例还会竞争写 `widget-data.json`、日志文件、托盘菜单，Windows 上更会争抢桌面层级的窗口 reparent。

**建议**: 引入 `tauri-plugin-single-instance`，二次启动时转发 argv/deep-link 到主实例并聚焦主窗口。~0.5 天。

### P1-2 widget 扩展 Release 是 ad-hoc 签名，公证分发必败

**位置**: `src-tauri/widget-extension/project.yml:22-27`（Release `CODE_SIGN_IDENTITY: "-"`、`ENABLE_HARDENED_RUNTIME: NO`）、`scripts/build-widget-extension.cjs:29-32`（xcodebuild 不传任何签名参数）

**问题**: CI 已校验 `APPLE_SIGNING_IDENTITY`（`validate-release-signing.cjs`）但从不使用——`.appex` 以 ad-hoc 签名嵌入 Developer-ID 签名的应用，公证失败或 Gatekeeper/`pluginkit` 在运行时拒绝加载扩展。即便 P0-1 修好数据链路，正式发布的 widget 依然装不上。

**建议**: `build-widget-extension.cjs` 把 `APPLE_SIGNING_IDENTITY`/`DEVELOPMENT_TEAM` 透传给 xcodebuild（本地无签名环境回退 `"-"`），project.yml Release 开 `ENABLE_HARDENED_RUNTIME: YES`。~0.5 天，需真机验证。

### P1-3 窗口位置不持久化（规格明确要求的能力缺失）

**位置**: 全仓无任何窗口位置持久化代码（`grep` 证实）；`DESIGN_AND_BUILD_SPEC.md` §8 将 "Window persistence" 列为后端职责。

**问题**: 用户把桌面 widget 拖到顺手的位置，**每次重启应用都回到默认位置**。对一个常驻桌面、位置讲究的 widget 来说是显著体验缺陷，且属于规格承诺未实现。

**建议**: 关闭/移动时把窗口位置存入 settings（或独立轻量文件），启动时恢复；需处理显示器拔插/分辨率变化的越界钳制（`windowMotion.ts` 的 workArea 钳制逻辑可复用到恢复路径）。~1 天。

---

## 4. P2 中优先级

| # | 项目 | 位置 | 问题与建议 |
|---|------|------|-----------|
| P2-1 | timeline `.never` 无兜底刷新 | `Provider.swift:16-20` | 应用崩溃/退出前未写、或 `reloadAllTimelines` 在扩展注册前调用丢失时，widget 永久停留旧数据/空态。改为 `.after(now + 15min)` 兜底，与 app 驱动刷新叠加。 |
| P2-2 | 构建脚本 skip 时 bundle 断链 | `scripts/build-widget-extension.cjs:11-20`、`tauri.conf.json:48-50` | 无 Xcode 环境时脚本 exit 0 跳过，但 bundle 配置无条件映射 `.appex` → 打包报"文件不存在"或静默缺失扩展。skip 时应输出明确错误（release 构建 fail hard）或条件剥离 files 映射。 |
| P2-3 | Windows watchdog 跨线程 HWND 操作、无退避 | `desktop_widget/windows.rs:179-207` | 后台线程每 5s 直接 `SetParent`/`SetWindowLongPtrW`（同步消息发往 UI 线程），UI 忙时 watchdog 卡顿，宿主探测瞬时失败会引发重附风暴。改为 `app.run_on_main_thread` 派发 + 连续失败指数退避 + main 窗口不存在时停止重试。 |
| P2-4 | 日志脱敏靠调用点自觉，已有 3 处缺口 | `git/commands.rs:39`（git 全路径含用户名）、`lib.rs:79`（deep-link 全 URL 含 query）、`Provider.swift:27`（全路径） | `redact_path` 本身可靠但无强制。加一个中心化 log 包装：对 `X:\Users\...`、`/Users/...` 模式统一打码；deep link 只记 scheme+host。 |
| P2-5 | `branch_state` 每仓库 4-5 次 git spawn | `git/commands.rs:204-266` | 遗留性能项：用 `git status --porcelain=v2 --branch` + 一次 `rev-list` 折叠为 1-2 次 spawn，30 仓库的刷新 spawn 数从 ~150 降到 ~60。与工作池（已做）叠加后刷新延迟显著下降。需完整回归状态分类测试。 |
| P2-6 | 手动刷新在自动刷新 in-flight 时被静默丢弃 | `useWidgetView.ts:181` | `refreshInFlightRef` 对手动/自动一视同仁直接 return。手动点击应有反馈地"排队"（记 pending 标记，in-flight 结束后补一次），而不是让用户以为点了没用。 |
| P2-7 | deep link 无路由、widget 无点击目标 | `lib.rs:77-93`、Swift Views | `on_open_url` 只判断 scheme 就聚焦窗口，host/path 全丢弃；Swift 视图没有 `.widgetURL`，点 widget 没有任何反应（WidgetKit 必须显式声明）。加 `.widgetURL(URL(string:"gitaview://open"))` + 路由 `gitaview://open/repo/<id>`。 |

---

## 5. P3 低优先级

| # | 项目 | 位置 | 建议 |
|---|------|------|------|
| P3-1 | settings 归一化不校验 version、不去重 | `domain/settings.rs:99-135` | `version: 99` 被当 v1 静默改写；重复 repo `id` / 重复组名导致 `find_repo` 遮蔽、`remove_repository` 双删。`normalized()` 补版本上限检查与去重。 |
| P3-2 | `system_open` URL 校验是前缀式的 | `system_open.rs:12-18,42-45` | 放行控制字符/空白（`open`/explorer 可能吞掉）；`spawn()` 结果丢弃，Linux `xdg-open` 失败用户无感知。拒绝 <0x20 字符；映射退出码为错误消息。 |
| P3-3 | Windows 签名验证不绑定证书 | `scripts/verify-windows-signatures.ps1:11-17` | 只查 `Status -eq Valid`，任何受信证书都过——补 `Thumbprint -eq $env:WINDOWS_CERTIFICATE_THUMBPRINT` 断言。 |
| P3-4 | 版本校验脚本子串匹配 | `scripts/validate-release-version.cjs:18-20` | `includes('version = "0.3.2"')` 会命中依赖行；改为解析 `[package]` 段。 |
| P3-5 | `.corrupt` 留档无限累积 | `storage/store.rs` | 留最近 N 份（如 3），超出删除最旧。 |
| P3-6 | `add_repository` 已存在时仍重写设置文件 | `app_commands.rs` | no-op 路径提前返回，避免无谓 I/O 与 mtime 抖动。 |
| P3-7 | 登录 shell 探测取整段 stdout | `git/commands.rs:88` | profile echo 会污染解析；取最后一个非空行。 |
| P3-8 | `formatActionResult` 空串语义 | `src/lib/actionResults.ts` | `message ?? fallback` 把 `""` 当有效消息渲染空 span；改 `||` 或入口归一。 |
| P3-9 | widget 数据路径双语言各自硬编码 | `Provider.swift:23`、`widget_data.rs:75-81` | 从 bundle identifier 派生或以 fixture 常量共享，防漂移（P0-1 就是这种漂移的实证）。 |

另有一项**实机确认**：折叠态加"数据未更新"提示后与多状态桶同屏的溢出表现（第一轮改动引入，CSS 无 overflow 处理），需在真机看一眼。

---

## 6. 架构演进（延续第一轮，本轮仍开放）

1. **单一状态所有者**：托盘刷新、IPC 刷新、widget 写入仍是 `collect_repo_statuses` 的三个平行消费者（虽有 debounce/锁保护），无共享缓存。引入 `tauri::State` 持有的状态服务（快照 + 版本号 + 订阅），三个消费者收敛后可同时消除重复 git spawn 和多处竞态防御代码。
2. **类型化错误模型**：后端全线 `Result<_, String>` + 各层拼中文文案，IPC 边界无法枚举/测试。改为带错误码的 serde 枚举 DTO，前端按码决定呈现等级与重试策略。这是 P2-6（刷新排队）、P2-7（deep link）等功能的自然地基。

建议在 P0/P1 稳定后、且 dependabot 大版本升级（vite 8 等）消化完之后，作为独立 PR 系列推进，避免与行为修复混在同一批。

---

## 7. 工程体系与流程

### 7-1 跨语言/跨进程契约测试（本轮最大结构性投资）

本轮 P0 + P1-2 + P3-9 三个发现全部位于**没有测试覆盖的边界**上。具体缺口：Swift 端零测试（project.yml 无 test target）、`widget-bridge/WidgetReloader.m`、全部 `scripts/*`、`desktop_widget/macos.rs`、`lib.rs` 装配层均无测试。优先级排序：

1. **Rust ↔ Swift JSON 契约**（随 P0-1 一起做）：共享 fixture + 双端断言。
2. **发布脚本冒烟**：validate-*.cjs 至少用临时 fixture 跑通正反两例。
3. **Swift 单测 target**：Provider.loadEntry 对坏 JSON/缺文件/空文件的容错。

### 7-2 文本契约测试迁移（第一轮 P2-11 延续）

29 个测试文件中 14 个仍以正则/`toContain` 断言源码文本（含对 README 和其他测试文件的断言）。已实证脆弱（第一轮 4 个失效）。迁移策略：UI 结构断言 → `renderSmoke` 渲染模式；跨语言边界 → §7-1 的 fixture 契约；纯编译期保证（import 存在等）→ 删除。

### 7-3 交互层测试

补三个核心用户流程的组件交互测试（Testing Library，不必上 Playwright）：Pull/Push 二次确认全流程（确认态 → invoke → 成功/失败双分支呈现）、筛选联动（组 → 状态计数）、设置页导航与保存。`renderToStaticMarkup` 覆盖不了事件路径，这是当前的空白。

### 7-4 dependabot 消化（9 个 PR 开放中）

- **可安全合并**：#18（npm/cargo minor+patch 组，9 项）已三平台 CI 全绿。
- **需单独评估的大版本**：#20 vite 7→8、#21 plugin-react 5→6、#19 @types/node 24→26、#13 tauri-action 0→1。各建独立分支验证（`npm run build` + `npm test` + 一次 `tauri build`），特别是 vite 8 的 plugin 兼容矩阵。
- cargo/windows crate 升级（#17）在 Windows runner 上验证即可。

### 7-5 其他流程项（延续开放）

- ESLint/Prettier 决策（建议引入 eslint + `@typescript-eslint` 最小规则集，进 CI 的 `npm run lint`）。
- `docs/reviews/` 凭据文件的 git 历史清除（需 force-push 决策）。
- `docs/platform-acceptance-checklist.md` 实机验收（对 v0.3.2 仍全 `_TBD_`；P0-1 修复后 mac 清单必须重跑）。
- release.yml 签名/未签名双 tauri-action 块条件合并（~30 行重复）。

---

## 8. 产品规格漂移治理

`DESIGN_AND_BUILD_SPEC.md`（v1 契约）与实现已有多处口径分叉，建议一次性对齐（以实现为准，除非有意改实现）：

| 章节 | 规格说 | 实现是 |
|------|--------|--------|
| §4 设置导航 | 5 项：`仓库/分组/刷新/安全操作/外观` | 2 组：`仓库设置`（含分组卡片）/`常规设置`（刷新+安全+外观） |
| §6 状态模型 | 5 种 relation，无 `error` | 5 + 应用层 `error`（README 已说明，spec 未跟上），排序首位 |
| §7 AppSettings | `safety.confirmPull` 单项、`appearance.compactMode` | 双确认强制为 true（不可配置）、`compactMode` 已删、新增 `allowWidgetDrag` |
| §5 色板 | `--gv-muted: #667488`、无 amber-text | 无障碍修复后 `#5a6a80` + 新增 `--gv-amber-text` |
| §3.3 动作反馈 | "超过 300ms 才显示 loading" | loading 立即显示（无延迟门槛） |
| §7 数据模型 | `RepoStatus` 无 `hasRemote` | 实现有 `hasRemote`（操作可用性判定依赖） |

`PRODUCT.md` 的 a11y 要求（reduced-motion、非纯色传达、键盘可达）经核查**已达成**，无漂移。

`NATIVE_WIDGET_IMPLEMENTATION_PLAN.md` 状态行已标"已实现"，但 P0-1 修复前应追加说明"数据链路修复中"。

---

## 9. 实施路线图

| 阶段 | 内容 | 出口标准 |
|------|------|----------|
| **一（1 周）** | P0-1（含跨语言契约测试基建）、P1-1、P1-2、P1-3 | 真 macOS 实机上 widget 显示真实仓库数据并随刷新更新；双开启动只激活单实例；带签名身份的 `.appex` 构建；重启后窗口位置恢复 |
| **二（1–2 周）** | P2-1 ~ P2-7 | 兜底刷新生效；无 Xcode 环境的构建行为明确；watchdog 主线程化 + 退避；日志无用户名/URL 泄漏；`branch_state` 1-2 次 spawn；手动刷新排队；widget 点击打开应用 |
| **三（并行/持续）** | §7 体系项 + §8 规格对齐 | 契约 fixture 入 CI；文本契约测试 <50%；三个交互测试场景落地；9 个 dependabot PR 清零；spec 与实现口径一致 |
| **四（择机）** | §6 架构演进、updater 通道（tauri-plugin-updater + latest.json）、i18n 字符串模块 | 状态服务落地、错误码枚举、更新通道上线（或文档明确推迟）、`全部分组` sentinel 移除 |

依赖关系：P0-1 必须最先（它是"旗舰功能从未工作"级别的问题）；P1-2 的签名验证依赖 P0-1 之后才有意义（数据对了才有东西可显示）；§6 架构演进放在 dependabot 大版本消化之后，避免变更叠加。

---

## 10. 验证与验收

```bash
npm test
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
npm run build
```

专项验收：
- P0-1：**macOS 真机**（Apple Developer 签名环境下）widget 显示真实数据、刷新后 15s 内更新、`console`/统一日志无 decode 错误。
- P1-1：应用运行中再次启动（含 `gitaview://` 链接），只激活已有实例。
- P1-3：拖动 widget → 退出 → 重启，位置还原；外接显示器拔除后启动不越界。
- P2-5：状态分类回归全绿（synced/local_ahead/remote_ahead/diverged/no_remote/detached/error 全分支）。
- 行为修复一律先补回归测试（AGENTS.md 既有约定）。

---

## 附录：本轮分析实际执行的验证

| 验证 | 结果 |
|------|------|
| Swift `WidgetData.swift` / `Provider.swift` 与 Rust `WidgetPayload` 逐行对照 | P0-1 双重必败证实（键名 + Date 策略） |
| `grep single.instance` 全仓 | 无单实例插件，证实 P1-1 |
| `project.yml` Release 段 | `CODE_SIGN_IDENTITY: "-"` + `ENABLE_HARDENED_RUNTIME: NO`，证实 P1-2 |
| 全仓 grep 窗口位置持久化 | 无任何实现，证实 P1-3（spec §8 要求） |
| `gh pr list` | 9 个 dependabot PR 开放（#13–#21） |
| `gh pr checks 18` | minor+patch 组三平台绿 |
| `grep prefers-reduced-motion` | widget.css:568 已支持（PRODUCT.md a11y 达标） |
| `DESIGN_AND_BUILD_SPEC.md` 逐节对照 | §4/§5/§6/§7/§3.3 五处漂移（§8 详表） |
