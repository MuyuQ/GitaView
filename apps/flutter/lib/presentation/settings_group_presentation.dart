// 设置页分组展示逻辑（归属下拉选中项归类）。
//
// 语义来源：apps/windows/GitaView.Native/SettingsGroupPresentation.cs
// （C# 逐条对应移植）。
// - [SettingsGroupPresentation.selectedIndex]：归属下拉第 0 项是占位
//   （未分组），名称可能与占位相同，所以按 ID 而不是名称定位，偏移 +1；
// - [SettingsGroupPresentation.assignmentFromSelection]：把下拉的
//   SelectionChanged 归类为“忽略”（null）或“归属修改”。
//   容器再生（LoadFromEngine 重挂 ItemsSource）会把 SelectedIndex 瞬时重置为
//   -1 并触发回声事件；若据此发送空归属，会把用户刚保存的归属覆盖掉
//   （实测：归属保存约 230ms 后被回声事件写回空归属，revision +2 净效果为零）。
//   用户选择永远产生 >=0 的索引，-1 一定是回声。
// WinUI 下拉控件本身不移植：selectedIndex / options 是纯输入参数。
library;

import '../contracts/contracts.dart';

/// 设置页固定标签（C# SettingsPageTag 常量的对应物）。
abstract final class SettingsPageTag {
  /// 分组列表占位项（未归属仓库的展示名）。注意：允许普通分组也叫这个名字，
  /// 因此身份判定只认 ID。
  static const String ungrouped = '（未分组）';

  /// 视图分组筛选的“全部”标签。
  static const String allGroups = '全部分组';
}

/// 分组归属展示逻辑（C# 静态类的对应物）。
abstract final class SettingsGroupPresentation {
  /// 按分组 ID 在下拉数据源中定位选中索引（0 = 占位“未分组”）。
  /// ID 不存在（已删除）或为 null 时返回 0。
  static int selectedIndex(List<GroupSettings> groups, String? groupId) {
    for (var i = 0; i < groups.length; i++) {
      if (groups[i].id == groupId) return i + 1;
    }
    return 0;
  }

  /// 把归属下拉的选择归类为“忽略”（null）或“归属修改”。
  ///
  /// 忽略条件（与 C# 逐条对应）：
  /// - 索引越界（< 0 或 >= options.length）——-1 是容器再生回声；
  /// - 选中的分组 ID 与当前归属相同（重复选择）；
  /// - 选项标签已失效（找不到同名分组，按“未分组”解析）且当前本来就未归属。
  ///
  /// 索引 0 恒表示未分组（groupId null）。
  static RepoGroupAssignment? assignmentFromSelection(
    int selectedIndex,
    List<String> options,
    List<GroupSettings> groups,
    String repoId,
    String? currentGroupId,
  ) {
    if (selectedIndex < 0 || selectedIndex >= options.length) return null;
    final String? groupId = selectedIndex == 0
        ? null
        : _groupIdByName(groups, options[selectedIndex]);
    if (groupId == currentGroupId) return null;
    return RepoGroupAssignment(repoId: repoId, groupId: groupId);
  }

  /// 按名称找分组 ID；找不到（陈旧标签）返回 null（= 未分组）。
  /// 对应 C# `groups.FirstOrDefault(group => group.Name == option)?.Id`。
  static String? _groupIdByName(List<GroupSettings> groups, String name) {
    for (final group in groups) {
      if (group.name == name) return group.id;
    }
    return null;
  }
}
