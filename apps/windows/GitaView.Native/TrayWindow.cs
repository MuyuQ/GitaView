// Win32 托盘图标（N14）：正式 Shell_NotifyIcon 机制。
// - Explorer 重启后经 TaskbarCreated 广播重新注册图标；
// - 左键单击 → 打开主窗口；右键 → 原生弹出菜单（打开 / 刷新 / 退出）；
// - WM_POWERBROADCAST 恢复通知 → 唤醒回调（定时模式合并一次刷新）。
// 消息窗口创建在 UI 线程，经 XAML 消息泵分发；回调内先复制普通值再路由。

using System.Runtime.InteropServices;

namespace GitaView_Native;

internal sealed class TrayWindow : IDisposable
{
    public const uint WmAppTrayCallback = 0x8000; // WM_APP
    public const uint WmAppCommand = 0x8001;
    private const uint WmCommand = 0x0111;      // WM_COMMAND（菜单命令）
    private const uint WmPowerBroadcast = 0x0218;
    private const uint PbtApmResumeAutomatic = 0x0012;

    public const int CmdOpen = 1;
    public const int CmdRefresh = 2;
    public const int CmdExit = 3;

    private const uint NimAdd = 0x00000000;
    private const uint NimDelete = 0x00000002;
    private const uint NifMessage = 0x00000001;
    private const uint NifIcon = 0x00000002;
    private const uint NifTip = 0x00000004;

    private const uint WmLbuttonUp = 0x0202;
    private const uint WmRbuttonUp = 0x0205; // legacy 回调模式：lParam = 鼠标消息
    private const uint WmNull = 0x0000;
    private const uint WmClose = 0x0010;

    private static uint _taskbarCreatedMessage;
    private volatile bool _iconAdded;
    private nint _hwnd;
    private readonly string _tooltip;
    private readonly Action _onOpen;
    private readonly Action _onToggleFlyout;
    private readonly Action _onRefresh;
    private readonly Action _onExit;
    private readonly Action _onSystemResume;

    public bool IsAvailable => _hwnd != 0 && _iconAdded;

    public TrayWindow(string tooltip, Action onOpen, Action onToggleFlyout, Action onRefresh,
        Action onExit, Action onSystemResume)
    {
        _tooltip = tooltip;
        _onOpen = onOpen;
        _onToggleFlyout = onToggleFlyout;
        _onRefresh = onRefresh;
        _onExit = onExit;
        _onSystemResume = onSystemResume;
        _taskbarCreatedMessage = RegisterWindowMessageW("TaskbarCreated");
        _hwnd = CreateMessageWindow();
        AddIcon();
    }

    public void Dispose()
    {
        _reRegisterTimer?.Dispose();
        _reRegisterTimer = null;
        if (_hwnd != 0)
        {
            RemoveIcon();
            DestroyWindow(_hwnd);
            _hwnd = 0;
        }
    }

    // —— 图标 ——

    private void AddIcon()
    {
        if (_iconAdded || _hwnd == 0)
        {
            return;
        }
        var data = CreateNotifyIconData();
        if (Shell_NotifyIconW(NimAdd, ref data))
        {
            _iconAdded = true;
        }
    }

    private void RemoveIcon()
    {
        if (!_iconAdded)
        {
            return;
        }
        var data = CreateNotifyIconData();
        Shell_NotifyIconW(NimDelete, ref data);
        _iconAdded = false;
    }

