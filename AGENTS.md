# GitaView PROJECT KNOWLEDGE BASE

**Generated:** 2026-05-10
**Commit:** initial (no commits yet)
**Branch:** main

## OVERVIEW

GitaView — 跨平台桌面 Git 仓库状态总览 widget。Tauri 2 + Rust 后端 + React/TypeScript/Vite 前端。

灵感来自 `gita`，但 v1 不依赖 gita CLI 配置，自管仓库列表、分组、设置和 UI。

## STRUCTURE

```
GitaView/
├── DESIGN_AND_BUILD_SPEC.md    # 完整设计规格（产品需求 + 技术约束）
├── docs/superpowers/plans/     # 实现计划文档
├── .superpowers/brainstorm/    # UI 原型 HTML（折叠态、展开态、设置页）
├── src/                        # [未创建] 前端 React + TypeScript
│   ├── components/             # [未创建] UI 组件
│   ├── styles/                 # [未创建] CSS
│   ├── main.tsx                # [未创建] 入口
│   └── App.tsx                 # [未创建] 根组件
├── src-tauri/                  # [未创建] Tauri + Rust 后端
│   ├── Cargo.toml              # [未创建] Rust 依赖
│   ├── tauri.conf.json         # [未创建] Tauri 配置
│   ├── capabilities/           # [未创建] 权限声明
│   └── src/                    # [未创建] Rust 源码
│       ├── domain/             # [未创建] 领域模型 (status, repo, settings)
│       ├── commands/           # [未创建] Tauri 命令
│       └── persistence/        # [未创建] 设置持久化
└── AGENTS.md                   # 本文件
```

## WHERE TO LOOK

| Task | Location | Notes |
|------|----------|-------|
| 产品需求 | DESIGN_AND_BUILD_SPEC.md | 自包含手递文档 |
| 实现计划 | docs/superpowers/plans/ | 任务清单 + 文件结构 |
| UI 原型 | .superpowers/brainstorm/ | HTML 原型文件 |
| 状态分类逻辑 | src-tauri/src/domain/status.rs | [未创建] 5 种状态枚举 |
| Git 操作 | src-tauri/src/ | [未创建] Rust 后端 |
| 前端组件 | src/components/ | [未创建] React 组件 |

## CONVENTIONS

### 测试约定
- 前端: Vitest，`src/**/*.test.{ts,tsx}` 同目录放置
- 后端: Rust `#[cfg(test)]` 内联模块
- 运行: `npm test` / `cargo test --manifest-path src-tauri/Cargo.toml`

### 样式
- Plain CSS 或 CSS Modules
- 禁止 emoji 图标，使用一致 SVG 图标
- 状态不能仅靠颜色传达，必须含文字标签 + 数字

## ANTI-PATTERNS (THIS PROJECT)

### 技术栈禁令
- **禁止 Electron**
- **禁止 Python 运行时嵌入**
- **v1 禁止任意 shell 命令执行**
- **禁止依赖 gita CLI 配置** — 仓库来源仅限 app 自有管理

### 功能约束 (v1)
- **Push 作为显式保留功能** — 必须有确认流程，且只允许在本地领先或分叉状态下触发
- **不实现任意命令面板 (arbitrary command panels)**

### UI/UX 约束
- **折叠态不能有前导图标**
- **悬停/聚焦状态不能导致布局偏移**
- **无远端仓库必须始终排在最后**（折叠摘要、过滤器、列表排序）

### 安全约束
- **Pull 必须确认**（修改工作树）
- **远端按钮在无远程 URL 时必须禁用**

## UNIQUE STYLES

### 状态分类 (5 种)
| 状态 | 颜色 | 含义 |
|------|------|------|
| synced | green | 本地与远端一致 |
| local_ahead | yellow | 本地领先 |
| remote_ahead | yellow | 远程领先 |
| diverged | red | 分叉 |
| no_remote | gray | 无远端 |

### 排序规则
1. Green synced → 2. Yellow syncable → 3. Red diverged → 4. Gray no remote

### 筛选模型
双维度联动：分组筛选 → 状态计数动态更新

## COMMANDS

```bash
# 开发模式
cargo tauri dev

# 前端构建
npm run build

# 前端测试
npm test                    # vitest run

# Rust 测试
cargo test --manifest-path src-tauri/Cargo.toml

# 最终验收流程
cargo test && npm test && npm run build && cargo tauri dev
```

## NOTES

- 项目处于**设计阶段**，尚未开始编码实现
- 所有 `src/` 和 `src-tauri/` 目录尚未创建
- 实施起点: 按 DESIGN_AND_BUILD_SPEC.md 初始化 Tauri 2 项目骨架
- 目标平台: macOS (菜单栏 + 桌面 widget) / Windows (系统托盘 + 桌面 widget)
