// 托盘浮窗内容组件：品牌 + 刷新时间 + 2×2 统计格 + 待处理列表 + 底部操作。
//
// 视觉规格：design/preview/index.html（docs/flutter-ui-redesign-plan.md §5）：
// - 头部：22px 标志 + 名称 15.5 + 时间 11.5 mono，下缘 lineSoft；
// - 统计 2×2：待推送 localAhead(teal) / 待拉取 remoteAhead(blue) /
//   需关注 diverged+error(red) / 未比较 noRemote(slate)；数字 17 mono w600，
//   值为 0 时 muted + w500；标签 11.5 muted；
// - 刷新失败：头部琥珀提示块（不覆盖统计与历史列表）；
// - 正文行：8px 点 + 名称 + 状态文字胶囊；分叉 / 失败行 red 7% 底；
//   补充行：noRemote > 0 时虚线上分隔的 fly-note；
// - 空态文案复用 TrayFlyoutPresentation.emptyMessage（纯逻辑层不动）；
// - 底部右对齐三按钮：打开 GitaView / 刷新 / 退出。
//
// 数据注入：本组件只接收 [summary] 参数与三个回调，不持有任何服务——
// 第二窗口（desktop_multi_window）的数据注入与窗口定位在 Windows 阶段接线。
// 注意：SummaryRepo 契约不含 path（紧凑投影刻意省略），故行内只有名称与
// 状态文字，不显示设计稿中的路径行。
library;

import 'package:fluent_ui/fluent_ui.dart';

import '../contracts/contracts.dart';
import '../presentation/error_text.dart';
import '../presentation/tray_flyout_presentation.dart';
import 'theme/tokens.dart';

/// 托盘浮窗内容。[summary] 为 null（数据尚未注入）时显示占位空态。
class FlyoutWindowContent extends StatelessWidget {
  final SummarySnapshot? summary;
  final VoidCallback onOpen;
  final VoidCallback onRefresh;
  final VoidCallback onExit;

  const FlyoutWindowContent({
    super.key,
    required this.summary,
    required this.onOpen,
    required this.onRefresh,
    required this.onExit,
  });

  @override
  Widget build(BuildContext context) {
    final colors = gvColorsOf(context);
    final snapshot = summary;
    final counts = snapshot?.counts ?? _zeroCounts;
    final refreshState = snapshot?.refreshState ?? RefreshState.pending;
    final attention = snapshot?.attention ?? const <SummaryRepo>[];
    return Container(
      width: 352,
      color: colors.flyoutTint,
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          // 头部：品牌 + 时间 + 统计格（+ 失败提示块），下缘 lineSoft。
          Container(
            width: double.infinity,
            padding: const EdgeInsets.fromLTRB(16, 15, 16, 14),
            decoration: BoxDecoration(
                border: Border(bottom: BorderSide(color: colors.lineSoft))),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  children: [
                    const GvBrandMark(size: 22),
                    const SizedBox(width: 9),
                    Text(
                      'GitaView',
                      style: GvTextStyles.body(colors.ink,
                              weight: GvTextStyles.semiBold)
                          .copyWith(fontSize: 15.5),
                    ),
                    const Spacer(),
                    Text(
                      snapshot?.lastSuccessAtMs == null
                          ? '尚未刷新'
                          : _formatTime(snapshot!.lastSuccessAtMs!),
                      style: GvTextStyles.mono(colors.muted,
                          size: GvFontSizes.small),
                    ),
                  ],
                ),
                const SizedBox(height: 14),
                _StatsGrid(counts: counts, colors: colors),
                if (refreshState == RefreshState.failed) ...[
                  const SizedBox(height: 11),
                  _FailedAlert(colors: colors, snapshot: snapshot),
                ],
              ],
            ),
          ),
          // 待处理列表 / 空态（超过 300px 滚动）。
          Flexible(
            child: ConstrainedBox(
              constraints: const BoxConstraints(maxHeight: 300),
              child: SingleChildScrollView(
                padding: const EdgeInsets.fromLTRB(10, 8, 10, 6),
                child: attention.isEmpty
                    ? Padding(
                        padding: const EdgeInsets.fromLTRB(8, 22, 8, 28),
                        child: Text(
                          snapshot == null
                              ? '正在读取仓库状态…'
                              : TrayFlyoutPresentation.emptyMessage(
                                  counts, refreshState),
                          textAlign: TextAlign.center,
                          style: GvTextStyles.subtitle(colors.muted)
                              .copyWith(fontSize: 12.5, height: 1.65),
                        ),
                      )
                    : Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          for (final item in attention)
                            _AttentionRow(item: item, colors: colors),
                          if (counts.noRemote > 0) ...[
                            const SizedBox(height: 8),
                            Padding(
                              padding:
                                  const EdgeInsets.symmetric(horizontal: 8),
                              child: _DashedDivider(color: colors.lineSoft),
                            ),
                            Padding(
                              padding:
                                  const EdgeInsets.fromLTRB(8, 9, 8, 4),
                              child: Text(
                                '另有 ${counts.noRemote} 个仓库未配置远端，未计入待处理。',
                                style: GvTextStyles.subtitle(colors.muted)
                                    .copyWith(height: 1.5),
                              ),
                            ),
                          ],
                        ],
                      ),
              ),
            ),
          ),
          // 底部操作区：右对齐安静按钮（上缘 lineSoft）。
          Container(
            width: double.infinity,
            padding: const EdgeInsets.fromLTRB(12, 9, 12, 11),
            decoration: BoxDecoration(
                border: Border(top: BorderSide(color: colors.lineSoft))),
            child: Row(
              mainAxisAlignment: MainAxisAlignment.end,
              children: [
                _QuietFlyoutButton(label: '打开 GitaView', onTap: onOpen),
                const SizedBox(width: 2),
                _QuietFlyoutButton(label: '刷新', onTap: onRefresh),
                const SizedBox(width: 2),
                _QuietFlyoutButton(label: '退出', onTap: onExit),
              ],
            ),
          ),
        ],
      ),
    );
  }

  static String _formatTime(int unixMs) {
    final time = DateTime.fromMillisecondsSinceEpoch(unixMs).toLocal();
    String two(int value) => value.toString().padLeft(2, '0');
    return '${two(time.hour)}:${two(time.minute)}:${two(time.second)}';
  }
}

