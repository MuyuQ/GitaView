// 主状态页：工具栏（品牌 + 搜索 + 刷新 / 设置）+ 两层筛选 chips + 仓库表格
// （sticky 表头 + ListView.builder 行虚拟化，6 列）+ 选中行操作面板 + 四态
// （骨架屏 / 空库 / 无匹配 / 刷新失败）+ 状态栏。
//
// 视觉规格：design/preview/index.html（docs/flutter-ui-redesign-plan.md 落地）：
// - 工具栏高 66：品牌（30px 标志 + 名称 / 副标题）+ 搜索（⌘K / Ctrl+K 平台化）
//   + 刷新 + 设置；Windows 下工具栏同时是窗口拖拽区，右端附加系统标题栏按钮；
// - 两层筛选：分组（首项“全部分组”）与状态（计数为 0 的状态不出现，
//   no_remote 最后），chips 复用 RepositoryFilterPresentation，文案 = 名称 + 计数；
// - 表格 6 列：仓库（名称 + 路径双行，提示并入第二行）/ 分组 / 分支 /
//   状态胶囊（点 + statusLabel）/ 差异 / 展开箭头——与
//   docs/native-app-ui-design.md §3 的 7 列约定是有意偏差（信息不丢失）；
// - 状态色映射（跨文件一致）：同步 green / 本地领先 teal / 远端领先 blue /
//   分叉 amber / 读取失败 red / 未比较与无远端 slate；
// - 选中行展开操作面板：元信息行 / 操作行（Pull / Push 两段式确认，动作文字
//   用 tintInk）/ 确认框 / 结果行四层；窄宽度换行不推挤其他列；
// - 快捷键：⌘K（macOS）/ Ctrl+K（Windows）聚焦搜索，Esc 取消确认。
library;

import 'dart:io' show Platform;
import 'dart:math' as math;

import 'package:fluent_ui/fluent_ui.dart';
import 'package:flutter/services.dart' show
    HardwareKeyboard,
    KeyDownEvent,
    KeyRepeatEvent,
    LogicalKeyboardKey;
import 'package:window_manager/legacy.dart' show DragToMoveArea;

import '../app_controller.dart';
import '../contracts/contracts.dart' show RelationFilter;
// FilterChip 与 fluent_ui（material_ui）重名：用别名引用纯逻辑层的 chip 模型。
import '../presentation/repository_filter_presentation.dart' as filter;
import 'theme/tokens.dart';

/// 搜索快捷键标签（平台化：macOS ⌘K / Windows Ctrl+K）。
String get _searchShortcutLabel => Platform.isMacOS ? '⌘K' : 'Ctrl+K';

/// 主状态页。
class MainPage extends StatelessWidget {
  final AppController controller;

  /// 打开设置（C# App.MainWindow.OpenSettings）。
  final VoidCallback onOpenSettings;

  /// Windows 沉浸式标题栏：true 时工具栏作为拖拽区（对应 TitleBar.IsDragRegion）。
  final bool dragToolbar;

  /// 沉浸式标题栏右端的系统窗口按钮（WindowCaption；仅 Windows 接线）。
  final Widget? captionBar;

  const MainPage({
    super.key,
    required this.controller,
    required this.onOpenSettings,
    this.dragToolbar = false,
    this.captionBar,
  });

  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
      animation: controller,
      builder: (context, _) => _MainPageBody(
        controller: controller,
        onOpenSettings: onOpenSettings,
        dragToolbar: dragToolbar,
        captionBar: captionBar,
      ),
    );
  }
}

class _MainPageBody extends StatefulWidget {
  final AppController controller;
  final VoidCallback onOpenSettings;
  final bool dragToolbar;
  final Widget? captionBar;

  const _MainPageBody({
    required this.controller,
    required this.onOpenSettings,
    required this.dragToolbar,
    this.captionBar,
  });

  @override
  State<_MainPageBody> createState() => _MainPageBodyState();
}

class _MainPageBodyState extends State<_MainPageBody> {
  final FocusNode _shortcutFocus = FocusNode();
  final GlobalKey<_SearchBoxState> _searchKey = GlobalKey<_SearchBoxState>();

  @override
  void dispose() {
    _shortcutFocus.dispose();
    super.dispose();
  }

  /// 页面级快捷键：⌘K / Ctrl+K 聚焦搜索；Esc 取消当前确认（状态栏同文案）。
  void _handleKey(KeyEvent event) {
    if (event is KeyDownEvent || event is KeyRepeatEvent) {
      if (event.logicalKey == LogicalKeyboardKey.keyK) {
        final shortcutPressed = Platform.isMacOS
            ? HardwareKeyboard.instance.isMetaPressed
            : HardwareKeyboard.instance.isControlPressed;
        if (shortcutPressed) {
          _searchKey.currentState?.focusField();
        }
      } else if (event.logicalKey == LogicalKeyboardKey.escape) {
        final selectedId = widget.controller.selectedRepoId;
        if (selectedId != null) {
          for (final row in widget.controller.rows) {
            if (row.id == selectedId && row.confirmVisible) {
              widget.controller.cancelConfirmation(row);
            }
          }
        }
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final colors = gvColorsOf(context);
    final controller = widget.controller;
    return KeyboardListener(
      focusNode: _shortcutFocus,
      autofocus: true,
      onKeyEvent: _handleKey,
      child: Column(
        children: [
          _Toolbar(
            controller: controller,
            onOpenSettings: widget.onOpenSettings,
            dragEnabled: widget.dragToolbar,
            captionBar: widget.captionBar,
            searchKey: _searchKey,
          ),
          if (controller.startupRecovered)
            InfoBar(
              title: const Text('配置已恢复为默认值'),
              content: const Text(
                  '原设置文件损坏或无法识别，已隔离保留。当前使用默认设置，请重新添加仓库。'),
              severity: InfoBarSeverity.warning,
            ),
          // 两层筛选共用一个面（设计稿 filter-deck：surface 底 + lineSoft 下缘）。
          Container(
            width: double.infinity,
            padding: const EdgeInsets.fromLTRB(
                GvSpacing.page, 13, GvSpacing.page, 12),
            decoration: BoxDecoration(
              color: colors.surface,
              border: Border(
                  bottom: BorderSide(color: colors.lineSoft)),
            ),
            child: Column(
              children: [
                _FilterRow<String?>(
                  label: '分组',
                  chips: controller.groupChips,
                  onSelected: (chip) => controller.selectGroup(chip.value),
                ),
                const SizedBox(height: 7),
                _FilterRow<RelationFilter>(
                  label: '状态',
                  chips: controller.statusChips,
                  onSelected: (chip) => controller.selectRelation(chip.value),
                ),
              ],
            ),
          ),
          Expanded(
              child: _ContentArea(
                  controller: controller,
                  onOpenSettings: widget.onOpenSettings)),
          _StatusBar(controller: controller),
        ],
      ),
    );
  }
}

// —— 工具栏（设计稿 topbar：高 66，品牌 + 搜索 + 刷新 / 设置） ——

class _Toolbar extends StatelessWidget {
  final AppController controller;
  final VoidCallback onOpenSettings;
  final bool dragEnabled;
  final Widget? captionBar;
  final GlobalKey<_SearchBoxState> searchKey;

  const _Toolbar({
    required this.controller,
    required this.onOpenSettings,
    required this.dragEnabled,
    required this.searchKey,
    this.captionBar,
  });

  @override
  Widget build(BuildContext context) {
    final colors = gvColorsOf(context);
    // macOS 沉浸式标题栏：左端为系统红绿灯保留空间。实测三枚按钮右缘约
    // 61 逻辑像素，既不能压进红绿灯（48 会顶到绿灯下方），也不宜离得太远
    // （78 显得悬空），故留 68。拖拽区从品牌之后开始。
    final leading = dragEnabled && Platform.isMacOS
        ? GvSpacing.macTrafficLightsClearance
        : GvSpacing.page;
    Widget toolbar = Container(
      height: GvSpacing.toolbarHeightMain,
      color: colors.surface,
      padding: EdgeInsets.only(
          left: leading, right: captionBar == null ? GvSpacing.page : 4),
      child: Row(
        children: [
          // 品牌图形相对两行文字（17 + 11.5）略作下移：行高让文字墨迹整体
          // 偏下，几何居中时图形会显得偏高，下移约 4 后与文字视觉居中。
          const Padding(
            padding: EdgeInsets.only(top: 8),
            child: GvBrandMark(size: 30),
          ),
          const SizedBox(width: 11),
          Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              Text('GitaView', style: GvTextStyles.brand(colors.ink)),
              const SizedBox(height: 2),
              Text('本地仓库状态台', style: GvTextStyles.subtitle(colors.muted)),
            ],
          ),
          Expanded(
            child: Center(
              // 左右各留一档间距：窄窗下搜索框会填满中间剩余宽度，
              // 否则边框会贴住品牌副标题（640 宽最小窗口时实测 ≈0）。
              child: Padding(
                padding:
                    const EdgeInsets.symmetric(horizontal: GvSpacing.content),
                child: _SearchBox(
                  key: searchKey,
                  controller: controller,
                ),
              ),
            ),
          ),
          const SizedBox(width: GvSpacing.small),
          _RefreshButton(controller: controller),
          _IconButton(
            icon: FluentIcons.settings,
            semanticLabel: '设置',
            onPressed: onOpenSettings,
          ),
          ?captionBar,
        ],
      ),
    );
    if (!dragEnabled) return toolbar;
    return DragToMoveArea(child: toolbar);
  }
}

