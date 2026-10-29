using System.Runtime.InteropServices;
using GitaView_Native.WidgetProvider;

namespace GitaView.Native.Tests;

public class WidgetProviderLifetimeFacts
{
    [Fact]
    public void committed_shutdown_rejects_new_callbacks_before_factory_revocation()
    {
        var lifetime = new WidgetProviderLifetime(new object(), () => true);
        lifetime.RequestIdleExit();
        var calls = new List<string>();
        Assert.True(lifetime.TryShutdown(
            () => calls.Add("suspend"),
            () => calls.Add("resume"),
            () =>
            {
                calls.Add("revoke");
                var error = Assert.Throws<COMException>(() => lifetime.EnterCallback());
                Assert.Equal(WidgetProviderLifetime.ServerStopping, error.HResult);
                Assert.Equal(WidgetProviderLifetime.ServerStopping, lifetime.LockServer(true));
            }));
        Assert.Equal(new[] { "suspend", "revoke" }, calls);
    }

    [Fact]
    public async Task callback_accepted_before_exit_commit_is_allowed_to_complete()
    {
        var lifetime = new WidgetProviderLifetime(new object(), () => true);
        lifetime.RequestIdleExit();
        var entered = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        var release = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        var callback = Task.Run(async () =>
        {
            using var lease = lifetime.EnterCallback();
            entered.SetResult();
            await release.Task;
        });
        try
        {
            await entered.Task.WaitAsync(TimeSpan.FromSeconds(5));
            int resumed = 0, revoked = 0;
            Assert.False(lifetime.TryShutdown(() => { }, () => resumed++, () => revoked++));
            Assert.Equal(1, resumed);
            Assert.Equal(0, revoked);
            Assert.False(lifetime.ExitRequested.WaitOne(0));
        }
        finally { release.SetResult(); }
        await callback.WaitAsync(TimeSpan.FromSeconds(5));
        Assert.True(lifetime.ExitRequested.WaitOne(0));
        Assert.True(lifetime.TryShutdown(() => { }, () => { }, () => { }));
    }

    [Fact]
    public void new_instance_during_factory_suspension_cancels_shutdown()
    {
        var gate = new object();
        int instances = 0, resumed = 0, revoked = 0;
        var lifetime = new WidgetProviderLifetime(gate, () => instances == 0);
        lifetime.RequestIdleExit();
        Assert.False(lifetime.TryShutdown(
            () =>
            {
                using var callback = lifetime.EnterCallback();
                lock (gate)
                {
                    instances++;
                    lifetime.CancelIdleExit();
                }
            }, () => resumed++, () => revoked++));
        Assert.Equal(1, resumed);
        Assert.Equal(0, revoked);
        Assert.False(lifetime.ExitRequested.WaitOne(0));
        lock (gate) instances--;
        // Becoming empty alone does not bypass the next idle grace period.
        Assert.False(lifetime.TryShutdown(() => { }, () => { }, () => revoked++));
        lifetime.RequestIdleExit();
        Assert.True(lifetime.TryShutdown(() => { }, () => { }, () => revoked++));
        Assert.Equal(1, revoked);
    }

    [Fact]
    public void factory_locks_keep_server_alive_until_the_last_unlock()
    {
        var lifetime = new WidgetProviderLifetime(new object(), () => true);
        Assert.Equal(0, lifetime.LockServer(true));
        Assert.Equal(0, lifetime.LockServer(true));
        lifetime.RequestIdleExit();
        Assert.False(lifetime.ExitRequested.WaitOne(0));
        Assert.False(lifetime.TryShutdown(() => { }, () => { }, () => throw new Exception()));
        Assert.Equal(0, lifetime.LockServer(false));
        Assert.False(lifetime.ExitRequested.WaitOne(0));
        Assert.Equal(0, lifetime.LockServer(false));
        Assert.True(lifetime.ExitRequested.WaitOne(0));
        Assert.True(lifetime.TryShutdown(() => { }, () => { }, () => { }));
    }

    [Fact]
    public void disposing_one_lease_twice_cannot_release_another_callback()
    {
        var lifetime = new WidgetProviderLifetime(new object(), () => true);
        var first = lifetime.EnterCallback();
        var second = lifetime.EnterCallback();
        lifetime.RequestIdleExit();
        first.Dispose();
        first.Dispose();
        Assert.False(lifetime.TryShutdown(() => { }, () => { }, () => throw new Exception()));
        second.Dispose();
        Assert.True(lifetime.ExitRequested.WaitOne(0));
        Assert.True(lifetime.TryShutdown(() => { }, () => { }, () => { }));
    }

    [Fact]
    public void failed_factory_suspension_does_not_commit_shutdown()
    {
        var lifetime = new WidgetProviderLifetime(new object(), () => true);
        lifetime.RequestIdleExit();
        Assert.Throws<COMException>(() => lifetime.TryShutdown(
            () => throw new COMException(),
            () => throw new Exception("must not resume after failed suspend"),
            () => throw new Exception("must not revoke after failed suspend")));
        using var callback = lifetime.EnterCallback();
    }
}
