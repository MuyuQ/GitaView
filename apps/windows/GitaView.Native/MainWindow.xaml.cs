using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace GitaView_Native;

/// <summary>
/// 主窗口：单 Frame 承载状态明细页与设置页（统一设计，无顶层导航）；
/// 托盘可用时关闭驻留，否则正常退出。页面 NavigationCacheMode=Required，返回保留上下文。
/// </summary>
public sealed partial class MainWindow : Window
{
    public MainWindow()
    {
        InitializeComponent();

        ExtendsContentIntoTitleBar = true;
        RootFrame.Navigated += (_, _) =>
        {
            if (RootFrame.Content is MainPage main) UseTitleBar(main.ActiveTitleBar);
        else if (RootFrame.Content is SettingsPage settings) UseTitleBar(settings.ActiveTitleBar);
        };
        RootShell.ActualThemeChanged += (_, _) => DispatcherQueue.TryEnqueue(UpdateCaptionButtons);
        RootShell.Loaded += (_, _) =>
        {
            UpdateCaptionButtons();
            App.HostLog?.Log("INFO", "ui.rendering",
                $"dpi={GetDpiForWindow(WinRT.Interop.WindowNative.GetWindowHandle(this))} rasterScale={RootShell.XamlRoot.RasterizationScale} font=Microsoft YaHei UI language=zh-CN");
        };
#if DEBUG
        var trayPreview = new Microsoft.UI.Xaml.Input.KeyboardAccelerator
        {
            Key = Windows.System.VirtualKey.F11,
            Modifiers = Windows.System.VirtualKeyModifiers.Control | Windows.System.VirtualKeyModifiers.Shift,
        };
        trayPreview.Invoked += (_, args) =>
        {
            args.Handled = true;
            (App.Current as App)?.PreviewTrayFlyout();
        };
        RootShell.KeyboardAccelerators.Add(trayPreview);
        // Native-resolution, lossless content export for typography QA. The system
        // caption buttons are outside the XAML tree and are not part of this export.
        var export = new Microsoft.UI.Xaml.Input.KeyboardAccelerator
        {
            Key = Windows.System.VirtualKey.F12,
            Modifiers = Windows.System.VirtualKeyModifiers.Control | Windows.System.VirtualKeyModifiers.Shift,
        };
        export.Invoked += async (_, args) =>
        {
            args.Handled = true;
            try { await ExportPreviewAsync(); }
            catch (Exception exception) { App.HostLog?.Log("ERROR", "ui.preview.failed", exception.Message); }
        };
        RootShell.KeyboardAccelerators.Add(export);
        RootShell.KeyboardAcceleratorPlacementMode = Microsoft.UI.Xaml.Input.KeyboardAcceleratorPlacementMode.Hidden;
#endif

        AppWindow.SetIcon("Assets/AppIcon.ico");
        if (Windows.Storage.ApplicationData.Current.LocalSettings.Values.TryGetValue("appearanceTheme", out var preference)
            && Enum.TryParse<ElementTheme>(preference as string, out var theme))
            RootShell.RequestedTheme = theme;

        // 先取消系统关闭：只有托盘可用时才隐藏；否则等待 App 完整退出。
        AppWindow.Closing += (_, args) =>
        {
            if (App.Current is App app)
            {
                args.Cancel = true;
                app.HandleMainWindowClose();
            }
        };

        ResizeForContent(840, 520);
        RootFrame.Navigate(typeof(MainPage));
    }

    private TitleBar? _activeTitleBar;
    public ElementTheme Appearance => RootShell.RequestedTheme;
    public ElementTheme ContentTheme => RootShell.ActualTheme;
    public void SetAppearance(ElementTheme theme)
    {
        RootShell.RequestedTheme = theme;
        Windows.Storage.ApplicationData.Current.LocalSettings.Values["appearanceTheme"] = theme.ToString();
    }
    public void UseTitleBar(TitleBar titleBar)
    {
        // Cached pages keep their controls alive. Only the visible title bar owns
        // the window's drag regions.
        if (_activeTitleBar is { } previous) previous.AutoRefreshDragRegions = false;
        _activeTitleBar = titleBar;
        SetTitleBar(titleBar);
        titleBar.AutoRefreshDragRegions = true;
        AppWindow.TitleBar.PreferredHeightOption = Microsoft.UI.Windowing.TitleBarHeightOption.Tall;
        UpdateCaptionButtons();
        if (titleBar.IsLoaded) titleBar.RecomputeDragRegions();
    }

