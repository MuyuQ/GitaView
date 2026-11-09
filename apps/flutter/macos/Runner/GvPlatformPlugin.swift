import Cocoa
import FlutterMacOS

/// macOS 托盘 = 菜单栏状态项（NSStatusItem）：`gitaview/platform` 通道的
/// macOS 侧实现，与 Windows gv_platform.cpp 同一契约。
///
/// - ensureTray / removeTray / getTrayContext —— 注册 / 移除状态项，并按契约
///   返回图标矩形 + 所在屏幕工作区（左上原点物理像素）与缩放；
/// - 点击语义与 Windows 托盘一致：左键 → onTrayLeftClick（浮窗开合由 Dart
///   决定），右键 → 弹出系统菜单（菜单外观由系统负责），菜单项经
///   onTrayMenuAction 以整数回传 Dart（1=打开主程序 2=刷新 3=退出）；
/// - styleFlyoutWindow —— 把 desktop_multi_window fork 创建的第二窗口调成
///   浮窗形态（fork 无对应 API，见方法注释）；
/// - getSystemPaths / getStartupTaskState / openTarget 为 Windows 专属，
///   回 NotImplemented（main.dart 在 macOS 不经通道调用它们）。
final class GvPlatformPlugin: NSObject, FlutterPlugin {
  /// 菜单项动作（与 Windows gv_platform.cpp kMenuOpenMain/Refresh/Exit 一致）。
  private enum MenuAction {
    static let openMain = 1
    static let refresh = 2
    static let exit = 3
  }

  private var channel: FlutterMethodChannel?
  private var statusItem: NSStatusItem?
  private var trayMenu: NSMenu?

  // 生命周期假设：插件与进程同生命周期（channel ↔ delegate 互相持有，
  // 与 Windows gv_platform 静态 instance_ 同款）；removeTray 仅在 Dart
  // 退出序列中显式调用，不依赖 deinit。

  static func register(with registrar: FlutterPluginRegistrar) {
    let instance = GvPlatformPlugin()
    let channel = FlutterMethodChannel(
      name: "gitaview/platform", binaryMessenger: registrar.messenger)
    instance.channel = channel
    registrar.addMethodCallDelegate(instance, channel: channel)
  }

  public func handle(_ call: FlutterMethodCall, result: @escaping FlutterResult) {
    switch call.method {
    case "ensureTray":
      result(ensureTray())
    case "removeTray":
      removeTray()
      result(nil)
    case "getTrayContext":
      result(trayContext())
    case "styleFlyoutWindow":
      styleForkSubWindows()
      result(nil)
    default:
      result(FlutterMethodNotImplemented)
    }
  }

  // MARK: - 状态项生命周期

  /// 注册菜单栏状态项；重复注册幂等（Windows EnsureTray tray_added_ 同语义）。
  /// 失败（取不到按钮）返回 false，Dart 侧回退"关闭 = 退出"（与 Windows
  /// 托盘创建失败同一降级）。
  private func ensureTray() -> Bool {
    if statusItem != nil { return true }
    let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
    guard let button = item.button else {
      NSStatusBar.system.removeStatusItem(item)
      return false
    }
    if let image = NSImage(named: "MenuBarIcon") {
      button.image = image
    } else {
      // 资产缺失兜底（对应 Windows LoadImageW 失败回退 IDI_APPLICATION）：
      // 文本占位保持托盘可达（菜单含退出），不因图标缺失丢失退出入口。
      button.title = "GV"
    }
    button.toolTip = "GitaView"
    button.setAccessibilityLabel("GitaView")
    // 左键 / 右键都走 action，在 action 里按事件类型分流（Windows
    // WM_LBUTTONUP / WM_RBUTTONUP 的对应物）。
    button.target = self
    button.action = #selector(onTrayButtonClick(_:))
    button.sendAction(on: [.leftMouseUp, .rightMouseUp])
    statusItem = item
    return true
  }

  /// 状态项点击分流：左键 → onTrayLeftClick（浮窗开合）；右键 → 系统菜单。
  /// 右键用"临时挂菜单 + performClick"：菜单由系统定位到图标下方（与
  /// Windows TrackPopupMenu 锚定托盘图标一致），菜单关闭后解除挂载，恢复
  /// 左键 action 语义。
  @objc private func onTrayButtonClick(_ sender: NSStatusBarButton) {
    guard let event = NSApp.currentEvent else { return }
    if event.type == .rightMouseUp || event.type == .rightMouseDown {
      statusItem?.menu = buildMenu()
      sender.performClick(nil)
      statusItem?.menu = nil
    } else {
      channel?.invokeMethod("onTrayLeftClick", arguments: nil)
    }
  }

