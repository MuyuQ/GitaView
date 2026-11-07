// Windows 平台通道封装：gv_platform C++ 插件（runner）+ 目录选择器。
//
// 对应 C# 宿主：
// - TrayWindow.cs —— 托盘注册 / 图标矩形 / 左键回调 / 右键菜单动作回调；
// - TrayFlyoutWindow 定位输入 —— 图标矩形 + 所在显示器工作区（物理像素）
//   与 DPI 缩放（C# XamlRoot.RasterizationScale 的对应物）；
// - App.xaml.cs —— LocalState 路径（LocalState\data / LocalState\logs）；
// - SettingsPage —— StartupTask 状态（注册表镜像整数 → 文案映射）；
// - Launcher / FolderPicker —— ShellExecute 打开目录 / 浏览器安全 URL；
//   目录选择用 file_selector（Windows 实现为系统对话框）。
//
// 非 Windows 平台：所有方法抛 UnsupportedError 之外的路径不存在——
// 调用方必须用 Platform.isWindows 守护（main.dart 已守护）。
library;

import 'package:file_selector/file_selector.dart';
import 'package:flutter/services.dart';

import '../presentation/tray_flyout_presentation.dart';

/// StartupTask 注册表镜像状态 → 设置页文案（best-effort：
/// 镜像值语义以实机验证为准，未知值回退到"无法读取"）。
String startupTaskStateText(int state) => switch (state) {
      2 || 4 => '已启用开机启动。',
      0 || 1 || 3 => '已禁用开机启动。',
      _ => '无法读取开机启动状态，返回此页后重试。',
    };

/// 托盘定位上下文：图标矩形 + 工作区（物理像素）与缩放。
class TrayContext {
  final FlyoutBounds? icon;
  final FlyoutBounds workArea;
  final double scale;

  const TrayContext({required this.workArea, required this.scale, this.icon});
}

/// gv_platform 通道封装。测试可注入 [invoke] 替换真实通道。
class GvPlatformChannel {
  final Future<Object?> Function(String method, [Object? arguments]) invoke;

  static const MethodChannel _channel = MethodChannel('gitaview/platform');

  GvPlatformChannel() : invoke = _defaultInvoke;

  const GvPlatformChannel.test(this.invoke);

  static Future<Object?> _defaultInvoke(String method, [Object? arguments]) {
    return _channel.invokeMethod(method, arguments);
  }

  /// 注册托盘回调。必须在 Windows 引擎就绪后调用一次。
  void setHandlers({
    required Future<void> Function() onTrayLeftClick,
    required Future<void> Function(int action) onTrayMenuAction,
  }) {
    _channel.setMethodCallHandler((call) async {
      switch (call.method) {
        case 'onTrayLeftClick':
          await onTrayLeftClick();
        case 'onTrayMenuAction':
          final action = (call.arguments as num?)?.toInt() ?? 0;
          await onTrayMenuAction(action);
      }
      return null;
    });
  }

  /// 系统路径：LocalState\data 与 LocalState\logs（未打包为 %LOCALAPPDATA%）。
  Future<({String dataDirectory, String logDirectory})> systemPaths() async {
    final map = await _mapCall('getSystemPaths');
    return (
      dataDirectory: map['dataDirectory']! as String,
      logDirectory: map['logDirectory']! as String,
    );
  }

  /// 注册托盘图标；失败（如 Explorer 短暂不可用）返回 false。
  Future<bool> ensureTray() async {
    final result = await invoke('ensureTray');
    return result == true;
  }

  /// 退出前移除托盘图标（进程退出后 Explorer 也会清理，显式移除更干净）。
  Future<void> removeTray() async {
    await invoke('removeTray');
  }

  /// 托盘定位上下文；托盘未就绪时 icon 为 null（position 的无锚点回退）。
  Future<TrayContext> trayContext() async {
    final map = await _mapCall('getTrayContext');
    FlyoutBounds? icon;
    if (map['hasIcon'] == true && map['icon'] is Map) {
      final raw = (map['icon']! as Map).cast<String, Object?>();
      icon = FlyoutBounds((raw['x']! as num).toInt(), (raw['y']! as num)
          .toInt(), (raw['width']! as num).toInt(), (raw['height']! as num)
          .toInt());
    }
    final work = (map['workArea']! as Map).cast<String, Object?>();
    return TrayContext(
      icon: icon,
      workArea: FlyoutBounds((work['x']! as num).toInt(), (work['y']! as num)
          .toInt(), (work['width']! as num).toInt(), (work['height']! as num)
          .toInt()),
      scale: (map['scale']! as num).toDouble(),
    );
  }

  /// StartupTask 注册表镜像状态（-1 = 读不到）。
  Future<int> startupTaskState() async {
    final result = await invoke('getStartupTaskState');
    return (result as num?)?.toInt() ?? -1;
  }

  /// 打开目录 / 远端 URL（返回是否成功；失败时调用方反馈"未知错误"）。
  Future<bool> openTarget({required String kind, required String value}) async {
    final result = await invoke('openTarget', {'kind': kind, 'value': value});
    return result == true;
  }

  /// 目录选择器（C# FolderPicker 的对应物；取消返回 null）。
  /// [title] 保留接口兼容：Windows 系统对话框不支持自定义标题。
  Future<String?> pickFolder(String title) => getDirectoryPath();

  Future<Map<Object?, Object?>> _mapCall(String method) async {
    final result = await invoke(method);
    if (result is Map) {
      return result.cast<Object?, Object?>();
    }
    throw StateError('平台通道 $method 返回异常');
  }
}