/// 刷新按钮（设计稿 btn-quiet：透明底、悬停浅层、忙态“刷新中”+ 图标旋转）。
class _RefreshButton extends StatelessWidget {
  final AppController controller;

  const _RefreshButton({required this.controller});

  @override
  Widget build(BuildContext context) {
    return HoverButton(
      onPressed: controller.refreshBusy ? null : () => controller.refresh(),
      builder: (context, states) {
        final colors = gvColorsOf(context);
        final busy = controller.refreshBusy;
        final hovered = states.contains(WidgetState.hovered) && !busy;
        final foreground = hovered ? colors.ink2 : colors.muted;
        return Opacity(
          opacity: busy ? 0.55 : 1,
          child: Container(
            height: 34,
            padding: const EdgeInsets.symmetric(horizontal: 12),
            alignment: AlignmentDirectional.center,
            decoration: BoxDecoration(
              color: hovered ? colors.hover.withValues(alpha: 0.55) : null,
              borderRadius: BorderRadius.circular(GvSpacing.radius),
            ),
            child: Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                if (busy)
                  const _SpinningIcon()
                else
                  Icon(FluentIcons.refresh, size: 16, color: foreground),
                const SizedBox(width: 8),
                Text(busy ? '刷新中' : '刷新',
                    style: GvTextStyles.body(foreground)
                        .copyWith(fontSize: 13)),
              ],
            ),
          ),
        );
      },
    );
  }
}

/// 忙态旋转图标（设计稿 spin 动画；系统减少动效时静止）。
class _SpinningIcon extends StatefulWidget {
  const _SpinningIcon();

  @override
  State<_SpinningIcon> createState() => _SpinningIconState();
}

class _SpinningIconState extends State<_SpinningIcon>
    with SingleTickerProviderStateMixin {
  late final AnimationController _controller = AnimationController(
    vsync: this,
    duration: const Duration(seconds: 1),
  )..repeat();

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final colors = gvColorsOf(context);
    if (MediaQuery.disableAnimationsOf(context)) {
      return Icon(FluentIcons.refresh, size: 16, color: colors.muted);
    }
    return RotationTransition(
      turns: _controller,
      child: Icon(FluentIcons.refresh, size: 16, color: colors.muted),
    );
  }
}

/// 图标按钮（设计稿 icon-btn：34×34、悬停浅层，不移动内容）。
class _IconButton extends StatelessWidget {
  final IconData icon;
  final String semanticLabel;
  final VoidCallback onPressed;

  const _IconButton({
    required this.icon,
    required this.semanticLabel,
    required this.onPressed,
  });

  @override
  Widget build(BuildContext context) {
    return HoverButton(
      onPressed: onPressed,
      semanticLabel: semanticLabel,
      builder: (context, states) {
        final colors = gvColorsOf(context);
        final hovered = states.contains(WidgetState.hovered);
        return Container(
          width: 34,
          height: 34,
          alignment: AlignmentDirectional.center,
          decoration: BoxDecoration(
            color: hovered ? colors.hover.withValues(alpha: 0.55) : null,
            borderRadius: BorderRadius.circular(GvSpacing.radius),
          ),
          child: Icon(icon, size: 18, color: hovered ? colors.ink : colors.muted),
        );
      },
    );
  }
}

/// 搜索框（设计稿 search：高 36、bg 底、line 边、radius 9、聚焦 3px tint 环、
/// 右侧快捷键提示；占位文案“搜索仓库、路径或分支”——核心 viewQuery 的
/// haystack 确认含 branch，见 crates/gitaview-core/src/view.rs）。
class _SearchBox extends StatefulWidget {
  final AppController controller;

  const _SearchBox({super.key, required this.controller});

  @override
  State<_SearchBox> createState() => _SearchBoxState();
}

class _SearchBoxState extends State<_SearchBox> {
  final FocusNode _focus = FocusNode();
  final TextEditingController _text = TextEditingController();

  @override
  void initState() {
    super.initState();
    _focus.addListener(_onFocusChanged);
  }

  void _onFocusChanged() => setState(() {});

  @override
  void dispose() {
    _focus.removeListener(_onFocusChanged);
    _focus.dispose();
    _text.dispose();
    super.dispose();
  }

  /// ⌘K / Ctrl+K 的聚焦入口（_MainPageBodyState 调用）：聚焦并全选，
  /// 与浏览器搜索框一致，便于直接覆盖旧关键词。
  void focusField() {
    _focus.requestFocus();
    _text.selection = TextSelection(
      baseOffset: 0,
      extentOffset: _text.text.length,
    );
  }

