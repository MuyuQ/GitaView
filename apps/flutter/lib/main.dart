// 应用入口：窗口初始化、单实例、引擎装配、托盘 / 浮窗、主题与页面导航壳。
//
// 对应 C# 宿主：Program.cs / App.xaml.cs / MainWindow.xaml.cs。
// - 窗口 840×520（最小 640×400），沉浸式标题栏（C# ExtendsContentIntoTitleBar
//   + TitleBarStyle.hidden）；Windows 上背景用 mica（C# MicaBackdrop BaseAlt）；
// - 单实例（windows_single_instance）：第二实例的参数回传首实例后退出——
//   协议激活 / 二次启动都只导航已有实例，不创建第二个引擎；
// - 深链 gitaview-native://：冷启动请求缓存，就绪后经 ProtocolNavigation
//   派发（未知 / 已删 ID 回总列表）；热请求立即派发；
// - 托盘（gv_platform C++ 插件）：左键切换浮窗，右键菜单
//   打开主程序 / 刷新 / 退出；托盘不可用时回退"关闭 = 退出"；
// - 浮窗 = desktop_multi_window 第二窗口（flyout_app.dart 子 isolate）；
// - 关闭策略接线 MainWindowClosePolicy：托盘可用 = 隐藏驻留，否则优雅退出；
// - 页面用 IndexedStack 常驻组件树，等价 C# NavigationCacheMode=Required：
//   设置页返回状态页保留筛选、搜索与行选择上下文。
//
// window_manager 0.6 的 legacy 桥（windowManager / WindowOptions /
// WindowListener）是包文档明确支持的迁移路径，且 setPreventClose →
// onWindowClose 的否决语义与 C# AppWindow.Closing 一一对应；新 nativeapi
// Window 尚无等价的"拦截关闭"入口。等上游补齐后切换并移除本忽略标记。
library;

// ignore_for_file: deprecated_member_use

import 'dart:async' show StreamController, StreamSubscription, unawaited;
import 'dart:io' show Directory, Platform;

import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:fluent_ui/fluent_ui.dart';
import 'package:flutter/scheduler.dart' show SchedulerBinding;
import 'package:flutter/services.dart' show MethodCall, PlatformException;
import 'package:flutter_acrylic/flutter_acrylic.dart' as acrylic;
import 'package:window_manager/legacy.dart' as wm;
import 'package:windows_single_instance/windows_single_instance.dart';

import 'app_controller.dart';
import 'engine/engine_gateway.dart';
import 'engine/engine_service.dart';
import 'flyout/flyout_app.dart';
import 'flyout/flyout_host.dart';
import 'platform/platform_channel.dart';
import 'presentation/main_window_close_policy.dart';
import 'presentation/protocol_navigation.dart';
import 'ui/main_page.dart';
import 'ui/settings_page.dart';
import 'ui/theme/tokens.dart';

/// 单实例管道名（Windows 命名管道）。
const String _singleInstancePipe = 'gitaview-native-single-instance';

/// 开机启动状态读不到时的文案（与 AppController 构造默认一致）。
const String _startupStateUnread = '无法读取开机启动状态，返回此页后重试。';

/// 从启动参数中提取 gitaview-native:// 深链（协议激活以参数形式传入）。
String? _deepLinkFromArgs(List<String> args) {
  for (final arg in args) {
    if (arg.startsWith('gitaview-native://')) return arg;
  }
  return null;
}