  private func removeTray() {
    guard let item = statusItem else { return }
    NSStatusBar.system.removeStatusItem(item)
    statusItem = nil
  }

  // MARK: - 菜单（外观由系统负责，命令含义与 Windows 托盘一致）

  /// 托盘菜单：构建一次复用（右键每次重建虽非热点，但缓存零成本；动作
  /// 经 target-action 回传，与 Windows 每次重建 TrackPopupMenu 语义一致）。
  private func buildMenu() -> NSMenu {
    if let menu = trayMenu {
      return menu
    }
    let menu = NSMenu()
    menu.addItem(trayItem("打开 GitaView", tag: MenuAction.openMain))
    menu.addItem(trayItem("刷新", tag: MenuAction.refresh))
    menu.addItem(NSMenuItem.separator())
    menu.addItem(trayItem("退出", tag: MenuAction.exit))
    trayMenu = menu
    return menu
  }

  private func trayItem(_ title: String, tag: Int) -> NSMenuItem {
    let menuItem = NSMenuItem(
      title: title, action: #selector(onMenuAction(_:)), keyEquivalent: "")
    menuItem.tag = tag
    menuItem.target = self
    return menuItem
  }

  @objc private func onMenuAction(_ sender: NSMenuItem) {
    channel?.invokeMethod("onTrayMenuAction", arguments: sender.tag)
  }

  // MARK: - 浮窗子窗口样式（desktop_multi_window fork 无对应 API）

  /// 把 fork 创建的第二窗口调成浮窗形态：隐藏标题栏按钮簇（fork
  /// showTitleBar(false) 同款手法，但不改 isOpaque / hasShadow / 背景——
  /// 浮窗需要窗口自带的圆角与投影），并浮于普通窗口之上（浮窗开在菜单栏
  /// 下方，须盖过其他应用窗口）。可缩放与可移动由 Dart 侧
  /// resizable(false) / setMovable(false) 关闭。
  ///
  /// 子窗口按运行时类名匹配：fork 的 NSWindow 子类是
  /// desktop_multi_window.FlutterWindowInner（FlutterWindow 只是包装对象）。
  /// fork 依赖钉在固定 commit，若升级后类名变化需同步这里。
  private func styleForkSubWindows() {
    for window in NSApp.windows {
      let name = NSStringFromClass(window.classForCoder)
      guard name.hasSuffix("FlutterWindowInner") else {
        continue
      }
      let close = window.standardWindowButton(.closeButton)
      close?.superview?.superview?.isHidden = true
      window.level = .floating
    }
  }

  // MARK: - 定位上下文（契约同 Windows：左上原点物理像素 + 缩放）

  /// 图标矩形与所在屏幕工作区。macOS 全局坐标以主屏左下为原点（点），换算为
  /// 契约的左上原点物理像素；scale = 图标所在屏幕 backingScaleFactor。
  /// 注：混合 DPI 多屏下跨屏位置以各矩形所在屏幕缩放换算，跨屏精确拼接
  /// 属浮窗（第二层）接入时的待办；单屏（含 Retina）为精确值。
  private func trayContext() -> [String: Any] {
    let window = statusItem?.button?.window
    let screen = window?.screen ?? NSScreen.main
    let primaryHeight = NSScreen.screens.first?.frame.height ?? 0
    let scale = screen?.backingScaleFactor ?? 1.0
    let zero: [String: Any] = ["x": 0, "y": 0, "width": 0, "height": 0]
    let icon = window.map {
      contractBounds($0.frame, primaryHeight: primaryHeight, scale: scale)
    }
    let workArea = screen.map {
      contractBounds($0.visibleFrame, primaryHeight: primaryHeight, scale: scale)
    } ?? icon ?? zero
    return [
      "icon": icon ?? zero,
      "workArea": workArea,
      "scale": scale,
      "hasIcon": window != nil,
    ]
  }

  /// 全局坐标（主屏左下原点，点）→ 左上原点物理像素（Windows 虚拟屏幕同形）。
  private func contractBounds(
    _ frame: NSRect, primaryHeight: CGFloat, scale: CGFloat
  ) -> [String: Any] {
    [
      "x": Int(round(frame.minX * scale)),
      "y": Int(round((primaryHeight - frame.maxY) * scale)),
      "width": Int(round(frame.width * scale)),
      "height": Int(round(frame.height * scale)),
    ]
  }
}
