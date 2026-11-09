import Cocoa
import FlutterMacOS

@main
class AppDelegate: FlutterAppDelegate {
  override func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
    return true
  }

  // 托盘驻留期间主窗口可隐藏（关闭 = 隐藏驻留）：Dock 图标点击恢复主窗口，
  // 与"打开 GitaView"托盘菜单动作同一效果。仅在没有可见窗口时处理，
  // 避免打断正常的窗口切换。
  override func applicationShouldHandleReopen(
    _ sender: NSApplication, hasVisibleWindows flag: Bool
  ) -> Bool {
    if !flag {
      for window in sender.windows where window is MainFlutterWindow {
        window.makeKeyAndOrderFront(self)
        if #available(macOS 14.0, *) {
          sender.activate()
        } else {
          sender.activate(ignoringOtherApps: true)
        }
        break
      }
    }
    return true
  }

  override func applicationSupportsSecureRestorableState(_ app: NSApplication) -> Bool {
    return true
  }
}