Future<void> main(List<String> args) async {
  // desktop_multi_window 子窗口：独立 Flutter 引擎重新执行 main。
  if (args.length >= 2 && args[0] == 'multi_window') {
    await runFlyoutApp(args);
    return;
  }
  WidgetsFlutterBinding.ensureInitialized();

  // —— 单实例（非 Windows 为 no-op）：第二实例参数转交后退出 ——
  final coldStartLink = _deepLinkFromArgs(args);
  final hotLinks = StreamController<String>();
  await WindowsSingleInstance.ensureSingleInstance(
    args,
    _singleInstancePipe,
    bringWindowToFront: true,
    onSecondWindow: (secondArgs) {
      final link = _deepLinkFromArgs(secondArgs);
      if (link != null && !hotLinks.isClosed) {
        hotLinks.add(link);
      }
    },
  );

  // —— 窗口（仅 Windows；macOS 测试 / 开发不触碰 Windows 通道） ——
  if (Platform.isWindows) {
    await wm.windowManager.ensureInitialized();
    await wm.windowManager.waitUntilReadyToShow(
      const wm.WindowOptions(
        size: Size(840, 520),
        minimumSize: Size(640, 400),
        titleBarStyle: wm.TitleBarStyle.hidden, // 沉浸式标题栏
      ),
      () async {
        await wm.windowManager.show();
        await wm.windowManager.focus();
      },
    );
    await acrylic.Window.initialize();
    await _applyMica(
        SchedulerBinding.instance.platformDispatcher.platformBrightness);
  }

  // —— 平台路径 ——
  // 打包身份下使用 LocalState（与 0.1.5.x 的 settings.json 同一份数据，
  // 原位升级不迁移）；未打包 / 非 Windows 退回本地目录占位。
  final platform = GvPlatformChannel();
  var dataDirectory = '';
  var logDirectory = '';
  if (Platform.isWindows) {
    try {
      final paths = await platform.systemPaths();
      dataDirectory = paths.dataDirectory;
      logDirectory = paths.logDirectory;
    } on PlatformException {
      // 通道未就绪（如插件注册失败）：回退默认目录，引擎创建仍走绝对路径。
      final fallback = _resolveAppDataRoot();
      dataDirectory = '${fallback.path}/data';
      logDirectory = '${fallback.path}/logs';
    }
  } else {
    final fallback = _resolveAppDataRoot();
    dataDirectory = '${fallback.path}/data';
    logDirectory = '${fallback.path}/logs';
  }

  // —— 引擎 ——
  final service = EngineService(
    dataDirectory: dataDirectory,
    logDirectory: logDirectory,
  );

  // 启动项状态（best-effort：注册表镜像；读不到时回退"无法读取"文案）。
  var startupText = '';
  if (Platform.isWindows) {
    try {
      startupText = startupTaskStateText(await platform.startupTaskState());
    } on PlatformException {
      startupText = startupTaskStateText(-1);
    }
  }

  runApp(GitaViewApp(
    service: service,
    platform: platform,
    coldStartLink: coldStartLink,
    hotLinks: hotLinks.stream,
    startupStateText: startupText,
  ));
}

/// 非 Windows 开发环境的数据根（保证绝对路径且可写）。
Directory _resolveAppDataRoot() {
  final localAppData = Platform.environment['LOCALAPPDATA'];
  if (localAppData != null && localAppData.isNotEmpty) {
    return Directory('$localAppData/GitaView');
  }
  return Directory('${Directory.systemTemp.path}/gitaview-appdata');
}

/// Windows mica 背景；主题明暗变化时由 [GitaViewAppState] 更新。
Future<void> _applyMica(Brightness brightness) async {
  await acrylic.Window.setEffect(
    effect: acrylic.WindowEffect.mica,
    dark: brightness == Brightness.dark,
  );
}

class GitaViewApp extends StatefulWidget {
  final EngineService service;
  final GvPlatformChannel platform;
  final String? coldStartLink;
  final Stream<String> hotLinks;
  final String startupStateText;

  const GitaViewApp({
    super.key,
    required this.service,
    required this.platform,
    required this.coldStartLink,
    required this.hotLinks,
    required this.startupStateText,
  });

  @override
  State<GitaViewApp> createState() => _GitaViewAppState();
}

