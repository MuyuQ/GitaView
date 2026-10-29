// 托盘浮窗（借鉴 Magpie 交互）：点击托盘图标动效弹出，点击外部 / Esc 关闭。
// - 无边框、置顶、不进任务栏与 Alt+Tab；DWM 圆角 + 阴影（不自己画窗框）；
// - 位置锚定托盘图标上方（TrayWindow 提供图标矩形），失败回退工作区右下角；
// - 内容 = 紧凑摘要语义（WidgetSnapshot 安全投影：总数 · 待同步 · 需关注 ·
//   无远端 + 需注意仓库列表）+ 打开 / 刷新 / 退出；
// - 显示期间订阅 SnapshotArrived 实时更新；不读路径 / 凭据（同 Widget 投影边界）。

using GitaView.Native.Contracts;
using GitaView.Native.Interop;
using Microsoft.UI;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Animation;

namespace GitaView_Native;

/// <summary>浮窗内需要关注的仓库行。</summary>
public sealed class FlyoutRepoVm
{
    public string Name { get; init; } = "";
    public string StatusText { get; init; } = "";
    public Brush Dot { get; init; } = RepoRowVm.ThemeBrush("GvSlateBrush");
    public Brush StatusBrush { get; init; } = RepoRowVm.ThemeBrush("GvInkBrush");
}

public sealed partial class TrayFlyoutWindow : Window
{
    private bool _refreshBusy;
    private bool _subscribed;
    private readonly SnapshotVersionGate _contentVersions = new();
    private Storyboard? _entranceStoryboard;
    internal bool IsFlyoutVisible { get; private set; }
    private Windows.Graphics.RectInt32 _trayRect;
    private bool _hasTrayRect;

    public TrayFlyoutWindow()
    {
        InitializeComponent();
        ConfigureChrome();
        // App caches this window. System close (including Alt+F4) must hide it
        // so the next tray click can show the same live AppWindow again.
        AppWindow.Closing += (_, args) =>
        {
            args.Cancel = true;
            Dismiss();
        };
        Root.RequestedTheme = App.MainWindow?.Appearance ?? ElementTheme.Default;
        // 窗口高度跟随内容（固定 400 逻辑宽度）：内容布局变化即改窗口客户区并
        // 重新锚定；显示后再按窗口所在显示器的 DPI 校准。
        Root.SizeChanged += (_, e) => ResizeToContent(e.NewSize.Height);
        Root.Loaded += (_, _) => ResizeToContent(null);
        Activated += (_, args) =>
        {
            if (args.WindowActivationState == WindowActivationState.Deactivated)
            {
                Dismiss();
            }
        };
    }

    private void ConfigureChrome()
    {
        var presenter = AppWindow.Presenter as Microsoft.UI.Windowing.OverlappedPresenter
            ?? throw new InvalidOperationException("flyout requires overlapped presenter");
        presenter.SetBorderAndTitleBar(hasBorder: false, hasTitleBar: false);
        presenter.IsAlwaysOnTop = true;
        presenter.IsResizable = false;
        presenter.IsMinimizable = false;
        presenter.IsMaximizable = false;
        AppWindow.IsShownInSwitchers = false;
        AppWindow.Title = "GitaView";
        // DWM 圆角（含阴影）：无边框窗口仍获得系统圆角观感。
        ApplyChromeAttributes();
    }

    /// <summary>
    /// 圆角 + 去 DWM 边框。DwmSetWindowAttribute 第三参为 LPCVOID：必须传值的
    /// 地址（ref），按值传递会被当作指针解引用（0.1.4.4 闪退根因）。
    /// 边框属性在 SetBorderAndTitleBar / Show 之后可能被重置，显示后需重放。
    /// </summary>
    private void ApplyChromeAttributes()
    {
        var hwnd = WinRT.Interop.WindowNative.GetWindowHandle(this);
        // SetBorderAndTitleBar(false, false) can leave WS_DLGFRAME behind when
        // IsResizable is false (WinUI #9978). DWM border color does not remove it.
        string frameState;
        try
        {
            var frame = NativeWindowFrame.RemoveDialogFrame(hwnd);
            frameState = $"style=0x{frame.Before:X8}->0x{frame.After:X8}";
        }
        catch (System.ComponentModel.Win32Exception exception)
        {
            frameState = $"frame_error={exception.NativeErrorCode}";
            App.HostLog?.Log("ERROR", "flyout.frame.failed", frameState);
        }
        const int dwmwaWindowCornerPreference = 33;
        const int dwmwcpRound = 2;
        var corner = dwmwcpRound;
        var cornerResult = DwmSetWindowAttribute(hwnd, dwmwaWindowCornerPreference, ref corner, sizeof(int));
        const int dwmwaBorderColor = 34;
        var borderNone = unchecked((int)0xFFFFFFFE); // DWMWA_COLOR_NONE
        var borderResult = DwmSetWindowAttribute(hwnd, dwmwaBorderColor, ref borderNone, sizeof(int));
        App.HostLog?.Log("INFO", "flyout.chrome",
            $"{frameState} corner={cornerResult} border={borderResult}");
    }

