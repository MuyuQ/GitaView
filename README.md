# GitaView

跨平台桌面 Git 仓库状态总览 widget。

灵感来自 `gita`，但 v1 不依赖 gita CLI 配置，应用自管理仓库列表、分组、设置和 UI。

## 功能

- **状态总览**：一目了然查看多个 Git 仓库的同步状态
- **分组管理**：按项目分组整理仓库
- **快捷操作**：Fetch、Pull，以及带二次确认的 Push
- **桌面 Widget**：小巧窗口，常驻桌面，随时可见
- **仓库扫描**：递归扫描目录，自动发现 Git 仓库

## 状态分类

| 状态 | 颜色 | 含义 |
|------|------|------|
| synced | 绿色 | 本地与远端一致 |
| local_ahead | 黄色 | 本地有新提交 |
| remote_ahead | 黄色 | 远端有新提交 |
| diverged | 红色 | 本地与远端分叉 |
| no_remote | 灰色 | 未配置远端 |

> `error` 是应用层状态，用于表示仓库状态读取失败；它不属于 Git 关系分类中的五种状态。

## 安装与开发

```bash
# 安装依赖
npm install

# 开发模式（启动前端开发服务器 + Tauri 窗口）
npm run tauri dev

# 仅构建前端
npm run build

# 构建生产版本
npm run tauri build
```

## 技术栈

- **后端**：Tauri 2 + Rust
- **前端**：React + TypeScript + Vite
- **样式**：Plain CSS / CSS Modules
- **测试**：Vitest（前端）、Rust 内联测试（后端）

## 测试

```bash
# 前端测试
npm test

# Rust 测试
cargo test --manifest-path src-tauri/Cargo.toml

# 完整验收流程
cargo test --manifest-path src-tauri/Cargo.toml && npm test && npm run build && npm run tauri dev
```

## 目标平台

- **macOS**：菜单栏 + 桌面 widget
- **Windows**：系统托盘 + 桌面 widget

## 设计约束 (v1)

- 不依赖 `gita` CLI 配置
- Push 仅在本地领先或分叉状态下显示，并始终要求二次确认
- v1 仅管理名为 `origin` 的远端 URL；其他远端拓扑暂不支持
- 不执行任意 shell 命令
- 状态不能仅靠颜色传达，必须含文字标签 + 数字
- `无远端` 状态始终排在最后

## License

MIT
