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
}

internal static class SettingsPageTag
{
    public const string Ungrouped = "（未分组）";
    public const string AllGroups = "全部分组";
}
