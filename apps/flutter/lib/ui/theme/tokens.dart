// 设计 token 表：颜色角色 / 字号 / 字体 / 间距 / 品牌图形（浅色 + 深色两套）。
//
// 语义来源（2026-11 界面重设计，见 docs/flutter-ui-redesign-plan.md）：
// - 颜色取值唯一来源是定稿设计稿 design/preview/index.html 的 :root（浅色）
//   与 html[data-theme="dark"]（深色）两段；不再逐条对应
//   apps/windows/GitaView.Native/Theme/GitaViewTheme.xaml（WinUI 宿主尚未
//   同步本次语义变更，见 docs/native-platform-acceptance.md 待办）；
// - teal / blue / tint / tintInk / ink2 / lineSoft 为 Flutter 侧新增角色；
// - amber 语义由"可同步"收窄为"分叉"（原 amberText 改名 amber；本地领先改
//   teal、远端领先改 blue，分叉不再是红色）；red 收窄为"读取失败"；
// - bg / surface / hover / filterSurface / tableHeader / rowSelected /
//   flyoutTint 由半透明改为不透明：设计稿是实心表面，半透明底会让表头等
//   区域的对比度随桌面壁纸漂移。副作用是 Windows 丙烯 / mica 材质在窗口内
//   不再可见——这是刻意决定，勿"顺手"改回半透明；
// - 字号：品牌 17 / 页面标题 17 / 正文 14 / 辅助 13 / 表头与状态栏 12 /
//   小字 11.5 / 差异数字 13.5；docs/native-app-ui-design.md §8.2 的下限
//   （主数据 ≥13）通过把设计稿 12 / 12.5 的路径、分组、分支列取 13 落实；
// - 状态不依赖颜色（颜色始终与文字 / 数字并用）——
//   docs/native-app-ui-design.md 第 4 节的要求继续成立。
//
// 高对比度：C# 走 WinUI SystemColor* 主题资源；Flutter 侧由系统无障碍机制
// 处理，本表不定义第三套配色（Windows 阶段核对后如有缺口再补）。
library;

import 'package:flutter/widgets.dart';
import 'package:fluent_ui/fluent_ui.dart' show FluentTheme;

/// 颜色角色集合（一个主题一套）。
class GvColorSet {
  /// 正文。
  final Color ink;

  /// 二级正文（展开面板元信息、确认框说明等）。
  final Color ink2;

  /// 次要文字。
  final Color muted;

  /// 分隔线 / 边框。
  final Color line;

  /// 弱分隔线（行分隔、面板下缘等）。
  final Color lineSoft;

  /// 输入等内容的底层背景。
  final Color bg;

  /// 页面与卡片表面（不透明，见文件头说明）。
  final Color surface;

  /// 悬停层（不透明，使用处按设计稿再叠透明度）。
  final Color hover;

  /// 筛选条 / 侧栏等次级表面（不透明；设计稿以 surface-soft 混合实现，
  /// 本表用同一取值的独立角色承载）。
  final Color filterSurface;

  /// 表头背景（不透明；表头实际底色为 surface 92% + 本值的混合）。
  final Color tableHeader;

  /// 行选中 / 导航选中底。
  final Color rowSelected;

  /// 同步。
  final Color green;

  /// 本地领先。
  final Color teal;

  /// 远端领先。
  final Color blue;

  /// 分叉（原 amberText；语义由"可同步"收窄为"分叉"）。
  final Color amber;

  /// 读取失败（分叉不再使用红色）。
  final Color red;

  /// 未比较 / 无远端 / 待读。
  final Color slate;

  /// 品牌强调（焦点环、展开指示条、动作文字的底色来源）。
  final Color tint;

  /// 强调底色上的文字（Pull / Push 动作文字色）。
  final Color tintInk;

  /// 琥珀实底上的文字（浅色白 / 深色近黑；当前界面无实底琥珀元素，预留）。
  final Color amberOnPill;

  /// 强选中底（chip 选中态、主按钮）。
  final Color selection;

  /// 强选中底上的文字。
  final Color onSelection;

  /// 浮窗整体着色（不透明，见文件头说明）。
  final Color flyoutTint;

  const GvColorSet({
    required this.ink,
    required this.ink2,
    required this.muted,
    required this.line,
    required this.lineSoft,
    required this.bg,
    required this.surface,
    required this.hover,
    required this.filterSurface,
    required this.tableHeader,
    required this.rowSelected,
    required this.green,
    required this.teal,
    required this.blue,
    required this.amber,
    required this.red,
    required this.slate,
    required this.tint,
    required this.tintInk,
    required this.amberOnPill,
    required this.selection,
    required this.onSelection,
    required this.flyoutTint,
  });
}

