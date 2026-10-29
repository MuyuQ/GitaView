using System.Runtime.InteropServices;
using GitaView_Native;

namespace GitaView.Native.Tests;

public class NativeWindowFrameFacts
{
    private const uint WsPopup = 0x80000000;
    private const uint WsDlgFrame = 0x00400000;

    [Fact]
    public void removes_the_dialog_inset_without_moving_resizing_or_showing_the_window()
    {
        nint hwnd = CreateHiddenWindow(WsPopup | WsDlgFrame);
        try
        {
            Assert.True(GetWindowRect(hwnd, out var outerBefore));
            Assert.True(GetClientRect(hwnd, out var clientBefore));
            Assert.True(clientBefore.Width < outerBefore.Width);
            Assert.False(IsWindowVisible(hwnd));

            var result = NativeWindowFrame.RemoveDialogFrame(hwnd);

            Assert.NotEqual(0, result.Before & (int)WsDlgFrame);
            Assert.Equal(0, GetWindowLongW(hwnd, -16) & (int)WsDlgFrame);
            Assert.True(GetWindowRect(hwnd, out var outerAfter));
            Assert.True(GetClientRect(hwnd, out var clientAfter));
            Assert.Equal(outerBefore, outerAfter);
            Assert.Equal(outerAfter.Width, clientAfter.Width);
            Assert.Equal(outerAfter.Height, clientAfter.Height);
            Assert.False(IsWindowVisible(hwnd));
        }
        finally
        {
            Assert.True(DestroyWindow(hwnd));
        }
    }

    [Fact]
    public void reapplying_does_not_change_client_geometry_or_window_style()
    {
        nint hwnd = CreateHiddenWindow(WsPopup | WsDlgFrame);
        try
        {
            var first = NativeWindowFrame.RemoveDialogFrame(hwnd);
            Assert.True(GetClientRect(hwnd, out var clientBefore));

            var second = NativeWindowFrame.RemoveDialogFrame(hwnd);

            Assert.Equal(first.After, second.Before);
            Assert.Equal(second.Before, second.After);
            Assert.True(GetClientRect(hwnd, out var clientAfter));
            Assert.Equal(clientBefore, clientAfter);
            Assert.False(IsWindowVisible(hwnd));
        }
        finally
        {
            Assert.True(DestroyWindow(hwnd));
        }
    }

    private static nint CreateHiddenWindow(uint style)
    {
        nint hwnd = CreateWindowExW(0, "STATIC", "GitaView frame regression", style,
            100, 100, 500, 250, 0, 0, 0, 0);
        Assert.NotEqual(nint.Zero, hwnd);
        return hwnd;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct Rect
    {
        public int Left;
        public int Top;
        public int Right;
        public int Bottom;
        public readonly int Width => Right - Left;
        public readonly int Height => Bottom - Top;
    }

    [DllImport("user32.dll", ExactSpelling = true, CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern nint CreateWindowExW(uint extendedStyle, string className, string windowName,
        uint style, int x, int y, int width, int height, nint parent, nint menu, nint instance, nint parameter);

    [DllImport("user32.dll", ExactSpelling = true)]
    private static extern int GetWindowLongW(nint hwnd, int index);

    [DllImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool GetClientRect(nint hwnd, out Rect rect);

    [DllImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool GetWindowRect(nint hwnd, out Rect rect);

    [DllImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool IsWindowVisible(nint hwnd);

    [DllImport("user32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool DestroyWindow(nint hwnd);
}
