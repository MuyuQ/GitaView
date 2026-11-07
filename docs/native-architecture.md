# 原生应用架构与构建

更新日期：2026-11-07。本文描述当前源码入口；原生改造的早期规划和审查记录用于查阅历史，不覆盖当前目录与命令。

## 当前结构

```text
Cargo.toml / Cargo.lock                 共享 Rust workspace
crates/gitaview-core/                   Git、设置、引擎、查询与 Summary 投影
crates/gitaview-ffi/                    C ABI、缓冲区与句柄生命周期
contracts/native/v1/                   JSON Schema、fixtures、期望值
apps/flutter/                           Flutter 宿主（0.2.0.0，Windows 验收门禁前与 WinUI 并行）
apps/windows/GitaView.Native.slnx       Windows 解决方案（0.1.5.x WinUI 宿主）
apps/windows/GitaView.Native/           WinUI 3 主程序、设置、托盘与浮窗
apps/windows/GitaView.Native.Interop/   C ABI 包装
apps/windows/GitaView.Native.Contracts/ DTO 与契约 JSON 验证
apps/windows/GitaView.Native.Tests/     C# 契约与真实 FFI 测试
scripts/native/                        测试、构建、打包与资源生成
assets/brand/gitaview.svg               品牌矢量源
.github/workflows/native-ci.yml         唯一应用 CI
```

Flutter 宿主（迁移进行中）在 `apps/flutter`：契约严格解码 → dart:ffi 绑定 →
双 isolate 引擎服务 → 8 个纯逻辑类 → fluent_ui 界面；Windows 侧 runner 直接
构建 Rust DLL，`gv_platform` C++ 插件承载托盘 / LocalState / 启动项状态，
MSIX 身份 `GitaView.Native` 0.2.0.0 原位升级（LocalState 与 0.1.5.x 共用）。
验收门禁（Windows 实机 GUI / 签名安装）通过前不删除 WinUI 宿主。

macOS 宿主尚未实现，不存在可构建的 `apps/macos` 工程；Flutter 宿主的 Dart
层（契约 / FFI / 引擎服务 / 纯逻辑 / 界面测试）在 macOS 开发与 CI 上以真实
dylib 运行，这不代表 macOS 图形应用或签名安装通过。

主应用持有唯一扫描引擎。配置事务先校验并原子落盘，再提交内存和版本化快照；UI 数据由快照派生。显式退出后扫描停止，无组件独立扫描或持久缓存。Git 命令使用固定参数、进程树约束和超时，日志通过有界队列写入并脱敏。

## 构建与检查

共享核心：`cargo test --workspace --locked`、`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`。

Windows 全套检查：`powershell -NoProfile -File scripts/native/test-windows.ps1`，包括 12 项打包布局检查、Rust workspace 测试和真实 FFI 的 C# 测试。测试数量随新增回归变化，以本次命令输出为准。

构建：`powershell -NoProfile -File scripts/native/build-windows.ps1 -Configuration Release`。脚本按配置构建 Rust FFI，发布自包含 WinUI 应用，并根据官方 build recipe 组装打包布局。Debug / Release 分别使用 `target/debug` / `target/release` 的 Rust DLL。布局检查验证 DLL 哈希和必需的 .NET 运行时文件。

打包：`powershell -NoProfile -File scripts/native/package-windows.ps1 -Configuration Release -SkipBuild`。输出 `apps/windows/GitaView.Native/GitaView.Native_<清单版本>_x64.msix`。默认无签名，不安装、不启动应用。

Flutter 宿主（迁移进行中，见 apps/flutter/README.md）：Dart 层在 macOS 开发与 CI 直接以真实 dylib 验证——`cd apps/flutter && flutter pub get && flutter analyze --no-pub && flutter test --no-pub`（测试需先 `cargo build -p gitaview-ffi --release`，加载真实动态库与真实引擎）。Windows 构建与无签名打包需 Windows + Flutter SDK：`flutter build windows --release` 后运行 `powershell -NoProfile -File scripts/native/package-flutter-windows.ps1 -SkipBuild`；runner 构建自动执行 `cargo build --release --locked` 产出 `gitaview_ffi.dll` 并随包安装。

契约检查：先安装 `scripts/native/requirements-test.txt`，再执行 `python scripts/native/test-contract-schema.py`。Python 只用于开发验证，不嵌入应用。

## 签名与安装

需要签名时，设置 `GITAVIEW_SIGN_THUMBPRINT`，再给打包命令增加 `-Sign`。证书必须在本机证书存储中可用，且与包清单 Publisher 一致。脚本调用 Windows SDK 的 `signtool`；不得提交私钥、证书口令或本机发布配置。

`-Install` 还要求 `-Sign`。该选项会移除已安装的同身份包及其 LocalState，再安装新包。它不是普通构建步骤，仅在用户要求安装时使用。当前没有自动正式发布工作流，`v*` 标签不会触发安装包发布。

## 资源生成

普通构建使用已提交的图标，不需要 npm。修改品牌 SVG 时运行：

```powershell
npm ci --prefix scripts/native
npm run generate --prefix scripts/native
```

此目录的 npm 包只包含 Sharp 资源工具。根目录不再有 React / TypeScript / Vite 工程。

## 历史资料

旧客户端和仅适用于旧客户端的规格、CI、签名脚本及设计稿已从当前树移除。需要追溯时查看清理前提交 `64462c5`。原生实施和审查文档仍保留早期决策与证据；其中“保留旧工程”“提取旧源码”及旧路径只描述当时的工作，不要求恢复这些文件。
