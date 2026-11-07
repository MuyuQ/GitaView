// GvPlatform：GitaView 宿主平台通道（托盘、系统路径、启动项状态）。
//
// 语义对应 C# 宿主：
// - 托盘 = TrayWindow.cs：隐藏顶层窗口 + Shell_NotifyIconW，WM_APP 回调，
//   Explorer 重启（TaskbarCreated）后重新注册；左键回调打开浮窗，
//   右键 TrackPopupMenu（SetForegroundWindow + WM_NULL 焦点规则）。
// - 路径 = App.xaml.cs StartEngine()：LocalState\data 与 LocalState\logs，
//   保证 0.1.5.x → 0.2.0.0 原位升级读取同一 settings.json。
// - 启动项 = SettingsPage 显示系统实际状态（StartupTask 注册表镜像，
//   读取为尽力而为，映射在 Dart 侧完成）。
//
// 通道名 "gitaview/platform"：
// - 宿主 → Dart 调用：onTrayLeftClick()、onTrayMenuAction(int index)
//   （1=打开主程序 2=刷新 3=退出）。
// - Dart → 宿主调用：getSystemPaths / ensureTray / removeTray /
//   getTrayContext / getStartupTaskState / openTarget。
#pragma once

#include <windows.h>
#include <shellapi.h>

#include <flutter/encodable_value.h>
#include <flutter/method_channel.h>
#include <flutter/plugin_registrar_windows.h>
#include <flutter/standard_method_codec.h>

#include <memory>

namespace gitaview {

class GvPlatform {
 public:
  // 注册通道与托盘消息窗口；整个进程生命周期持有。
  static void Register(flutter::PluginRegistrarWindows* registrar);

  GvPlatform(const GvPlatform&) = delete;
  GvPlatform& operator=(const GvPlatform&) = delete;

 private:
  explicit GvPlatform(flutter::PluginRegistrarWindows* registrar);

  // Dart → 宿主。
  void HandleMethodCall(
      const flutter::MethodCall<flutter::EncodableValue>& call,
      std::unique_ptr<flutter::MethodResult<flutter::EncodableValue>> result);

  // 托盘。
  bool EnsureTray();
  void RemoveTray();
  bool GetTrayIconRect(RECT* rect) const;
  void SendTrayContext(
      std::unique_ptr<flutter::MethodResult<flutter::EncodableValue>> result)
      const;
  void ShowContextMenu();
  static LRESULT CALLBACK TrayWndProc(HWND window, UINT message, WPARAM wparam,
                                      LPARAM lparam);
  void OnTrayMessage(HWND window, UINT message, WPARAM wparam, LPARAM lparam);

  void NotifyDart(const char* method,
                  std::unique_ptr<flutter::EncodableValue> argument) const;

  static GvPlatform* instance_;

  flutter::PluginRegistrarWindows* registrar_;
  std::unique_ptr<flutter::MethodChannel<flutter::EncodableValue>> channel_;
  HWND tray_window_ = nullptr;
  NOTIFYICONDATAW notify_icon_ = {};
  UINT taskbar_created_message_ = 0;
  bool tray_added_ = false;
};

}  // namespace gitaview