  /// 鼠标点击容器内任一位置（含放大镜、左右留白）都聚焦，光标落到末尾。
  void _focusFromPointer() {
    if (_focus.hasFocus) return;
    _focus.requestFocus();
    _text.selection = TextSelection.collapsed(offset: _text.text.length);
  }

  @override
  Widget build(BuildContext context) {
    final colors = gvColorsOf(context);
    final brightness = FluentTheme.of(context).brightness;
    // 外部清空（清除筛选）时同步输入框内容；正常键入两者已一致。
    // 用 TextEditingValue 整体替换：`.text` setter 会把 selection 置为 -1，
    // 清空后光标位置会丢，重新聚焦时游标不出现。
    if (_text.text != widget.controller.query) {
      _text.value = TextEditingValue(
        text: widget.controller.query,
        selection: TextSelection.collapsed(
          offset: widget.controller.query.length,
        ),
      );
    }
    final focused = _focus.hasFocus;
    return ConstrainedBox(
      constraints: const BoxConstraints(maxWidth: 430),
      child: AnimatedContainer(
          duration: const Duration(milliseconds: 180),
          curve: Curves.easeOut,
          height: 36,
          padding: const EdgeInsets.only(left: 12, right: 10),
          decoration: BoxDecoration(
            color: focused ? colors.surface : colors.bg,
            borderRadius: BorderRadius.circular(9),
            border: Border.all(
              color: focused
                  ? Color.lerp(colors.line, colors.ink, 0.45)!
                  : colors.line,
            ),
            boxShadow: focused
                ? [
                    BoxShadow(
                      color: gvTintSoftOf(colors, brightness),
                      blurRadius: 0,
                      spreadRadius: 3,
                    ),
                  ]
                : null,
          ),
          child: GestureDetector(
            behavior: HitTestBehavior.opaque,
            onTap: _focusFromPointer,
            child: Row(
              children: [
                Icon(FluentIcons.search, size: 16, color: colors.muted),
                const SizedBox(width: 9),
                Expanded(
                  // 自绘输入区：fluent_ui 的 TextBox 会在容器内再画一层背景填充
                  // （默认 controlFillColorDefault，悬停还会变浅）和一条底部前景
                  // 描边（聚焦 2px 强调色下划线），decoration 只能关掉外框、关不掉
                  // 这两层，胶囊里就出现“盒中盒 + 一条蓝线”。故直接用 EditableText。
                  child: MouseRegion(
                    cursor: SystemMouseCursors.text,
                    child: Stack(
                      alignment: AlignmentDirectional.centerStart,
                      children: [
                        EditableText(
                          controller: _text,
                          focusNode: _focus,
                          maxLines: 1,
                          style: GvTextStyles.body(colors.ink)
                              .copyWith(fontSize: 13.5),
                          cursorColor: colors.ink,
                          backgroundCursorColor: colors.muted,
                          selectionColor: gvTintSoftOf(colors, brightness),
                          onChanged: widget.controller.setQuery,
                          // 保留右键复制 / 粘贴菜单（fluent 样式，与 TextBox 一致）。
                          contextMenuBuilder: (context, state) =>
                              WindowsTextSelectionToolbar.editableText(
                                  editableTextState: state),
                        ),
                        if (_text.text.isEmpty)
                          Positioned.fill(
                            child: IgnorePointer(
                              child: Align(
                                alignment: AlignmentDirectional.centerStart,
                                child: Text(
                                  '搜索仓库、路径或分支',
                                  maxLines: 1,
                                  softWrap: false,
                                  overflow: TextOverflow.clip,
                                  style: GvTextStyles.body(colors.muted)
                                      .copyWith(fontSize: 13.5),
                                ),
                              ),
                            ),
                          ),
                      ],
                    ),
                  ),
                ),
                const SizedBox(width: 9),
                Container(
                  padding:
                      const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
                  decoration: BoxDecoration(
                    color: colors.surface,
                    borderRadius: BorderRadius.circular(5),
                    border: Border.all(color: colors.line),
                  ),
                  child: Text(
                    _searchShortcutLabel,
                    style: GvTextStyles.mono(colors.muted, size: 11),
                  ),
                ),
              ],
            ),
          ),
        ),
    );
  }
}

// —— 筛选条（设计稿 filter-deck：label 32 + chips，选中 = ink 混合底） ——

class _FilterRow<T> extends StatelessWidget {
  final String label;
  final List<filter.FilterChip<T>> chips;
  final void Function(filter.FilterChip<T> chip) onSelected;

  const _FilterRow({
    required this.label,
    required this.chips,
    required this.onSelected,
  });

  @override
  Widget build(BuildContext context) {
    final colors = gvColorsOf(context);
    return Row(
      children: [
        SizedBox(
          width: 32,
          child: Text(
            label,
            style: GvTextStyles.subtitle(colors.muted)
                .copyWith(letterSpacing: 0.9),
          ),
        ),
        Expanded(
          child: SingleChildScrollView(
            scrollDirection: Axis.horizontal,
            child: Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                for (final chip in chips)
                  Padding(
                    padding: const EdgeInsets.only(right: 6),
                    child: _FilterChipButton<T>(
                      label: chip.label,
                      isSelected: chip.selected,
                      onTap: () => onSelected(chip),
                    ),
                  ),
              ],
            ),
          ),
        ),
      ],
    );
  }
}

/// 单个筛选 chip（设计稿 chip：高 29、radius 7；选中 = ink 9% 底 + ink 30% 边
/// + w600；悬停 / 焦点不移动内容）。
class _FilterChipButton<T> extends StatelessWidget {
  final String label;
  final bool isSelected;
  final VoidCallback onTap;

  const _FilterChipButton({
    required this.label,
    required this.isSelected,
    required this.onTap,
  });

