# GitaView

原生桌面 Git 仓库状态总览。应用自管理仓库列表、分组和刷新策略，使用用户安装的 Git，不依赖 `gita` CLI。

## 当前平台状态

| 平台 | 实现与验证范围 |
| --- | --- |
| Windows | 现行 C# / WinUI 3 主程序（0.1.5.x，Win32 托盘与浮窗）；Rust + Flutter 宿主（0.2.0.0）已实现并通过 Dart 层测试，构建 / 打包 / GUI 待 CI 与实机验收，通过门禁前两套宿主并行。CI 覆盖两链测试、构建和无签名 MSIX 打包。 |
| macOS | 共享 Rust 核心与 Flutter 宿主 Dart 层（契约 / FFI / 引擎服务 / 界面测试）在 macOS 开发与 CI 以真实 dylib 验证。macOS 图形应用打包尚未实现，当前没有 macOS 应用安装包。 |

仓库只保留原生应用。旧 Tauri / React 工程、桌面贴附实现及其构建和发布流程已移除；历史代码可从 Git 历史查看。

## 功能与边界

- 管理多个仓库，支持分组、搜索、关系筛选、目录扫描和候选添加。
- 显示分支、提交差异、同步关系及读取错误；状态同时使用文字、数字和颜色。
- Fetch 需要配置 origin；Pull / Push 始终要求确认，核心在执行前再次校验。
- 定时刷新只读取本地 Git 状态，不执行自动 Fetch。
- Windows 主窗口与托盘浮窗使用同一引擎快照；显式退出后停止扫描，无任何组件独立扫描或持久缓存。
- 仅管理 origin，不提供任意 shell 面板，不读取或迁移旧版设置。

macOS 与 Windows 使用统一的产品布局与状态语义，窗口装饰、菜单栏 / 托盘遵循各平台原生行为。

## 开发依赖

共享核心需要 Rust 1.95 或以上、Cargo 和 Git。Windows 主程序需要 Windows 11（项目最低版本 10.0.22000.0）、.NET SDK 10、Windows SDK 10.0.26100 工具及 Windows App SDK 构建支持；实际 NuGet 版本见项目文件。Windows SDK 提供 `makeappx` 和 `signtool`。

```powershell
# 共享 Rust 核心与 C ABI
cargo test --workspace --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings

# Windows：打包布局检查、Rust 测试、真实 FFI 的 C# 测试
powershell -NoProfile -File scripts/native/test-windows.ps1

# Windows：构建 Release 自包含应用与打包布局
powershell -NoProfile -File scripts/native/build-windows.ps1 -Configuration Release

# Windows：生成无签名 MSIX 开发产物，不安装
powershell -NoProfile -File scripts/native/package-windows.ps1 -Configuration Release -SkipBuild
```

Windows 解决方案为 `apps/windows/GitaView.Native.slnx`。构建布局位于 `apps/windows/GitaView.Native/pkg/build/x64/<Configuration>/`，MSIX 位于主应用目录。无签名产物供检查，安装需要有效签名；签名与安装说明见[原生构建说明](docs/native-architecture.md#签名与安装)。

Flutter 宿主（`apps/flutter`）的 Dart 层检查与测试见 [Flutter 宿主说明](apps/flutter/README.md)；Windows 构建与无签名打包命令见[原生构建说明](docs/native-architecture.md#构建与检查)。

## 契约验证

```powershell
python -m pip install -r scripts/native/requirements-test.txt
python scripts/native/test-contract-schema.py
```

`.github/workflows/native-ci.yml` 是应用的唯一构建验证流程，覆盖 Windows 全链（WinUI 与 Flutter 双宿主）、Flutter 宿主 Dart 层和 macOS 共享核心 + Dart 层，上传 `gitaview-native-windows-dev` 与 `gitaview-flutter-windows-dev`。当前没有自动正式发布流程，推送 `v*` 标签不会构建或发布安装包。

## 品牌资源

应用图标由 `assets/brand/gitaview.svg` 生成，PNG / ICO 已随源码提交，正常构建不需要 Node.js。修改图标时才需要安装独立的资源工具：

```powershell
npm ci --prefix scripts/native
npm run generate --prefix scripts/native
```

Node.js / Sharp 仅用于资源生成，不参与原生 UI 或 Git 核心运行。

## 项目资料

- [产品原则](PRODUCT.md)
- [当前架构、命令与签名说明](docs/native-architecture.md)
- [双平台统一界面设计](docs/native-app-ui-design.md)
- [平台工具链与验收范围](docs/native-platform-matrix.md)
- [验收记录](docs/native-platform-acceptance.md)
- [进度与历史实施记录](docs/native-app-progress.md)
- [原生契约与测试数据](contracts/native/v1/README.md)

## License

MIT
