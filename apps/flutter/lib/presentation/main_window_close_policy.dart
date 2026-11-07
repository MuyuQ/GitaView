// 主窗口关闭策略（托盘隐藏 vs 优雅退出）。
//
// 语义来源：apps/windows/GitaView.Native/MainWindowClosePolicy.cs
// （C# 逐条对应移植）。
// - 已在退出流程中（exitRequested）时忽略后续关闭：现有退出流程正在停止
//   引擎，第二次关闭不得绕过那次清理；
// - 托盘可用时关闭 = 隐藏窗口（保持引擎与后台扫描存活）；
// - 无托盘时关闭 = 请求优雅退出（由调用方置位 exitRequested 并停止引擎）。
// C# 的 XAML Window 不移植：hide / requestExit 是宿主注入的回调。
library;

/// 主窗口关闭策略（C# 静态类的对应物）。
abstract final class MainWindowClosePolicy {
  /// 处理一次关闭请求。回调与 C# `Action hide, Action requestExit` 一一对应。
  static void handle({
    required bool exitRequested,
    required bool trayAvailable,
    required void Function() hide,
    required void Function() requestExit,
  }) {
    // 退出流程进行中：保持窗口存活直到现有流程完成引擎停止，二次关闭不得
    // 绕过那次清理。
    if (exitRequested) return;
    if (trayAvailable) {
      hide();
    } else {
      requestExit();
    }
  }
}
