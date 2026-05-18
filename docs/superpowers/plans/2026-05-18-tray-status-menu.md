# 托盘菜单仓库状态展示增强计划

## Summary
将当前原生托盘菜单从“两项操作”升级为跨 macOS/Windows 可用的“状态摘要 + 未同步仓库列表 + 操作区”。使用 Tauri 原生菜单，不做自绘弹窗；接受系统菜单样式限制，换取稳定的跨平台行为。

计划保存路径：`docs/superpowers/plans/2026-05-18-tray-status-menu.md`。

## Key Behavior
- 菜单顶部显示：
  - `GitaView · 共 N 个仓库`
  - `已完全同步 X`
- `synced` 仓库只计入数量，不逐条展示。
- 非 `synced` 仓库逐条展示：
  - `分叉 · repo · branch · changeLabel`
  - `远程领先 · repo · branch · changeLabel`
  - `本地领先 · repo · branch · changeLabel`
  - `读取失败 · repo · hint`
  - `无远端 · repo · branch`
- 非 `synced` 包含：`error`、`diverged`、`remote_ahead`、`local_ahead`、`no_remote`。
- 菜单底部操作项：
  - `刷新状态`
  - `显示 GitaView`
  - `退出`
- 不改变托盘触发方式，继续使用当前 `.show_menu_on_left_click(false)` 行为。

## Implementation Changes
- 新增/拆分 Rust 侧托盘状态模块，集中负责：
  - 从 app settings 读取仓库列表。
  - 复用现有 Git 状态收集和排序规则。
  - 构建 tray menu。
  - 后台刷新并替换 tray menu。
- 将 `collect_repo_statuses` 相关逻辑从 `app_commands.rs` 私有范围中抽到共享 helper，供 `list_repo_statuses` 和 tray 刷新共同使用。
- 使用稳定托盘 id：`TrayIconBuilder::with_id("main-tray")`，后续通过 `app.tray_by_id("main-tray")` 更新菜单。
- 启动时先创建 loading 菜单：`正在读取仓库状态...`，setup 结束后后台刷新一次。
- 展示型菜单项必须使用 disabled `MenuItem::with_id(..., enabled=false, None::<&str>)`，不能用 `.text(...)`，避免状态行变成可点击操作。
- `刷新状态` 的 menu event 不直接运行 Git；它只启动后台任务：
  - 先把菜单替换为 loading 状态。
  - 在 `spawn_blocking` 中读取 Git 状态。
  - 完成后回到菜单更新逻辑，调用 `tray.set_menu(Some(menu))`。
- `list_repo_statuses` 成功后用同一份 statuses 更新托盘菜单，避免重复跑 Git。

## Test Plan
- Rust/contract tests:
  - 托盘菜单包含总仓库数和已完全同步数量。
  - `synced` 仓库不出现在未同步列表。
  - `error`、`diverged`、`remote_ahead`、`local_ahead`、`no_remote` 均会出现在未同步列表。
  - `no_remote` 保持排在最后。
  - 展示型状态项使用 disabled menu item，不使用 enabled `.text(...)`。
  - setup 使用 `TrayIconBuilder::with_id("main-tray")`。
  - setup 包含 loading 初始菜单和后台刷新入口。
- Full verification:
  - `npm test`
  - `cargo test --manifest-path src-tauri/Cargo.toml`
  - `npm run build`
  - `npm run tauri dev`
- Manual acceptance:
  - macOS 菜单栏打开托盘菜单，确认摘要、未同步列表、刷新/显示/退出可用。
  - Windows 系统托盘打开菜单，确认同样内容和操作可用。

## Assumptions
- 采用原生托盘菜单方案，不做 HTML/CSS 自绘状态弹窗。
- 菜单文案保持中文，不使用 emoji。
- 未同步列表暂时不做数量上限；若仓库过多，后续再加折叠策略。
- 当前计划只增强托盘菜单，不改变主 widget、折叠态右键菜单或 Git 操作权限。
