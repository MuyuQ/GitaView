using System.Text.Json.Nodes;
using GitaView.Native.Contracts;

namespace GitaView.Native.Tests;

public class WidgetDataReaderFacts : IDisposable
{
    private readonly string _directory = Path.Combine(
        Path.GetTempPath(), "gitaview-widget-reader-" + Guid.NewGuid().ToString("N"));
    private string DataPath => Path.Combine(_directory, "widget-data.json");

    public WidgetDataReaderFacts() => Directory.CreateDirectory(_directory);

    public void Dispose() => Directory.Delete(_directory, recursive: true);

    [Fact]
    public void reads_valid_cache_from_the_supplied_path()
    {
        File.WriteAllText(DataPath, WidgetCacheValidationFacts.ValidJson);
        var snapshot = WidgetDataReader.TryRead(DataPath);
        Assert.NotNull(snapshot);
        Assert.Equal("cache", snapshot.SessionId);
        Assert.Equal(1UL, snapshot.Generation);
        Assert.Equal(0U, snapshot.Counts.Total);
    }

    [Fact]
    public void missing_cache_returns_null() => Assert.Null(WidgetDataReader.TryRead(DataPath));

    [Theory]
    [InlineData("{ broken")]
    [InlineData("null")]
    [InlineData("{}")]
    public void malformed_or_incomplete_cache_returns_null(string json)
    {
        File.WriteAllText(DataPath, json);
        Assert.Null(WidgetDataReader.TryRead(DataPath));
    }

    [Fact]
    public void unknown_schema_returns_null()
    {
        var cache = JsonNode.Parse(WidgetCacheValidationFacts.ValidJson)!;
        cache["schemaVersion"] = 999;
        File.WriteAllText(DataPath, cache.ToJsonString());
        Assert.Null(WidgetDataReader.TryRead(DataPath));
    }

    [Fact]
    public void null_required_members_return_null()
    {
        var cache = JsonNode.Parse(WidgetCacheValidationFacts.ValidJson)!;
        cache["counts"] = null;
        File.WriteAllText(DataPath, cache.ToJsonString());
        Assert.Null(WidgetDataReader.TryRead(DataPath));
    }

    [Fact]
    public void sharing_failure_returns_null_and_next_read_recovers()
    {
        File.WriteAllText(DataPath, WidgetCacheValidationFacts.ValidJson);
        using (var locked = new FileStream(DataPath, FileMode.Open, FileAccess.ReadWrite, FileShare.None))
        {
            Assert.Null(WidgetDataReader.TryRead(DataPath));
        }
        Assert.NotNull(WidgetDataReader.TryRead(DataPath));
    }
}
