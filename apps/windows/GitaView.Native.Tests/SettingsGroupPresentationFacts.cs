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

    private static readonly string[] Options = [SettingsPageTag.Ungrouped, "工作"];

    [Fact]
    public void container_rebind_echo_with_reset_index_is_ignored()
    {
        // 回归：LoadFromEngine 重挂 ItemsSource 时 SelectedIndex 瞬时重置为 -1，
        // 回声事件若按“未分组”发送空归属，会把刚保存的归属在 ~230ms 内覆盖掉。
        Assert.Null(SettingsGroupPresentation.AssignmentFromSelection(
            -1, Options, Groups, "repo-1", currentGroupId: "work"));
    }

    [Fact]
    public void out_of_range_selection_is_ignored()
        => Assert.Null(SettingsGroupPresentation.AssignmentFromSelection(
            Options.Length, Options, Groups, "repo-1", currentGroupId: null));

    [Fact]
    public void selecting_current_group_again_is_ignored()
        => Assert.Null(SettingsGroupPresentation.AssignmentFromSelection(
            1, Options, Groups, "repo-1", currentGroupId: "work"));

    [Fact]
    public void selecting_ungrouped_again_is_ignored()
        => Assert.Null(SettingsGroupPresentation.AssignmentFromSelection(
            0, Options, Groups, "repo-1", currentGroupId: null));

    [Fact]
    public void selecting_a_group_emits_assignment_with_group_identity()
    {
        var assignment = SettingsGroupPresentation.AssignmentFromSelection(
            1, Options, Groups, "repo-1", currentGroupId: null);
        Assert.NotNull(assignment);
        Assert.Equal("repo-1", assignment!.RepoId);
        Assert.Equal("work", assignment.GroupId);
    }

    [Fact]
    public void selecting_ungrouped_over_existing_group_emits_null_group_assignment()
    {
        var assignment = SettingsGroupPresentation.AssignmentFromSelection(
            0, Options, Groups, "repo-1", currentGroupId: "work");
        Assert.NotNull(assignment);
        Assert.Equal("repo-1", assignment!.RepoId);
        Assert.Null(assignment.GroupId);
    }

    [Fact]
    public void stale_option_label_resolves_to_null_group_and_unassigned_current_is_ignored()
        => Assert.Null(SettingsGroupPresentation.AssignmentFromSelection(
            1, [SettingsPageTag.Ungrouped, "已删除的组"], Groups, "repo-1", currentGroupId: null));
}