    /// <summary>窗口尺寸跟随内容（固定 400 逻辑宽度）。缩放统一取
    /// GetDpiForWindow：显示前返回 96（先按估算了布局，显示后由
    /// ShowNearTray 的队列校准到真实 DPI），XamlRoot 在此场景不可靠。</summary>
    private void ResizeToContent(double? contentHeightDip)
    {
        var scale = GetDpiForWindow(WinRT.Interop.WindowNative.GetWindowHandle(this)) / 96.0;
        double heightDip = contentHeightDip ?? Root.ActualHeight;
        if (heightDip <= 0)
        {
            return; // 等待首次内容布局，不能把外框高度当作客户区高度。
        }
        int height = (int)Math.Ceiling(heightDip * scale);
        var request = new Windows.Graphics.SizeInt32(
            (int)Math.Round(400 * scale), height);
        AppWindow.ResizeClient(request);
        App.HostLog?.Log("INFO", "flyout.resize",
            $"scale={scale:F2} req={request.Width}x{request.Height} after={AppWindow.Size.Width}x{AppWindow.Size.Height}");
        PositionAboveTray();
    }

    /// <summary>在托盘图标上方显示（Toggle 用；已可见时由 App 先 Dismiss 再调这里）。</summary>
    public void ShowNearTray(Windows.Graphics.RectInt32? trayRect)
    {
        IsFlyoutVisible = true;
        Root.RequestedTheme = App.MainWindow?.Appearance ?? ElementTheme.Default;
        _hasTrayRect = trayRect is { Width: > 0, Height: > 0 };
        _trayRect = trayRect.GetValueOrDefault();
        PositionAboveTray();
        PlayEntrance();
        AppWindow.Show(activateWindow: true);
        Activate();
        ApplyChromeAttributes(); // 显示后重放：无边框切换可能重置边框颜色
        DispatcherQueue?.TryEnqueue(() =>
        {
            if (!IsFlyoutVisible)
            {
                return;
            }
            ResizeToContent(null);
            FlyoutOpenButton.Focus(FocusState.Programmatic);
        });
        _ = RefreshContentAsync();
        if (!_subscribed)
        {
            App.SnapshotArrived += OnSnapshotArrived;
            _subscribed = true;
        }
    }

    /// <summary>关闭浮窗（点击外部 / Esc / 再次点托盘 / 主窗口打开）。</summary>
    public void Dismiss()
    {
        IsFlyoutVisible = false;
        _entranceStoryboard?.Stop();
        _entranceStoryboard = null;
        if (_subscribed)
        {
            App.SnapshotArrived -= OnSnapshotArrived;
            _subscribed = false;
        }
        AppWindow.Hide();
    }

    private void OnSnapshotArrived(AppSnapshot snapshot)
    {
        DispatcherQueue?.TryEnqueue(() => _ = RefreshContentAsync());
    }

    // —— 位置与尺寸（高度跟随内容：Root.SizeChanged 回调到这里）——

