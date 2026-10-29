using GitaView.Native.Contracts;
using GitaView_Native;

namespace GitaView.Native.Tests;

public class RepositoryFilterPresentationFacts
{
    [Theory]
    [InlineData(0u)]
    [InlineData(3u)]
    public void no_groups_still_provides_the_all_groups_choice(uint total)
    {
        var chip = Assert.Single(RepositoryFilterPresentation.Groups(total, [], null));
        Assert.Null(chip.Value);
        Assert.Equal($"全部分组 {total}", chip.Label);
        Assert.True(chip.Selected);
    }

    [Fact]
    public void repository_count_changes_update_all_groups_without_group_changes()
    {
        var before = Assert.Single(RepositoryFilterPresentation.Groups(1, [], null));
        var after = Assert.Single(RepositoryFilterPresentation.Groups(2, [], null));
        Assert.Equal(before.Value, after.Value);
        Assert.Equal("全部分组 2", after.Label);
    }

    [Fact]
    public void selection_uses_group_ids_even_when_names_contain_separators()
    {
        GroupCount[] groups =
        [
            new() { Id = "work", Name = "工作:项目|1", Count = 2 },
            new() { Id = "tools", Name = "工具", Count = 1 },
        ];
        var chips = RepositoryFilterPresentation.Groups(3, groups, "tools");
        Assert.Equal("工作:项目|1 2", chips[1].Label);
        Assert.Equal("tools", Assert.Single(chips.Where(chip => chip.Selected)).Value);
    }

    [Fact]
    public void status_choices_hide_zero_counts_and_keep_no_remote_last()
    {
        var counts = new WidgetCounts
        {
            Total = 4, Synced = 1, LocalAhead = 0, RemoteAhead = 0,
            Diverged = 1, NoRemote = 1, Error = 1, Pending = 0,
        };
        var chips = RepositoryFilterPresentation.Statuses(counts, RelationFilter.Diverged);
        Assert.Equal(
            new[] { RelationFilter.All, RelationFilter.Error, RelationFilter.Diverged, RelationFilter.Synced, RelationFilter.NoRemote },
            chips.Select(chip => chip.Value));
        Assert.Equal(RelationFilter.Diverged, Assert.Single(chips.Where(chip => chip.Selected)).Value);
        Assert.Equal("全部 4", chips[0].Label);
        Assert.Equal("未比较 1", chips[^1].Label);
    }
}
