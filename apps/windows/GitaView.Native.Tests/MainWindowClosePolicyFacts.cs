using GitaView_Native;

namespace GitaView.Native.Tests;

public class MainWindowClosePolicyFacts
{
    [Theory]
    [InlineData(false, true, 1, 0)]
    [InlineData(false, false, 0, 1)]
    [InlineData(true, true, 0, 0)]
    [InlineData(true, false, 0, 0)]
    public void close_routes_to_the_tray_or_graceful_exit_and_preserves_an_exit_in_progress(
        bool exitRequested, bool trayAvailable, int expectedHides, int expectedExits)
    {
        int hides = 0, exits = 0;
        MainWindowClosePolicy.Handle(exitRequested, trayAvailable, () => hides++, () => exits++);
        Assert.Equal(expectedHides, hides);
        Assert.Equal(expectedExits, exits);
    }

    [Fact]
    public void repeated_close_without_a_tray_starts_cleanup_once_and_keeps_the_window_visible()
    {
        bool exitRequested = false;
        int hides = 0, cleanupRequests = 0;
        void RequestExit() { exitRequested = true; cleanupRequests++; }

        MainWindowClosePolicy.Handle(exitRequested, false, () => hides++, RequestExit);
        MainWindowClosePolicy.Handle(exitRequested, false, () => hides++, RequestExit);

        Assert.Equal(1, cleanupRequests);
        Assert.Equal(0, hides);
    }
}