    private void PositionAboveTray()
    {
        var hwnd = WinRT.Interop.WindowNative.GetWindowHandle(this);
        var scale = GetDpiForWindow(hwnd) / 96.0;
        var size = AppWindow.Size;
        var display = _hasTrayRect
            ? Microsoft.UI.Windowing.DisplayArea.GetFromRect(
                _trayRect, Microsoft.UI.Windowing.DisplayAreaFallback.Nearest)
            : Microsoft.UI.Windowing.DisplayArea.GetFromWindowId(
                AppWindow.Id, Microsoft.UI.Windowing.DisplayAreaFallback.Nearest);
        var work = display.WorkArea;
        FlyoutBounds? anchor = _hasTrayRect
            ? new(_trayRect.X, _trayRect.Y, _trayRect.Width, _trayRect.Height) : null;
        var point = TrayFlyoutPresentation.Position(
            new(work.X, work.Y, work.Width, work.Height), anchor, size.Width, size.Height, scale);
        if (AppWindow.Position.X != point.X || AppWindow.Position.Y != point.Y)
        {
            AppWindow.Move(new Windows.Graphics.PointInt32(point.X, point.Y));
        }
    }

    /// <summary>Magpie 式入场：自下 16px 滑入 + 淡入（180ms ease-out）。</summary>
    private void PlayEntrance()
    {
        _entranceStoryboard?.Stop();
        _entranceStoryboard = null;
        Root.Opacity = 1;
        Root.RenderTransform = null;
        if (!new Windows.UI.ViewManagement.UISettings().AnimationsEnabled)
        {
            return;
        }
        var translate = new TranslateTransform { Y = 16 };
        Root.RenderTransform = translate;
        Root.Opacity = 0;
        var storyboard = new Storyboard();
        var fade = new DoubleAnimation
        {
            To = 1,
            Duration = new Duration(TimeSpan.FromMilliseconds(180)),
            EasingFunction = new CubicEase { EasingMode = EasingMode.EaseOut },
        };
        Storyboard.SetTarget(fade, Root);
        Storyboard.SetTargetProperty(fade, "Opacity");
        var rise = new DoubleAnimation
        {
            To = 0,
            Duration = new Duration(TimeSpan.FromMilliseconds(180)),
            EasingFunction = new CubicEase { EasingMode = EasingMode.EaseOut },
        };
        Storyboard.SetTarget(rise, translate);
        Storyboard.SetTargetProperty(rise, "Y");
        storyboard.Children.Add(fade);
        storyboard.Children.Add(rise);
        _entranceStoryboard = storyboard;
        storyboard.Begin();
    }

    // —— 内容 ——

    private async Task RefreshContentAsync()
    {
        var engine = App.Engine;
        if (engine is null)
        {
            return;
        }
        WidgetSnapshot snapshot;
        _contentVersions.BeginSession(engine.SessionId);
        try
        {
            snapshot = await Task.Run(() => engine.WidgetSnapshot());
        }
        catch (Exception exception) when (exception is GvException or ObjectDisposedException)
        {
            return; // 停止中 / 已关闭：浮窗将随退出消失
        }
        if (!IsFlyoutVisible || !_contentVersions.TryAccept(snapshot.SessionId, snapshot.StateRevision)) return;
        FlyoutRefreshTime.Text = snapshot.LastSuccessAtMs is { } lastSuccess
            ? FormatTime(lastSuccess)
            : "尚未刷新";
        var counts = snapshot.Counts;
        FlyoutEmptyText.Text = TrayFlyoutPresentation.EmptyMessage(counts, snapshot.RefreshState);
        FlyoutSummary.Foreground = snapshot.RefreshState == RefreshState.Failed
            ? ThemeBrush("GvAmberTextBrush")
            : ThemeBrush("GvMutedBrush");
        if (counts.Total == 0)
        {
            FlyoutSummary.Text = snapshot.RefreshState == RefreshState.Failed
                ? $"刷新失败：{ErrorText.For(snapshot.RefreshErrorCode)}"
                : "还没有添加仓库";
            FlyoutEmptyText.Visibility = Visibility.Visible;
            AttentionList.ItemsSource = null;
            return;
        }
        var syncable = counts.LocalAhead + counts.RemoteAhead;
        var attention = counts.Diverged + counts.Error;
        var summary = $"{counts.Total} 个仓库 · 待同步 {syncable} · 需关注 {attention}"
            + $" · 未比较 {counts.NoRemote}";
        if (counts.Pending > 0)
        {
            summary += $" · 待读 {counts.Pending}";
        }
        if (snapshot.RefreshState == RefreshState.Failed)
        {
            summary += $" · 刷新失败：{ErrorText.For(snapshot.RefreshErrorCode)}";
        }
        FlyoutSummary.Text = summary;

        AttentionList.ItemsSource = snapshot.Attention.Select(item => new FlyoutRepoVm
        {
            Name = item.Name,
            StatusText = StatusTextOf(item),
            Dot = DotOf(item),
            StatusBrush = StatusBrushOf(item),
        }).ToList();
        FlyoutEmptyText.Visibility = snapshot.Attention.Count == 0
            ? Visibility.Visible
            : Visibility.Collapsed;
    }

