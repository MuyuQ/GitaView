using System.Text.Json;
using GitaView.Native.Contracts;
using GitaView_Native.WidgetProvider;

namespace GitaView.Native.Tests;

public class WidgetCardContentFacts
{
    private const ulong Now = 1_700_000_000_000;

    private static WidgetSnapshot Snapshot() => new()
    {
        SchemaVersion = 1, SessionId = "test", Generation = 1,
        StateRevision = 1, SettingsRevision = 1,
        AttemptedAtMs = Now - 1_000, LastSuccessAtMs = Now - 60_000,
        RefreshState = RefreshState.Ready,
        RefreshPolicy = new() { Enabled = true, IntervalMinutes = 5 },
        Counts = new()
        {
            Total = 3, Synced = 2, LocalAhead = 0, RemoteAhead = 0,
            Diverged = 1, NoRemote = 0, Error = 0, Pending = 0,
        },
        Compact = new() { Synced = 2, Syncable = 0, NeedsAttention = 1, NoRemote = 0 },
        Attention = [],
    };

    [Theory]
    [InlineData("repo")]
    [InlineData("设计系统与组件库")]
    [InlineData("repo \"quoted\" \\ path\n<&>")]
    public void attention_names_round_trip_without_extra_json_encoding(string name)
    {
        var snapshot = Snapshot() with
        {
            Attention = [new() { Id = "repo", Name = name, ReadState = ReadState.Ok, Relation = Relation.Diverged }],
        };
        using var data = JsonDocument.Parse(WidgetCardContent.DataJson(snapshot, Now));
        var row = data.RootElement.GetProperty("attention")[0];
        Assert.Equal(name, row.GetProperty("name").GetString());
        Assert.Equal("分叉", row.GetProperty("state").GetString());
        Assert.False(row.TryGetProperty("path", out _));
    }

    [Theory]
    [InlineData(0)]
    [InlineData(1)]
    [InlineData(2)]
    public void every_size_shows_failure_and_time_anomaly_without_hiding_cached_counts(int size)
    {
        var original = Snapshot();
        AssertStatusVisible(size, original with { RefreshState = RefreshState.Failed }, "刷新失败");
        AssertStatusVisible(size, original with { LastSuccessAtMs = Now + 60_000 }, "时间异常");
        AssertStatusVisible(size, original with
        {
            LastSuccessAtMs = Now + 60_000, RefreshState = RefreshState.Failed,
        }, "刷新失败", "时间异常");
        AssertStatusVisible(size, original with
        {
            LastSuccessAtMs = null, RefreshState = RefreshState.Failed,
        }, "刷新失败", "尚无成功结果");
        AssertStatusVisible(size, original with
        {
            LastSuccessAtMs = null, RefreshState = RefreshState.Pending,
        }, "正在读取");

        using var ready = JsonDocument.Parse(WidgetCardContent.DataJson(original, Now));
        using var template = JsonDocument.Parse(WidgetCardContent.TemplateForIndex(size));
        Assert.False(StatusVisible(template.RootElement, ready.RootElement));
        using var noData = JsonDocument.Parse(WidgetCardContent.DataJson(null, Now));
        Assert.True(StatusVisible(template.RootElement, noData.RootElement));
        Assert.Contains("打开 GitaView", noData.RootElement.GetProperty("stateText").GetString());
    }

    private static void AssertStatusVisible(int size, WidgetSnapshot snapshot, params string[] messages)
    {
        using var data = JsonDocument.Parse(WidgetCardContent.DataJson(snapshot, Now));
        using var template = JsonDocument.Parse(WidgetCardContent.TemplateForIndex(size));
        foreach (var message in messages)
            Assert.Contains(message, data.RootElement.GetProperty("stateText").GetString());
        Assert.Equal(snapshot.Counts.Total, data.RootElement.GetProperty("total").GetUInt32());
        Assert.Equal(snapshot.Counts.Synced, data.RootElement.GetProperty("synced").GetUInt32());
        Assert.True(StatusVisible(template.RootElement, data.RootElement));
    }

    [Theory]
    [InlineData(0)]
    [InlineData(1)]
    [InlineData(2)]
    public void every_size_displays_all_buckets_and_pending_with_uncompared_last(int size)
    {
        var snapshot = Snapshot() with
        {
            Counts = new()
            {
                Total = 28, Synced = 1, LocalAhead = 2, RemoteAhead = 3,
                Diverged = 4, Error = 5, Pending = 6, NoRemote = 7,
            },
            Compact = new() { Synced = 1, Syncable = 5, NeedsAttention = 9, NoRemote = 7 },
        };
        using var data = JsonDocument.Parse(WidgetCardContent.DataJson(snapshot, Now));
        var payload = data.RootElement;
        var fields = new[] { "synced", "syncable", "needsAttention", "pending", "noRemote" };
        Assert.Equal(new uint[] { 1, 5, 9, 6, 7 },
            fields.Select(field => payload.GetProperty(field).GetUInt32()));
        Assert.Equal(payload.GetProperty("total").GetUInt32(),
            fields.Aggregate(0u, (sum, field) => sum + payload.GetProperty(field).GetUInt32()));

        using var template = JsonDocument.Parse(WidgetCardContent.TemplateForIndex(size));
        var summary = string.Join(" ", template.RootElement.GetProperty("body").EnumerateArray()
            .Where(block => block.TryGetProperty("text", out _))
            .Select(block => block.GetProperty("text").GetString()));
        foreach (var field in fields) Assert.Contains("${" + field + "}", summary);
        Assert.True(summary.IndexOf("${noRemote}", StringComparison.Ordinal)
            > fields.Take(4).Max(field => summary.IndexOf("${" + field + "}", StringComparison.Ordinal)));
        using var noData = JsonDocument.Parse(WidgetCardContent.DataJson(null, Now));
        foreach (var field in fields) Assert.Equal(0u, noData.RootElement.GetProperty(field).GetUInt32());
    }

    // Resolve the status block's visibility binding against its serialized payload.
    // Only the template's simple boolean and string-equality forms are supported.
    private static bool StatusVisible(JsonElement template, JsonElement data)
    {
        var status = Assert.Single(template.GetProperty("body").EnumerateArray(),
            block => block.TryGetProperty("text", out var text) && text.GetString() == "${stateText}");
        var condition = status.GetProperty("$when").GetString()!;
        Assert.StartsWith("${", condition);
        Assert.EndsWith("}", condition);
        var expression = condition[2..^1].Trim();
        var equality = expression.Split("==", 2, StringSplitOptions.TrimEntries);
        return equality.Length == 2
            ? data.GetProperty(equality[0]).GetString() == equality[1].Trim('\'')
            : data.GetProperty(expression).GetBoolean();
    }
}