/// 浅色主题（design/preview/index.html :root 逐条对应）。
const GvColorSet gvLightColors = GvColorSet(
  ink: Color(0xFF202733),
  ink2: Color(0xFF3F4B5C),
  muted: Color(0xFF54617A),
  line: Color(0xFFD5DDE7),
  lineSoft: Color(0xFFE6ECF2),
  bg: Color(0xFFF7F9FC),
  surface: Color(0xFFFFFFFF),
  hover: Color(0xFFE7EDF4),
  filterSurface: Color(0xFFF4F7FA),
  tableHeader: Color(0xFFEEF3F8),
  rowSelected: Color(0xFFEAF1F7),
  green: Color(0xFF0F7A56),
  teal: Color(0xFF0E7490),
  blue: Color(0xFF2F6BD0),
  amber: Color(0xFFB45309),
  red: Color(0xFFC2332C),
  slate: Color(0xFF64748B),
  tint: Color(0xFF7CE2B6),
  tintInk: Color(0xFF146C4C),
  amberOnPill: Color(0xFFFFFFFF),
  selection: Color(0xFF202733),
  onSelection: Color(0xFFFFFFFF),
  flyoutTint: Color(0xFFF7F9FC),
);

/// 深色主题（design/preview/index.html data-theme="dark" 逐条对应）。
const GvColorSet gvDarkColors = GvColorSet(
  ink: Color(0xFFEDF0F5),
  ink2: Color(0xFFC6CFDB),
  muted: Color(0xFF98A5B6),
  line: Color(0xFF3C4757),
  lineSoft: Color(0xFF293240),
  bg: Color(0xFF0F1318),
  surface: Color(0xFF191E26),
  hover: Color(0xFF252D38),
  filterSurface: Color(0xFF151A21),
  tableHeader: Color(0xFF1D242E),
  rowSelected: Color(0xFF232B36),
  green: Color(0xFF46D69D),
  teal: Color(0xFF67D4E0),
  blue: Color(0xFF7FA8F0),
  amber: Color(0xFFF2B45E),
  red: Color(0xFFFF7B72),
  slate: Color(0xFFA6B2C3),
  tint: Color(0xFF7CE2B6),
  tintInk: Color(0xFF9DEFC9),
  amberOnPill: Color(0xFF241703),
  selection: Color(0xFFDCE5F2),
  onSelection: Color(0xFF15191F),
  flyoutTint: Color(0xFF15191F),
);

/// 按当前 Fluent 主题取颜色角色集合。
GvColorSet gvColorsOf(BuildContext context) =>
    FluentTheme.of(context).brightness == Brightness.dark
        ? gvDarkColors
        : gvLightColors;

/// 设计稿 `--tint-soft`：tint 的低透明度底（浅色 18% / 深色 12%），
/// 用于搜索聚焦环与操作确认框底色。
Color gvTintSoftOf(GvColorSet colors, Brightness brightness) =>
    colors.tint.withValues(alpha: brightness == Brightness.dark ? 0.12 : 0.18);

/// 字号角色。brand 与 title 同为 17（设计稿 brand-name 17px / sec 17px）。
abstract final class GvFontSizes {
  /// 品牌名称（设计稿 brand-name）。
  static const double brand = 17;

  /// 页面标题（设置标题、空态标题）。
  static const double title = 17;

  /// 正文 / 数据。
  static const double body = 14;

  /// 辅助信息（说明文字、路径 / 分组 / 分支列）。
  static const double caption = 13;

  /// 表头 / 状态栏 / 状态胶囊。
  static const double micro = 12;

  /// 小字（副标题、计数、浮窗标签）。
  static const double small = 11.5;

  /// 差异数字（等宽）。
  static const double numeric = 13.5;
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

  /// 品牌名称（17 / SemiBold / ink）。
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

  /// 辅助信息（13）。
  static TextStyle caption(Color color) => TextStyle(
        fontSize: GvFontSizes.caption,
        color: color,
        fontFamily: GvFonts.ui,
        fontFamilyFallback: GvFonts.uiFallback,
      );

