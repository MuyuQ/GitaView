using GitaView.Native.Contracts;

namespace GitaView_Native.WidgetProvider;

// Callers enqueue IDs only. One worker resolves current instance state and reads
// data at execution time, so delayed requests cannot carry stale sizes/snapshots.
internal sealed class WidgetUpdateDispatcher(
    Func<string, int?> currentSize,
    Func<WidgetSnapshot?> readSnapshot,
    Action<string, int, WidgetSnapshot?> publish,
    Action afterUpdate,
    Action<string, Exception> reportError)
{
    private readonly object _gate = new();
    private readonly Queue<string> _pending = new();
    private readonly HashSet<string> _queued = new(StringComparer.Ordinal);
    private bool _running;
    private Task _worker = Task.CompletedTask;

    internal void Enqueue(string id)
    {
        lock (_gate)
        {
            if (_queued.Add(id)) _pending.Enqueue(id);
            if (_running) return;
            _running = true;
            _worker = Task.Run(Drain);
        }
    }

    internal Task WhenIdleAsync()
    {
        lock (_gate) return _worker;
    }

    private void Drain()
    {
        while (true)
        {
            string id;
            lock (_gate)
            {
                if (_pending.Count == 0)
                {
                    _running = false;
                    return;
                }
                id = _pending.Dequeue();
                _queued.Remove(id);
            }
            try
            {
                if (currentSize(id) is null) continue;
                var snapshot = readSnapshot();
                // Delete/deactivate/resize may have arrived while disk I/O ran.
                if (currentSize(id) is not { } size) continue;
                publish(id, size, snapshot);
                afterUpdate();
            }
            catch (Exception exception)
            {
                try { reportError(id, exception); }
                catch { /* A diagnostic failure must not strand queued updates. */ }
            }
        }
    }
}
