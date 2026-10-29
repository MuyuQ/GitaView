using GitaView.Native.Contracts;
using GitaView_Native;

namespace GitaView.Native.Tests;

public class SettingsGroupPresentationFacts
{
    private static readonly GroupSettings[] Groups =
    [
        new() { Id = "work", Name = "工作" },
        new() { Id = "literal-ungrouped", Name = SettingsPageTag.Ungrouped },
    ];

    [Theory]
    [InlineData(null, 0)]
    [InlineData("deleted", 0)]
    [InlineData("work", 1)]
    [InlineData("literal-ungrouped", 2)]
    public void repository_group_is_selected_by_identity(string? id, int expected)
        => Assert.Equal(expected, SettingsGroupPresentation.SelectedIndex(Groups, id));

    [Fact]
    public void selection_tracks_group_reordering_even_when_display_name_matches_placeholder()
    {
        var reordered = Groups.Reverse().ToArray();
        Assert.Equal(1, SettingsGroupPresentation.SelectedIndex(reordered, "literal-ungrouped"));
        Assert.Equal(2, SettingsGroupPresentation.SelectedIndex(reordered, "work"));
        Assert.Equal(0, SettingsGroupPresentation.SelectedIndex([], "literal-ungrouped"));
    }
}