  /// 表头标签（12 + 0.05em 字距，对应设计稿 table-head）。
  static TextStyle headerLabel(Color color) => TextStyle(
        fontSize: GvFontSizes.micro,
        letterSpacing: 0.6,
        color: color,
        fontFamily: GvFonts.ui,
        fontFamilyFallback: GvFonts.uiFallback,
      );

  /// 状态胶囊文字（12 / w500）。
  static TextStyle pill(Color color) => TextStyle(
        fontSize: GvFontSizes.micro,
        fontWeight: medium,
        color: color,
        fontFamily: GvFonts.ui,
        fontFamilyFallback: GvFonts.uiFallback,
      );

  /// 小字（11.5）。
  static TextStyle subtitle(Color color) => TextStyle(
        fontSize: GvFontSizes.small,
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

/// 间距 / 几何档位。
abstract final class GvSpacing {
  /// 主内容左右边界（设计稿工具栏 / 筛选条 / 表格 / 状态栏的 22px）。
  static const double page = 22;

  /// 表格区内边距（与 page 一致；独立命名便于表格布局引用）。
  static const double table = 22;

  /// 卡片与控件的标准内边距。
  static const double content = 16;

  /// 控件间距小档。
  static const double small = 8;

  /// 控件间距中档（按钮组 / 行内间距 6–9）。
  static const double medium = 9;

  /// 控件间距大档（分区之间 18–20）。
  static const double large = 18;

  /// macOS 沉浸式标题栏红绿灯占位宽度：系统按钮组右缘实测约 61，
  /// 再留约 7 的呼吸间隙即贴合红绿灯簇；78 会让品牌被推得太靠右、显得悬空。
  static const double macTrafficLightsClearance = 68;

  /// 分区标题与内容的次级间距。
  static const double section = 12;

  /// 通用圆角（按钮 / 行卡片 7–8）。
  static const double radius = 8;

  /// 主界面工具栏高度（设计稿 topbar 66）。
  static const double toolbarHeightMain = 66;

  /// 设置页标题栏高度（沿用原 56 档；设计稿 66，以实施方案为准）。
  static const double toolbarHeight = 56;

  /// 按钮最小高度（空态按钮 / 安静按钮 34）。
  static const double buttonMinHeight = 34;

  /// 筛选 chip 高度（设计稿 29）。
  static const double chipMinHeight = 29;
}

/// 品牌图形（assets/brand/gitaview.svg 的 CustomPaint 复刻：深色圆角底板、
/// 浅色 Git 分支线、薄荷绿节点）。颜色为品牌固定值，不随主题变化；
/// 不可引入 SVG 依赖，故在 token 层以矢量绘制共用（工具栏 / 空态 /
/// 设置侧栏 / 浮窗）。
class GvBrandMark extends StatelessWidget {
  final double size;

  const GvBrandMark({super.key, required this.size});

  @override
  Widget build(BuildContext context) {
    return CustomPaint(
      size: Size.square(size),
      painter: _GvBrandMarkPainter(),
    );
  }
}

class _GvBrandMarkPainter extends CustomPainter {
  static const Color _plate = Color(0xFF202733);
  static const Color _stroke = Color(0xFFF7F9FC);
  static const Color _node = Color(0xFF7CE2B6);

  @override
  void paint(Canvas canvas, Size size) {
    final double s = size.width / 128;
    canvas.drawRRect(
      RRect.fromRectAndRadius(
        Rect.fromLTWH(4 * s, 4 * s, 120 * s, 120 * s),
        Radius.circular(28 * s),
      ),
      Paint()..color = _plate,
    );
    final Paint stroke = Paint()
      ..color = _stroke
      ..style = PaintingStyle.stroke
      ..strokeWidth = 9 * s
      ..strokeCap = StrokeCap.round
      ..strokeJoin = StrokeJoin.round;
    final Path path = Path()
      ..moveTo(46 * s, 37 * s)
      ..lineTo(46 * s, 91 * s)
      ..moveTo(82 * s, 37 * s)
      ..lineTo(82 * s, 49 * s)
      ..cubicTo(82 * s, 69 * s, 46 * s, 61 * s, 46 * s, 81 * s);
    canvas.drawPath(path, stroke);
    void circle(double x, double y, Color color) =>
        canvas.drawCircle(Offset(x * s, y * s), 11 * s, Paint()..color = color);
    circle(46, 34, _stroke);
    circle(46, 94, _stroke);
    circle(82, 34, _node);
  }

  @override
  bool shouldRepaint(_GvBrandMarkPainter oldDelegate) => false;
}