    private static string StatusTextOf(WidgetRepo item) => item.ReadState switch
    {
        ReadState.Error => "读取失败",
        ReadState.Pending => "待读",
        _ => item.Relation switch
        {
            Relation.Diverged => "分叉",
            Relation.LocalAhead => "本地领先",
            Relation.RemoteAhead => "远端领先",
            _ => "",
        },
    };

    private static Brush DotOf(WidgetRepo item) => item.ReadState switch
    {
        ReadState.Error => ThemeBrush("GvRedBrush"),
        ReadState.Pending => ThemeBrush("GvSlateBrush"),
        _ => item.Relation switch
        {
            Relation.Diverged => ThemeBrush("GvRedBrush"),
            Relation.LocalAhead => ThemeBrush("GvAmberTextBrush"),
            Relation.RemoteAhead => ThemeBrush("GvAmberTextBrush"),
            _ => ThemeBrush("GvSlateBrush"),
        },
    };

    private static Brush StatusBrushOf(WidgetRepo item) => item.ReadState switch
    {
        ReadState.Error => ThemeBrush("GvRedBrush"),
        ReadState.Pending => ThemeBrush("GvMutedBrush"),
        _ when item.Relation == Relation.Diverged => ThemeBrush("GvRedBrush"),
        _ => item.Relation is Relation.LocalAhead or Relation.RemoteAhead
            ? ThemeBrush("GvAmberTextBrush")
            : ThemeBrush("GvMutedBrush"),
    };

    private static string FormatTime(ulong unixMs) =>
        DateTimeOffset.FromUnixTimeMilliseconds((long)unixMs).ToLocalTime().ToString("HH:mm:ss");

    internal static Brush ThemeBrush(string key)
    {
        var theme = App.MainWindow?.ContentTheme
            ?? (Application.Current.RequestedTheme == ApplicationTheme.Dark
                ? ElementTheme.Dark : ElementTheme.Light);
        var name = new Windows.UI.ViewManagement.AccessibilitySettings().HighContrast
            ? "HighContrast" : theme == ElementTheme.Dark ? "Dark" : "Light";
        var themeResources = Application.Current.Resources.MergedDictionaries.Last();
        return (Brush)((ResourceDictionary)themeResources.ThemeDictionaries[name])[key];
    }

    // —— 操作 ——

    private void OnDismissAccelerator(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args)
    {
        args.Handled = true;
        Dismiss();
    }

    private void OnOpenClick(object sender, RoutedEventArgs e)
    {
        Dismiss();
        if (App.MainWindow is { } window)
        {
            window.AppWindow.Show();
            window.Activate();
        }
    }

    private async void OnFlyoutRefreshClick(object sender, RoutedEventArgs e)
    {
        var engine = App.Engine;
        if (engine is null || _refreshBusy)
        {
            return;
        }
        _refreshBusy = true;
        FlyoutRefreshButton.IsEnabled = false;
        FlyoutRefreshButton.Content = "刷新中";
        try
        {
            var request = await Task.Run(() => engine.RefreshRequest());
            await Task.Run(async () =>
            {
                while (!engine.RefreshCompleted(request.RequestId))
                {
                    await Task.Delay(50);
                }
            });
        }
        catch (Exception exception)
        {
            App.HostLog?.Log("WARN", "flyout.refresh.failed",
                $"{exception.GetType().Name}: {exception.Message}");
        }
        finally
        {
            _refreshBusy = false;
            FlyoutRefreshButton.Content = "刷新";
            FlyoutRefreshButton.IsEnabled = true;
        }
    }

    private void OnExitClick(object sender, RoutedEventArgs e)
    {
        Dismiss();
        (App.Current as App)?.TriggerExit();
    }

    [System.Runtime.InteropServices.DllImport("user32.dll")]
    private static extern uint GetDpiForWindow(nint hwnd);

    [System.Runtime.InteropServices.DllImport("dwmapi.dll")]
    private static extern int DwmSetWindowAttribute(
        nint hwnd, int attribute, ref int value, int size);
}
