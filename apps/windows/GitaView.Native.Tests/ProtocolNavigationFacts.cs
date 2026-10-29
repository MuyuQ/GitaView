using GitaView_Native;

namespace GitaView.Native.Tests;

public class ProtocolNavigationFacts
{
    [Theory]
    [InlineData("gitaview-native://open", null)]
    [InlineData("gitaview-native://open?confirmed=true&command=push", null)]
    [InlineData("gitaview-native://repo/my-repo", "my-repo")]
    [InlineData("GITAVIEW-NATIVE://REPO/My-Repo", "My-Repo")]
    [InlineData("gitaview-native://repo/%E8%AE%BE%E8%AE%A1%E7%B3%BB%E7%BB%9F", "设计系统")]
    [InlineData("gitaview-native://repo/repo%20name?confirmed=true#ignored", "repo name")]
    [InlineData("gitaview-native://repo/name%252Fpart", "name%2Fpart")]
    [InlineData("gitaview-native://repo", null)]
    [InlineData("gitaview-native://repo/", null)]
    [InlineData("gitaview-native://repo/id/extra", null)]
    [InlineData("gitaview-native://push/my-repo?confirmed=true", null)]
    [InlineData("gitaview-native://unknown", null)]
    [InlineData("gitaview-native://user@repo/my-repo", null)]
    [InlineData("gitaview-native://repo:123/my-repo", null)]
    [InlineData("https://repo/my-repo", null)]
    [InlineData("file:///C:/private/repo", null)]
    [InlineData("not a URI", null)]
    [InlineData(null, null)]
    public void accepts_only_navigation_routes_and_decodes_repository_ids_once(string? uri, string? repoId)
    {
        Assert.Equal(repoId, ProtocolRoute.Parse(uri).RepoId);
    }

    [Fact]
    public void cold_activation_waits_for_readiness_and_resolves_against_current_repositories()
    {
        var navigation = new ProtocolNavigation();
        var targets = new List<ProtocolRoute>();
        int lookups = 0;
        navigation.Request("gitaview-native://repo/pending-repo");
        Assert.Empty(targets);
        navigation.SetReady(id => { lookups++; return id == "pending-repo"; }, targets.Add);
        Assert.Equal("pending-repo", Assert.Single(targets).RepoId);
        Assert.Equal(1, lookups);
        navigation.SetReady(_ => throw new InvalidOperationException("must not replay"), targets.Add);
        Assert.Single(targets);
    }

    [Fact]
    public void latest_cold_navigation_replaces_an_older_request()
    {
        var navigation = new ProtocolNavigation();
        var targets = new List<ProtocolRoute>();
        navigation.Request("gitaview-native://repo/old-target");
        navigation.Request("gitaview-native://open");
        navigation.SetReady(_ => throw new InvalidOperationException("open needs no repo lookup"), targets.Add);
        Assert.Null(Assert.Single(targets).RepoId);
    }

    [Fact]
    public void hot_activation_navigates_each_time_and_unknown_or_deleted_repositories_return_to_all()
    {
        var navigation = new ProtocolNavigation();
        var targets = new List<ProtocolRoute>();
        var ids = new HashSet<string>(StringComparer.Ordinal) { "repo" };
        navigation.SetReady(ids.Contains, targets.Add);
        navigation.Request("gitaview-native://repo/repo?confirmed=true");
        navigation.Request("gitaview-native://open");
        navigation.Request("gitaview-native://unknown");
        ids.Remove("repo");
        navigation.Request("gitaview-native://repo/repo");
        Assert.Equal(new string?[] { "repo", null, null, null }, targets.Select(target => target.RepoId));
    }

    [Fact]
    public void repository_removed_before_initialization_finishes_is_not_selected()
    {
        var navigation = new ProtocolNavigation();
        var targets = new List<ProtocolRoute>();
        navigation.Request("gitaview-native://repo/removed");
        navigation.SetReady(_ => false, targets.Add);
        Assert.Null(Assert.Single(targets).RepoId);
    }
}
