// 托盘浮窗内容组件：品牌 + 刷新时间 + 紧凑摘要 + 关注列表 + 底部操作。
//
// 行为规格：apps/windows/GitaView.Native/TrayFlyoutWindow.xaml(.cs)
// （语义对齐，不是像素复刻）：
// - 摘要行口径与主窗口底部一致：总数 · 待同步 · 需关注 · 未比较 [+ 待读]
//   [+ 刷新失败：原因]；总数为 0 时显示空态口径的摘要；
// - 空态文案复用 TrayFlyoutPresentation.emptyMessage（已移植纯逻辑）；
// - 关注列表条目 = 状态点 + 名称 + 状态文字（颜色仅辅助）；
// - 底部右对齐三按钮：打开 GitaView / 刷新 / 退出。
//
// 数据注入：本组件只接收 [summary] 参数与三个回调，不持有任何服务——
// 第二窗口（desktop_multi_window）的数据注入与窗口定位在 Windows 阶段接线。
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
    final counts = snapshot?.counts;
    final refreshState = snapshot?.refreshState ?? RefreshState.pending;
    return Container(
      width: 352,
      color: colors.flyoutTint,
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          // 头部：品牌 + 最近成功读取时间 + 摘要（下缘分隔线）。
          Container(
            width: double.infinity,
            padding:
                const EdgeInsets.fromLTRB(20, 16, 20, 12),
            decoration: BoxDecoration(
                border: Border(bottom: BorderSide(color: colors.line))),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  children: [
                    Text('GitaView', style: GvTextStyles.brand(colors.ink)),
                    const Spacer(),
                    Text(
                      snapshot?.lastSuccessAtMs == null
                          ? '尚未刷新'
                          : _formatTime(snapshot!.lastSuccessAtMs!),
                      style: GvTextStyles.caption(colors.muted)
                          .copyWith(fontSize: GvFontSizes.micro),
                    ),
                  ],
                ),
                const SizedBox(height: GvSpacing.small),
                Text(
                  _summaryText(counts, refreshState, snapshot),
                  style: GvTextStyles.caption(
                    refreshState == RefreshState.failed
                        ? colors.amberText
                        : colors.muted,
                  ).copyWith(height: 20 / GvFontSizes.caption),
                ),
              ],
            ),
          ),
          // 需注意仓库列表 / 空态（超过 300px 滚动）。
          Flexible(
            child: ConstrainedBox(
              constraints: const BoxConstraints(maxHeight: 300),
              child: SingleChildScrollView(
                padding: const EdgeInsets.all(12),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Padding(
                      padding:
                          const EdgeInsets.symmetric(horizontal: 10, vertical: 8),
                      child: Text(
                        counts == null
                            ? ''
                            : TrayFlyoutPresentation.emptyMessage(
                                counts, refreshState),
                        style:
                            GvTextStyles.caption(colors.muted).copyWith(height: 21 / 13),
                      ),
                    ),
                    for (final item in snapshot?.attention ?? const <SummaryRepo>[])
                      _AttentionRow(item: item),
                  ],
                ),
              ),
            ),
          ),
          // 底部操作区：右对齐安静按钮（上缘分隔线）。
          Container(
            width: double.infinity,
            padding: const EdgeInsets.fromLTRB(14, 10, 14, 12),
            decoration: BoxDecoration(
                border: Border(top: BorderSide(color: colors.line))),
            child: Row(
              mainAxisAlignment: MainAxisAlignment.end,
              children: [
                _QuietFlyoutButton(label: '打开 GitaView', onTap: onOpen),
                const SizedBox(width: 4),
                _QuietFlyoutButton(label: '刷新', onTap: onRefresh),
                const SizedBox(width: 4),
                _QuietFlyoutButton(label: '退出', onTap: onExit),
              ],
            ),
          ),
        ],
      ),
    );
  }

  /// 摘要行（C# RefreshContentAsync 的分支逐条对应）。
  String _summaryText(
    SummaryCounts? counts,
    RefreshState refreshState,
    SummarySnapshot? snapshot,
  ) {
    if (counts == null) return '正在读取仓库状态…';
    if (counts.total == 0) {
      return refreshState == RefreshState.failed
          ? '刷新失败：${ErrorText.forCode(snapshot?.refreshErrorCode)}'
          : '还没有添加仓库';
    }
    final syncable = counts.localAhead + counts.remoteAhead;
    final attention = counts.diverged + counts.error;
    var text = '${counts.total} 个仓库 · 待同步 $syncable'
        ' · 需关注 $attention · 未比较 ${counts.noRemote}';
    if (counts.pending > 0) text += ' · 待读 ${counts.pending}';
    if (refreshState == RefreshState.failed) {
      text += ' · 刷新失败：${ErrorText.forCode(snapshot?.refreshErrorCode)}';
    }
    return text;
  }

  static String _formatTime(int unixMs) {
    final time = DateTime.fromMillisecondsSinceEpoch(unixMs).toLocal();
    String two(int value) => value.toString().padLeft(2, '0');
    return '${two(time.hour)}:${two(time.minute)}:${two(time.second)}';
  }
}

/// 关注列表条目（C# FlyoutRepoVm：状态文字 + 状态点，颜色仅辅助）。
class _AttentionRow extends StatelessWidget {
  final SummaryRepo item;

  const _AttentionRow({required this.item});

  @override
  Widget build(BuildContext context) {
    final colors = gvColorsOf(context);
    return Container(
      margin: const EdgeInsets.only(left: 9, right: 9, bottom: 4),
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
      decoration: BoxDecoration(
        color: colors.rowSelected,
        borderRadius: BorderRadius.circular(GvSpacing.radius),
      ),
      child: Row(
        children: [
          Container(
            width: 8,
            height: 8,
            decoration: BoxDecoration(
              color: _dotColor(colors),
              shape: BoxShape.circle,
            ),
          ),
          Expanded(
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 12),
              child: Text(
                item.name,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: GvTextStyles.body(colors.ink),
              ),
            ),
          ),
          Text(
            _statusText(item),
            style: GvTextStyles.caption(_statusColor(colors)),
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
        _ => item.relation == Relation.diverged
            ? colors.red
            : (item.relation == Relation.localAhead ||
                    item.relation == Relation.remoteAhead)
                ? colors.amberText
                : colors.slate,
      };

  Color _statusColor(GvColorSet colors) => switch (item.readState) {
        ReadState.error => colors.red,
        ReadState.pending => colors.muted,
        _ => item.relation == Relation.diverged
            ? colors.red
            : (item.relation == Relation.localAhead ||
                    item.relation == Relation.remoteAhead)
                ? colors.amberText
                : colors.muted,
      };
}

/// 浮窗安静按钮（C# GvQuietButton，右对齐操作区）。
class _QuietFlyoutButton extends StatelessWidget {
  final String label;
  final VoidCallback onTap;

  const _QuietFlyoutButton({required this.label, required this.onTap});

  @override
  Widget build(BuildContext context) {
    final colors = gvColorsOf(context);
    return HoverButton(
      onPressed: onTap,
      builder: (context, states) => Container(
        padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 7),
        decoration: BoxDecoration(
          color: states.contains(WidgetState.pressed)
              ? colors.hover.withValues(alpha: 0.8)
              : states.contains(WidgetState.hovered)
                  ? colors.hover.withValues(alpha: 0.45)
                  : Colors.transparent,
          borderRadius: BorderRadius.circular(GvSpacing.radius),
        ),
        child: Text(label, style: GvTextStyles.body(colors.muted)),
      ),
    );
  }
}
