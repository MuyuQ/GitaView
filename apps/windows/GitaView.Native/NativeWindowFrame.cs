using System.ComponentModel;
using System.Runtime.InteropServices;

namespace GitaView_Native;

internal static class NativeWindowFrame
{
    private const int GwlStyle = -16;
    private const int WsDlgFrame = 0x00400000;

    internal static (int Before, int After) RemoveDialogFrame(nint hwnd)
    {
        Marshal.SetLastPInvokeError(0);
        int before = GetWindowLongW(hwnd, GwlStyle);
        int error = Marshal.GetLastPInvokeError();
        if (before == 0 && error != 0) throw new Win32Exception(error);
        if ((before & WsDlgFrame) == 0) return (before, before);

        Marshal.SetLastPInvokeError(0);
        int previous = SetWindowLongW(hwnd, GwlStyle, before & ~WsDlgFrame);
        error = Marshal.GetLastPInvokeError();
        if (previous == 0 && error != 0) throw new Win32Exception(error);

        // Recalculate the client area without moving, resizing, activating,
        // showing, or changing the window's z-order.
        const uint frameChanged = 0x0020 | 0x0001 | 0x0002 | 0x0004 | 0x0010;
        if (!SetWindowPos(hwnd, 0, 0, 0, 0, 0, frameChanged))
        {
            throw new Win32Exception(Marshal.GetLastPInvokeError());
        }
        return (before, GetWindowLongW(hwnd, GwlStyle));
    }

    // GWL_STYLE contains a 32-bit style value on both x86 and x64, not a pointer.
    [DllImport("user32.dll", ExactSpelling = true, SetLastError = true)]
    private static extern int GetWindowLongW(nint hwnd, int index);

    [DllImport("user32.dll", ExactSpelling = true, SetLastError = true)]
    private static extern int SetWindowLongW(nint hwnd, int index, int value);

    [DllImport("user32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool SetWindowPos(nint hwnd, nint insertAfter, int x, int y, int width, int height, uint flags);
}
