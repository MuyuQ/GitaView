// 仓库列表筛选 chips（分组筛选 + 状态筛选）。
//
// 语义来源：apps/windows/GitaView.Native/RepositoryFilterPresentation.cs
// （C# 逐条对应移植）。
// - 分组筛选以“分组 ID”为筛选值（ID 稳定，名称可含分隔符），null 表示全部分组；
// - 状态筛选以 RelationFilter 为值，“全部”永远第一，其余按固定展示顺序排列，
//   计数为 0 的状态不出现，no_remote 永远最后（契约要求）。
// 标签均为“名称 计数”文本，不依赖颜色表达选中态。
library;

import '../contracts/contracts.dart';

/// 筛选 chip：值 + 文案 + 是否选中（C# `FilterChip<T>` record struct 的对应物）。
class FilterChip<T> {
  final T value;
  final String label;
  final bool selected;

  const FilterChip(this.value, this.label, this.selected);

  @override
  bool operator ==(Object other) =>
      other is FilterChip<T> &&
      other.value == value &&
      other.label == label &&
      other.selected == selected;

  @override
  int get hashCode => Object.hash(value, label, selected);

  @override
  String toString() => 'FilterChip($value, $label, $selected)';
}

/// 筛选 chips 生成（C# 静态类的对应物）。
abstract final class RepositoryFilterPresentation {
  /// 状态 chip 的固定展示顺序（非枚举声明顺序）：错误最前，no_remote 最后。
  static const List<(String, RelationFilter)> _statusOrder = [
    ('读取失败', RelationFilter.error),
    ('分叉', RelationFilter.diverged),
    ('远端领先', RelationFilter.remoteAhead),
    ('本地领先', RelationFilter.localAhead),
    ('同步', RelationFilter.synced),
    ('未比较', RelationFilter.noRemote),
  ];

  /// 分组筛选 chips。首项恒为“全部分组”（value null）；
  /// 无分组时也只有这一项，仓库数变化只改计数不改选项集合。
  static List<FilterChip<String?>> groups(
    int total,
    List<GroupCount> groups,
    String? selected,
  ) {
    final chips = <FilterChip<String?>>[
      FilterChip<String?>(null, '全部分组 $total', selected == null),
    ];
    for (final group in groups) {
      chips.add(FilterChip<String?>(
        group.id,
        '${group.name} ${group.count}',
        group.id == selected,
      ));
    }
    return List.unmodifiable(chips);
  }

  /// 状态筛选 chips。首项恒为“全部 {total}”；计数为 0 的状态隐藏；
  /// no_remote 永远最后。
  static List<FilterChip<RelationFilter>> statuses(
    SummaryCounts counts,
    RelationFilter selected,
  ) {
    final chips = <FilterChip<RelationFilter>>[
      FilterChip<RelationFilter>(
        RelationFilter.all,
        '全部 ${counts.total}',
        selected == RelationFilter.all,
      ),
    ];
    for (final (label, filter) in _statusOrder) {
      final int count = switch (filter) {
        RelationFilter.error => counts.error,
        RelationFilter.diverged => counts.diverged,
        RelationFilter.remoteAhead => counts.remoteAhead,
        RelationFilter.localAhead => counts.localAhead,
        RelationFilter.synced => counts.synced,
        RelationFilter.noRemote => counts.noRemote,
        RelationFilter.all => 0,
      };
      if (count > 0) {
        chips.add(FilterChip<RelationFilter>(filter, '$label $count',
            filter == selected));
      }
    }
    return List.unmodifiable(chips);
  }
}
