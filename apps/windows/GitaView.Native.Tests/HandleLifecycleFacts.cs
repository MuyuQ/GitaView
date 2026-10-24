// review R01/R02 回归：调用租约保证"取句柄 + 计数"原子；Dispose 期间在途
// 调用不会把已释放句柄传入 FFI；Dispose 幂等，关闭后新调用抛
// ObjectDisposedException，等待中的 SnapshotWait 经租约归还后安全销毁。

using GitaView.Native.Interop;

namespace GitaView.Native.Tests;

public class HandleLifecycleFacts
{
    [Fact]
    public void dispose_rejects_new_calls_and_is_idempotent()
    {
        string dir = TestEnvironment.TempDir("gitaview_r01_dispose");
        var engine = TestEnvironment.CreateEngine(dir);
        Assert.NotNull(engine.SettingsGet());

        engine.Dispose();
        Assert.Throws<ObjectDisposedException>(() => engine.SettingsGet());
        Assert.Throws<ObjectDisposedException>(() => engine.SnapshotGet());
        engine.Dispose(); // 二次释放是安全 no-op
    }

    [Fact]
    public void concurrent_calls_with_dispose_never_use_freed_handle()
    {
        string dir = TestEnvironment.TempDir("gitaview_r01_race");
        var engine = TestEnvironment.CreateEngine(dir);
        using var stop = new CancellationTokenSource();
        var errors = new System.Collections.Concurrent.ConcurrentBag<Exception>();
        var workers = Enumerable.Range(0, 4).Select(_ => Task.Run(() =>
        {
            while (!stop.IsCancellationRequested)
            {
                try
                {
                    var settings = engine.SettingsGet();
                    Assert.NotNull(settings);
                }
                catch (ObjectDisposedException)
                {
                    break; // 关闭后拒绝新租约：预期退出路径
                }
                catch (Exception exception)
                {
                    errors.Add(exception); // 其他失败 = 租约/句柄缺陷
                    break;
                }
            }
        })).ToList();

        Thread.Sleep(200); // 让业务线程先进入在途调用
        engine.Dispose();
        stop.Cancel();
        Assert.True(Task.WaitAll(workers.ToArray(), TimeSpan.FromSeconds(30)));
        Assert.Empty(errors);
    }

    [Fact]
    public void snapshot_wait_lease_is_returned_before_destroy()
    {
        // SnapshotWait 持租约期间 Dispose：RequestStop 让等待立即返回
        // （Ready 或 Stopping），Dispose 等租约归还后销毁——有界完成，不崩溃。
        string dir = TestEnvironment.TempDir("gitaview_r01_wait_lease");
        var engine = TestEnvironment.CreateEngine(dir);
        var waitTask = Task.Run(() => engine.SnapshotWait(ulong.MaxValue, 60_000));
        Thread.Sleep(300); // 等待已持租约进入 FFI

        var sw = System.Diagnostics.Stopwatch.StartNew();
        engine.Dispose();
        sw.Stop();
        Assert.True(sw.ElapsedMilliseconds < 30_000, "dispose must not deadlock");

        var result = waitTask.Result;
        Assert.True(
            result.Kind is SnapshotWaitKind.Ready or SnapshotWaitKind.Stopping,
            $"unexpected kind {result.Kind}");
    }

    [Fact]
    public void concurrent_disposes_serialize_entire_close()
    {
        // review S01 回归：带屏障的并发 Dispose。关闭 owner 串行整个序列
        // （stop → 排空 → 等待 → destroy → 清句柄），任何一方在 destroy 之后
        // 都不得再对旧句柄发起 native 调用；另一方幂等返回。
        string dir = TestEnvironment.TempDir("gitaview_s01_close_race");
        var engine = TestEnvironment.CreateEngine(dir);
        // 等待中的 SnapshotWait 持租约：close 的 RequestStop 使其立即归还。
        var waitTask = Task.Run(() => engine.SnapshotWait(ulong.MaxValue, 60_000));
        Thread.Sleep(300);

        using var barrier = new Barrier(2);
        var results = Enumerable.Range(0, 2).Select(_ => Task.Run(() =>
        {
            barrier.SignalAndWait(TimeSpan.FromSeconds(10));
            try
            {
                engine.Dispose();
                return "closed";
            }
            catch (GvException)
            {
                return "busy"; // 租约未排空的可恢复失败
            }
        })).ToList();

        Assert.True(Task.WaitAll(results.ToArray(), TimeSpan.FromSeconds(30)));
        // 无论谁完成关闭，最终状态一致：句柄已释放、新调用被拒绝、
        // 没有对已释放句柄的 native 调用（真实 DLL 下表现为无崩溃/无错误）。
        Assert.All(results, r => Assert.True(r.Status == TaskStatus.RanToCompletion, r.Status.ToString()));
        Assert.Throws<ObjectDisposedException>(() => engine.SettingsGet());

        var result = waitTask.Result;
        Assert.True(result.Kind is SnapshotWaitKind.Ready or SnapshotWaitKind.Stopping);
    }

    [Fact]
    public void dispose_retry_after_lease_drain_failure_completes_close()
    {
        // review S01：关闭失败（在途租约未在期限内排空）必须保留 owner，
        // 再次 Dispose 能继续完成关闭。
        string dir = TestEnvironment.TempDir("gitaview_s01_retry");
        var engine = TestEnvironment.CreateEngine(dir);
        // 0s 排空期限 + 在途调用风暴：首次 Dispose 大概率观察到未归还租约。
        engine.LeaseDrainTimeout = TimeSpan.Zero;
        using var stop = new CancellationTokenSource();
        var storm = Task.Run(() =>
        {
            while (!stop.IsCancellationRequested)
            {
                try
                {
                    var settings = engine.SettingsGet();
                    Assert.NotNull(settings);
                }
                catch (ObjectDisposedException)
                {
                    return;
                }
            }
        });
        Thread.Sleep(200); // 风暴已进入在途调用

        try
        {
            engine.Dispose();
        }
        catch (GvException)
        {
            // 可恢复失败：owner 保留，重试。
        }
        stop.Cancel();
        _ = storm.Wait(TimeSpan.FromSeconds(10)); // 等风暴退出（已被拒）

        // 重试关闭：必须完成（或首次已成功则幂等返回），并进入关闭后状态。
        engine.LeaseDrainTimeout = TimeSpan.FromSeconds(30);
        engine.Dispose();
        Assert.Throws<ObjectDisposedException>(() => engine.SettingsGet());
        engine.Dispose(); // 幂等
    }
}
