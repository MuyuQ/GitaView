using System.Runtime.InteropServices;

namespace GitaView_Native.WidgetProvider;

// Shares the instance table's gate. Exit is committed only after accepted
// callbacks finish; admission and that decision cannot pass each other.
internal sealed class WidgetProviderLifetime(object gate, Func<bool> isEmpty)
{
    internal const int ServerStopping = unchecked((int)0x80080008);
    private int _callbacks;
    private int _serverLocks;
    private bool _idleExitRequested;
    private bool _stopping;
    internal ManualResetEvent ExitRequested { get; } = new(false);

    internal IDisposable EnterCallback()
    {
        lock (gate)
        {
            if (_stopping) throw new COMException("The widget provider is stopping.", ServerStopping);
            _callbacks++;
            ExitRequested.Reset();
            return new CallbackLease(this);
        }
    }

    internal int LockServer(bool retain)
    {
        lock (gate)
        {
            if (retain)
            {
                if (_stopping) return ServerStopping;
                _serverLocks++;
                ExitRequested.Reset();
            }
            else if (_serverLocks > 0)
            {
                _serverLocks--;
                SignalIfIdleLocked();
            }
            return 0;
        }
    }

    internal void RequestIdleExit()
    {
        lock (gate)
        {
            if (_stopping || !isEmpty()) return;
            _idleExitRequested = true;
            SignalIfIdleLocked();
        }
    }

    internal void CancelIdleExit()
    {
        lock (gate)
        {
            _idleExitRequested = false;
            ExitRequested.Reset();
        }
    }

    internal bool TryShutdown(Action suspendClassObjects, Action resumeClassObjects, Action revokeClassObject)
    {
        // COM suspension stops fresh activation. Existing interfaces may still
        // invoke callbacks, so their admission must share the final check.
        suspendClassObjects();
        bool committed;
        lock (gate)
        {
            committed = _idleExitRequested && !_stopping && isEmpty()
                && _callbacks == 0 && _serverLocks == 0;
            if (committed) _stopping = true;
            else ExitRequested.Reset();
        }
        if (!committed)
        {
            resumeClassObjects();
            return false;
        }
        revokeClassObject();
        return true;
    }

    private void LeaveCallback()
    {
        lock (gate)
        {
            _callbacks--;
            SignalIfIdleLocked();
        }
    }

    private void SignalIfIdleLocked()
    {
        if (_idleExitRequested && !_stopping && isEmpty() && _callbacks == 0 && _serverLocks == 0)
            ExitRequested.Set();
    }

    private sealed class CallbackLease(WidgetProviderLifetime owner) : IDisposable
    {
        private WidgetProviderLifetime? _owner = owner;
        public void Dispose() => Interlocked.Exchange(ref _owner, null)?.LeaveCallback();
    }
}