  @override
  Widget build(BuildContext context) {
    return HoverButton(
      onPressed: onTap,
      builder: (context, states) {
        final colors = gvColorsOf(context);
        final hovered = states.contains(WidgetState.hovered) ||
            states.contains(WidgetState.focused);
        // chip 文案来自纯逻辑层（“名称 计数”）；计数恒为末段数字，
        // 拆开后计数用小号次要文字（设计稿 chip .n）。
        final splitIndex = label.lastIndexOf(' ');
        final name = splitIndex < 0 ? label : label.substring(0, splitIndex);
        final count = splitIndex < 0 ? '' : label.substring(splitIndex + 1);
        final Color background;
        final Color border;
        final Color nameColor;
        final Color countColor;
        if (isSelected) {
          background = Color.lerp(colors.surface, colors.ink, 0.09)!;
          border = Color.lerp(colors.line, colors.ink, 0.30)!;
          nameColor = colors.ink;
          countColor = colors.ink2;
        } else {
          background =
              hovered ? colors.hover.withValues(alpha: 0.6) : Colors.transparent;
          border = Colors.transparent;
          nameColor = colors.ink2;
          countColor = colors.muted;
        }
        return Container(
          constraints: const BoxConstraints(
              minHeight: GvSpacing.chipMinHeight),
          padding: const EdgeInsets.symmetric(horizontal: 11),
          alignment: AlignmentDirectional.center,
          decoration: BoxDecoration(
            color: background,
            borderRadius: BorderRadius.circular(7),
            // 为选中 / 未选中保留相同边框厚度，切换不改变按钮宽度。
            border: Border.all(color: border, width: 1),
          ),
          // 名称 + 计数一体渲染（设计稿 chip：计数 11.5 次要色）；
          // 用 Text.rich 保持整体文案可被测试匹配（findRichText）。
          child: Text.rich(
            TextSpan(
              children: [
                TextSpan(
                  text: name,
                  style: TextStyle(
                    fontSize: 12.5,
                    height: 1,
                    fontWeight:
                        isSelected ? FontWeight.w600 : FontWeight.w400,
                    color: nameColor,
                    fontFamily: GvFonts.ui,
                    fontFamilyFallback: GvFonts.uiFallback,
                  ),
                ),
                if (count.isNotEmpty)
                  TextSpan(
                    text: ' $count',
                    style: GvTextStyles.subtitle(countColor)
                        .copyWith(height: 1),
                  ),
              ],
            ),
            maxLines: 1,
          ),
        );
      },
    );
  }
}

// —— 内容区：骨架屏 / 空库 / 无匹配 / 表格 ——

class _ContentArea extends StatelessWidget {
  final AppController controller;
  final VoidCallback onOpenSettings;

  const _ContentArea({required this.controller, required this.onOpenSettings});

  @override
  Widget build(BuildContext context) {
    final colors = gvColorsOf(context);
    Widget content;
    if (!controller.loaded) {
      content = _frame(const _SkeletonView());
    } else if (controller.totalRepos == 0) {
      content = _frame(_EmptyState(onOpenSettings: onOpenSettings));
    } else if (controller.rows.isEmpty) {
      content = _frame(_NoMatch(controller: controller));
    } else {
      content = _frame(_RepoTable(controller: controller));
    }
    // 设计稿 .content 的实心表面底。ScaffoldPage 在非 Windows 下走 Mica
    //（不铺底），列表短于视口时否则露出黑色窗底。
    return ColoredBox(color: colors.surface, child: content);
  }

  /// 设计稿 content-inner：最大宽 1000 居中。
  Widget _frame(Widget child) => Align(
        alignment: Alignment.topCenter,
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 1000),
          child: child,
        ),
      );
}

/// 首次加载骨架屏（设计稿 skeleton：5 行、高 60、shimmer 1.5s；
/// 系统减少动效时退化为静态灰条）。
class _SkeletonView extends StatefulWidget {
  const _SkeletonView();

  @override
  State<_SkeletonView> createState() => _SkeletonViewState();
}

class _SkeletonViewState extends State<_SkeletonView>
    with SingleTickerProviderStateMixin {
  late final AnimationController _controller = AnimationController(
    vsync: this,
    duration: const Duration(milliseconds: 1500),
  )..repeat();

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final colors = gvColorsOf(context);
    final animate = !MediaQuery.disableAnimationsOf(context);
    return LayoutBuilder(
      builder: (context, constraints) {
        // 与 _RepoTable 用同一套列宽：骨架条落在真实列上，加载完成时
        // 列头与列位置都不再“跳入/错位”。
        final metrics = _TableMetrics.calculate(
            math.max(constraints.maxWidth, _TableMetrics.minContentWidth));
        Widget body = SizedBox(
          width: metrics.contentWidth,
          child: Column(
            children: [
              _TableHeader(metrics: metrics),
              // 行区域用 ListView：与表格同构，极矮窗口下也不会溢出。
              Expanded(
                child: ListView.builder(
                  physics: const NeverScrollableScrollPhysics(),
                  itemCount: 5,
                  itemBuilder: (context, index) =>
                      _row(colors, metrics, animate),
                ),
              ),
            ],
          ),
        );
        if (constraints.maxWidth < metrics.contentWidth) {
          body = SingleChildScrollView(
            scrollDirection: Axis.horizontal,
            child: body,
          );
        }
        return body;
      },
    );
  }

  Widget _row(GvColorSet colors, _TableMetrics metrics, bool animate) {
    return Container(
      height: 60,
      padding: const EdgeInsets.symmetric(horizontal: GvSpacing.page),
      decoration: BoxDecoration(
        border: Border(bottom: BorderSide(color: colors.lineSoft)),
      ),
      child: Row(
        children: [
          // 仓库单元格是名称 + 路径两行，骨架同为两行。
          _cell(
            metrics,
            0,
            Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                _bar(colors, 0.5, animate),
                const SizedBox(height: 7),
                _bar(colors, 0.36, animate),
              ],
            ),
          ),
          _cell(metrics, 1, _bar(colors, 0.34, animate)),
          _cell(metrics, 2, _bar(colors, 0.56, animate)),
          _cell(
            metrics,
            3,
            Align(
              alignment: AlignmentDirectional.centerStart,
              child: _bar(colors, 0.62, animate),
            ),
          ),
          _cell(metrics, 4, _bar(colors, 0.34, animate), last: true),
        ],
      ),
    );
  }

  /// 与 _RepoRow._cell 同构：列宽 + 右侧 10 的间距。
  Widget _cell(_TableMetrics metrics, int index, Widget child,
          {bool last = false}) =>
      SizedBox(
        width: metrics.columns[index],
        child: Padding(
          padding: EdgeInsets.only(right: last ? 0 : 10),
          child: child,
        ),
      );

  /// 骨架条（设计稿 sk-bar：9px 高、5px 圆角、lineSoft → line 混合渐变）。
  Widget _bar(GvColorSet colors, double widthFactor, bool animate) {
    Widget bar(Gradient? gradient) => FractionallySizedBox(
          widthFactor: widthFactor,
          alignment: AlignmentDirectional.centerStart,
          child: Container(
            height: 9,
            decoration: BoxDecoration(
              borderRadius: BorderRadius.circular(5),
              color: gradient == null ? colors.lineSoft : null,
              gradient: gradient,
            ),
          ),
        );
    if (!animate) return bar(null);
    return AnimatedBuilder(
      animation: _controller,
      builder: (context, _) {
        final t = _controller.value;
        return bar(LinearGradient(
          begin: Alignment(2.4 * t - 2.2, 0),
          end: Alignment(2.4 * t + 0.2, 0),
          colors: [
            colors.lineSoft,
            Color.lerp(colors.line, colors.surface, 0.3)!,
            colors.lineSoft,
          ],
        ));
      },
    );
  }
}

/// 空库（设计稿 empty：品牌标志 16% 透明 + 标题 + 说明 + 两个入口按钮）。
class _EmptyState extends StatelessWidget {
  final VoidCallback onOpenSettings;

