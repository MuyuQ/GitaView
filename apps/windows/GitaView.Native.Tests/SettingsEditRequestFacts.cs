using GitaView.Native.Contracts;
using GitaView.Native.Interop;
using GitaView_Native;

namespace GitaView.Native.Tests;

public class SettingsEditRequestFacts
{
    [Theory]
    [InlineData("groups")]
    [InlineData("default")]
    [InlineData("assignment")]
    public async Task delayed_edit_keeps_its_original_revision_and_cannot_overwrite_a_completed_edit(string operation)
    {
        string dir = TestEnvironment.TempDir("gitaview_settings_delayed");
        try
        {
            using var engine = TestEnvironment.CreateEngine(dir);
            var original = engine.SettingsGet();
            var pending = operation switch
            {
                "groups" => SettingsEditRequest.ForGroups(original,
                    [new() { Id = "late", Name = "Late edit" }]),
                "default" => SettingsEditRequest.ForDefaultGroup(original, null),
                _ => SettingsEditRequest.ForRepoGroup(original, "missing-repo", null),
            };
            var release = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
            var delayed = Task.Run(async () =>
            {
                await release.Task;
                return engine.SettingsSave(pending);
            });
            SettingsData committed;
            try
            {
                committed = engine.SettingsSave(SettingsEditRequest.ForGroups(original,
                    [new() { Id = "first", Name = "Completed edit" }]));
            }
            finally { release.SetResult(); }
            var error = await Assert.ThrowsAsync<GvException>(async () => await delayed);
            Assert.Equal("SETTINGS_CONFLICT", error.Code);
            Assert.Equal(original.Revision, pending.ExpectedRevision);
            Assert.Equal(committed.Revision, engine.SettingsGet().Revision);
            Assert.Equal("first", Assert.Single(engine.SettingsGet().Groups).Id);
        }
        finally { Directory.Delete(dir, recursive: true); }
    }

    [Fact]
    public void group_request_owns_its_candidate_list_and_preserves_other_settings()
    {
        var candidates = new List<GroupSettings> { new() { Id = "g", Name = "Group" } };
        var current = new SettingsData
        {
            SchemaVersion = 1, Revision = 7, Repos = [], Groups = [.. candidates],
            DefaultGroupId = "g", Refresh = new() { Enabled = false, IntervalMinutes = 30 },
        };
        var request = SettingsEditRequest.ForGroups(current, candidates);
        candidates.Clear();
        Assert.Equal("g", Assert.Single(request.Settings.Groups).Id);
        Assert.Equal("g", request.Settings.DefaultGroupId);
        Assert.Equal(current.Refresh, request.Settings.Refresh);
        Assert.Empty(request.Settings.RepoGroups);
    }
}
