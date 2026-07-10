# GitaView 原生桌面小组件实现计划

**创建日期:** 2026-07-10
**状态:** 规划完成，待实现

---

## 目录

1. [概述](#概述)
2. [技术研究结果](#技术研究结果)
3. [macOS WidgetKit 实现方案](#macos-widgetkit-实现方案)
4. [Windows 小组件方案](#windows-小组件方案)
5. [实现计划](#实现计划)
6. [技术约束和注意事项](#技术约束和注意事项)
7. [文件清单](#文件清单)
8. [实现顺序](#实现顺序)

---

## 概述

GitaView 是一个跨平台桌面 Git 仓库状态 widget，使用 Tauri 2 + Rust + React + TypeScript 构建。

**目标:** 实现原生 macOS 桌面小组件 (WidgetKit)，支持三种尺寸 (小/中/大)，显示完整的仓库状态信息。

**Windows 方案:** 保持当前实现 (Progman/WorkerW 重父化)，因为 Windows 11 Widgets Board 没有公开的第三方扩展 API。

---

## 技术研究结果

### macOS WidgetKit 研究

| 项目 | 详情 |
|------|------|
| **可行性** | ✅ 可行 |
| **技术要求** | Swift/SwiftUI 代码、Xcode 项目、Widget Extension |
| **数据共享** | 共享文件路径 (无需 App Groups) |
| **最低系统** | macOS 11.0 (Big Sur) |
| **构建复杂度** | Tauri + Xcode 混合构建流程 |
| **Apple Developer 账户** | 不需要 (使用免费 Apple ID 即可) |

### Windows 小组件研究

| 项目 | 详情 |
|------|------|
| **可行性** | ❌ 不可行 |
| **原因** | Windows 11 Widgets Board 没有公开的第三方扩展 API |
| **当前方案** | GitaView 已实现最佳方案 (Progman/WorkerW 重父化) |
| **结论** | 保持当前实现 |

### 关键发现

1. **Apple Developer 账户不是严格必需的**
   - 免费 Apple ID 可以进行本地开发和测试
   - App Groups 需要付费账户，但可以用共享文件路径替代
   - 共享文件路径: `~/Library/Application Support/GitaView/widget-data.json`

2. **Tauri App Bundle 结构**
   - Widget Extension 应该放在 `Contents/PlugIns/` 目录
   - 使用 `bundle.macOS.files` 配置来嵌入 `.appex` 文件
   - Tauri 会自动签名 `PlugIns` 目录下的代码

3. **Rust-Swift FFI**
   - 使用 `@_cdecl` (Swift) 和 `extern "C"` (Rust) 进行函数声明
   - 字符串通过 `*const c_char` / `UnsafePointer<CChar>` 传递
   - 内存管理需要明确的所有权规则

4. **Deep Linking**
   - 使用 `widgetURL()` 或 `Link` 实现小组件点击打开应用
   - 需要注册自定义 URL scheme (如 `gitaview://`)
   - Tauri 有 `tauri-plugin-deep-link` 插件支持

---

## macOS WidgetKit 实现方案

### 架构总览

```
┌─────────────────────────────────────────────────────────┐
│                    macOS App Bundle                      │
│                                                         │
│  ┌──────────────────────────────────────────────────┐   │
│  │  Main App (Tauri/Rust)                           │   │
│  │  - React UI (WebView)                            │   │
│  │  - Rust backend (git operations)                 │   │
│  │  - 写入 JSON 到共享路径                           │   │
│  │  - 处理 deep link 打开应用                        │   │
│  └──────────────────────────────────────────────────┘   │
│                         ↓                               │
│  ┌──────────────────────────────────────────────────┐   │
│  │  Widget Extension (Swift/SwiftUI)                │   │
│  │  - TimelineProvider 读取共享文件                  │   │
│  │  - SwiftUI 视图渲染小组件                         │   │
│  │  - 点击小组件触发 deep link                       │   │
│  └──────────────────────────────────────────────────┘   │
│                                                         │
└─────────────────────────────────────────────────────────┘
```

### 三种小组件尺寸设计

#### 小尺寸 (systemSmall)
```
┌─────────────────────┐
│  GitaView           │
│  5 synced           │
│  2 local_ahead      │
│  1 remote_ahead     │
│  0 diverged         │
│  3 no_remote        │
└─────────────────────┘
```
- 显示状态摘要统计
- 点击打开主应用

#### 中尺寸 (systemMedium)
```
┌─────────────────────────────────────┐
│  GitaView                           │
│  ┌─────────────────────────────────┐│
│  │ GitaView        synced    main  ││
│  │ MyProject       local_ahead dev ││
│  │ AnotherRepo     diverged  feat  ││
│  └─────────────────────────────────┘│
└─────────────────────────────────────┘
```
- 显示摘要 + 最近 3-4 个仓库
- 每个仓库可点击，打开主应用并导航到该仓库

#### 大尺寸 (systemLarge)
```
┌─────────────────────────────────────┐
│  GitaView                           │
│  ┌─────────────────────────────────┐│
│  │ GitaView        synced    main  ││
│  │ MyProject       local_ahead dev ││
│  │ AnotherRepo     diverged  feat  ││
│  │ Project3        no_remote master││
│  │ Project4        synced    v2    ││
│  │ Project5        remote_ahead fix││
│  └─────────────────────────────────┘│
│  Summary: 2 synced, 1 local_ahead   │
└─────────────────────────────────────┘
```
- 显示更多仓库 (6-8 个)
- 底部显示详细摘要
- 每个仓库可点击

### 数据共享实现

**共享文件路径:** `~/Library/Application Support/GitaView/widget-data.json`

**数据格式:**
```json
{
  "version": 1,
  "lastUpdated": "2026-07-10T12:00:00Z",
  "repos": [
    {
      "id": "repo-1",
      "name": "GitaView",
      "path": "/Users/user/repos/GitaView",
      "relation": "synced",
      "branch": "main",
      "ahead": 0,
      "behind": 0
    }
  ],
  "summary": {
    "synced": 5,
    "localAhead": 2,
    "remoteAhead": 1,
    "diverged": 0,
    "noRemote": 3
  }
}
```

### Deep Link 实现

**URL Scheme:** `gitaview://`

**URL 模式:**
- `gitaview://open` - 打开主应用
- `gitaview://repo/{repo_id}` - 导航到特定仓库
- `gitaview://settings` - 打开设置

**需要添加的依赖:**
```toml
# src-tauri/Cargo.toml
[dependencies]
tauri-plugin-deep-link = "2.0.0"
tauri-plugin-single-instance = { version = "2.0.0", features = ["deep-link"] }
```

```json
// package.json
{
  "dependencies": {
    "@tauri-apps/plugin-deep-link": "^2.0.0"
  }
}
```

**tauri.conf.json 配置:**
```json
{
  "plugins": {
    "deep-link": {
      "desktop": {
        "schemes": ["gitaview"]
      }
    }
  }
}
```

**capabilities/default.json 权限:**
```json
{
  "permissions": [
    "deep-link:default",
    "core:event:default"
  ]
}
```

### 构建流程

**tauri.conf.json 配置:**
```json
{
  "build": {
    "beforeBundleCommand": "xcodebuild -project src-tauri/widget-extension/GitaViewWidget.xcodeproj -scheme GitaViewWidget -configuration Release -derivedDataPath src-tauri/widget-extension/build CODE_SIGN_IDENTITY='-'"
  },
  "bundle": {
    "macOS": {
      "minimumSystemVersion": "11.0",
      "files": {
        "PlugIns/GitaViewWidgetExtension.appex": "src-tauri/widget-extension/build/Build/Products/Release/GitaViewWidgetExtension.appex"
      }
    }
  }
}
```

---

## Windows 小组件方案

### 当前实现 (保持不变)

GitaView 已经实现了最佳的第三方桌面 widget 方案:

**技术:** Progman/WorkerW 重父化

**实现文件:** `src-tauri/src/desktop_widget/windows.rs`

**工作原理:**
1. 获取 GitaView 的 HWND
2. 找到 Progman 窗口 (Explorer 的桌面管理器)
3. 发送 `WM_CREATE_DESKTOP_WORKER` (0x052C) 消息
4. 使用 `EnumWindows` 找到包含 `SHELLDLL_DefView` 的窗口
5. 调用 `SetParent()` 将 GitaView HWND 重父化到桌面宿主
6. 调整窗口样式 (添加 `WS_CHILD | WS_VISIBLE`，移除 `WS_POPUP`)
7. 使用 `SetWindowPos` 设置为 `HWND_TOP`

**特点:**
- 真正的桌面层，位于桌面图标之上
- 可以承受 "显示桌面" (Win+D) 操作
- 包含看门狗线程，每 5 秒检查一次，自动恢复 Explorer 重启后的状态
- 使用 Rust `windows` crate 直接调用 Win32 API

**结论:** 这是第三方桌面 widget 的最佳方案，无需更改。

---

## 实现计划

### 阶段 1: 创建 Widget Extension 项目

**新建文件:**
```
src-tauri/widget-extension/
├── GitaViewWidget/
│   ├── GitaViewWidget.swift          # Widget 定义
│   ├── Provider.swift                # TimelineProvider
│   ├── Models/
│   │   └── RepoStatus.swift          # 数据模型
│   ├── Views/
│   │   ├── SmallWidgetView.swift     # 小尺寸视图
│   │   ├── MediumWidgetView.swift    # 中尺寸视图
│   │   └── LargeWidgetView.swift     # 大尺寸视图
│   ├── Info.plist                    # Widget 扩展信息
│   └── Assets.xcassets/              # 资源文件
├── GitaViewWidget.xcodeproj          # Xcode 项目
└── build/                            # 构建输出目录
```

### 阶段 2: 数据共享实现

**新建文件:**
- `src-tauri/src/widget_data.rs` - Widget 数据写入模块

**实现功能:**
- 定义共享数据结构
- 在 git 状态更新时写入 JSON 到共享路径
- 确保目录存在 (`~/Library/Application Support/GitaView/`)

### 阶段 3: Deep Link 集成

**修改文件:**
- `src-tauri/Cargo.toml` - 添加 `tauri-plugin-deep-link` 依赖
- `src-tauri/tauri.conf.json` - 添加 deep link 配置
- `src-tauri/capabilities/default.json` - 添加 deep link 权限
- `src-tauri/src/lib.rs` - 注册插件、处理 deep link 事件
- `package.json` - 添加 `@tauri-apps/plugin-deep-link` 依赖

**实现功能:**
- 注册 `gitaview://` URL scheme
- 处理 deep link 事件
- 显示和聚焦主窗口
- 导航到特定仓库

### 阶段 4: 构建流程集成

**修改文件:**
- `src-tauri/tauri.conf.json` - 添加 `beforeBundleCommand` 和 `bundle.macOS.files`

**实现功能:**
- 在 Tauri 构建前编译 Widget Extension
- 将 `.appex` 嵌入到 app bundle 的 `Contents/PlugIns/` 目录
- 提升最低系统版本到 11.0

### 阶段 5: 测试和调试

**测试内容:**
- 小组件显示是否正确
- 数据更新是否及时
- Deep Link 是否正常工作
- 三种尺寸是否都正常显示

**测试命令:**
```bash
# 测试 deep link
open "gitaview://open"
open "gitaview://repo/my-project"
open "gitaview://settings"

# 查看 widget 日志
log show --predicate 'subsystem == "com.apple.widgetkit"' --last 1m
```

---

## 技术约束和注意事项

| 约束 | 影响 | 解决方案 |
|------|------|----------|
| Swift 代码必需 | Widget Extension 必须用 Swift | 创建独立的 Xcode 项目 |
| Xcode 构建必需 | 无法用 Cargo/npm 构建 Widget Extension | 使用 `beforeBundleCommand` 调用 xcodebuild |
| 代码签名 | Widget Extension 需要签名 | 使用 ad-hoc 签名 (`CODE_SIGN_IDENTITY='-'`) |
| 最低系统版本 | 需要 macOS 11.0+ | 从 10.13 提升到 11.0 |
| 刷新预算 | 每天约 40 次刷新 | 缓存数据，仅在 git 状态变化时刷新 |
| Deep Link 测试 | 需要安装应用才能测试 | 开发时使用 `open "gitaview://open"` 测试 |
| 共享文件路径 | 需要确保目录存在 | 在 Rust 代码中创建目录 |
| 内存管理 | FFI 边界需要明确的所有权 | 使用 `CString::into_raw` 和 `CString::from_raw` |

---

## 文件清单

### 新建文件 (Widget Extension)

1. `src-tauri/widget-extension/GitaViewWidget.xcodeproj` - Xcode 项目
2. `src-tauri/widget-extension/GitaViewWidget/GitaViewWidget.swift` - Widget 主入口
3. `src-tauri/widget-extension/GitaViewWidget/Provider.swift` - TimelineProvider
4. `src-tauri/widget-extension/GitaViewWidget/Models/RepoStatus.swift` - 数据模型
5. `src-tauri/widget-extension/GitaViewWidget/Views/SmallWidgetView.swift` - 小尺寸视图
6. `src-tauri/widget-extension/GitaViewWidget/Views/MediumWidgetView.swift` - 中尺寸视图
7. `src-tauri/widget-extension/GitaViewWidget/Views/LargeWidgetView.swift` - 大尺寸视图
8. `src-tauri/widget-extension/GitaViewWidget/Info.plist` - Widget 扩展信息
9. `src-tauri/widget-extension/GitaViewWidget/Assets.xcassets/` - 资源文件

### 新建文件 (Rust 侧)

10. `src-tauri/src/widget_data.rs` - Widget 数据写入模块

### 修改文件

11. `src-tauri/Cargo.toml` - 添加 `tauri-plugin-deep-link` 依赖
12. `src-tauri/tauri.conf.json` - 添加 deep link 配置、bundle.macOS.files、提升最低版本
13. `src-tauri/capabilities/default.json` - 添加 deep link 权限
14. `src-tauri/src/lib.rs` - 注册插件、处理 deep link 事件、调用 widget_data 更新
15. `src-tauri/src/app_commands.rs` - 在状态更新时触发 widget 数据刷新
16. `package.json` - 添加 `@tauri-apps/plugin-deep-link` 依赖

---

## 实现顺序建议

1. **阶段 1**: 创建 Widget Extension 项目和基本视图 (1-2 天)
2. **阶段 2**: 实现数据共享 (Rust 写入 JSON) (1 天)
3. **阶段 3**: 实现 Deep Link 集成 (1 天)
4. **阶段 4**: 集成构建流程 (1 天)
5. **阶段 5**: 测试和调试 (1-2 天)

**总计估计:** 5-7 天

---

## 参考资源

### Tauri 2 文档
- [Tauri 2 Bundle Configuration](https://tauri.app/reference/config/#bundleconfig)
- [Tauri 2 Deep Link Plugin](https://tauri.app/plugin/deep-link/)
- [Tauri 2 macOS Private API](https://tauri.app/reference/config/#macosprivateapi)

### Apple 文档
- [WidgetKit Documentation](https://developer.apple.com/documentation/widgetkit)
- [SwiftUI Widget Tutorial](https://developer.apple.com/documentation/widgetkit/creating-a-widget-extension)
- [macOS App Extensions](https://developer.apple.com/app-extensions/)

### Rust-Swift FFI
- [swift-bridge Crate](https://github.com/nicklimmm/swift-bridge)
- [uniffi Crate](https://github.com/nicklimmm/uniffi-rs)
- [cbindgen Crate](https://github.com/nicklimmm/cbindgen)

### Windows 桌面 Widget
- [Progman/WorkerW 技术](https://www.codeproject.com/Articles/856020/Draw-Behind-Desktop-Icons-in-Windows)
- [Windows Crate for Rust](https://crates.io/crates/windows)

---

## 更新日志

- **2026-07-10**: 创建初始实现计划
  - 完成技术研究
  - 确定 macOS WidgetKit 实现方案
  - 确定 Windows 保持当前实现
  - 制定详细实现计划

---

## 已知限制

### macOS Widget Extension 签名要求

macOS 要求 WidgetKit 扩展必须使用有效的 Apple Developer 证书签名才能被系统注册。自签名证书（包括通过钥匙串访问创建的）不包含 TeamIdentifier，无法满足此要求。

**解决方案：**
1. 注册 Apple Developer Program（$99/年）
2. 使用 Xcode 自动签名（需要 Apple ID 登录）
3. 或使用企业开发者证书

**当前状态：**
- Widget Extension 代码已完成且可编译
- 自签名证书无法注册 Widget Extension
- 需要有效的 Apple Developer 账户才能使 Widget Extension 正常工作
