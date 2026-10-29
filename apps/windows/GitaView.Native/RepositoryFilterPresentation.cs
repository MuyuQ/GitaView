using GitaView.Native.Contracts;

namespace GitaView_Native;

internal readonly record struct FilterChip<T>(T Value, string Label, bool Selected);

internal static class RepositoryFilterPresentation
{
    private static readonly (string Label, RelationFilter Filter)[] StatusOrder =
    [
        ("读取失败", RelationFilter.Error),
        ("分叉", RelationFilter.Diverged),
        ("远端领先", RelationFilter.RemoteAhead),
        ("本地领先", RelationFilter.LocalAhead),
        ("同步", RelationFilter.Synced),
        ("未比较", RelationFilter.NoRemote),
    ];

    internal static IReadOnlyList<FilterChip<string?>> Groups(
        uint total, IReadOnlyList<GroupCount> groups, string? selected)
    {
        var chips = new List<FilterChip<string?>>
        {
            new(null, $"全部分组 {total}", selected is null),
        };
        chips.AddRange(groups.Select(group => new FilterChip<string?>(
            group.Id, $"{group.Name} {group.Count}", group.Id == selected)));
        return chips;
    }

    internal static IReadOnlyList<FilterChip<RelationFilter>> Statuses(WidgetCounts counts, RelationFilter selected)
    {
        var chips = new List<FilterChip<RelationFilter>>
        {
            new(RelationFilter.All, $"全部 {counts.Total}", selected == RelationFilter.All),
        };
        foreach (var (label, filter) in StatusOrder)
        {
            uint count = filter switch
            {
                RelationFilter.Error => counts.Error,
                RelationFilter.Diverged => counts.Diverged,
                RelationFilter.RemoteAhead => counts.RemoteAhead,
                RelationFilter.LocalAhead => counts.LocalAhead,
                RelationFilter.Synced => counts.Synced,
                RelationFilter.NoRemote => counts.NoRemote,
                _ => 0,
            };
            if (count > 0)
            {
                chips.Add(new(filter, $"{label} {count}", filter == selected));
            }
        }
        return chips;
    }
}