  const _EmptyState({required this.onOpenSettings});

  @override
  Widget build(BuildContext context) {
    final colors = gvColorsOf(context);
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Opacity(opacity: 0.16, child: const GvBrandMark(size: 58)),
          const SizedBox(height: 16),
          Text('从第一个仓库开始',
              style: GvTextStyles.body(colors.ink, weight: GvTextStyles.semiBold)
                  .copyWith(fontSize: GvFontSizes.title)),
          const SizedBox(height: 9),
          ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 360),
            child: Text(
              '添加仓库目录，或扫描一个工作目录。',
              textAlign: TextAlign.center,
              style: GvTextStyles.caption(colors.muted)
                  .copyWith(height: 1.7),
            ),
          ),
          const SizedBox(height: 18),
          Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              _PrimaryButton(
                label: '添加仓库',
                icon: FluentIcons.add,
                onPressed: onOpenSettings,
              ),
              const SizedBox(width: 8),
              _OutlinedButton(
                label: '扫描目录',
                icon: FluentIcons.search,
                onPressed: onOpenSettings,
              ),
            ],
          ),
        ],
      ),
    );
  }
}

/// 无匹配（设计稿 no-match：标题 + 说明 + 清除筛选按钮）。
class _NoMatch extends StatelessWidget {
  final AppController controller;

  const _NoMatch({required this.controller});

  @override
  Widget build(BuildContext context) {
    final colors = gvColorsOf(context);
    return Padding(
      padding: const EdgeInsets.fromLTRB(
          GvSpacing.page, 56, GvSpacing.page, 22),
      child: Align(
        alignment: Alignment.topCenter,
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 440),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Row(
                children: [
                  Icon(FluentIcons.search, size: 16, color: colors.ink),
                  const SizedBox(width: 8),
                  Text('没有匹配的仓库',
                      style: GvTextStyles.body(colors.ink,
                          weight: GvTextStyles.semiBold)),
                ],
              ),
              const SizedBox(height: 8),
              Text('请调整筛选或搜索条件。',
                  style: GvTextStyles.caption(colors.muted)),
              const SizedBox(height: 14),
              _OutlinedButton(
                label: '清除筛选',
                onPressed: () {
                  controller.setQuery('');
                  controller.selectGroup(null);
                  controller.selectRelation(RelationFilter.all);
                },
              ),
            ],
          ),
        ),
      ),
    );
  }
}

// —— 表格（sticky 表头 + ListView.builder 行虚拟化；6 列，最小宽 680） ——

class _RepoTable extends StatelessWidget {
  final AppController controller;

  const _RepoTable({required this.controller});

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, constraints) {
        final available = constraints.maxWidth;
        final metrics = _TableMetrics.calculate(
            math.max(available, _TableMetrics.minContentWidth));
        Widget body = SizedBox(
          width: metrics.contentWidth,
          child: Column(
            children: [
              _TableHeader(metrics: metrics),
              Expanded(
                child: ListView.builder(
                  itemCount: controller.rows.length,
                  padding: const EdgeInsets.only(bottom: 10),
                  itemBuilder: (context, index) => _RepoRow(
                    controller: controller,
                    row: controller.rows[index],
                    metrics: metrics,
                  ),
                ),
              ),
            ],
          ),
        );
        if (available < metrics.contentWidth) {
          // 窄窗口：次要列收缩不足时允许横向滚动（ui-design 8.3）。
          body = SingleChildScrollView(
            scrollDirection: Axis.horizontal,
            child: body,
          );
        }
        return body;
      },
    );
  }
}

/// 列宽计算（设计稿 grid-template-columns：
/// minmax(220px, 2.5fr) 90px minmax(120px, 1.3fr) 112px 92px 26px，
/// 表头与行水平内边距 22×2；最小内容宽 680，不足横向滚动。
/// 可用空间不足以同时满足两列下限时按弹性比例收缩——与设计稿 grid
/// 在 680 下限处的表现一致，避免溢出）。
class _TableMetrics {
  final List<double> columns;
  final double contentWidth;

  const _TableMetrics(this.columns, this.contentWidth);

  static const double minContentWidth = 680;

  static _TableMetrics calculate(double availableWidth) {
    const double repoMin = 220;
    const double branchMin = 120;
    const double fixed = 90 + 112 + 92 + 26;
    const double flexTotal = 2.5 + 1.3;
    const double hPadding = 44;
    final inner = math.max(0.0, availableWidth - hPadding);
    final flex = math.max(0.0, inner - fixed);
    double repo;
    double branch;
    if (flex >= repoMin + branchMin) {
      // 先满足列下限，余量按 2.5fr : 1.3fr 分配。
      final rest = flex - repoMin - branchMin;
      repo = repoMin + rest * 2.5 / flexTotal;
      branch = branchMin + rest * 1.3 / flexTotal;
    } else {
      repo = flex * 2.5 / flexTotal;
      branch = flex * 1.3 / flexTotal;
    }
    final columns = <double>[repo, 90, branch, 112, 92, 26];
    final sum = columns.fold(0.0, (a, b) => a + b);
    return _TableMetrics(List.unmodifiable(columns), sum + hPadding);
  }
}

/// 表头（sticky：位于 ListView 之外，不随行滚动；高 37、下缘 line）。
class _TableHeader extends StatelessWidget {
  final _TableMetrics metrics;

  const _TableHeader({required this.metrics});

  static const List<String> _labels = ['仓库', '分组', '分支', '状态', '差异', ''];

  @override
  Widget build(BuildContext context) {
    final colors = gvColorsOf(context);
    return Container(
      height: 37,
      padding: const EdgeInsets.symmetric(horizontal: GvSpacing.page),
      decoration: BoxDecoration(
        // 设计稿：surface 92% + table-header 8% 混合。
        color: Color.lerp(colors.surface, colors.tableHeader, 0.08),
        border: Border(bottom: BorderSide(color: colors.line)),
      ),
      child: Row(
        children: [
          for (var i = 0; i < _labels.length; i++)
            SizedBox(
              width: metrics.columns[i],
              child: Padding(
                padding: EdgeInsets.only(right: i == _labels.length - 1 ? 0 : 10),
                child: Align(
                  alignment: AlignmentDirectional.centerStart,
                  child: Text(_labels[i],
                      style: GvTextStyles.headerLabel(colors.muted)),
                ),
              ),
            ),
        ],
      ),
    );
  }
}

/// 数据行 + 展开面板。行满幅、min-height 60、lineSoft 分隔；展开行
/// rowSelected 底 + 左侧 3px tint 指示条（上下内缩 9，不移动内容）。
class _RepoRow extends StatelessWidget {
  final AppController controller;
  final RepoRowVm row;
  final _TableMetrics metrics;

