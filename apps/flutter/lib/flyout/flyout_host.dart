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
library;

import 'dart:async';
import 'dart:convert';
import 'dart:ui' show Rect;

import 'package:desktop_multi_window/desktop_multi_window.dart';

import '../contracts/contracts.dart';
import '../platform/platform_channel.dart' show TrayContext;
import '../presentation/tray_flyout_presentation.dart';

/// 浮窗逻辑宽度（与 FlyoutWindowContent 的 352 内容宽一致）。
const double kFlyoutLogicalWidth = 352;

/// 估算逻辑高度：头部 + 摘要 + 关注列表若干行 + 按钮行。
/// 子窗口会按内容自校准；这里只决定首帧定位。
const double kFlyoutEstimatedLogicalHeight = 420;

final class FlyoutWindowHost {
  WindowController? _window;
  bool _visible = false;

  bool get isVisible => _visible;

  /// 托盘左键：显示 / 收起浮窗。
  Future<void> toggle({
    required TrayContext tray,
    required SummarySnapshot? summary,
  }) async {
    if (_visible) {
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
    _window ??= await DesktopMultiWindow.createWindow(
        summary == null ? '' : jsonEncode(summary.toJson()));
    final scale = tray.scale;
    final width = kFlyoutLogicalWidth * scale;
    final height = kFlyoutEstimatedLogicalHeight * scale;
    final position = TrayFlyoutPresentation.position(
      tray.workArea,
      tray.icon,
      width.round(),
      height.round(),
      scale,
    );
    await _window!.setFrame(
        Rect.fromLTWH(position.x.toDouble(), position.y.toDouble(), width, height));
    await _window!.show();
    _visible = true;
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
    if (window == null || !_visible) return;
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
