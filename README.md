# GitaView

跨平台桌面 Git 仓库状态总览 widget。

## 功能

- **状态总览**：一目了然查看多个 Git 仓库的同步状态
- **分组管理**：按项目分组整理仓库
- **快捷操作**：Fetch、Pull、Push 一键执行
- **桌面 Widget**：小巧窗口，常驻桌面，随时可见

## 状态分类

| 状态 | 含义 |
|------|------|
| synced | 本地与远端一致 |
| local_ahead | 本地有新提交 |
| remote_ahead | 远端有新提交 |
| diverged | 本地与远端分叉 |
| no_remote | 未配置远端 |

## 安装

```bash
# 安装依赖
npm install

# 开发模式
npm run tauri dev

# 构建
npm run tauri build
```

## 技术栈

- **后端**：Tauri 2 + Rust
- **前端**：React + TypeScript + Vite

## 测试

```bash
# Rust 测试
cargo test --manifest-path src-tauri/Cargo.toml

# 前端测试
npm test
```

## 目标平台

- macOS：菜单栏 + 桌面 widget
- Windows：系统托盘 + 桌面 widget

## License

MIT