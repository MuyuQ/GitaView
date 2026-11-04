using GitaView.Native.Contracts;

namespace GitaView_Native;

internal static class SettingsGroupPresentation
{
    internal static int SelectedIndex(IReadOnlyList<GroupSettings> groups, string? groupId)
    {
        // Index 0 is a placeholder. Labels can match it, so identity uses IDs.
        for (int i = 0; i < groups.Count; i++)
        {
            if (groups[i].Id == groupId) return i + 1;
        }
        return 0;
    }

    /// <summary>
    /// 把归属下拉的 SelectionChanged 归类为“忽略”（返回 null）或“归属修改”。
    /// 容器再生（LoadFromEngine 重挂 ItemsSource）会把 SelectedIndex 瞬时重置为
    /// -1 并触发回声事件；若据此发送空归属，会把用户刚保存的归属覆盖掉
    /// （实测：归属保存约 230ms 后被回声事件写回空归属，revision +2 净效果为零）。
    /// 用户选择永远产生 ≥0 的索引，-1 一定是回声。
    /// </summary>
    internal static RepoGroupAssignment? AssignmentFromSelection(
        int selectedIndex,
        IReadOnlyList<string> options,
        IReadOnlyList<GroupSettings> groups,
        string repoId,
        string? currentGroupId)
    {
        if (selectedIndex < 0 || selectedIndex >= options.Count) return null;
        var groupId = selectedIndex == 0
            ? null
            : groups.FirstOrDefault(group => group.Name == options[selectedIndex])?.Id;
        if (groupId == currentGroupId) return null;
        return new RepoGroupAssignment { RepoId = repoId, GroupId = groupId };
    }
}

internal static class SettingsPageTag
{
    public const string Ungrouped = "（未分组）";
    public const string AllGroups = "全部分组";
}