  const _RepoRow({
    required this.controller,
    required this.row,
    required this.metrics,
  });

  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        HoverButton(
          onPressed: () => controller.toggleRow(row.id),
          hitTestBehavior: HitTestBehavior.opaque,
          builder: (context, states) {
            final colors = gvColorsOf(context);
            final hovered = states.contains(WidgetState.hovered) ||
                states.contains(WidgetState.focused);
            final Color background;
            if (row.isExpanded) {
              background = colors.rowSelected;
            } else if (hovered) {
              background = colors.hover.withValues(alpha: 0.38);
            } else {
              background = colors.surface;
            }
            return Stack(
              children: [
                Container(
                  constraints: const BoxConstraints(minHeight: 60),
                  padding: const EdgeInsets.symmetric(
                      horizontal: GvSpacing.page),
                  decoration: BoxDecoration(
                    color: background,
                    border:
                        Border(bottom: BorderSide(color: colors.lineSoft)),
                  ),
                  alignment: AlignmentDirectional.center,
                  child: _buildCells(context, colors, hovered),
                ),
                if (row.isExpanded)
                  Positioned(
                    left: 0,
                    top: 9,
                    bottom: 9,
                    child: Container(
                      width: 3,
                      decoration: BoxDecoration(
                        color: colors.tint,
                        borderRadius: const BorderRadius.horizontal(
                            right: Radius.circular(3)),
                      ),
                    ),
                  ),
              ],
            );
          },
        ),
        if (row.isExpanded) _RowActionsPanel(controller: controller, row: row),
      ],
    );
  }

  Widget _cell(int index, Widget child, {bool last = false}) => SizedBox(
        width: metrics.columns[index],
        child: Padding(
          padding: EdgeInsets.only(right: last ? 0 : 10),
          child: child,
        ),
      );

  Widget _buildCells(BuildContext context, GvColorSet colors, bool hovered) {
    // 仓库单元格（用户反馈去冗余）：首行 = 显示名（别名优先）+ 状态提示
    //（warn 用 red、quiet 用 slate）；设了别名时第二行 = 原始目录名；
    // 仓库路径不常驻，展开面板的元信息行展示（点击行即可见）。
    final hintColor = row.dot == GvStatusColor.red ? colors.red : colors.slate;
    final tooltip = row.hint.isEmpty ? row.path : '${row.path} · ${row.hint}';
    return Row(
      children: [
        _cell(
          0,
          Tooltip(
            message: tooltip,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                Text.rich(
                  TextSpan(
                    children: [
                      TextSpan(
                        text: row.name,
                        style: GvTextStyles.body(
                            colors.ink, weight: GvTextStyles.medium),
                      ),
                      if (row.hint.isNotEmpty)
                        TextSpan(
                          text: ' · ${row.hint}',
                          style: GvTextStyles.subtitle(hintColor),
                        ),
                    ],
                  ),
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
              ],
            ),
          ),
        ),
        _cell(
          1,
          Text(row.groupDisplay,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: GvTextStyles.caption(colors.muted)),
        ),
        _cell(
          2,
          Text(row.branch,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: GvTextStyles.mono(colors.muted,
                  size: GvFontSizes.caption)),
        ),
        _cell(
          3,
          Align(
            alignment: AlignmentDirectional.centerStart,
            child: _StatusPill(row: row, colors: colors),
          ),
        ),
        _cell(4, _ChangesText(row: row, colors: colors)),
        _cell(
          5,
          Align(
            alignment: AlignmentDirectional.centerEnd,
            child: AnimatedOpacity(
              duration: const Duration(milliseconds: 160),
              opacity: row.isExpanded || hovered ? 1 : 0.5,
              child: AnimatedRotation(
                duration: const Duration(milliseconds: 200),
                turns: row.isExpanded ? 0.25 : 0,
                child: Icon(
                  FluentIcons.chevron_right,
                  size: 16,
                  color: row.isExpanded ? colors.ink2 : colors.muted,
                ),
              ),
            ),
          ),
          last: true,
        ),
      ],
    );
  }
}

/// 状态胶囊（设计稿 status-pill：高 24、点 7px、边 currentColor 22%、
/// 底 currentColor 9%；色 = 状态色映射，文字 = 现成 statusLabel）。
class _StatusPill extends StatelessWidget {
  final RepoRowVm row;
  final GvColorSet colors;

  const _StatusPill({required this.row, required this.colors});

  @override
  Widget build(BuildContext context) {
    final color = _colorOf(row.dot, colors);
    return Container(
      height: 24,
      padding: const EdgeInsets.only(left: 8, right: 10),
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(999),
        border: Border.all(color: color.withValues(alpha: 0.22)),
        color: color.withValues(alpha: 0.09),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Container(
            width: 7,
            height: 7,
            decoration: BoxDecoration(color: color, shape: BoxShape.circle),
          ),
          const SizedBox(width: 6),
          Text(row.statusLabel, style: GvTextStyles.pill(color)),
        ],
      ),
    );
  }
}

/// 差异列（设计稿 cell-changes：13.5 mono w600；色 teal / blue / amber，
/// 其余 muted w500）。
class _ChangesText extends StatelessWidget {
  final RepoRowVm row;
  final GvColorSet colors;

  const _ChangesText({required this.row, required this.colors});

  @override
  Widget build(BuildContext context) {
    final (color, weight) = switch (row.dot) {
      GvStatusColor.teal => (colors.teal, FontWeight.w600),
      GvStatusColor.blue => (colors.blue, FontWeight.w600),
      GvStatusColor.amber => (colors.amber, FontWeight.w600),
      _ => (colors.muted, FontWeight.w500),
    };
    return Text(
      row.changesLabel,
      maxLines: 1,
      overflow: TextOverflow.ellipsis,
      style: GvTextStyles.mono(color, size: GvFontSizes.numeric)
          .copyWith(fontWeight: weight),
    );
  }
}

/// 状态色角色 → 主题色（穷举 switch；新增 teal / blue 时编译器强制补全）。
Color _colorOf(GvStatusColor role, GvColorSet colors) => switch (role) {
      GvStatusColor.green => colors.green,
      GvStatusColor.teal => colors.teal,
      GvStatusColor.blue => colors.blue,
      GvStatusColor.amber => colors.amber,
      GvStatusColor.red => colors.red,
      GvStatusColor.slate => colors.slate,
      GvStatusColor.ink => colors.ink,
      GvStatusColor.muted => colors.muted,
    };

// —— 选中行操作面板（元信息 / 操作 / 确认 / 结果四层） ——

class _RowActionsPanel extends StatelessWidget {
  final AppController controller;
  final RepoRowVm row;

  const _RowActionsPanel({required this.controller, required this.row});

