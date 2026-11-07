# GitaView Flutter 宿主

Rust 共享核心 + 冻结 C ABI v2 的 Flutter 桌面宿主（Windows 先行，取代 0.1.5.x
WinUI 宿主；包身份 `GitaView.Native` 0.2.0.0 原位升级）。

```powershell
# 静态检查与测试（Dart 层直接加载真实 gitaview_ffi 动态库）
cd apps/flutter
flutter pub get
flutter analyze --no-pub
flutter test --no-pub        # 需先 cargo build -p gitaview-ffi --release

# Windows 构建与无签名 MSIX（需 Windows + Flutter SDK + Windows SDK）
flutter build windows --release
powershell -NoProfile -File scripts\native\package-flutter-windows.ps1 -SkipBuild
```

分层：`lib/contracts`（契约严格解码）→ `lib/ffi`（dart:ffi 绑定）→
`lib/engine`（工作 isolate 服务）→ `lib/presentation`（纯逻辑）→ `lib/ui`。
Windows 集成在 `windows/`：runner CMake 直接构建 Rust DLL，`gv_platform`
C++ 插件承载托盘 / LocalState 路径 / 启动项状态。契约 fixture 与 expectations
在 `contracts/native/v1/`，由 Rust / Dart 两端共同消费。