/// 无数据时的零计数（统计格照常显示 0）。
const SummaryCounts _zeroCounts = SummaryCounts(
  total: 0,
  synced: 0,
  localAhead: 0,
  remoteAhead: 0,
  diverged: 0,
  noRemote: 0,
  error: 0,
  pending: 0,
);

/// 2×2 统计格（设计稿 fly-stats）。
class _StatsGrid extends StatelessWidget {
  final SummaryCounts counts;
  final GvColorSet colors;

  const _StatsGrid({required this.counts, required this.colors});

  @override
  Widget build(BuildContext context) {
    Widget tile(String label, int value, Color? tone) {
      final active = value > 0 && tone != null;
      return Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            '$value',
            style: GvTextStyles.mono(active ? tone : colors.muted, size: 17)
                .copyWith(
                    fontWeight:
                        active ? GvTextStyles.semiBold : GvTextStyles.medium,
                    height: 1.1),
          ),
          const SizedBox(height: 2),
          Text(label, style: GvTextStyles.subtitle(colors.muted)),
        ],
      );
    }

    Widget row(Widget left, Widget right) => Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Expanded(child: left),
            Expanded(child: right),
          ],
        );

    return Column(
      children: [
        row(
          tile('待推送', counts.localAhead, colors.teal),
          tile('待拉取', counts.remoteAhead, colors.blue),
        ),
        const SizedBox(height: 9),
        row(
          tile('需关注', counts.diverged + counts.error, colors.red),
          // 未比较永远不计入强调色（slate；0 时 muted）。
          tile('未比较', counts.noRemote,
              counts.noRemote > 0 ? colors.slate : null),
        ),
      ],
    );
  }
}

/// 刷新失败提示块（设计稿 fly-alert：amber 8% 底 + 26% 边）。
class _FailedAlert extends StatelessWidget {
  final GvColorSet colors;
  final SummarySnapshot? snapshot;

  const _FailedAlert({required this.colors, required this.snapshot});

  @override
  Widget build(BuildContext context) {
    return Container(
      width: double.infinity,
      padding: const EdgeInsets.symmetric(horizontal: 9, vertical: 7),
      decoration: BoxDecoration(
        color: colors.amber.withValues(alpha: 0.08),
        borderRadius: BorderRadius.circular(7),
        border: Border.all(color: Color.lerp(colors.line, colors.amber, 0.26)!),
      ),
      child: Row(
        children: [
          Icon(FluentIcons.warning, size: 14, color: colors.amber),
          const SizedBox(width: 7),
          Expanded(
            child: Text(
              '刷新失败：${ErrorText.forCode(snapshot?.refreshErrorCode)}',
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: GvTextStyles.subtitle(colors.amber),
            ),
          ),
        ],
      ),
    );
  }
}

/// 待处理列表条目（设计稿 fly-row：8px 点 + 名称 + 状态胶囊；
/// 分叉 / 失败行 red 7% 底）。
class _AttentionRow extends StatelessWidget {
  final SummaryRepo item;
  final GvColorSet colors;