  @override
  Widget build(BuildContext context) {
    final colors = gvColorsOf(context);
    return TweenAnimationBuilder<double>(
      tween: Tween(begin: 0, end: 1),
      duration: const Duration(milliseconds: 200),
      builder: (context, t, child) => Opacity(
        opacity: t,
        child: Transform.translate(
          offset: Offset(0, -3 * (1 - t)),
          child: child,
        ),
      ),
      child: Container(
        width: double.infinity,
        padding: const EdgeInsets.only(
            top: 2, right: GvSpacing.page, bottom: 16, left: 32),
        color: colors.rowSelected,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            // 元信息行：路径 + 对照 origin/<branch> · 差异（右对齐）。
            Row(
              children: [
                Icon(FluentIcons.folder_open, size: 14, color: colors.muted),
                const SizedBox(width: 7),
                Expanded(
                  child: Text(
                    row.path,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: GvTextStyles.mono(colors.muted,
                        size: GvFontSizes.small),
                  ),
                ),
                const SizedBox(width: 14),
                _metaLine(context, colors),
              ],
            ),
            const SizedBox(height: 10),
            // 操作行：窄宽度换行（ui-design 8.3：不推挤其他仓库列）。
            Wrap(
              spacing: 6,
              runSpacing: 6,
              children: [
                _ActionButton(
                  label: '打开目录',
                  icon: FluentIcons.folder_open,
                  enabled: row.canOpenDirectory,
                  onTap: () => controller.openDirectory(row),
                ),
                _ActionButton(
                  label: '远端',
                  icon: FluentIcons.link,
                  enabled: row.canOpenRemote,
                  onTap: () => controller.openRemote(row),
                ),
                _ActionButton(
                  label: 'Fetch',
                  icon: FluentIcons.refresh,
                  enabled: row.canFetch,
                  busy: row.isFetchBusy,
                  onTap: () => controller.fetch(row),
                ),
                _ActionButton(
                  label: row.pullLabel,
                  icon: row.confirmPull ? FluentIcons.check_mark : FluentIcons.download,
                  enabled: row.canPull && !row.isBusy,
                  foreground: colors.tintInk,
                  onTap: () => controller.pull(row),
                ),
                _ActionButton(
                  label: row.pushLabel,
                  icon: row.confirmPush ? FluentIcons.check_mark : FluentIcons.upload,
                  enabled: row.canPush && !row.isBusy,
                  foreground: colors.tintInk,
                  onTap: () => controller.push(row),
                ),
              ],
            ),
            if (row.confirmVisible) ...[
              const SizedBox(height: 10),
              _ConfirmBox(controller: controller, row: row),
            ],
            if (row.resultVisible) ...[
              const SizedBox(height: 9),
              Row(
                children: [
                  if (row.isBusy)
                    const Padding(
                      padding: EdgeInsets.only(right: 7),
                      child: SizedBox(
                        width: 14,
                        height: 14,
                        child: ProgressRing(strokeWidth: 2),
                      ),
                    )
                  else
                    Padding(
                      padding: const EdgeInsets.only(right: 7),
                      child: Icon(
                        row.resultIsError
                            ? FluentIcons.warning
                            : FluentIcons.check_mark,
                        size: 14,
                        color: row.resultIsError ? colors.red : colors.green,
                      ),
                    ),
                  Expanded(
                    child: Text(
                      row.resultText,
                      style: GvTextStyles.body(
                        row.resultIsError
                            ? colors.red
                            : row.isBusy
                                ? colors.ink2
                                : colors.green,
                      ).copyWith(fontSize: 12.5),
                    ),
                  ),
                ],
              ),
            ],
          ],
        ),
      ),
    );
  }

  /// 元信息（由 branch / relationLabel / changesLabel 推导，不新增字段）：
  /// 读取失败 →“读取失败，暂不能操作”；无远端 →“未配置远端，只能查看本地”；
  /// 待读 →“尚未读取”；其余 → 对照 `origin/<branch>` · `<差异>`。
  Widget _metaLine(BuildContext context, GvColorSet colors) {
    switch (row.relationLabel) {
      case '读取失败':
        return Text('读取失败，暂不能操作',
            style: GvTextStyles.subtitle(colors.muted));
      case '无远端':
        return Text('未配置远端，只能查看本地',
            style: GvTextStyles.subtitle(colors.muted));
      case '尚未读取':
        return Text('尚未读取', style: GvTextStyles.subtitle(colors.muted));
    }
    final tone = switch (row.dot) {
      GvStatusColor.teal => colors.teal,
      GvStatusColor.blue => colors.blue,
      GvStatusColor.amber => colors.amber,
      _ => colors.muted,
    };
    return Text.rich(
      TextSpan(
        children: [
          TextSpan(text: '对照 ', style: GvTextStyles.subtitle(colors.muted)),
          TextSpan(
            text: 'origin/${row.branch}',
            style: GvTextStyles.mono(colors.ink2, size: GvFontSizes.small),
          ),
          TextSpan(text: ' · ', style: GvTextStyles.subtitle(colors.muted)),
          TextSpan(
            text: row.changesLabel,
            style: GvTextStyles.mono(tone, size: GvFontSizes.small)
                .copyWith(fontWeight: GvTextStyles.medium),
          ),
        ],
      ),
      maxLines: 1,
      overflow: TextOverflow.ellipsis,
    );
  }
}

/// 确认框（设计稿 confirm-box：tintSoft 底 + tint 55% 边；strong + span + 右侧
/// 取消确认 / 确认执行两个按钮——取消不做裸文字按钮）。
class _ConfirmBox extends StatelessWidget {
  final AppController controller;
  final RepoRowVm row;

  const _ConfirmBox({required this.controller, required this.row});

  @override
  Widget build(BuildContext context) {
    final colors = gvColorsOf(context);
    final brightness = FluentTheme.of(context).brightness;
    final isPull = row.confirmPull;
    final executeLabel = isPull ? row.pullLabel : row.pushLabel;
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
      decoration: BoxDecoration(
        color: gvTintSoftOf(colors, brightness),
        borderRadius: BorderRadius.circular(GvSpacing.radius),
        border: Border.all(color: Color.lerp(colors.line, colors.tint, 0.55)!),
      ),
      child: Row(
        children: [
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  row.confirmText,
                  style: GvTextStyles.body(colors.ink,
                          weight: GvTextStyles.semiBold)
                      .copyWith(fontSize: 12.5),
                ),
                const SizedBox(height: 3),
                Text(
                  '再次点击「$executeLabel」执行，或取消返回。',
                  style: GvTextStyles.subtitle(colors.ink2),
                ),
              ],
            ),
          ),
          const SizedBox(width: 12),
          _PanelButton(
            label: '取消确认',
            quiet: true,
            onTap: () => controller.cancelConfirmation(row),
          ),
          const SizedBox(width: 8),
          _PanelButton(
            label: executeLabel,
            onTap: isPull
                ? () => controller.pull(row)
                : () => controller.push(row),
          ),
        ],
      ),
    );
  }
}

/// 面板小按钮（设计稿 btn / btn.quiet.small：高 28、padding 0/10、字号 12）。
class _PanelButton extends StatelessWidget {
  final String label;
  final bool quiet;
  final VoidCallback onTap;

  const _PanelButton({
    required this.label,
    required this.onTap,
    this.quiet = false,
  });

