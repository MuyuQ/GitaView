// N16：Provider 侧 staleness 边界与 Rust widget.staleness 同语义的锁定测试。

using GitaView.Native.Contracts;
using GitaView_Native.WidgetProvider;

namespace GitaView.Native.Tests;

public class WidgetStalenessFacts
{
    private static WidgetSnapshot Snapshot(bool enabled, ulong intervalMinutes, ulong? lastSuccessMs) => new()
    {
        SchemaVersion = 1,
        SessionId = "s",
        Generation = 1,
        StateRevision = 1,
        SettingsRevision = 1,
        AttemptedAtMs = lastSuccessMs,
        LastSuccessAtMs = lastSuccessMs,
        RefreshState = RefreshState.Ready,
        RefreshPolicy = new RefreshPolicy { Enabled = enabled, IntervalMinutes = (uint)intervalMinutes },
        Counts = new WidgetCounts { Total = 0, Synced = 0, LocalAhead = 0, RemoteAhead = 0, Diverged = 0, NoRemote = 0, Error = 0, Pending = 0 },
        Compact = new CompactCounts { Synced = 0, Syncable = 0, NeedsAttention = 0, NoRemote = 0 },
        Attention = [],
    };

    private const ulong Last = 1_000_000_000;

    [Fact]
    public void manual_mode_never_expires()
    {
        var snapshot = Snapshot(enabled: false, 5, Last);
        Assert.Equal(WidgetStaleness.Manual, snapshot.ComputeStaleness(Last + 999_999_999));
    }

    [Fact]
    public void boundary_is_exclusive_two_intervals()
    {
        var snapshot = Snapshot(enabled: true, 5, Last);
        ulong threshold = 2 * 5 * 60_000;
        Assert.Equal(WidgetStaleness.Fresh, snapshot.ComputeStaleness(Last + threshold));
        Assert.Equal(WidgetStaleness.Stale, snapshot.ComputeStaleness(Last + threshold + 1));
        // 触发时刻必须越过边界（+1ms）。
        Assert.Equal(Last + threshold + 1, snapshot.StalenessBoundaryMs(Last));
    }

    [Fact]
    public void clock_rollback_is_future_time_and_not_scheduled()
    {
        var snapshot = Snapshot(enabled: true, 5, Last);
        Assert.Equal(WidgetStaleness.FutureTime, snapshot.ComputeStaleness(Last - 1));
        Assert.Null(snapshot.StalenessBoundaryMs(Last - 1));
    }

    [Fact]
    public void no_success_time_is_fresh_pending_state()
    {
        var snapshot = Snapshot(enabled: true, 5, null);
        Assert.Equal(WidgetStaleness.Fresh, snapshot.ComputeStaleness(ulong.MaxValue / 2));
    }

    [Fact]
    public void data_json_carries_stale_flag_and_no_paths()
    {
        // 成功时间取一分钟前（现实时钟内为新鲜）；过期卡由 staleness 边界测试锁定。
        ulong minuteAgo = unchecked((ulong)DateTimeOffset.UtcNow.ToUnixTimeMilliseconds()) - 60_000;
        var snapshot = Snapshot(enabled: true, 5, minuteAgo);
        using var doc = System.Text.Json.JsonDocument.Parse(WidgetCardContent.DataJson(snapshot));
        var root = doc.RootElement;
        Assert.False(root.GetProperty("stale").GetBoolean());
        Assert.Equal(0, root.GetProperty("total").GetInt32());
        // path / 远端字段不出现在卡片数据里（安全投影）。
        Assert.False(root.TryGetProperty("path", out _));
        Assert.False(root.TryGetProperty("remoteUrl", out _));
    }

    [Fact]
    public void null_snapshot_builds_nodata_placeholder()
    {
        using var doc = System.Text.Json.JsonDocument.Parse(WidgetCardContent.DataJson(null));
        var root = doc.RootElement;
        Assert.Equal("noData", root.GetProperty("state").GetString());
        Assert.Contains("GitaView", root.GetProperty("stateText").GetString());
    }

    [Fact]
    public void templates_exist_for_all_sizes()
    {
        Assert.NotEmpty(WidgetCardContent.TemplateForIndex(0));
        Assert.NotEmpty(WidgetCardContent.TemplateForIndex(1));
        Assert.NotEmpty(WidgetCardContent.TemplateForIndex(2));
    }
}
