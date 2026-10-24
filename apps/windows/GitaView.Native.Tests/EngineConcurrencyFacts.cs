// N18 排障回归：SnapshotWait 不得阻塞其他 FFI 调用（ABI §9.7）。
// 修复前 C# _gate 被 SnapshotWait 持有至超时（15s），UI 线程所有调用排队
// （日志实测 ui.requery.slow duration_ms=15009，Rust 层 0ms）。

using System.Diagnostics;
using GitaView.Native.Interop;

namespace GitaView.Native.Tests;

public class EngineConcurrencyFacts
{
    [Fact]
    public void snapshot_wait_does_not_block_other_calls()
    {
        string dir = TestEnvironment.TempDir("gitaview_n18_concurrent");
        using var engine = TestEnvironment.CreateEngine(dir);

        // 后台等待（无变化可等 → 直到超时）。
        var waitTask = Task.Run(() => engine.SnapshotWait(ulong.MaxValue, 5_000));
        Thread.Sleep(300); // 确保等待已进入 FFI

        // 并行的读 / 写调用必须快速完成（旧锁下会阻塞至等待超时）。
        var sw = Stopwatch.StartNew();
        var settings = engine.SettingsGet();
        sw.Stop();
        Assert.NotNull(settings);
        Assert.True(sw.ElapsedMilliseconds < 1_000,
            $"SettingsGet blocked {sw.ElapsedMilliseconds}ms behind SnapshotWait");

        var sw2 = Stopwatch.StartNew();
        var view = engine.ViewQuery(new GitaView.Native.Contracts.ViewQuery());
        sw2.Stop();
        Assert.True(view.Rows.Count >= 0 && sw2.ElapsedMilliseconds < 1_000);

        // 手动刷新请求也不被等待阻塞。
        var sw3 = Stopwatch.StartNew();
        engine.RefreshRequest();
        sw3.Stop();
        Assert.True(sw3.ElapsedMilliseconds < 1_000);
    }

    [Fact]
    public void concurrent_calls_on_two_threads_both_succeed()
    {
        string dir = TestEnvironment.TempDir("gitaview_n18_threads");
        using var engine = TestEnvironment.CreateEngine(dir);

        var t1 = Task.Run(() => engine.SettingsGet());
        var t2 = Task.Run(() => engine.SnapshotGet());
        var t3 = Task.Run(() => engine.WidgetSnapshot());

        Assert.NotNull(t1.Result);
        Assert.NotNull(t2.Result);
        Assert.NotNull(t3.Result);
    }
}