class _GitaViewAppState extends State<GitaViewApp>
    with WidgetsBindingObserver {
  late final AppController controller = AppController(
    gateway: EngineServiceGateway(widget.service),
    engineEvents: widget.service,
    launchTarget: Platform.isWindows
        ? (kind, value) => widget.platform.openTarget(kind: kind, value: value)
        : null,
    pickFolder: Platform.isWindows
        ? (title) => widget.platform.pickFolder(title)
        : null,
    startupStateText: widget.startupStateText.isEmpty
        ? _startupStateUnread
        : widget.startupStateText,
  );

  /// 页面索引：0 = 状态明细，1 = 设置（托盘菜单 / 深链也可切换）。
  final ValueNotifier<int> pageIndex = ValueNotifier<int>(0);

  late final FlyoutWindowHost flyout = FlyoutWindowHost();
  final ProtocolNavigation _protocol = ProtocolNavigation();
  bool _exitRequested = false;
  bool _trayAvailable = false;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    if (Platform.isWindows) {
      // 先拦截系统关闭，交给 MainWindowClosePolicy 决定隐藏还是退出
      // （对应 C# AppWindow.Closing → HandleMainWindowClose）。
      wm.windowManager.setPreventClose(true).then((_) {
        wm.windowManager.addListener(_CloseInterceptor(this));
      });
      // 引擎启动 + 托盘 + 深链就绪（对应 C# App.OnLaunched 序列）。
      unawaited(_bootstrap());
      // 浮窗可见时跟随快照推送更新。
      widget.service.addListener(_onServiceChanged);
      // 子窗口动作回传（打开 / 刷新 / 退出）。
      DesktopMultiWindow.setMethodHandler(_onFlyoutAction);
    } else {
      // 非 Windows 开发环境同样启动引擎（共享核心在 macOS CI 有覆盖）。
      unawaited(widget.service.start().then((_) => controller.requery()));
    }
    // 深链：冷启动请求缓存，首份快照就绪后派发；热请求立即派发。
    if (widget.coldStartLink != null) {
      _protocol.request(widget.coldStartLink);
    }
    final subscription = widget.hotLinks.listen(_protocol.request);
    // 深链回调：回总列表 = 状态页；具体仓库 = 状态页 + 展开该行。
    _protocol.setReady(
      (repoId) =>
          widget.service.snapshot?.repos.any((repo) => repo.id == repoId) ??
          false,
      (route) {
        pageIndex.value = 0;
        final id = route.repoId;
        if (id != null) {
          controller.toggleRow(id);
        }
      },
    );
    // 热链接流本身与页面同生命周期；dispose 时一并取消。
    _hotLinkSubscription = subscription;
  }

  StreamSubscription<String>? _hotLinkSubscription;

  Future<void> _bootstrap() async {
    await widget.service.start();
    await controller.requery();
    // 托盘注册：失败则回退"关闭 = 退出"（C# 托盘创建失败同样回退）。
    try {
      _trayAvailable = await widget.platform.ensureTray();
    } on PlatformException {
      _trayAvailable = false;
    }
    if (_trayAvailable) {
      widget.platform.setHandlers(
        onTrayLeftClick: _toggleFlyout,
        onTrayMenuAction: _onTrayMenuAction,
      );
    }
  }

  /// 托盘左键：显示 / 收起浮窗（定位到托盘图标，C# TrayWindow 点击语义）。
  Future<void> _toggleFlyout() async {
    final context = widget.platform.trayContext();
    try {
      await flyout.toggle(tray: await context, summary: widget.service.summary);
    } on PlatformException {
      // 托盘上下文短暂不可读：忽略本次点击。
    }
  }

  /// 托盘右键菜单（1=打开主程序 2=刷新 3=退出，与 C# TrayWindow 一致）。
  Future<void> _onTrayMenuAction(int action) async {
    switch (action) {
      case 1:
        await _showMainWindow();
      case 2:
        await controller.refresh();
      case 3:
        await _requestExit();
    }
  }

  /// 浮窗按钮动作（子窗口 isolate 经 DesktopMultiWindow 回传）。
  Future<Object?> _onFlyoutAction(MethodCall call, int fromWindowId) async {
    if (call.method != 'flyoutAction') return null;
    final args = call.arguments;
    final action = args is Map ? args['action'] as String? : null;
    switch (action) {
      case 'open':
        await flyout.hide();
        await _showMainWindow();
      case 'refresh':
        await controller.refresh();
      case 'exit':
        await _requestExit();
    }
    return null;
  }

  Future<void> _showMainWindow() async {
    if (Platform.isWindows) {
      await wm.windowManager.show();
      await wm.windowManager.focus();
    }
    pageIndex.value = 0;
  }

  void _onServiceChanged() {
    if (flyout.isVisible) {
      unawaited(flyout.push(widget.service.summary));
    }
  }

  @override
  void didChangePlatformBrightness() {
    super.didChangePlatformBrightness();
    if (Platform.isWindows) {
      unawaited(_applyMica(
          SchedulerBinding.instance.platformDispatcher.platformBrightness));
    }
  }

  /// 优雅退出：移除托盘与浮窗 → 停止引擎 → 销毁窗口（C# App.RequestExit 序列）。
  Future<void> _requestExit() async {
    if (_exitRequested) return;
    _exitRequested = true;
    if (Platform.isWindows) {
      unawaited(widget.platform.removeTray());
      await flyout.dispose();
      DesktopMultiWindow.setMethodHandler(null);
    }
    await _hotLinkSubscription?.cancel();
    await widget.service.requestExit();
    if (Platform.isWindows) {
      await wm.windowManager.destroy();
    }
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    widget.service.removeListener(_onServiceChanged);
    _hotLinkSubscription?.cancel();
    pageIndex.dispose();
    controller.dispose();
    widget.service.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return FluentApp(
      title: 'GitaView',
      themeMode: ThemeMode.system,
      theme: _buildTheme(Brightness.light),
      darkTheme: _buildTheme(Brightness.dark),
      home: _HomeShell(state: this),
    );
  }
}

