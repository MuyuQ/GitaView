// 托盘浮窗第二窗口（子 isolate 入口）。
//
// desktop_multi_window 为每个子窗口启动独立 Flutter 引擎，重新执行 main：
// args 固定为 `["multi_window", windowId, argumentsString]`（插件源码约定）。
// 本文件只在子窗口 isolate 内运行：
// - 主窗口经 DesktopMultiWindow.invokeMethod(windowId, 'summary', json)
//   推送 SummarySnapshot，这里严格解码后驱动 FlyoutWindowContent；
// - 三个动作（打开 / 刷新 / 退出）回传主窗口 isolate 决定（windowId 0）；
// - 失焦自动隐藏（C# TrayFlyoutWindow Deactivated → Dismiss），Esc 同语义；
//   系统关闭请求一律转为隐藏（子窗口不退出进程）；
// - 每次内容变化后自量高度并保持底边锚定（浮窗从托盘向上展开，
//   主窗口按估算高度定位，这里收缩到实际内容高度）。
library;

import 'dart:async';
import 'dart:convert';
import 'dart:io' show Platform;

import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:fluent_ui/fluent_ui.dart';
import 'package:flutter/scheduler.dart';
import 'package:flutter/services.dart' show LogicalKeyboardKey;

import '../contracts/contracts.dart';
import '../ui/flyout_window.dart';

SummarySnapshot? _decodeSummary(String? json) {
  if (json == null || json.isEmpty) return null;
  try {
    return SummarySnapshot.decode(jsonDecode(json), 'flyout');
  } on ContractException {
    // 自家推送不应失败；防御性忽略畸形数据，保留上一份内容。
    return null;
  }
}

/// 子窗口入口：`main(List<String> args)` 检测到 multi_window 后调用。
Future<void> runFlyoutApp(List<String> args) async {
  WidgetsFlutterBinding.ensureInitialized();
  final windowId = int.parse(args[1]);
  final summary = ValueNotifier<SummarySnapshot?>(
      _decodeSummary(args.length > 2 && args[2].isNotEmpty ? args[2] : null));

  DesktopMultiWindow.setMethodHandler((call, fromWindowId) async {
    if (call.method == 'summary') {
      summary.value = _decodeSummary(call.arguments as String?);
    }
    return null;
  });

  Future<void> sendAction(String action) async {
    await DesktopMultiWindow.invokeMethod(0, 'flyoutAction', {'action': action});
  }

  final controller = WindowController.fromWindowId(windowId);
  // 系统关闭 / 失焦都转为隐藏，浮窗引擎常驻（对应 C# 子窗口生命周期）。
  await controller.setPreventClose(true);
  DesktopMultiWindow.addListener(_FlyoutWindowListener(controller));

  runApp(_FlyoutApp(
    windowId: windowId,
    summary: summary,
    onOpen: () => sendAction('open'),
    onRefresh: () => sendAction('refresh'),
    onExit: () => sendAction('exit'),
  ));
}

class _FlyoutWindowListener extends MultiWindowListener {
  final WindowController controller;

  _FlyoutWindowListener(this.controller);

  @override
  void onWindowBlur() {
    // 失焦即收起（C# Deactivated）；托盘左键再次点击由主窗口切换。
    unawaited(controller.hide());
  }

  @override
  void onWindowClose() {
    unawaited(controller.hide());
  }
}

class _FlyoutApp extends StatelessWidget {
  final int windowId;
  final ValueNotifier<SummarySnapshot?> summary;
  final Future<void> Function() onOpen;
  final Future<void> Function() onRefresh;
  final Future<void> Function() onExit;

  const _FlyoutApp({
    required this.windowId,
    required this.summary,
    required this.onOpen,
    required this.onRefresh,
    required this.onExit,
  });

  @override
  Widget build(BuildContext context) {
    return FluentApp(
      debugShowCheckedModeBanner: false,
      themeMode: ThemeMode.system,
      theme: FluentThemeData(brightness: Brightness.light),
      darkTheme: FluentThemeData(brightness: Brightness.dark),
      home: _FlyoutScaffold(
        windowId: windowId,
        summary: summary,
        onOpen: onOpen,
        onRefresh: onRefresh,
        onExit: onExit,
      ),
    );
  }
}

class _FlyoutScaffold extends StatefulWidget {
  final int windowId;
  final ValueNotifier<SummarySnapshot?> summary;
  final Future<void> Function() onOpen;
  final Future<void> Function() onRefresh;
  final Future<void> Function() onExit;

  const _FlyoutScaffold({
    required this.windowId,
    required this.summary,
    required this.onOpen,
    required this.onRefresh,
    required this.onExit,
  });

  @override
  State<_FlyoutScaffold> createState() => _FlyoutScaffoldState();
}

class _FlyoutScaffoldState extends State<_FlyoutScaffold> {
  final GlobalKey _contentKey = GlobalKey();

  @override
  Widget build(BuildContext context) {
    // Esc = Dismiss（C# TrayFlyoutWindow Escape 键语义）。
    return CallbackShortcuts(
      bindings: {
        const SingleActivator(LogicalKeyboardKey.escape): () {
          unawaited(WindowController.fromWindowId(widget.windowId).hide());
        },
      },
      child: Focus(
        autofocus: true,
        child: ValueListenableBuilder<SummarySnapshot?>(
          valueListenable: widget.summary,
          builder: (context, summary, _) {
            // 内容高度随关注列表变化：每帧后校准窗口高度。
            SchedulerBinding.instance
                .addPostFrameCallback((_) => unawaited(_autoSize()));
            return KeyedSubtree(
              key: _contentKey,
              child: FlyoutWindowContent(
                summary: summary,
                onOpen: () => widget.onOpen(),
                onRefresh: () => widget.onRefresh(),
                onExit: () => widget.onExit(),
              ),
            );
          },
        ),
      ),
    );
  }

  Future<void> _autoSize() async {
    final contentContext = _contentKey.currentContext;
    if (contentContext == null || !mounted) return;
    final logicalHeight = contentContext.size?.height;
    if (logicalHeight == null || logicalHeight <= 0) return;
    // 窗口 API 单位：macOS（fork）为左上原点逻辑点，直接用逻辑高；
    // Windows 为物理像素，按设备像素比放大。
    final nativeHeight = Platform.isMacOS
        ? logicalHeight
        : logicalHeight * View.of(context).devicePixelRatio;
    final controller = WindowController.fromWindowId(widget.windowId);
    final frame = await controller.getFrame();
    if ((frame.height - nativeHeight).abs() < 1.0) return;
    // 底边保持不动，高度收缩 / 扩张到实际内容高度。
    await controller.setFrame(Rect.fromLTWH(
      frame.left,
      frame.bottom - nativeHeight,
      frame.width,
      nativeHeight,
    ));
  }
}
