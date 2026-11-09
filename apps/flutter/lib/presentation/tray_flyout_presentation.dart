// 托盘浮窗展示逻辑（空态文案 + 定位）。
//
// 语义来源：apps/windows/GitaView.Native/TrayFlyoutPresentation.cs（C# 逐条对应移植）。
// - [TrayFlyoutPresentation.emptyMessage] 的文案分支顺序即优先级：
//   刷新失败 > 无仓库 > 读取中 > 全部同步 > 有 no_remote > 默认；
// - [TrayFlyoutPresentation.position] 把浮窗锚定到托盘图标（屏幕物理像素），
//   上方放不下时翻转到下方，无锚点时退回工作区右下角，最后钳制在工作区内。
// C# 的 Win32 显示代码不在移植范围；宿主拿到坐标后自行调用原生窗口 API。
library;

import 'dart:math' as math;

import '../contracts/contracts.dart';

/// 屏幕物理像素矩形（C# FlyoutBounds record struct 的对应物）。
class FlyoutBounds {
  final int x;
  final int y;
  final int width;
  final int height;

  const FlyoutBounds(this.x, this.y, this.width, this.height);

  @override
  bool operator ==(Object other) =>
      other is FlyoutBounds &&
      other.x == x &&
      other.y == y &&
      other.width == width &&
      other.height == height;

  @override
  int get hashCode => Object.hash(x, y, width, height);

  @override
  String toString() => 'FlyoutBounds($x, $y, $width, $height)';
}

/// 托盘浮窗展示逻辑（C# 静态类的对应物）。
abstract final class TrayFlyoutPresentation {
  /// 浮窗空态文案。所有分支都必须给出可操作或可理解的中文说明，
  /// 不依赖颜色表达状态。
  static String emptyMessage(SummaryCounts counts, RefreshState refreshState) {
    if (refreshState == RefreshState.failed) {
      return '刷新未完成，打开 GitaView 查看详情或重试。';
    }
    if (counts.total == 0) {
      return '点击下方「打开 GitaView」，添加或扫描仓库。';
    }
    if (counts.pending > 0) {
      return '正在读取仓库状态…';
    }
    if (counts.synced == counts.total) {
      return '所有仓库均已同步。';
    }
    return counts.noRemote > 0
        ? '暂无待同步或需关注的仓库。部分仓库缺少远端比较条件。'
        : '暂无待同步或需关注的仓库。';
  }

  /// 计算浮窗左上角坐标（屏幕物理像素；工作区由托盘图标所在显示器提供）。
  ///
  /// 对应 C# `Position(FlyoutBounds, FlyoutBounds?, int, int, double)`：
  /// - 有锚点时右上对齐托盘图标并留 [gap]；若图标上方空间不足且下方放得下，
  ///   则翻转到图标下方；
  /// - 无锚点时退回工作区右下角并留 [fallbackGap]；
  /// - 结果钳制在工作区内（四边留 [margin]）。
  static ({int x, int y}) position(
    FlyoutBounds workArea,
    FlyoutBounds? anchor,
    int width,
    int height,
    double scale,
  ) {
    final int margin = _scaleRound(4 * scale);
    final int gap = _scaleRound(8 * scale);
    final int fallbackGap = _scaleRound(12 * scale);
    int x;
    int y;
    if (anchor != null) {
      x = anchor.x + anchor.width - width;
      y = anchor.y - height - gap;
      if (y < workArea.y &&
          anchor.y + anchor.height + gap + height <=
              workArea.y + workArea.height) {
        y = anchor.y + anchor.height + gap;
      }
    } else {
      x = workArea.x + workArea.width - width - fallbackGap;
      y = workArea.y + workArea.height - height - fallbackGap;
    }
    final int minX = workArea.x + margin;
    final int minY = workArea.y + margin;
    return (
      x: _clamp(x, minX, math.max(minX, workArea.x + workArea.width - width - margin)),
      y: _clamp(y, minY, math.max(minY, workArea.y + workArea.height - height - margin)),
    );
  }

  /// C# `Math.Round(double)` 默认为中点舍入到偶数（MidpointRounding.ToEven），
  /// Dart 的 `double.round()` 是远离零；DPI 缩放系数可能出现中点值，
  /// 这里显式对齐 C# 语义。
  static int _scaleRound(double value) {
    final double floor = value.floorToDouble();
    final double diff = value - floor;
    if (diff > 0.5) return floor.toInt() + 1;
    if (diff < 0.5) return floor.toInt();
    final int integral = floor.toInt();
    return integral.isEven ? integral : integral + 1;
  }

  /// 物理像素定位结果 → 宿主窗口 API 坐标（含宽高）。
  ///
  /// Windows：物理像素原样（Win32 MoveWindow 语义）；macOS：desktop_multi_window
  /// fork 的 setFrame/getFrame 以左上原点逻辑点为单位，按所在屏缩放除回。
  /// [position] 是 [TrayFlyoutPresentation.position] 的返回值；
  /// [width] / [height] 是估算的物理宽高。
  static ({double x, double y, double width, double height}) nativeFrame({
    required ({int x, int y}) position,
    required int width,
    required int height,
    required double scale,
    required bool logicalUnits,
  }) {
    if (!logicalUnits) {
      return (
        x: position.x.toDouble(),
        y: position.y.toDouble(),
        width: width.toDouble(),
        height: height.toDouble(),
      );
    }
    return (
      x: position.x / scale,
      y: position.y / scale,
      width: width / scale,
      height: height / scale,
    );
  }

  /// C# `Math.Clamp(int, int, int)`；调用方保证 lower <= upper（与 C# 相同，
  /// 由 `Math.Max` 下界保护保证）。
  static int _clamp(int value, int lower, int upper) {
    if (value < lower) return lower;
    if (value > upper) return upper;
    return value;
  }
}
