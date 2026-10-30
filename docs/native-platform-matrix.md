# 原生应用平台与工具链矩阵

> 2026-10-30 清理说明：旧客户端已从当前源码树移除。本文保留原生改造的历史方案或检查记录；旧源码路径、“保留旧工程”和旧发布命令不再适用。当前目录、命令与平台状态以 [原生架构说明](native-architecture.md) 和仓库 README 为准。

维护规则：本文件由 N00 首次填写，每次工具链或身份变化时更新。状态字段：

- 已验证：本机实际命令输出确认，附检查日期。
- 待验证：版本已选定，尚未在目标环境实际构建。
- 环境阻塞：缺少设备 / 身份 / 工具链，附具体缺口。

不填写伪造的 Team ID、Publisher 或 App Group 授权。

## 1. 共享核心（N01.shared）

| 项目 | 选定值 | 状态 | 证据 |
| --- | --- | --- | --- |
| Rust 工具链 | stable 1.95.0 (59807616e 2026-04-14) | 已验证（Windows 宿主） | 2026-10-24 `rustc --version` |
| Cargo | 1.95.0 (f2d3ce0bd 2026-03-21) | 已验证 | 2026-10-24 `cargo --version` |
| Host triple | x86_64-pc-windows-msvc | 已验证 | `rustup show` |
| 已装 targets | i686-pc-windows-msvc、x86_64-pc-windows-msvc、x86_64-apple-darwin | 已验证 | `rustup show`；apple-darwin 仅能 `cargo check`，无法链接 / 测试 |
| workspace | 根 `Cargo.toml`（members: crates/gitaview-core、crates/gitaview-ffi；resolver 2；锁文件根 Cargo.lock 已生成） | 已验证 | 2026-10-24 `cargo test/check/clippy/fmt --workspace` 通过；`cargo tree` 仅 serde / serde_json / windows / libc，无 Tauri |
| ABI / schema | abiVersion=1、schemaVersion=1 | 待验证 | N02 冻结原型子集，N09 冻结全集 |
| 协议 | `gitaview-native://open`、`gitaview-native://repo/<percent-encoded-repo-id>` | 待验证 | N11 / N14 实测 |
| 契约目录 | `contracts/native/v1/`（README、schema.json、fixtures、expectations.json，N02 建立，原型子集） | 已验证 | Rust 测试以 expectations.json 为唯一期望来源断言（core tests 5 项通过） |
| FFI 产物 | `target/debug/gitaview_ffi.dll` 导出 `gv_abi_version` / `gv_probe_repository` / `gv_buffer_free`（dumpbin /exports 实测）；staticlib `gitaview_ffi.lib` 已生成，符号清单待 N09 在两端工具链正式验证 | 已验证（DLL 导出）；待验证（staticlib / Swift 链接） | 2026-10-24 dumpbin |
| Git | 调用用户安装的 git.exe，固定参数 | 已验证（参考实现） | 旧 crate 96 测试通过；核心提取后重验 |

共享 crate 不依赖 UI 框架。当前只有根 workspace 和 Cargo.lock；原生宿主独立调用 FFI。

## 2. Windows（N01.Windows）

