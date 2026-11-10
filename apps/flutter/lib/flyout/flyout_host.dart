// 托盘浮窗宿主（主窗口 isolate 侧）：第二窗口的创建、定位、数据推送。
//
// 对应 C# TrayWindow / TrayFlyoutWindow 协作：
// - 左键托盘 → toggle（可见即收起，不可见即按 TrayFlyoutPresentation.position
//   锚定到托盘图标后显示）；
// - 快照推送 → DesktopMultiWindow.invokeMethod(flyoutId, 'summary', json)，
//   子窗口严格解码（见 flyout_app.dart）；
// - 子窗口动作回调（flyoutAction）由 main.dart 的通道分发处理；
// - 窗口尺寸 352 逻辑宽 × 估算高，底边在托盘上方（gap/翻转到下方的规则
//   在 TrayFlyoutPresentation.position），子窗口按内容自校准高度。
//
// 坐标单位：定位纯逻辑输出物理像素；Windows 窗口 API 同单位原样使用，
// macOS 的 desktop_multi_window fork setFrame/getFrame 以左上原点逻辑点
// 为单位，经 TrayFlyoutPresentation.nativeFrame 除以缩放换算。
library;

import 'dart:async';
import 'dart:convert';
import 'dart:io' show Platform;
import 'dart:ui' show Rect;

import 'package:desktop_multi_window/desktop_multi_window.dart';

import '../contracts/contracts.dart';
import '../platform/platform_channel.dart' show TrayContext;
import '../presentation/tray_flyout_presentation.dart';

/// 估算逻辑高度：头部 + 摘要 + 关注列表若干行 + 按钮行。
/// 子窗口按内容自校准（flyout_app _autoSize）；这里只决定首帧定位。
const double kFlyoutEstimatedLogicalHeight = 420;

final class FlyoutWindowHost {
  FlyoutWindowHost({this.onWindowCreated});

  /// 子窗口首次创建后的宿主侧收尾（macOS：styleFlyoutWindow 调整浮窗形态）。
  final Future<void> Function()? onWindowCreated;

  WindowController? _window;
  bool _visible = false;
  bool _styled = false;
  bool _firstShowChecked = false;

  bool get isVisible => _visible;

  /// 托盘左键：显示 / 收起浮窗。
  Future<void> toggle({
    required TrayContext tray,
    required SummarySnapshot? summary,
  }) async {
    // 以子窗口真实可见状态为准：子窗口失焦会自藏（flyout_app 监听 blur），
    // 本地 _visible 缓存随之过时——若按缓存 toggle，下一次点击会走 hide
    // 分支导致浮窗"打不开"（Windows / macOS 同款竞态）。
    final bool hidden = _window == null || await _window!.isHidden();
    if (!hidden) {
      await hide();
      return;
    }
    await show(tray: tray, summary: summary);
  }

  /// 按托盘上下文定位并显示（已存在则复用同一子窗口）。
  Future<void> show({
    required TrayContext tray,
    required SummarySnapshot? summary,
  }) async {
    if (_window == null) {
      _window = await DesktopMultiWindow.createWindow(
          summary == null ? '' : jsonEncode(summary.toJson()));
      if (Platform.isMacOS) {
        // 浮窗形态收尾仅 macOS 需要：fork 的 resizable/setMovable 是 macOS
        // 专属方法（Windows 侧回 NotImplemented）。
        await _window!.resizable(false);
        await _window!.setMovable(false);
      } else if (Platform.isWindows) {
        // fork 在 Windows 用 WS_OVERLAPPEDWINDOW 创建子窗口——带标准标题栏
        // （最小化/最大化/关闭），托盘弹出窗不该有这些。showTitleBar(false)
        // 走 fork 的 WM_NCCALCSIZE 隐藏分支去除系统边框（RustDesk 式无边框）；
        // 关闭走浮窗内「退出」，隐藏走失焦与托盘开合。
        await _window!.showTitleBar(false);
      }
    }
    final scale = tray.scale;
    final width = TrayFlyoutPresentation.flyoutLogicalWidth * scale;
    final height = kFlyoutEstimatedLogicalHeight * scale;
    final position = TrayFlyoutPresentation.position(
      tray.workArea,
      tray.icon,
      width.round(),
      height.round(),
      scale,
    );
    final frame = TrayFlyoutPresentation.nativeFrame(
      position: position,
      width: width.round(),
      height: height.round(),
      scale: scale,
      logicalUnits: Platform.isMacOS,
    );
    await _window!
        .setFrame(Rect.fromLTWH(frame.x, frame.y, frame.width, frame.height));
    await _window!.show();
    if (Platform.isMacOS && !_firstShowChecked) {
      // fork 的 FlutterWindowInner 覆写 order：每个子窗口首次 order 会被
      // hiddenSubWindowAtLaunch 强制隐藏一次（per-window 防启动闪烁旗标）。
      // 首 show 被吞掉时补一次——旗标 per-window 只命中一次，后续 show
      // 不再做 isHidden 往返。
      _firstShowChecked = true;
      if (await _window!.isHidden()) {
        await _window!.show();
      }
    }
    _visible = true;
    if (Platform.isMacOS && !_styled) {
      // 标题栏按钮簇在窗口首次上屏后才实例化——创建时 standardWindowButton
      // 链为空，隐藏无效；上屏后再收尾（仅一次，样式随窗口保持）。
      await onWindowCreated?.call();
      _styled = true;
    }
    await push(summary);
  }

  /// 推送最新 Summary 快照到浮窗（未创建时静默跳过）。
  Future<void> push(SummarySnapshot? summary) async {
    final window = _window;
    if (window == null || !_visible) return;
    await DesktopMultiWindow.invokeMethod(
        window.windowId, 'summary', summary == null ? '' : jsonEncode(summary.toJson()));
  }

  Future<void> hide() async {
    final window = _window;
    if (window == null) return;
    await window.hide();
    _visible = false;
  }

  /// 退出序列：关闭子窗口（子窗口 setPreventClose(true) 把 close 转为 hide，
  /// 所以先解除拦截再关闭，避免引擎常驻泄漏）。
  Future<void> dispose() async {
    final window = _window;
    if (window == null) return;
    await window.setPreventClose(false);
    await window.close();
    _window = null;
    _visible = false;
  }
}
