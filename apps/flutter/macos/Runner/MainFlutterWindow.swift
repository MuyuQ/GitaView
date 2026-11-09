import Cocoa
import FlutterMacOS

class MainFlutterWindow: NSWindow {
  override func awakeFromNib() {
    let flutterViewController = FlutterViewController()
    let windowFrame = self.frame
    self.contentViewController = flutterViewController
    self.setFrame(windowFrame, display: true)

    RegisterGeneratedPlugins(registry: flutterViewController)

    // 托盘插件：gitaview/platform 通道的 macOS 侧（契约见 GvPlatformPlugin.swift）。
    // 与 pub 插件分开手动注册，保持 runner 内聚（同 Windows gv_platform 模式）。
    GvPlatformPlugin.register(
      with: flutterViewController.registrar(forPlugin: "GvPlatformPlugin"))

    super.awakeFromNib()
  }
}
