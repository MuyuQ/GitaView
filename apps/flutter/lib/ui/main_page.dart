// 主状态页：工具栏 + 两层筛选 chips + 仓库表格 + 选中行操作面板 + 空态 /
// 加载 / 全局失败横幅 + 底部摘要。
//
// 行为规格：apps/windows/GitaView.Native/MainPage.xaml(.cs)（语义对齐，
// 不是像素复刻）：
// - 工具栏（沉浸式标题栏内容区）：GitaView 品牌 + 搜索仓库 + 刷新 / 设置；
//   Windows 下工具栏同时是窗口拖拽区，右端附加系统标题栏按钮；
// - 两层筛选：分组（首项“全部分组”）与状态（计数为 0 的状态不出现），
//   chips 复用 RepositoryFilterPresentation，文案 = 名称 + 计数；
// - 表格列序固定：状态点 · 仓库 · 分组 · 分支 · 关系 · 差异 · 提示，
//   状态 = 文字 + 数字 + 颜色并用（不单靠颜色）；
// - 选中行展开操作面板：目录 / 远端 / Fetch / Pull / Push（Pull / Push
//   两段式确认），确认 / 结果独立占行，窄宽度换行不推挤其他列；
// - 状态文本与颜色角色映射来自 AppController（C# MainPage 的 *Of 系列）。
library;

import 'dart:math' as math;

import 'package:fluent_ui/fluent_ui.dart';
import 'package:window_manager/legacy.dart' show DragToMoveArea;

import '../app_controller.dart';
import '../contracts/contracts.dart' show RelationFilter;
// FilterChip 与 fluent_ui（material）重名：用别名引用纯逻辑层的 chip 模型。
import '../presentation/repository_filter_presentation.dart' as filter;
import 'theme/tokens.dart';

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

class _MainPageBody extends StatelessWidget {
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
  Widget build(BuildContext context) {
    final colors = gvColorsOf(context);
    final warning = controller.refreshWarningText;
    return Column(
      children: [
        _Toolbar(
          controller: controller,
          onOpenSettings: onOpenSettings,
          dragEnabled: dragToolbar,
          captionBar: captionBar,
        ),
        if (controller.startupRecovered)
          InfoBar(
            title: const Text('配置已恢复为默认值'),
            content: const Text(
                '原设置文件损坏或无法识别，已隔离保留。当前使用默认设置，请重新添加仓库。'),
            severity: InfoBarSeverity.warning,
          ),
        if (warning != null)
          Padding(
            padding: const EdgeInsets.fromLTRB(GvSpacing.page, 6, GvSpacing.page, 0),
            child: Align(
              alignment: AlignmentDirectional.centerStart,
              child: Text(
                warning,
                style: GvTextStyles.body(colors.amberText),
              ),
            ),
          ),
        _FilterBar<String?>(
          label: '分组',
          groupBar: true,
          chips: controller.groupChips,
          onSelected: (chip) => controller.selectGroup(chip.value),
          topPadding: 10,
          bottomPadding: 4,
        ),
        _FilterBar<RelationFilter>(
          label: '状态',
          groupBar: false,
          chips: controller.statusChips,
          onSelected: (chip) => controller.selectRelation(chip.value),
          topPadding: 4,
          bottomPadding: 10,
        ),
        Expanded(child: _ContentArea(controller: controller, onOpenSettings: onOpenSettings)),
        _Footer(controller: controller),
      ],
    );
  }
}

// —— 工具栏（C# TitleBar.Content） ——

class _Toolbar extends StatelessWidget {
  final AppController controller;
  final VoidCallback onOpenSettings;
  final bool dragEnabled;
  final Widget? captionBar;

  const _Toolbar({
    required this.controller,
    required this.onOpenSettings,
    required this.dragEnabled,
    this.captionBar,
  });