  const _AttentionRow({required this.item, required this.colors});

  @override
  Widget build(BuildContext context) {
    final dotColor = _dotColor(colors);
    final statusColor = _statusColor(colors);
    final urgent =
        item.readState == ReadState.error || item.relation == Relation.diverged;
    return Container(
      margin: const EdgeInsets.only(bottom: 2),
      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 9),
      decoration: BoxDecoration(
        color: urgent ? colors.red.withValues(alpha: 0.07) : Colors.transparent,
        borderRadius: BorderRadius.circular(GvSpacing.radius),
      ),
      child: Row(
        children: [
          Container(
            width: 8,
            height: 8,
            decoration:
                BoxDecoration(color: dotColor, shape: BoxShape.circle),
          ),
          const SizedBox(width: 9),
          Expanded(
            child: Text(
              item.name,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: GvTextStyles.caption(colors.ink),
            ),
          ),
          const SizedBox(width: 9),
          Container(
            padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 2),
            decoration: BoxDecoration(
              color: statusColor.withValues(alpha: 0.11),
              borderRadius: BorderRadius.circular(999),
            ),
            child: Text(
              _statusText(item),
              style: GvTextStyles.subtitle(statusColor).copyWith(fontSize: 11),
            ),
          ),
        ],
      ),
    );
  }

  String _statusText(SummaryRepo item) => switch (item.readState) {
        ReadState.error => '读取失败',
        ReadState.pending => '待读',
        _ => switch (item.relation) {
            Relation.diverged => '分叉',
            Relation.localAhead => '本地领先',
            Relation.remoteAhead => '远端领先',
            _ => '',
          },
      };

  Color _dotColor(GvColorSet colors) => switch (item.readState) {
        ReadState.error => colors.red,
        ReadState.pending => colors.slate,
        _ => switch (item.relation) {
            Relation.diverged => colors.amber,
            Relation.localAhead => colors.teal,
            Relation.remoteAhead => colors.blue,
            _ => colors.slate,
          },
      };

  Color _statusColor(GvColorSet colors) => switch (item.readState) {
        ReadState.error => colors.red,
        ReadState.pending => colors.muted,
        _ => switch (item.relation) {
            Relation.diverged => colors.amber,
            Relation.localAhead => colors.teal,
            Relation.remoteAhead => colors.blue,
            _ => colors.muted,
          },
      };
}

/// 虚线分隔（设计稿 fly-note 的 dashed 上分隔线）。
class _DashedDivider extends StatelessWidget {
  final Color color;

  const _DashedDivider({required this.color});

  @override
  Widget build(BuildContext context) {
    return CustomPaint(
      size: const Size(double.infinity, 1),
      painter: _DashedPainter(color: color),
    );
  }
}

class _DashedPainter extends CustomPainter {
  final Color color;

  const _DashedPainter({required this.color});

  @override
  void paint(Canvas canvas, Size size) {
    final paint = Paint()..color = color;
    const double dashWidth = 4;
    const double gap = 3;
    double x = 0;
    while (x < size.width) {
      final width = (x + dashWidth) > size.width ? size.width - x : dashWidth;
      canvas.drawRect(Rect.fromLTWH(x, 0, width, size.height), paint);
      x += dashWidth + gap;
    }
  }

  @override
  bool shouldRepaint(_DashedPainter oldDelegate) => oldDelegate.color != color;
}

/// 浮窗安静按钮（设计稿 fly-btn：高 30、radius 7、12.5，右对齐操作区）。
class _QuietFlyoutButton extends StatelessWidget {
  final String label;
  final VoidCallback onTap;

  const _QuietFlyoutButton({required this.label, required this.onTap});

  @override
  Widget build(BuildContext context) {
    final colors = gvColorsOf(context);
    return HoverButton(
      onPressed: onTap,
      builder: (context, states) {
        final hovered = states.contains(WidgetState.hovered);
        final pressed = states.contains(WidgetState.pressed);
        return Container(
          height: 30,
          padding: const EdgeInsets.symmetric(horizontal: 13),
          alignment: AlignmentDirectional.center,
          decoration: BoxDecoration(
            color: pressed
                ? colors.hover.withValues(alpha: 0.8)
                : hovered
                    ? colors.hover.withValues(alpha: 0.55)
                    : Colors.transparent,
            borderRadius: BorderRadius.circular(7),
          ),
          child: Text(
            label,
            style: GvTextStyles.body(hovered ? colors.ink : colors.muted)
                .copyWith(fontSize: 12.5),
          ),
        );
      },
    );
  }
}