/// 关闭拦截：window_manager 的 onWindowClose → 关闭策略（C# AppWindow.Closing）。
class _CloseInterceptor with wm.WindowListener {
  final _GitaViewAppState _state;

  _CloseInterceptor(this._state);

  @override
  void onWindowClose() {
    // 托盘可用 = 关闭隐藏驻留；否则优雅退出（C# MainWindowClosePolicy）。
    MainWindowClosePolicy.handle(
      exitRequested: _state._exitRequested,
      trayAvailable: _state._trayAvailable,
      hide: () => unawaited(wm.windowManager.hide()),
      requestExit: () => unawaited(_state._requestExit()),
    );
  }
}

/// 主题：token 表映射为 FluentThemeData（浅 / 深同一套语义角色）。
FluentThemeData _buildTheme(Brightness brightness) {
  final colors = brightness == Brightness.dark ? gvDarkColors : gvLightColors;
  final base = brightness == Brightness.dark
      ? Typography.fromBrightness(brightness: Brightness.dark)
      : Typography.fromBrightness(brightness: Brightness.light);
  TextStyle withUiFont(TextStyle style) => style.copyWith(
        fontFamily: GvFonts.ui,
        fontFamilyFallback: GvFonts.uiFallback,
      );
  return FluentThemeData(
    brightness: brightness,
    scaffoldBackgroundColor:
        Platform.isWindows ? const Color(0x00000000) : colors.surface,
    typography: Typography.raw(
      display: withUiFont(base.display!),
      titleLarge: withUiFont(base.titleLarge!),
      title: withUiFont(base.title!),
      subtitle: withUiFont(base.subtitle!),
      bodyLarge: withUiFont(base.bodyLarge!),
      bodyStrong: withUiFont(base.bodyStrong!),
      body: withUiFont(base.body!),
      caption: withUiFont(base.caption!),
    ),
  );
}

/// 主页壳：引擎状态分支 + 页面切换（IndexedStack 保留上下文）。
class _HomeShell extends StatelessWidget {
  final _GitaViewAppState state;

  const _HomeShell({required this.state});

  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
      animation: Listenable.merge([state.widget.service, state.controller]),
      builder: (context, _) {
        final service = state.widget.service;
        switch (service.state) {
          case EngineServiceState.idle:
          case EngineServiceState.starting:
            return const ScaffoldPage(
              content: Center(child: ProgressRing()),
            );
          case EngineServiceState.failed:
            // 启动失败页（C# App：错误弹窗 + 退出；此处提供重试入口）。
            return ScaffoldPage(
              content: Center(
                child: ConstrainedBox(
                  constraints: const BoxConstraints(maxWidth: 480),
                  child: Column(
                    mainAxisSize: MainAxisSize.min,
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        'GitaView 启动失败：\n${service.startupError ?? ''}\n\n'
                        '请确认 gitaview_ffi.dll 与数据目录可用后重试。',
                        style: GvTextStyles.body(gvColorsOf(context).ink),
                      ),
                      const SizedBox(height: GvSpacing.content),
                      Button(
                        onPressed: () {
                          state.widget.service.start();
                        },
                        child: Text('重试',
                            style: GvTextStyles.body(gvColorsOf(context).ink)),
                      ),
                    ],
                  ),
                ),
              ),
            );
          case EngineServiceState.running:
          case EngineServiceState.stopping:
          case EngineServiceState.stopped:
            return _PageHost(state: state);
        }
      },
    );
  }
}

/// 页面宿主：状态页 / 设置页常驻（返回保留上下文）。
class _PageHost extends StatefulWidget {
  final _GitaViewAppState state;

  const _PageHost({required this.state});

  @override
  State<_PageHost> createState() => _PageHostState();
}

class _PageHostState extends State<_PageHost> {
  @override
  Widget build(BuildContext context) {
    final controller = widget.state.controller;
    return ValueListenableBuilder<int>(
      valueListenable: widget.state.pageIndex,
      builder: (context, index, _) {
        return IndexedStack(
          index: index,
          children: [
            MainPage(
              controller: controller,
              onOpenSettings: () => widget.state.pageIndex.value = 1,
              dragToolbar: Platform.isWindows,
              captionBar: Platform.isWindows
                  ? wm.WindowCaption(
                      brightness: FluentTheme.of(context).brightness,
                      backgroundColor: const Color(0x00000000),
                    )
                  : null,
            ),
            SettingsPage(
              controller: controller,
              onBack: () => widget.state.pageIndex.value = 0,
            ),
          ],
        );
      },
    );
  }
}
