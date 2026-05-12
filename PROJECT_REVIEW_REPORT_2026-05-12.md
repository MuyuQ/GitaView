# GitaView 项目技术审查报告

**审查日期**: 2026-05-12  
**项目版本**: 0.1.0  
**审查对象**: 当前工作区代码与项目约束  
**当前结论**: 原报告的架构评价大体可参考，但风险判断需要按最新产品决策和代码事实修正。Push 已由产品决策明确保留，不再作为范围违规项。

---

## 0. 总结

GitaView 当前架构清楚：Rust/Tauri 后端负责 Git 状态、设置持久化和桌面能力，React/TypeScript 前端负责 widget、设置页和交互。代码已经具备可运行基础，单元测试覆盖也比一般早期桌面项目更扎实。

本轮修正后，以下审查项已完成或收口：

- Push 从“v1 禁止项”调整为“显式保留功能”，并同步到 `AGENTS.md`。
- CSP 已从 `null` 改为显式策略。
- 本地目录打开和 HTTP/HTTPS 远端打开已拆分。
- `hasRemote` 与 `remoteUrl` 已拆分，前端 Fetch 可用性可以表达“有远端但不可打开 URL”的场景。
- 后端 Fetch 校验已同步 `hasRemote` 语义，允许“有 origin 但无可比较 upstream”的仓库执行 Fetch。
- 未使用的 `tauri-plugin-shell` 初始化、依赖和 `shell:allow-open` capability 已移除。

综合判断：**B+ / 可继续迭代；发布前主要补齐集成测试、文档和 Git 边界场景。**

---

## 1. 对原报告的认可与修正

### 1.1 认可的部分

- 架构分层基本合理：`domain`、`git`、`storage`、`app_commands` 边界清楚。
- 前端状态模型集中在 `src/lib/statusModel.ts`，排序、筛选、折叠摘要有测试。
- Git 操作放入 `spawn_blocking`，避免阻塞 Tauri 主线程。
- Git 子进程禁用交互式提示，并设置超时。
- 设置持久化有 normalized 修复逻辑，能修复默认分组、仓库分组和刷新间隔等数据。
- UI 基本遵守“无 emoji、状态不可仅靠颜色、无远端排最后”等约束。

### 1.2 已修正的原报告问题

- 原报告中“v1 不应实现 Push”的判断已按产品决策撤销。当前 Push 是保留功能，但必须保持确认流程和状态限制。
- 原报告中“CSP 当前关闭”的问题已处理。
- 原报告中“open_path 职责过宽”的问题已处理为 `open_directory` 与 `open_http_url`。
- 原报告中“hasRemote 与 remoteUrl 混用”的问题已处理。
- 原报告中“缺少 shell 插件依赖”的判断不准确；当前进一步移除了不再使用的 shell 插件和权限。
- 原报告中“调试日志未清理”“窗口圆角硬编码”等判断已过期。

---

## 2. 当前已验证结果

以下命令需要在最终交付前重新执行，本报告会以最新执行结果为准：

```text
cargo test --manifest-path src-tauri\Cargo.toml
npm test -- --run
npm run build
```

当前测试覆盖重点：

- Rust：Git 状态解析、远端 URL 规范化、仓库扫描、设置修复、Git 操作可用性校验。
- 前端：状态排序/筛选、分组计数、窗口拖拽、窗口尺寸插值、设置事件、运行时检测、圆角/阴影契约。

---

## 3. 关键发现与当前状态

### 已处理：Push 范围判断

**状态**: 产品决策保留。

Push 当前仍存在于：

- `src-tauri/src/app_commands.rs`
- `src-tauri/src/lib.rs`
- `src/lib/commands.ts`
- `src/components/RepoActions.tsx`
- `src/components/settings/SafetySettings.tsx`

保留条件：

- 默认需要确认。
- 只允许在 `local_ahead` 或 `diverged` 状态下触发。
- UI 必须明确显示确认文案。

### 已处理：Fetch 前后端语义不一致

**问题**:

前端用 `hasRemote` 启用 Fetch，但后端过去只看 `relation == NoRemote`，导致“有 origin 但没有可比较 upstream”的仓库按钮可点、后端拒绝。

