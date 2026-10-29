using System.Collections.Concurrent;
using GitaView.Native.Contracts;
using GitaView_Native.WidgetProvider;

namespace GitaView.Native.Tests;

public class WidgetUpdateDispatcherFacts
{
    private static WidgetSnapshot Snapshot(ulong generation) => new()
    {
        SchemaVersion = 1, SessionId = "test", Generation = generation,
        StateRevision = generation, SettingsRevision = 1, RefreshState = RefreshState.Ready,
        RefreshPolicy = new() { Enabled = false, IntervalMinutes = 5 },
        Counts = new() { Total = 0, Synced = 0, LocalAhead = 0, RemoteAhead = 0, Diverged = 0, NoRemote = 0, Error = 0, Pending = 0 },
        Compact = new() { Synced = 0, Syncable = 0, NeedsAttention = 0, NoRemote = 0 },
        Attention = [],
    };

    [Fact]
    public async Task slow_old_update_finishes_before_the_new_snapshot_and_size_are_published()
    {
        var entered = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        using var release = new ManualResetEventSlim();
        var publications = new ConcurrentQueue<(ulong Generation, int Size)>();
        var failures = new ConcurrentQueue<Exception>();
        int generation = 1, size = 0, reads = 0;
        var dispatcher = new WidgetUpdateDispatcher(
            _ => Volatile.Read(ref size),
            () => { Interlocked.Increment(ref reads); return Snapshot((ulong)Volatile.Read(ref generation)); },
            (_, currentSize, snapshot) =>
            {
                if (snapshot!.Generation == 1)
                {
                    entered.TrySetResult();
                    if (!release.Wait(TimeSpan.FromSeconds(5))) throw new TimeoutException();
                }
                publications.Enqueue((snapshot.Generation, currentSize));
            },
            () => { }, (_, error) => failures.Enqueue(error));

        dispatcher.Enqueue("repo");
        try
        {
            await entered.Task.WaitAsync(TimeSpan.FromSeconds(5));
            Volatile.Write(ref generation, 2);
            Volatile.Write(ref size, 2);
            dispatcher.Enqueue("repo");
            await Task.Delay(100);
            Assert.Equal(1, Volatile.Read(ref reads));
        }
        finally { release.Set(); }
        await dispatcher.WhenIdleAsync().WaitAsync(TimeSpan.FromSeconds(5));
        Assert.Empty(failures);
        Assert.Equal(new[] { (1UL, 0), (2UL, 2) }, publications.ToArray());
    }

    [Fact]
    public async Task pending_bursts_use_current_size_and_skip_deleted_or_inactive_widgets()
    {
        var entered = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        using var release = new ManualResetEventSlim();
        var sizes = new ConcurrentDictionary<string, int>(new[]
        {
            new KeyValuePair<string, int>("block", 0), new("resize", 0),
            new("deleted", 1), new("inactive", 1),
        });
        var publications = new ConcurrentQueue<(string Id, int Size)>();
        var failures = new ConcurrentQueue<Exception>();
        var dispatcher = new WidgetUpdateDispatcher(
            id => sizes.TryGetValue(id, out var size) ? size : null,
            () => Snapshot(1),
            (id, size, _) =>
            {
                if (id == "block")
                {
                    entered.TrySetResult();
                    if (!release.Wait(TimeSpan.FromSeconds(5))) throw new TimeoutException();
                }
                publications.Enqueue((id, size));
            }, () => { }, (_, error) => failures.Enqueue(error));

        dispatcher.Enqueue("block");
        try
        {
            await entered.Task.WaitAsync(TimeSpan.FromSeconds(5));
            for (int i = 0; i < 100; i++) dispatcher.Enqueue("resize");
            dispatcher.Enqueue("deleted");
            dispatcher.Enqueue("inactive");
            sizes["resize"] = 2;
            sizes.TryRemove("deleted", out _);
            sizes.TryRemove("inactive", out _);
        }
        finally { release.Set(); }
        await dispatcher.WhenIdleAsync().WaitAsync(TimeSpan.FromSeconds(5));
        Assert.Empty(failures);
        Assert.Equal(new[] { ("block", 0), ("resize", 2) }, publications.ToArray());
    }

    [Theory]
    [InlineData(-1)]
    [InlineData(2)]
    public async Task target_changes_during_snapshot_read_are_checked_before_publication(int updatedSize)
    {
        var reading = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        using var release = new ManualResetEventSlim();
        int size = 0;
        var publications = new ConcurrentQueue<int>();
        var failures = new ConcurrentQueue<Exception>();
        var dispatcher = new WidgetUpdateDispatcher(
            _ => Volatile.Read(ref size) is var current && current >= 0 ? current : null,
            () =>
            {
                reading.TrySetResult();
                if (!release.Wait(TimeSpan.FromSeconds(5))) throw new TimeoutException();
                return Snapshot(1);
            }, (_, currentSize, _) => publications.Enqueue(currentSize),
            () => { }, (_, error) => failures.Enqueue(error));
        dispatcher.Enqueue("repo");
        try
        {
            await reading.Task.WaitAsync(TimeSpan.FromSeconds(5));
            Volatile.Write(ref size, updatedSize);
        }
        finally { release.Set(); }
        await dispatcher.WhenIdleAsync().WaitAsync(TimeSpan.FromSeconds(5));
        Assert.Empty(failures);
        Assert.Equal(updatedSize < 0 ? [] : new[] { updatedSize }, publications.ToArray());
    }

    [Fact]
    public async Task failed_updates_and_diagnostics_do_not_strand_later_requests_or_idle_restarts()
    {
        var entered = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        using var release = new ManualResetEventSlim();
        var publications = new ConcurrentQueue<string>();
        int failures = 0;
        var dispatcher = new WidgetUpdateDispatcher(
            _ => 1, () => Snapshot(1),
            (id, _, _) =>
            {
                if (id == "fail")
                {
                    entered.TrySetResult();
                    if (!release.Wait(TimeSpan.FromSeconds(5))) throw new TimeoutException();
                    throw new InvalidOperationException("injected RPC failure");
                }
                publications.Enqueue(id);
            }, () => { }, (_, _) =>
            {
                Interlocked.Increment(ref failures);
                throw new IOException("injected diagnostics failure");
            });
        dispatcher.Enqueue("fail");
        try
        {
            await entered.Task.WaitAsync(TimeSpan.FromSeconds(5));
            dispatcher.Enqueue("after-failure");
        }
        finally { release.Set(); }
        await dispatcher.WhenIdleAsync().WaitAsync(TimeSpan.FromSeconds(5));
        dispatcher.Enqueue("after-idle");
        await dispatcher.WhenIdleAsync().WaitAsync(TimeSpan.FromSeconds(5));
        Assert.Equal(1, Volatile.Read(ref failures));
        Assert.Equal(new[] { "after-failure", "after-idle" }, publications.ToArray());
    }
}
