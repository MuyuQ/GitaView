// review S04 回归：唯一后台写入器——多代投影合并为最后一次、失败有界重试、
// 可等待的停止 / flush、首刷前有效缓存保护。磁盘操作经注入钩子 / 临时目录，
// 不触碰包目录（ApplicationData 在测试进程不可用）。

using GitaView.Native.Contracts;

namespace GitaView.Native.Tests;

public class WidgetDataWriterFacts : IDisposable
{
    private readonly string _dir;

    public WidgetDataWriterFacts()
    {
        _dir = Path.Combine(Path.GetTempPath(), "gitaview_s04_" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(_dir);
        WidgetDataWriter.DataDirectory = _dir;
        WidgetDataWriter.RetryBaseDelay = TimeSpan.FromMilliseconds(10);
        WidgetDataWriter.WriteHookOverride = null;
    }

    public void Dispose()
    {
        // 停掉可能仍在运行的写入器，清空注入，避免跨测试污染。
        WidgetDataWriter.StopAsync().Wait(TimeSpan.FromSeconds(10));
        WidgetDataWriter.DataDirectory = null;
        WidgetDataWriter.WriteHookOverride = null;
        WidgetDataWriter.RetryBaseDelay = TimeSpan.FromSeconds(2);
        try
        {
            Directory.Delete(_dir, recursive: true);
        }
        catch (IOException)
        {
        }
    }

    private static WidgetSnapshot Snapshot(ulong generation, ulong settingsRevision) => new()
    {
        SchemaVersion = 1,
        SessionId = "s",
        Generation = generation,
        StateRevision = 1,
        SettingsRevision = settingsRevision,
        AttemptedAtMs = 1_000,
        LastSuccessAtMs = 1_000,
        RefreshState = RefreshState.Ready,
        RefreshPolicy = new RefreshPolicy { Enabled = true, IntervalMinutes = 5 },
        Counts = new WidgetCounts { Total = 1, Synced = 1, LocalAhead = 0, RemoteAhead = 0, Diverged = 0, NoRemote = 0, Error = 0, Pending = 0 },
        Compact = new CompactCounts { Synced = 1, Syncable = 0, NeedsAttention = 0, NoRemote = 0 },
        Attention = [],
    };

    private WidgetSnapshot? ReadWritten() => WidgetDataWriter.TryRead();

    [Fact]
    public async Task write_coalesces_to_latest_and_stop_flushes()
    {
        WidgetDataWriter.Start();
        for (var generation = 1ul; generation <= 50; generation++)
        {
            WidgetDataWriter.Write(Snapshot(generation, 1));
        }
        await WidgetDataWriter.StopAsync();

        var written = ReadWritten();
        Assert.NotNull(written);
        Assert.Equal(50ul, written!.Generation); // 多代合并为最后一次
        Assert.False(File.Exists(Path.Combine(_dir, "widget-data.json.tmp")), "no temp file left");

        // 二次 StopAsync 幂等。
        await WidgetDataWriter.StopAsync();
    }

    [Fact]
    public async Task write_failure_retries_bounded_then_recovers()
    {
        var failures = 0;
        WidgetDataWriter.WriteHookOverride = (path, json) =>
        {
            if (Interlocked.Increment(ref failures) <= 2)
            {
                throw new IOException("injected write failure");
            }
            // 第三次"成功"：模拟真实落盘（钩子替代真实写路径）。
            File.WriteAllText(path, json);
        };
        WidgetDataWriter.Start();
        WidgetDataWriter.Write(Snapshot(1, 1));
        // StopAsync 等待写入循环退出：重试耗尽注入失败后第三次成功。
        await WidgetDataWriter.StopAsync();

        Assert.Equal(3, failures);
        var written = ReadWritten();
        Assert.NotNull(written);
        Assert.Equal(1ul, written!.Generation);
    }

    [Fact]
    public async Task stop_without_any_write_exits_cleanly()
    {
        WidgetDataWriter.Start();
        await WidgetDataWriter.StopAsync();
        Assert.Null(ReadWritten()); // 无投影 → 无文件
    }

    [Fact]
    public async Task first_refresh_keeps_valid_cache_until_generation_advances_or_config_changes()
    {
        // 预写有效缓存：generation 3、settingsRevision 5（绕过写入器直接落盘）。
        File.WriteAllText(
            Path.Combine(_dir, "widget-data.json"),
            ContractsJson.Encode(Snapshot(3, 5)));

        // generation 0 + 同 settingsRevision：尚未首刷 → 保留缓存。
        WidgetDataWriter.Start();
        WidgetDataWriter.Write(Snapshot(0, 5));
        await WidgetDataWriter.StopAsync();
        Assert.Equal(3ul, ReadWritten()!.Generation);

        // generation 0 + 不同 settingsRevision：配置已变化 → 必须及时替换
        //（不能以保留历史为由展示旧配置内容）。
        WidgetDataWriter.Start();
        WidgetDataWriter.Write(Snapshot(0, 6));
        await WidgetDataWriter.StopAsync();
        var afterConfigChange = ReadWritten();
        Assert.NotNull(afterConfigChange);
        Assert.Equal(0ul, afterConfigChange!.Generation);
        Assert.Equal(6ul, afterConfigChange.SettingsRevision);

        // generation 1（首刷完成）→ 正常替换。
        WidgetDataWriter.Start();
        WidgetDataWriter.Write(Snapshot(1, 6));
        await WidgetDataWriter.StopAsync();
        Assert.Equal(1ul, ReadWritten()!.Generation);
    }
}