**当前状态**:

后端 `validate_repo_git_operation` 已接收 `has_remote`，Fetch 在 `NoRemote + has_remote` 时允许执行，真无远端仍拒绝。

### 已处理：shell 插件权限收窄

**问题**:

当前打开目录/URL 使用 Rust `std::process::Command`，不再需要 Tauri shell plugin。

**当前状态**:

- 已移除 `tauri_plugin_shell::init()`。
- 已移除 `tauri-plugin-shell` 依赖。
- 已移除 `shell:allow-open` capability。

### 已处理：CSP

**当前状态**:

`src-tauri/tauri.conf.json` 已设置 CSP：

```json
"csp": "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' asset: https://asset.localhost"
```

后续如果引入外部图片、字体或 Tauri asset 加载路径变化，应重新验证 CSP。

### 已处理：打开目录和远端 URL 的职责拆分

**当前状态**:

- `open_directory(path)` 仅用于打开已管理仓库目录。
- `open_http_url(url)` 仅允许 HTTP/HTTPS URL。
- `normalize_remote_url` 拒绝 `file://` 和裸 SSH URL，GitHub SSH URL 会转 HTTPS。

---

## 4. 剩余风险

### P1. Git 状态语义还可以继续细化

当前 `NoRemote` 仍覆盖多个情况：

- 真无 `origin`
- 有 `origin` 但没有 upstream
- upstream 失效且没有同名 `origin/<branch>` fallback

现在 hint 已能区分“未配置远端”和“未设置可比较的 upstream”，但筛选标签仍统一显示“无远端”。如果用户需要精确筛选，建议后续新增 `no_upstream` 状态。

### P1. 缺少 Tauri command 级集成测试

当前 Rust 测试主要覆盖纯函数和局部逻辑。真实 command 仍缺少集成测试，例如：

- `fetch_repo` 对 `NoRemote + hasRemote` 的行为。
- `open_repo_remote` 对 HTTP、GitHub SSH、裸 SSH、本地 file remote 的行为。
- settings 保存后窗口置顶状态是否更新。

### P2. 前端组件测试不足

当前前端测试主要覆盖 `src/lib`。建议增加组件测试或 Playwright 级验证：

- `RepoActions` 在不同 relation/hasRemote/remoteUrl 组合下的按钮状态。
- 设置页保存后跨组件同步。
- 折叠态和展开态尺寸、透明背景、表格溢出。

### P2. 文档仍需按真实功能补齐

已新增 README 和 LICENSE，但 README 还可以继续补充：

- 每种状态的用户解释。
- Push 的风险说明。
- upstream 失效时为什么会显示“未设置可比较的 upstream”。
- Windows/macOS 发布和安装方式。

---

## 5. 更新后的评分

| 维度 | 评级 | 说明 |
|------|------|------|
| 架构设计 | A- | 分层清楚，command 层略重 |
| 代码质量 | B+ | 主流程清晰，Git 边界语义仍需继续加测试 |
| 安全性 | B+ | CSP、权限收窄、确认流程已改善 |
| 测试覆盖 | B+ | 纯函数和 Rust 单测较好，集成/UI 测试不足 |
| UX 一致性 | B+ | 主流程接近规格，Push 已按产品决策保留 |
| 文档完整性 | B | README/LICENSE 已补，发布文档和 FAQ 仍缺 |
| 综合 | B+ | 可继续迭代，发布前补集成验证 |

---

## 6. 建议执行顺序

### P1

1. 增加 Tauri command 集成测试。
2. 增加 `RepoActions` 组件测试。
3. 评估是否新增 `no_upstream` 状态。

### P2

1. 完善 README 的状态解释、Push 风险说明和发布流程。
2. 抽出 `App.tsx` 中逐渐膨胀的 settings/repo/window hooks。
3. 增加视觉回归或 Playwright 截图验证。

---

## 7. 最终结论

原报告可以作为早期参考，但不适合作为当前验收结论。当前代码已处理多项审查反馈，Push 也已按产品决策明确保留。下一步重点不再是“是否能跑”，而是继续补齐真实 Git 拓扑、Tauri command 和 UI 行为的验证。
