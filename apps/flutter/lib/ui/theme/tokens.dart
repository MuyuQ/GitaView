// 设计 token 表：颜色角色 / 字号 / 字体 / 间距（浅色 + 深色两套）。
//
// 语义来源：
// - 颜色值逐条对应 apps/windows/GitaView.Native/Theme/GitaViewTheme.xaml
//   的 Light / Dark ThemeDictionaries（键名 → 字段名一一对应）；
// - 字号对应 GvBrandTitle(18) / GvTitleFontSize(17) / GvBodyFontSize(14) /
//   GvCaptionFontSize(13)，另收录浮窗时间戳的 12 号（TrayFlyoutWindow）；
// - 间距 / 圆角对应 MainPage / SettingsPage XAML 中重复出现的度量档位；
// - docs/native-app-ui-design.md 第 4 节要求浅 / 深色由同一套语义角色推导，
//   状态不依赖颜色（颜色只与文字 / 数字并用）。
//
// 高对比度：C# 走 WinUI SystemColor* 主题资源；Flutter 侧由系统无障碍机制
// 处理，本表不定义第三套配色（Windows 阶段核对后如有缺口再补）。
library;

import 'package:flutter/widgets.dart';
import 'package:fluent_ui/fluent_ui.dart' show FluentTheme;

/// 颜色角色集合（一个主题一套）。字段名与 C# GvXxxBrush 键对应。
class GvColorSet {
  /// 筛选条背景（浅色 #99F2F5F9 / 深色 #99151A21）。
  final Color filterSurface;

  /// 表头背景（浅色 #B3E7EDF4 / 深色 #B3232C38）。
  final Color tableHeader;

  /// 正文（GvInkBrush）。
  final Color ink;

  /// 次要文字（GvMutedBrush）。
  final Color muted;

  /// 分隔线 / 边框（GvLineBrush）。
  final Color line;

  /// 输入等内容的底层背景（GvBgBrush）。
  final Color bg;

  /// 页面与卡片表面（GvSurfaceBrush）。
  final Color surface;

  /// 悬停层（GvHoverBrush）。
  final Color hover;

  /// 同步（GvGreenBrush）。
  final Color green;

  /// 可同步琥珀文字（GvAmberTextBrush）。
  final Color amberText;

  /// 错误与分叉（GvRedBrush）。
  final Color red;

  /// 无远端 / 待读（GvSlateBrush）。
  final Color slate;

  /// 强选中底（chip 选中态，GvSelectionBrush）。
  final Color selection;

  /// 强选中底上的文字（GvOnSelectionBrush）。
  final Color onSelection;

  /// 行选中 / 导航选中底（GvRowSelectedBrush）。
  final Color rowSelected;

  /// 浮窗整体着色（GvFlyoutTintBrush）。
  final Color flyoutTint;

  const GvColorSet({
    required this.filterSurface,
    required this.tableHeader,
    required this.ink,
    required this.muted,
    required this.line,
    required this.bg,
    required this.surface,
    required this.hover,
    required this.green,
    required this.amberText,
    required this.red,
    required this.slate,
    required this.selection,
    required this.onSelection,
    required this.rowSelected,
    required this.flyoutTint,
  });
}

/// 浅色主题（GitaViewTheme.xaml Light 逐条对应）。
const GvColorSet gvLightColors = GvColorSet(
  filterSurface: Color(0x99F2F5F9),
  tableHeader: Color(0xB3E7EDF4),
  ink: Color(0xFF202733),
  muted: Color(0xFF445164),
  line: Color(0xFFD5DDE7),
  bg: Color(0x66F7F9FC),
  surface: Color(0xE6FFFFFF),
  hover: Color(0x66E7EDF4),
  green: Color(0xFF237B50),
  amberText: Color(0xFF8A5A0A),
  red: Color(0xFFB03D30),
  slate: Color(0xFF64748B),
  selection: Color(0xFF202733),
  onSelection: Color(0xFFFFFFFF),
  rowSelected: Color(0x73E7EDF4),
  flyoutTint: Color(0xF2F7F9FC),
);

