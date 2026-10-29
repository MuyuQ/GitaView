namespace GitaView_Native;

// A null ID means the unfiltered repository list. IDs are only resolved against
// managed repositories; URI parameters never become paths or Git operations.
internal readonly record struct ProtocolRoute(string? RepoId)
{
    internal static ProtocolRoute Parse(string? value)
    {
        if (!Uri.TryCreate(value, UriKind.Absolute, out var uri)
            || !uri.Scheme.Equals("gitaview-native", StringComparison.OrdinalIgnoreCase)
            || uri.UserInfo.Length != 0 || uri.Port != -1
            || !uri.Host.Equals("repo", StringComparison.OrdinalIgnoreCase))
            return new(null);

        var path = uri.AbsolutePath;
        if (path.Length < 2 || path[0] != '/' || path[1..].Contains('/')) return new(null);
        var id = Uri.UnescapeDataString(path[1..]);
        return id.Length == 0 ? new(null) : new(id);
    }
}

internal sealed class ProtocolNavigation
{
    private ProtocolRoute? _pending;
    private Func<string, bool>? _repoExists;
    private Action<ProtocolRoute>? _navigate;

    internal void Request(string? uri)
    {
        _pending = ProtocolRoute.Parse(uri);
        DispatchPending();
    }

    internal void SetReady(Func<string, bool> repoExists, Action<ProtocolRoute> navigate)
    {
        _repoExists = repoExists;
        _navigate = navigate;
        DispatchPending();
    }

    private void DispatchPending()
    {
        if (_pending is not { } route || _repoExists is not { } exists || _navigate is not { } navigate)
            return;
        _pending = null;
        if (route.RepoId is { } id && !exists(id)) route = new(null);
        navigate(route);
    }
}
