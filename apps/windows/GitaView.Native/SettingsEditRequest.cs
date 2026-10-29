using GitaView.Native.Contracts;

namespace GitaView_Native;

// Build on the UI thread, before scheduling work. The candidate and its
// revision always belong to the same loaded settings snapshot.
internal static class SettingsEditRequest
{
    internal static SettingsSaveInput ForGroups(SettingsData current, IReadOnlyList<GroupSettings> groups)
        => Capture(current, groups, current.DefaultGroupId, []);

    internal static SettingsSaveInput ForDefaultGroup(SettingsData current, string? groupId)
        => Capture(current, current.Groups, groupId, []);

    internal static SettingsSaveInput ForRepoGroup(SettingsData current, string repoId, string? groupId)
        => Capture(current, current.Groups, current.DefaultGroupId,
            [new() { RepoId = repoId, GroupId = groupId }]);

    private static SettingsSaveInput Capture(SettingsData current, IReadOnlyList<GroupSettings> groups,
        string? defaultGroupId, IReadOnlyList<RepoGroupAssignment> assignments) => new()
        {
            ExpectedRevision = current.Revision,
            Settings = new()
            {
                Groups = [.. groups], DefaultGroupId = defaultGroupId,
                Refresh = current.Refresh, RepoGroups = assignments,
            },
        };
}