  @override
  Widget build(BuildContext context) {
    return HoverButton(
      onPressed: onTap,
      builder: (context, states) {
        final colors = gvColorsOf(context);
        final hovered = states.contains(WidgetState.hovered);
        return Container(
          height: 28,
          padding: const EdgeInsets.symmetric(horizontal: 10),
          alignment: AlignmentDirectional.center,
          decoration: BoxDecoration(
            color: quiet
                ? (hovered ? colors.hover.withValues(alpha: 0.6) : null)
                : colors.surface,
            borderRadius: BorderRadius.circular(7),
            border: Border.all(
              color: quiet
                  ? Colors.transparent
                  : hovered
                      ? Color.lerp(colors.line, colors.ink, 0.35)!
                      : colors.line,
            ),
          ),
          child: Text(
            label,
            style: GvTextStyles.body(quiet ? colors.muted : colors.ink)
                .copyWith(fontSize: 12),
          ),
        );
      },
    );
  }
}

/// 操作按钮（设计稿 btn：高 30、padding 0/12、radius 7、字号 12.5、icon 14；
/// Pull / Push 文字色 = tintInk；禁用 36% 不悬停变色）。
class _ActionButton extends StatelessWidget {
  final String label;
  final IconData icon;
  final bool enabled;
  final bool busy;
  final Color? foreground;
  final VoidCallback onTap;

  const _ActionButton({
    required this.label,
    required this.icon,
    required this.enabled,
    required this.onTap,
    this.busy = false,
    this.foreground,
  });

  @override
  Widget build(BuildContext context) {
    return HoverButton(
      onPressed: enabled && !busy ? onTap : null,
      builder: (context, states) {
        final colors = gvColorsOf(context);
        final disabled = states.contains(WidgetState.disabled);
        final hovered = states.contains(WidgetState.hovered) && !disabled;
        final fg = foreground ?? colors.ink;
        return Opacity(
          opacity: disabled ? 0.36 : 1,
          child: Container(
            height: 30,
            padding: const EdgeInsets.symmetric(horizontal: 12),
            decoration: BoxDecoration(
              color: hovered
                  ? Color.lerp(colors.surface, colors.hover, 0.30)
                  : colors.surface,
              borderRadius: BorderRadius.circular(7),
              border: Border.all(
                color: hovered
                    ? Color.lerp(colors.line, colors.ink, 0.35)!
                    : colors.line,
              ),
            ),
            // 不用 alignment（Wrap 会给子项 maxWidth=整行，alignment 会撑满），
            // 垂直居中由 Row 的 crossAxisAlignment 承担。
            child: Row(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.center,
              children: [
                if (busy)
                  SizedBox(
                    width: 14,
                    height: 14,
                    child: ProgressRing(strokeWidth: 2),
                  )
                else
                  Icon(icon, size: 14, color: fg),
                const SizedBox(width: 6),
                Text(
                  label,
                  style: TextStyle(
                    fontSize: 12.5,
                    color: fg,
                    fontFamily: GvFonts.ui,
                    fontFamilyFallback: GvFonts.uiFallback,
                  ),
                ),
              ],
            ),
          ),
        );
      },
    );
  }
}

// —— 通用按钮（空态 / 无匹配入口） ——

/// 主按钮（设计稿 btn.primary：selection 底 + onSelection 字）。
class _PrimaryButton extends StatelessWidget {
  final String label;
  final IconData icon;
  final VoidCallback onPressed;

  const _PrimaryButton({
    required this.label,
    required this.icon,
    required this.onPressed,
  });

  @override
  Widget build(BuildContext context) {
    final colors = gvColorsOf(context);
    return HoverButton(
      onPressed: onPressed,
      builder: (context, states) {
        final hovered = states.contains(WidgetState.hovered);
        return Container(
          height: 34,
          padding: const EdgeInsets.symmetric(horizontal: 14),
          decoration: BoxDecoration(
            color: hovered
                ? Color.lerp(colors.selection, colors.tint, 0.12)
                : colors.selection,
            borderRadius: BorderRadius.circular(7),
          ),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.center,
            children: [
              Icon(icon, size: 14, color: colors.onSelection),
              const SizedBox(width: 6),
              Text(
                label,
                style: GvTextStyles.body(colors.onSelection)
                    .copyWith(fontSize: 12.5),
              ),
            ],
          ),
        );
      },
    );
  }
}

/// 次按钮（设计稿 btn：surface 底 + line 边）。
class _OutlinedButton extends StatelessWidget {
  final String label;
  final IconData? icon;
  final VoidCallback onPressed;

  const _OutlinedButton({
    required this.label,
    required this.onPressed,
    this.icon,
  });

  @override
  Widget build(BuildContext context) {
    return HoverButton(
      onPressed: onPressed,
      builder: (context, states) {
        final colors = gvColorsOf(context);
        final hovered = states.contains(WidgetState.hovered);
        return Container(
          height: 34,
          padding: const EdgeInsets.symmetric(horizontal: 14),
          decoration: BoxDecoration(
            color: hovered
                ? Color.lerp(colors.surface, colors.hover, 0.30)
                : colors.surface,
            borderRadius: BorderRadius.circular(7),
            border: Border.all(
              color: hovered
                  ? Color.lerp(colors.line, colors.ink, 0.35)!
                  : colors.line,
            ),
          ),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.center,
            children: [
              if (icon != null) ...[
                Icon(icon!, size: 14, color: colors.ink),
                const SizedBox(width: 6),
              ],
              Text(
                label,
                style: GvTextStyles.body(colors.ink).copyWith(fontSize: 12.5),
              ),
            ],
          ),
        );
      },
    );
  }
}

// —— 状态栏（设计稿 statusbar：高 42、上缘 lineSoft、字号 12） ——

class _StatusBar extends StatelessWidget {
  final AppController controller;

  const _StatusBar({required this.controller});

  @override
  Widget build(BuildContext context) {
    final colors = gvColorsOf(context);
    // 刷新失败口径：设计稿不给大横幅，只保留历史行 + 状态栏琥珀文案。
    final failed = controller.refreshWarningText != null;
    final filtered = controller.rows.length;
    final total = controller.totalRepos;
    final String left;
    if (failed) {
      left = '刷新失败 · 当前显示最近一次成功结果';
    } else if (total > 0 && filtered != total) {
      left = '已筛选 $filtered / $total 个仓库';
    } else {
      left =
          '$_searchShortcutLabel 搜索 · 单击仓库行展开操作 · Esc 取消确认';
    }
    return Container(
      height: 42,
      padding: const EdgeInsets.symmetric(horizontal: GvSpacing.page),
      decoration: BoxDecoration(
        color: colors.surface,
        border: Border(top: BorderSide(color: colors.lineSoft)),
      ),
      child: Row(
        children: [
          Expanded(
            child: Text(
              left,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: GvTextStyles.body(
                failed ? colors.amber : colors.muted,
              ).copyWith(fontSize: GvFontSizes.micro),
            ),
          ),
          const SizedBox(width: 16),
          Text(
            controller.refreshTimeText,
            style: GvTextStyles.subtitle(colors.muted),
          ),
        ],
      ),
    );
  }
}