/// 深色主题（GitaViewTheme.xaml Dark 逐条对应）。
const GvColorSet gvDarkColors = GvColorSet(
  filterSurface: Color(0x99151A21),
  tableHeader: Color(0xB3232C38),
  ink: Color(0xFFEDF0F5),
  muted: Color(0xFFBDC6D4),
  line: Color(0xFF3C4757),
  bg: Color(0x590F1318),
  surface: Color(0xD9191E26),
  hover: Color(0x66252D38),
  green: Color(0xFF75CE9D),
  amberText: Color(0xFFEDC078),
  red: Color(0xFFFF9E91),
  slate: Color(0xFFBDC6D4),
  selection: Color(0xFFDCE5F2),
  onSelection: Color(0xFF15191F),
  rowSelected: Color(0x73252D38),
  flyoutTint: Color(0xE615191F),
);

/// 按当前 Fluent 主题取颜色角色集合。
GvColorSet gvColorsOf(BuildContext context) =>
    FluentTheme.of(context).brightness == Brightness.dark
        ? gvDarkColors
        : gvLightColors;

/// 字号角色（GitaViewTheme.xaml 的 x:Double 资源）。
abstract final class GvFontSizes {
  /// 品牌标题（GvBrandTitle）。
  static const double brand = 18;

  /// 页面标题（GvTitleFontSize）。
  static const double title = 17;

  /// 正文 / 数据（GvBodyFontSize）。
  static const double body = 14;

  /// 辅助信息（GvCaptionFontSize）。
  static const double caption = 13;

  /// 浮窗时间戳（TrayFlyoutWindow FlyoutRefreshTime）。
  static const double micro = 12;
}

/// 字体族。Windows 用产品字体（Microsoft YaHei UI / Cascadia Mono）；
/// 其他平台退回系统可用的中文与等宽字体（ui-design 第 4 节允许）。
abstract final class GvFonts {
  static const String ui = 'Microsoft YaHei UI';
  static const List<String> uiFallback = ['PingFang SC', 'Microsoft YaHei'];
  static const String mono = 'Cascadia Mono';
  static const List<String> monoFallback = ['Menlo', 'Consolas'];
}

/// 常用文本样式工厂（角色 + 颜色组合的最小集合，页面直接取用）。
abstract final class GvTextStyles {
  static const FontWeight medium = FontWeight.w500;
  static const FontWeight semiBold = FontWeight.w600;

  /// 品牌标题（GvBrandTitle：18 / SemiBold / ink）。
  static TextStyle brand(Color ink) => TextStyle(
        fontSize: GvFontSizes.brand,
        fontWeight: semiBold,
        color: ink,
        fontFamily: GvFonts.ui,
        fontFamilyFallback: GvFonts.uiFallback,
      );

  /// 正文。
  static TextStyle body(Color color, {FontWeight weight = FontWeight.w400}) =>
      TextStyle(
        fontSize: GvFontSizes.body,
        fontWeight: weight,
        color: color,
        fontFamily: GvFonts.ui,
        fontFamilyFallback: GvFonts.uiFallback,
      );

  /// 辅助信息。
  static TextStyle caption(Color color) => TextStyle(
        fontSize: GvFontSizes.caption,
        color: color,
        fontFamily: GvFonts.ui,
        fontFamilyFallback: GvFonts.uiFallback,
      );

  /// 等宽数据（分支 / 提交差异；ui-design：数字与分支使用等宽字体角色）。
  static TextStyle mono(Color color, {double size = GvFontSizes.body}) =>
      TextStyle(
        fontSize: size,
        color: color,
        fontFamily: GvFonts.mono,
        fontFamilyFallback: GvFonts.monoFallback,
      );
}

/// 间距 / 圆角档位（MainPage / SettingsPage XAML 的重复度量）。
abstract final class GvSpacing {
  /// 筛选条与工具栏的左右边界（MainPage Padding="18,…"）。
  static const double page = 18;

  /// 表格区内边距（TableHeader Padding="24,10"）。
  static const double table = 24;

  /// 卡片与控件的标准内边距。
  static const double content = 16;

  /// 控件间距小档。
  static const double small = 8;

  /// 控件间距中档（按钮组 / 行内间距 6–9）。
  static const double medium = 9;

  /// 控件间距大档（分区之间 18–20）。
  static const double large = 18;

  /// 分区标题与内容的次级间距（SettingsPage Spacing 12）。
  static const double section = 12;

  /// 通用圆角（GvButton CornerRadius=8、行卡片 6–8）。
  static const double radius = 8;

  /// 工具栏高度（TitleBar Height=56）。
  static const double toolbarHeight = 56;

  /// 按钮最小高度（GvButton MinHeight=34）。
  static const double buttonMinHeight = 34;

  /// chip 最小高度（GvChipStyle MinHeight=30）。
  static const double chipMinHeight = 30;
}
