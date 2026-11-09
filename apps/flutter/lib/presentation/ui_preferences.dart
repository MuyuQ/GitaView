// 宿主本地 UI 偏好（主题等纯界面设置）。
//
// 定位：主题只影响界面呈现，不属于核心 settings.json 契约（契约承载仓库
// 管理 / 刷新策略等跨端配置；写入会触发引擎重扫）。C# 宿主对应 LocalSettings
// 方向——两端各自持久化，互不影响核心配置与仓库数据。
// - 文件：<数据根>/ui-preferences.json，形如 {"themeMode":"light"}；
// - 取值："system" / "light" / "dark"（wire 字符串，宿主层映射到 ThemeMode）；
// - 读取失败 / 文件损坏 / 字段未知 → 回退 "system"，不抛异常；
// - 写入失败（目录不可写等）→ 静默忽略：主题偏好丢失可接受，界面仍可用。
library;

import 'dart:convert';
import 'dart:io';

/// 主题模式 wire 值（与 FluentApp ThemeMode 的名称一一对应）。
typedef ThemeModeWire = String;

/// 宿主本地 UI 偏好（C# UiPreferences 的对应物，按宿主各自持久化）。
final class UiPreferences {
  final Directory directory;

  const UiPreferences(this.directory);

  File get _file => File('${directory.path}/ui-preferences.json');

  /// 读取主题模式 wire 值；任何失败回退 "system"。
  Future<ThemeModeWire> loadThemeMode() async {
    try {
      final map =
          jsonDecode(await _file.readAsString()) as Map<String, Object?>;
      return switch (map['themeMode']) {
        'light' => 'light',
        'dark' => 'dark',
        _ => 'system',
      };
    } catch (_) {
      return 'system';
    }
  }

  /// 保存主题模式 wire 值；失败静默忽略（界面偏好不阻塞操作）。
  Future<void> saveThemeMode(ThemeModeWire mode) async {
    try {
      await directory.create(recursive: true);
      await _file
          .writeAsString(jsonEncode({'themeMode': mode}), flush: true);
    } catch (_) {
      // 界面偏好写失败不提示：下次启动回退跟随系统。
    }
  }
}
