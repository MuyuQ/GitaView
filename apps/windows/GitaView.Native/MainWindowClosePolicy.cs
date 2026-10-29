namespace GitaView_Native;

internal static class MainWindowClosePolicy
{
    internal static void Handle(bool exitRequested, bool trayAvailable, Action hide, Action requestExit)
    {
        // Keep the XAML window alive while the existing exit flow stops the engine
        // and flushes widget data; a second close must not bypass that cleanup.
        if (exitRequested) return;
        if (trayAvailable) hide();
        else requestExit();
    }
}
