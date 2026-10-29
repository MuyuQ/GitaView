namespace GitaView_Native;

// Accessed on the UI thread before scheduling reads and after they complete.
internal sealed class SnapshotVersionGate
{
    private string? _session;
    private ulong _revision;

    internal void BeginSession(string session)
    {
        if (_session == session) return;
        _session = session;
        _revision = 0;
    }

    internal bool TryAccept(string session, ulong revision)
    {
        if (session != _session || revision < _revision) return false;
        _revision = revision;
        return true;
    }
}