  @override
  Widget build(BuildContext context) {
    final colors = gvColorsOf(context);
    Widget toolbar = Container(
      height: GvSpacing.toolbarHeight,
      color: colors.surface,
      padding: const EdgeInsets.only(left: GvSpacing.content, right: 4),
      child: Row(
        children: [
          Padding(
            padding: const EdgeInsets.only(right: GvSpacing.content),
            child: Text('GitaView', style: GvTextStyles.brand(colors.ink)),
          ),
          Expanded(
            child: Center(
              child: ConstrainedBox(
                constraints: const BoxConstraints(maxWidth: 240),
                child: TextBox(
                  placeholder: '搜索仓库',
                  onChanged: controller.setQuery,
                ),
              ),
            ),
          ),
          const SizedBox(width: GvSpacing.small),
          HoverButton(
            onPressed: controller.refreshBusy ? null : () => controller.refresh(),
            builder: (context, states) => _quietDecoration(
              context,
              states,
              child: Padding(
                padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 5),
                child: Text(
                  controller.refreshBusy ? '刷新中' : '刷新',
                  style: GvTextStyles.body(
                    controller.refreshBusy ? colors.muted.withValues(alpha: 0.4) : colors.muted,
                  ),
                ),
              ),
            ),
          ),
          HoverButton(
            onPressed: onOpenSettings,
            semanticLabel: '设置',
            builder: (context, states) => _quietDecoration(
              context,
              states,
              child: Padding(
                padding: const EdgeInsets.all(9),
                child: Icon(FluentIcons.settings, size: 16, color: colors.muted),
              ),
            ),
          ),
          ?captionBar,
        ],
      ),
    );
    if (!dragEnabled) return toolbar;
    return DragToMoveArea(child: toolbar);
  }

  /// 安静按钮（GvQuietButton：透明底、悬停浅色层、按下加深，不移动内容）。
  Widget _quietDecoration(
    BuildContext context,
    Set<WidgetState> states, {
    required Widget child,
  }) {
    final colors = gvColorsOf(context);
    final hovered = states.contains(WidgetState.hovered);
    final pressed = states.contains(WidgetState.pressed);
    return Container(
      decoration: BoxDecoration(
        color: pressed
            ? colors.hover.withValues(alpha: 0.8)
            : hovered
                ? colors.hover.withValues(alpha: 0.45)
                : Colors.transparent,
        borderRadius: BorderRadius.circular(GvSpacing.radius),
      ),
      child: child,
    );
  }
}

// —— 筛选条（C# GroupChipsPanel / StatusChipsPanel） ——

class _FilterBar<T> extends StatelessWidget {
  final String label;
  final bool groupBar;
  final List<filter.FilterChip<T>> chips;
  final void Function(filter.FilterChip<T> chip) onSelected;
  final double topPadding;
  final double bottomPadding;

  const _FilterBar({
    required this.label,
    required this.groupBar,
    required this.chips,
    required this.onSelected,
    required this.topPadding,
    required this.bottomPadding,
  });

  @override
  Widget build(BuildContext context) {
    final colors = gvColorsOf(context);
    return Container(
      width: double.infinity,
      color: colors.filterSurface,
      padding: EdgeInsets.fromLTRB(
          GvSpacing.page, topPadding, GvSpacing.page, bottomPadding),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.center,
        children: [
          SizedBox(
            width: 44,
            child: Text(label,
                style: GvTextStyles.body(colors.ink, weight: GvTextStyles.semiBold)),
          ),
          Expanded(
            child: SingleChildScrollView(
              scrollDirection: Axis.horizontal,
              child: Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  for (final chip in chips)
                    Padding(
                      padding: const EdgeInsets.only(right: 4),
                      child: _FilterChipButton<T>(
                        label: chip.label,
                        isSelected: chip.selected,
                        isGroupChip: groupBar,
                        onTap: () => onSelected(chip),
                      ),
                    ),
                ],
              ),
            ),
          ),
        ],
      ),
    );
  }
}

/// 单个筛选 chip（C# ApplyChipSelection：分组选中 = surface 底 + ink 字 +
/// line 边框；状态选中 = selection 底 + onSelection 字；悬停/焦点不移动内容）。
class _FilterChipButton<T> extends StatelessWidget {
  final String label;
  final bool isSelected;
  final bool isGroupChip;
  final VoidCallback onTap;

  const _FilterChipButton({
    required this.label,
    required this.isSelected,
    required this.isGroupChip,
    required this.onTap,
  });

  @override
  Widget build(BuildContext context) {
    return HoverButton(
      onPressed: onTap,
      builder: (context, states) {
        final colors = gvColorsOf(context);
        final hovered = states.contains(WidgetState.hovered);
        final pressed = states.contains(WidgetState.pressed);
        final Color background;
        final Color foreground;
        if (isSelected) {
          if (isGroupChip) {
            background = colors.surface;
            foreground = colors.ink;
          } else {
            background = colors.selection;
            foreground = colors.onSelection;
          }
        } else {
          background = hovered || pressed ? colors.hover : Colors.transparent;
          foreground = colors.muted;
        }
        return Container(
          constraints: const BoxConstraints(minHeight: GvSpacing.chipMinHeight),
          padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 5),
          decoration: BoxDecoration(
            color: background,
            borderRadius: BorderRadius.circular(GvSpacing.radius),
            // 为分组按钮预留相同边框厚度，选中态变化不改变按钮宽度。
            border: Border.all(
              color: isSelected && isGroupChip ? colors.line : Colors.transparent,
              width: 1,
            ),
          ),
          child: Text(
            label,
            style: GvTextStyles.body(
              foreground,
              weight: isGroupChip ? GvTextStyles.medium : FontWeight.w400,
            ),
          ),
        );
      },
    );
  }
}