    /// <summary>TaskbarCreated 后图标未恢复的有界重试：1s 间隔，最多 5 次。</summary>
    private void ScheduleReRegisterRetry()
    {
        lock (_reRegisterGate)
        {
            if (_iconAdded || _hwnd == 0)
            {
                _reRegisterTimer?.Dispose();
                _reRegisterTimer = null;
                return;
            }
            if (_reRegisterTimer is not null)
            {
                return; // 已有重试在排队
            }
            _reRegisterAttempts = 0;
            _reRegisterTimer = new System.Threading.Timer(_ =>
            {
                lock (_reRegisterGate)
                {
                    if (_iconAdded || _hwnd == 0 || ++_reRegisterAttempts > 5)
                    {
                        _reRegisterTimer?.Dispose();
                        _reRegisterTimer = null;
                        return;
                    }
                    AddIcon();
                    App.HostLog?.Log(_iconAdded ? "INFO" : "WARN", "tray.reregister.retry",
                        $"attempt={_reRegisterAttempts} ok={_iconAdded}");
                    if (_iconAdded)
                    {
                        _reRegisterTimer?.Dispose();
                        _reRegisterTimer = null;
                    }
                }
            }, null, 1000, 1000);
        }
    }

    private readonly object _reRegisterGate = new();
    private System.Threading.Timer? _reRegisterTimer;
    private int _reRegisterAttempts;