    private void UpdateCaptionButtons()
    {
        var highContrast = new Windows.UI.ViewManagement.AccessibilitySettings().HighContrast;
        var name = highContrast ? "HighContrast" : ContentTheme == ElementTheme.Dark ? "Dark" : "Light";
        var dictionary = (ResourceDictionary)Application.Current.Resources.MergedDictionaries.Last().ThemeDictionaries[name];
        Windows.UI.Color ColorOf(string key) => ((Microsoft.UI.Xaml.Media.SolidColorBrush)dictionary[key]).Color;
        var chrome = AppWindow.TitleBar;
        chrome.ButtonBackgroundColor = Microsoft.UI.Colors.Transparent;
        chrome.ButtonInactiveBackgroundColor = Microsoft.UI.Colors.Transparent;
        chrome.ButtonForegroundColor = ColorOf("GvInkBrush");
        chrome.ButtonInactiveForegroundColor = ColorOf("GvMutedBrush");
        chrome.ButtonHoverBackgroundColor = ColorOf("GvHoverBrush");
        chrome.ButtonHoverForegroundColor = ColorOf("GvInkBrush");
        chrome.ButtonPressedBackgroundColor = ColorOf("GvLineBrush");
        chrome.ButtonPressedForegroundColor = ColorOf("GvInkBrush");
    }

#if DEBUG
    private async Task ExportPreviewAsync()
    {
        var scale = RootShell.XamlRoot.RasterizationScale;
        var bitmap = new Microsoft.UI.Xaml.Media.Imaging.RenderTargetBitmap();
        // The default overload already renders at the XamlRoot's display scale.
        // Passing scaled dimensions here would apply the monitor scale twice.
        await bitmap.RenderAsync(RootShell);
        var buffer = await bitmap.GetPixelsAsync();
        var pixels = new byte[checked((int)buffer.Length)];
        using (var reader = Windows.Storage.Streams.DataReader.FromBuffer(buffer)) reader.ReadBytes(pixels);
        var folder = await Windows.Storage.ApplicationData.Current.LocalFolder.CreateFolderAsync(
            "ui-previews", Windows.Storage.CreationCollisionOption.OpenIfExists);
        var page = RootFrame.Content is SettingsPage ? "settings" : "repositories";
        var file = await folder.CreateFileAsync($"{page}-{ContentTheme.ToString().ToLowerInvariant()}.png",
            Windows.Storage.CreationCollisionOption.ReplaceExisting);
        using var stream = await file.OpenAsync(Windows.Storage.FileAccessMode.ReadWrite);
        var encoder = await Windows.Graphics.Imaging.BitmapEncoder.CreateAsync(
            Windows.Graphics.Imaging.BitmapEncoder.PngEncoderId, stream);
        encoder.SetPixelData(Windows.Graphics.Imaging.BitmapPixelFormat.Bgra8,
            Windows.Graphics.Imaging.BitmapAlphaMode.Premultiplied,
            (uint)bitmap.PixelWidth, (uint)bitmap.PixelHeight, 96 * scale, 96 * scale, pixels);
        await encoder.FlushAsync();
        App.HostLog?.Log("INFO", "ui.preview.saved",
            $"{page} {bitmap.PixelWidth}x{bitmap.PixelHeight} logical={RootShell.ActualWidth}x{RootShell.ActualHeight} scale={scale}");
    }
#endif

    private void SetMinimumSize()
    {
        var scale = GetDpiForWindow(WinRT.Interop.WindowNative.GetWindowHandle(this)) / 96.0;
        if (AppWindow.Presenter is Microsoft.UI.Windowing.OverlappedPresenter presenter)
        {
            // 收缩窗口模式已移除（用户决策）：紧凑摘要由托盘浮窗承载。
            presenter.PreferredMinimumWidth = (int)(640 * scale);
            presenter.PreferredMinimumHeight = (int)(400 * scale);
        }
    }

    private void ResizeForContent(int width, int height)
    {
        SetMinimumSize();
        var hwnd = WinRT.Interop.WindowNative.GetWindowHandle(this);
        var scale = GetDpiForWindow(hwnd) / 96.0;
        AppWindow.Resize(new Windows.Graphics.SizeInt32((int)(width * scale), (int)((height + 40) * scale)));
    }

    [System.Runtime.InteropServices.DllImport("user32.dll")]
    private static extern uint GetDpiForWindow(nint hwnd);

    /// <summary>打开设置（保留状态页实例与上下文）。</summary>
    public void OpenSettings()
    {
        if (RootFrame.Content is not SettingsPage)
        {
            RootFrame.Navigate(typeof(SettingsPage));
        }
    }

    /// <summary>返回状态明细页。</summary>
    public void NavigateBackToStatus()
    {
        if (RootFrame.Content is not MainPage)
        {
            RootFrame.Navigate(typeof(MainPage));
        }
    }

    internal Task NavigateFromProtocolAsync(ProtocolRoute route)
    {
        NavigateBackToStatus();
        return RootFrame.Content is MainPage page
            ? page.NavigateFromProtocolAsync(route)
            : Task.FromException(new InvalidOperationException("无法打开仓库列表"));
    }
}