// —— 内容区：空态 / 无匹配 / 表格 ——

class _ContentArea extends StatelessWidget {
  final AppController controller;
  final VoidCallback onOpenSettings;

  const _ContentArea({required this.controller, required this.onOpenSettings});

  @override
  Widget build(BuildContext context) {
    final colors = gvColorsOf(context);
    if (!controller.loaded) {
      // 首次加载：无文案（C# 同步渲染无此态；不发明新文案）。
      return Center(
        child: SizedBox(
          width: 20,
          height: 20,
          child: ProgressRing(strokeWidth: 3),
        ),
      );
    }
    if (controller.totalRepos == 0) {
      return Center(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Text('从第一个仓库开始',
                style: GvTextStyles.body(colors.ink,
                    weight: GvTextStyles.semiBold)
                    .copyWith(fontSize: 16)),
            const SizedBox(height: 10),
            Text('添加仓库目录，或扫描一个工作目录。',
                style: GvTextStyles.body(colors.muted)),
            const SizedBox(height: 10),
            Button(
              onPressed: onOpenSettings,
              style: _gvButtonStyle(context),
              child: Text('添加仓库', style: GvTextStyles.body(colors.ink)),
            ),
          ],
        ),
      );
    }
    if (controller.rows.isEmpty) {
      return Align(
        alignment: AlignmentDirectional.topStart,
        child: Padding(
          padding: const EdgeInsets.fromLTRB(
              GvSpacing.page, 52, GvSpacing.page, 0),
          child: Text('没有匹配的仓库，请调整筛选或搜索条件。',
              style: GvTextStyles.body(colors.muted)),
        ),
      );
    }
    return _RepoTable(controller: controller);
  }
}

// —— 表格（C# TableHeader / RepoList；列序固定） ——

class _RepoTable extends StatelessWidget {
  final AppController controller;

  const _RepoTable({required this.controller});

  /// C# TableContent MinWidth=624。
  static const double minContentWidth = 624;

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, constraints) {
        final available = constraints.maxWidth;
        final metrics = _TableMetrics.calculate(
            math.max(available, minContentWidth));
        Widget table = Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            _TableHeader(metrics: metrics),
            for (final row in controller.rows)
              _RepoRow(controller: controller, row: row, metrics: metrics),
          ],
        );
        if (available < minContentWidth) {
          // 窄窗口：次要列收缩不足时允许横向滚动（ui-design 8.3）。
          table = SingleChildScrollView(
            scrollDirection: Axis.horizontal,
            child: SizedBox(
              width: metrics.contentWidth,
              child: table,
            ),
          );
        }
        return SingleChildScrollView(
          child: table,
        );
      },
    );
  }
}

/// 列宽计算（C# TableHeader 列定义：30 / 2* / 76 / 1.2* / 94 / 66 / *，
/// 最小 140 / 90 / 80；48 = 表头与行的水平内边距合计）。
class _TableMetrics {
  final List<double> columns;
  final double contentWidth;

  const _TableMetrics(this.columns, this.contentWidth);

  static _TableMetrics calculate(double availableWidth) {
    const fixed = 30.0 + 76.0 + 94.0 + 66.0;
    const flexTotal = 2.0 + 1.2 + 1.0;
    final inner = math.max(0.0, availableWidth - 48);
    final flex = math.max(0.0, inner - fixed);
    final repo = math.max(140.0, flex * 2.0 / flexTotal);
    final branch = math.max(90.0, flex * 1.2 / flexTotal);
    final hint = math.max(80.0, flex * 1.0 / flexTotal);
    final columns = <double>[30.0, repo, 76.0, branch, 94.0, 66.0, hint];
    final sum = columns.fold(0.0, (a, b) => a + b);
    return _TableMetrics(List.unmodifiable(columns), sum + 48);
  }
}

class _TableHeader extends StatelessWidget {
  final _TableMetrics metrics;

  const _TableHeader({required this.metrics});

  static const List<String> _labels = ['', '仓库', '分组', '分支', '关系', '差异', '提示'];