    /// <summary>托盘图标当前屏幕矩形（浮窗锚定；失败返回 null）。</summary>
    public bool TryGetIconRect(out Windows.Graphics.RectInt32 rect)
    {
        rect = default;
        if (_hwnd == 0)
        {
            return false;
        }
        var identifier = new NOTIFYICONIDENTIFIER
        {
            cbSize = (uint)Marshal.SizeOf<NOTIFYICONIDENTIFIER>(),
            hWnd = _hwnd,
            uID = 1,
        };
        if (Shell_NotifyIconGetRect(ref identifier, out var win32Rect) == 0
            && win32Rect.Right > win32Rect.Left)
        {
            rect = new Windows.Graphics.RectInt32
            {
                X = win32Rect.Left,
                Y = win32Rect.Top,
                Width = win32Rect.Right - win32Rect.Left,
                Height = win32Rect.Bottom - win32Rect.Top,
            };
            return true;
        }
        return false;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct NOTIFYICONIDENTIFIER
    {
        public uint cbSize;
        public nint hWnd;
        public uint uID;
        public Guid guidItem;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct TRAYRECT
    {
        public int Left;
        public int Top;
        public int Right;
        public int Bottom;
    }

    [DllImport("shell32.dll")]
    private static extern int Shell_NotifyIconGetRect(ref NOTIFYICONIDENTIFIER identifier, out TRAYRECT iconLocation);

    private NOTIFYICONDATAW CreateNotifyIconData()
    {
        return new NOTIFYICONDATAW
        {
            cbSize = (uint)Marshal.SizeOf<NOTIFYICONDATAW>(),
            hWnd = _hwnd,
            uID = 1,
            uFlags = NifMessage | NifIcon | NifTip,
            uCallbackMessage = WmAppTrayCallback,
            hIcon = LoadIconFromApp(),
            szTip = _tooltip,
        };
    }

    private static nint LoadIconFromApp()
    {
        string? iconPath = Path.Combine(AppContext.BaseDirectory, "Assets", "AppIcon.ico");
        if (File.Exists(iconPath))
        {
            var icon = LoadImageW(
                GetModuleHandleW(null), iconPath, ImageIcon, 0, 0,
                LrLoadFromFile | LrDefaultSize);
            if (icon != 0)
            {
                return icon;
            }
        }
        return LoadIconW(0, IdiApplication);
    }

    // —— 窗口 ——

    private nint CreateMessageWindow()
    {
        const string className = "GitaView.Native.TrayWindow";
        var wndClass = new WNDCLASSEXW
        {
            cbSize = (uint)Marshal.SizeOf<WNDCLASSEXW>(),
            lpfnWndProc = Marshal.GetFunctionPointerForDelegate(_wndProcDelegate),
            lpszClassName = className,
            hInstance = GetModuleHandleW(null),
        };
        ushort atom = RegisterClassExW(ref wndClass);
        if (atom == 0)
        {
            return 0;
        }
        _ = atom;
        // 隐藏的普通顶层窗口（不是 message-only）：Shell 回调与 TaskbarCreated
        // 广播都只发给顶层窗口。从不调用 ShowWindow，用户不可见。
        return CreateWindowExW(
            0, className, "GitaView.Native.TrayWindow", 0x00C00000 /* WS_OVERLAPPED */,
            0, 0, 0, 0, 0, 0, wndClass.hInstance, 0);
    }

    private static readonly WndProcDelegate _wndProcDelegate = WndProc;

    private delegate nint WndProcDelegate(nint hwnd, uint message, nint wParam, nint lParam);

    private static nint WndProc(nint hwnd, uint message, nint wParam, nint lParam)
    {
        if (message == WmAppTrayCallback)
        {
            var instance = Current;
            if (instance is null)
            {
                return 0;
            }
            uint mouse = (uint)(lParam & 0xFFFF);
            if (mouse == WmLbuttonUp)
            {
                // 左键 = 托盘浮窗（Magpie 式交互）；打开主窗口走右键菜单。
                instance._onToggleFlyout();
            }
            else if (mouse == WmRbuttonUp)
            {
                instance.ShowMenu();
            }
            return 0;
        }
        if (message == WmCommand)
        {
            // 模态 TrackPopupMenu 的选择以 WM_COMMAND 发给属主窗口。
            var instance = Current;
            if (instance is null)
            {
                return 0;
            }
            switch ((int)(wParam & 0xFFFF))
            {
                case CmdOpen: instance._onOpen(); break;
                case CmdRefresh: instance._onRefresh(); break;
                case CmdExit: instance._onExit(); break;
            }
            return 0;
        }
        if (message == WmAppCommand)
        {
            var instance = Current;
            if (instance is null)
            {
                return 0;
            }
            switch ((int)wParam)
            {
                case CmdOpen: instance._onOpen(); break;
                case CmdRefresh: instance._onRefresh(); break;
                case CmdExit: instance._onExit(); break;
            }
            return 0;
        }
        if (message == _taskbarCreatedMessage && _taskbarCreatedMessage != 0)
        {
            // Explorer 重启：任务栏重建，旧图标已随旧任务栏失效。必须先清本地
            // 标记再 NIM_ADD（review R07：修复前 _iconAdded 仍为 true，AddIcon
            // 直接返回，图标一去不回），失败时有界重试。
            var instance = Current;
            if (instance is not null)
            {
                instance._iconAdded = false;
                instance.AddIcon();
                instance.ScheduleReRegisterRetry();
                App.HostLog?.Log("INFO", "tray.taskbar_created",
                    $"readded={instance._iconAdded}");
            }
            return 0;
        }
        if (message == WmPowerBroadcast && wParam == PbtApmResumeAutomatic)
        {
            Current?._onSystemResume();
            return 1; // 处理了电源广播
        }
        if (message == WmClose)
        {
            return 0; // 退出流由 App 管理，忽略外部 WM_CLOSE
        }
        return DefWindowProcW(hwnd, message, wParam, lParam);
    }

    private bool _menuOpen;

    private void ShowMenu()
    {
        if (_menuOpen)
        {
            return; // 重入守卫：右键连点不再堆叠菜单
        }
        _menuOpen = true;
        try
        {
            App.HostLog?.Log("INFO", "tray.menu.show", "opening");
            nint menu = CreatePopupMenu();
            if (menu == 0)
            {
                App.HostLog?.Log("ERROR", "tray.menu.create_failed", "");
                return;
            }
            AppendMenuW(menu, MfString, CmdOpen, "打开 GitaView");
            AppendMenuW(menu, MfString, CmdRefresh, "刷新");
            AppendMenuW(menu, MfSeparator, 0, null);
            AppendMenuW(menu, MfString, CmdExit, "退出");
            // 托盘菜单标准修复：先 SetForegroundWindow 再 TrackPopupMenu。
            SetForegroundWindow(_hwnd);
            GetCursorPos(out var point);
            TrackPopupMenu(menu, TpmRightalign | TpmBottomalign | TpmRightbutton,
                point.X, point.Y, 0, _hwnd, IntPtr.Zero);
            PostMessageW(_hwnd, WmNull, 0, 0);
            DestroyMenu(menu);
        }
        finally
        {
            // 任何提前返回 / 异常都不把重入守卫留在 true（否则菜单永久失效）。
            _menuOpen = false;
        }
    }

    private static TrayWindow? Current;

    public static bool TryCreate(string tooltip, Action onOpen, Action onToggleFlyout,
        Action onRefresh, Action onExit, Action onSystemResume, out TrayWindow tray)
    {
        Current = null;
        tray = new TrayWindow(tooltip, onOpen, onToggleFlyout, onRefresh, onExit, onSystemResume);
        if (tray._hwnd == 0 || !tray._iconAdded)
        {
            return false;
        }
        Current = tray;
        return true;
    }

    // —— Win32 ——

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    private struct NOTIFYICONDATAW
    {
        public uint cbSize;
        public nint hWnd;
        public uint uID;
        public uint uFlags;
        public uint uCallbackMessage;
        public nint hIcon;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 128)]
        public string szTip;
        public uint dwState;
        public uint dwStateMask;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 256)]
        public string szInfo;
        public uint uVersion;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 64)]
        public string szInfoTitle;
        public uint dwInfoFlags;
    }

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    private struct WNDCLASSEXW
    {
        public uint cbSize;
        public uint style;
        public nint lpfnWndProc;
        public int cbClsExtra;
        public int cbWndExtra;
        public nint hInstance;
        public nint hIcon;
        public nint hCursor;
        public nint hbrBackground;
        public nint lpszMenuName;
        public string lpszClassName;
        public nint hIconSm;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct POINT
    {
        public int X;
        public int Y;
    }

    private const uint ImageIcon = 1;
    private const uint LrLoadFromFile = 0x00000010;
    private const uint LrDefaultSize = 0x00000040;
    private const nint IdiApplication = 32512;
    private const uint MfString = 0x00000000;
    private const uint MfSeparator = 0x00000800;
    private const uint TpmRightalign = 0x0008;
    private const uint TpmBottomalign = 0x0020;
    private const uint TpmRightbutton = 0x0002;

    [DllImport("user32.dll")]
    private static extern nint CreatePopupMenu();

    [DllImport("shell32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool Shell_NotifyIconW(uint message, ref NOTIFYICONDATAW data);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    private static extern ushort RegisterClassExW(ref WNDCLASSEXW wndClass);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    private static extern nint CreateWindowExW(
        uint exStyle, string className, string windowName, uint style,
        int x, int y, int width, int height, nint parent, nint menu, nint instance, nint param);

    [DllImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool DestroyWindow(nint hwnd);

    [DllImport("user32.dll")]
    private static extern nint DefWindowProcW(nint hwnd, uint message, nint wParam, nint lParam);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    private static extern uint RegisterWindowMessageW(string message);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    private static extern nint LoadImageW(nint instance, string name, uint type, int cx, int cy, uint flags);

    [DllImport("user32.dll")]
    private static extern nint LoadIconW(nint instance, nint iconName);

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode)]
    private static extern nint GetModuleHandleW(string? name);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool AppendMenuW(nint menu, uint flags, nint idNewItem, string? newItem);

    [DllImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool TrackPopupMenu(nint menu, uint flags, int x, int y, int reserved, nint hwnd, nint rect);

    [DllImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool DestroyMenu(nint menu);

    [DllImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool SetForegroundWindow(nint hwnd);

    [DllImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool GetCursorPos(out POINT point);

    [DllImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool PostMessageW(nint hwnd, uint message, nint wParam, nint lParam);
}