| 项目 | 选定值 | 状态 | 证据 |
| --- | --- | --- | --- |
| 构建宿主 OS | Windows 11 26H2，build 26300.9711，x64 | 已验证 | 2026-10-24 `cmd /c ver` |
| .NET SDK | 10.0.402（含 NETCore / WindowsDesktop 运行时 10.0.13） | 已验证 | `dotnet --version`、`dotnet --list-sdks` |
| Windows App SDK | 2.5.1（2026-09-16 稳定版；2.0 起改用 SemVer，包名 Microsoft.WindowsAppSDK） | 已验证（本项目实际引用：框架包 Microsoft.WindowsAppRuntime.2 2.5.1.0 已在安装验证） | release notes + N03 安装实测 |
| 打包部署脚本 | scripts/native/{build,test,package}-windows.ps1（build→recipe 组包→makeappx→signtool→install→Widget 宿主重扫全链） | 已验证（0.1.0.6 全链 exit 0 安装成功） | N17；PowerShell 5.1 要求脚本带 UTF-8 BOM（实测） |
| IDE | Visual Studio 2022 Community 17.14.37216.2（含 ManagedDesktop 工作负载） | 已验证 | `vswhere` |
| MSBuild | VS 2022 内置 `MSBuild/Current/Bin/MSBuild.exe` | 已验证 | 路径存在 |
| Windows SDK | 10.0.26100.0（另有旧版本） | 已验证 | Windows Kits 目录 |
| MSIX 工具 | makeappx.exe、signtool.exe（Windows Kits 10.0.26100.0 x64） | 已验证 | 目录列表 |
| WinUI 项目模板 | 未安装（`dotnet new` 无 winui 模板；VS 已含 ManagedDesktop） | 待验证 | N03 用 `dotnet new install Microsoft.WindowsAppSDK.Templates` 或 VS 模板，记录实际来源 |
| 首发架构 | x64 | 已验证（本机） | — |
| 最低 OS | WinUI 3 桌面按 WASDK 2.5.1 支持范围；系统小组件需要 Windows 11 Widgets Board | 待验证 | N03/N18 在真实安装上确认，不照抄旧文档 |
| Widgets Board | 本机为 Windows 11 26H2，Board 应可用 | 待验证 | N03 安装 MSIX 后实测 |
| 包名 / 身份 | `GitaView.Native`；Publisher 未定 | 待验证 | 发布证书缺失，见下 |
| 签名 | 自签名测试证书（CN=c078af0d-…，2027-10-19 到期，私钥不可导出，签名测试通过）可用于本地 MSIX 测试签名；无 CA 信任的发布证书 | 已验证（测试证书存在）；环境阻塞（正式发布签名） | 2026-10-24 `certutil -store -user My` |
| 安装包类型 | MSIX（Widget Provider COM server 需要包身份） | 待验证 | N03 |
| 数据目录 | 包 `ApplicationData.LocalFolder`（MSIX 标准路径）；Widget 快照由主应用写入、同包 Provider 读取 | 待验证 | N03 实测路径 |
| 运行时依赖 | .NET 10 桌面运行时 + Windows App SDK 运行时部署方式 | 待验证 | N17 干净机器验证 |

## 3. macOS（N01.macOS）

| 项目 | 选定值 | 状态 | 证据 |
| --- | --- | --- | --- |
| 构建宿主 | 需 Mac + Xcode | 环境阻塞 | 本机为 Windows；无 Mac / Xcode / xcodebuild |
| Xcode / Swift / XcodeGen | 未定（到达 Mac 后选当前稳定版） | 环境阻塞 | 不猜填版本 |
| 首发架构 | arm64 + x86_64（先单架构验证，universal 需显式验证所有 slice） | 待验证 | — |
| 最低 OS | 计划 macOS 14（桌面小组件下限参考）；按实际使用 API 上调 | 待验证 | 无设备 |
| 主 bundle ID | `com.gitaview.native`（计划） | 待验证 | — |
| Widget bundle ID | `com.gitaview.native.widget`（计划） | 待验证 | — |
| App Group | 形式 `<TeamID>.com.gitaview.native`，主程序与扩展一致 | 环境阻塞 | 真实 Team ID 需 Apple 开发者账号 |
| WidgetKit 数据 | App Group 容器 URL（系统 API 解析），原子写 JSON 后 WidgetCenter reload | 待验证 | — |
| 签名 / 公证 | Developer ID + notarization + staple | 环境阻塞 | 无 Apple 签名身份 |
| 安装包 | DMG | 待验证 | — |

## 4. 旧规格适用范围

- `DESIGN_AND_BUILD_SPEC.md`、`NATIVE_WIDGET_IMPLEMENTATION_PLAN.md`、`docs/native-desktop-widget-layer-plan.md` 描述旧 Tauri 产品：其 WebView 技术栈、WorkerW / Progman 桌面贴附、拖动 / 置顶 / compactMode 等要求不是原生版本需求。
- 旧 `docs/platform-acceptance-checklist.md` 属旧桌面 widget，不用于原生验收；N18 新建清单。
- 旧 CI（ci.yml / release.yml）与 `v*` 标签发布流程保持原状；原生构建走独立 workflow（N17）。
- 旧设置目录、旧 Widget JSON、旧 Tauri IPC：原生版本不读取、不迁移、不清理。