  @override
  Widget build(BuildContext context) {
    final colors = gvColorsOf(context);
    return Container(
      width: metrics.contentWidth,
      padding: const EdgeInsets.symmetric(
          horizontal: GvSpacing.table, vertical: 10),
      decoration: BoxDecoration(
        color: colors.tableHeader,
        border: Border(bottom: BorderSide(color: colors.line)),
      ),
      child: Row(
        children: [
          for (var i = 0; i < _labels.length; i++)
            SizedBox(
              width: metrics.columns[i],
              child: Text(
                _labels[i],
                style: GvTextStyles.body(colors.ink, weight: GvTextStyles.semiBold),
              ),
            ),
        ],
      ),
    );
  }
}

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
    return HoverButton(
      onPressed: () => controller.toggleRow(row.id),
      hitTestBehavior: HitTestBehavior.opaque,
      builder: (context, states) {
        final colors = gvColorsOf(context);
        // 选中 = 展开行（C# RowBackground 由 _selectedRepoId 决定）；
        // 悬停不移动内容，仅换背景且不覆盖选中态。
        final Color background;
        if (row.isExpanded) {
          background = colors.rowSelected;
        } else if (states.contains(WidgetState.hovered) ||
                states.contains(WidgetState.focused)) {
          background = colors.hover;
        } else {
          background = colors.surface;
        }
        return Container(
          margin: const EdgeInsets.symmetric(horizontal: 8),
          decoration: BoxDecoration(
            color: background,
            borderRadius: BorderRadius.circular(6),
          ),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Container(
                padding: const EdgeInsets.symmetric(
                    horizontal: GvSpacing.content, vertical: 10),
                decoration:
                    BoxDecoration(border: Border(bottom: BorderSide(color: colors.line))),
                child: _buildCells(context, colors),
              ),
              if (row.isExpanded)
                _RowActionsPanel(controller: controller, row: row),
            ],
          ),
        );
      },
    );
  }

  Widget _buildCells(BuildContext context, GvColorSet colors) {
    Widget cell(int index, Widget child) => SizedBox(
          width: metrics.columns[index],
          child: Padding(
            padding: const EdgeInsets.only(right: 8),
            child: child,
          ),
        );

    return Row(
      children: [
        // 状态点：Semantics 提供文字状态（不依赖颜色）。
        cell(
          0,
          Semantics(
            label: row.statusLabel,
            child: Container(
              width: 6,
              height: 6,
              decoration:
                  BoxDecoration(color: _colorOf(row.dot, colors), shape: BoxShape.circle),
            ),
          ),
        ),
        cell(
          1,
          Tooltip(
            message: row.path,
            child: Text(
              row.name,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: GvTextStyles.body(colors.ink, weight: GvTextStyles.medium),
            ),
          ),
        ),
        cell(2, Text(row.groupDisplay,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: GvTextStyles.body(colors.muted))),
        cell(3, Text(row.branch,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: GvTextStyles.mono(colors.muted))),
        cell(4, Text(row.relationLabel,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: GvTextStyles.body(_colorOf(row.relationColor, colors)))),
        cell(5, Text(row.changesLabel,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: GvTextStyles.mono(colors.ink))),
        cell(
          6,
          Tooltip(
            message: row.hint.isEmpty ? row.relationLabel : row.hint,
            child: Text(row.hint,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: GvTextStyles.body(colors.muted)),
          ),
        ),
      ],
    );
  }

  static Color _colorOf(GvStatusColor role, GvColorSet colors) =>
      switch (role) {
        GvStatusColor.green => colors.green,
        GvStatusColor.amber => colors.amberText,
        GvStatusColor.red => colors.red,
        GvStatusColor.slate => colors.slate,
        GvStatusColor.ink => colors.ink,
        GvStatusColor.muted => colors.muted,
      };
}

// —— 选中行操作面板（C# DataTemplate 的展开区） ——

class _RowActionsPanel extends StatelessWidget {
  final AppController controller;
  final RepoRowVm row;

  const _RowActionsPanel({required this.controller, required this.row});

  @override
  Widget build(BuildContext context) {
    final colors = gvColorsOf(context);
    return Padding(
      padding: const EdgeInsets.fromLTRB(46, 0, 16, 12),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          // Wrap：窄宽度时换行（ui-design 8.3：不推挤其他仓库列）。
          Wrap(
            spacing: 6,
            runSpacing: 6,
            children: [
              _ActionButton(
                label: '打开目录',
                enabled: row.canOpenDirectory,
                onTap: () => controller.openDirectory(row),
              ),
              _ActionButton(
                label: '远端',
                enabled: row.canOpenRemote,
                onTap: () => controller.openRemote(row),
              ),
              _ActionButton(
                label: 'Fetch',
                enabled: row.canFetch,
                onTap: () => controller.fetch(row),
              ),
              _ActionButton(
                label: row.pullLabel,
                enabled: row.canPull,
                foreground: colors.amberText,
                onTap: () => controller.pull(row),
              ),
              _ActionButton(
                label: row.pushLabel,
                enabled: row.canPush,
                foreground: colors.amberText,
                onTap: () => controller.push(row),
              ),
            ],
          ),
          if (row.confirmVisible) ...[
            const SizedBox(height: 6),
            Text(row.confirmText,
                style: GvTextStyles.body(colors.amberText)),
            const SizedBox(height: 6),
            HoverButton(
              onPressed: () => controller.cancelConfirmation(row),
              builder: (context, states) => Padding(
                padding:
                    const EdgeInsets.symmetric(horizontal: 12, vertical: 5),
                child: Text('取消确认',
                    style: GvTextStyles.body(colors.muted)),
              ),
            ),
          ],
          if (row.resultVisible) ...[
            const SizedBox(height: 8),
            Row(
              children: [
                if (row.isBusy)
                  const Padding(
                    padding: EdgeInsets.only(right: 8),
                    child: SizedBox(
                      width: 14,
                      height: 14,
                      child: ProgressRing(strokeWidth: 2.5),
                    ),
                  ),
                Expanded(
                  child: Text(
                    row.resultText,
                    style: GvTextStyles.body(
                        row.resultIsError ? colors.red : colors.green),
                  ),
                ),
              ],
            ),
          ],
        ],
      ),
    );
  }
}

/// 操作按钮（GvActionButtonStyle：surface 底 + line 边框 + 圆角 8）。
class _ActionButton extends StatelessWidget {
  final String label;
  final bool enabled;
  final Color? foreground;
  final VoidCallback onTap;

  const _ActionButton({
    required this.label,
    required this.enabled,
    required this.onTap,
    this.foreground,
  });

  @override
  Widget build(BuildContext context) {
    final colors = gvColorsOf(context);
    return Button(
      onPressed: enabled ? onTap : null,
      style: _gvButtonStyle(context, foreground: foreground),
      child: Text(label,
          style: GvTextStyles.body(foreground ?? colors.ink)),
    );
  }
}

/// 通用按钮样式（C# GvButton：surface 底、line 边框、圆角 8、最小高 34）。
ButtonStyle _gvButtonStyle(BuildContext context, {Color? foreground}) {
  final colors = gvColorsOf(context);
  return ButtonStyle(
    padding: WidgetStateProperty.all(
        const EdgeInsets.symmetric(horizontal: 12, vertical: 5)),
    backgroundColor: WidgetStateProperty.resolveWith((states) =>
        states.contains(WidgetState.disabled)
            ? colors.surface.withValues(alpha: 0.4)
            : colors.surface),
    foregroundColor: WidgetStateProperty.resolveWith((states) =>
        states.contains(WidgetState.disabled)
            ? colors.muted.withValues(alpha: 0.4)
            : foreground ?? colors.ink),
    shape: WidgetStateProperty.all(RoundedRectangleBorder(
      borderRadius: BorderRadius.circular(GvSpacing.radius),
      side: BorderSide(color: colors.line),
    )),
    textStyle: WidgetStateProperty.all(GvTextStyles.body(colors.ink)),
  );
}

// —— 底部摘要（C# ResultCountText / RefreshTimeText） ——

class _Footer extends StatelessWidget {
  final AppController controller;

  const _Footer({required this.controller});

  @override
  Widget build(BuildContext context) {
    final colors = gvColorsOf(context);
    return Container(
      width: double.infinity,
      padding: const EdgeInsets.symmetric(
          horizontal: GvSpacing.table, vertical: 10),
      decoration: BoxDecoration(
        color: colors.filterSurface,
        border: Border(top: BorderSide(color: colors.line)),
      ),
      child: Row(
        children: [
          Expanded(
            child: Text(
              controller.summaryText,
              style: GvTextStyles.body(
                controller.summaryIsWarning ? colors.amberText : colors.muted,
              ),
            ),
          ),
          const SizedBox(width: 12),
          Text(controller.refreshTimeText, style: GvTextStyles.body(colors.muted)),
        ],
      ),
    );
  }
}
