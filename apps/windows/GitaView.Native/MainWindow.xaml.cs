using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace GitaView_Native;

/// <summary>
/// 主窗口：单 Frame 承载状态明细页与设置页（统一设计，无顶层导航）；
/// 关闭窗口 = 驻留托盘。页面 NavigationCacheMode=Required，返回保留上下文。
/// </summary>
public sealed partial class MainWindow : Window
{
    public MainWindow()
    {
        InitializeComponent();

        ExtendsContentIntoTitleBar = true;
        SetTitleBar(AppTitleBar);

        AppWindow.SetIcon("Assets/AppIcon.ico");

        // 关闭窗口 = 驻留托盘（引擎继续刷新）；托盘菜单"退出"才真正结束。
        AppWindow.Closing += (_, args) =>
        {
            if (App.Current is App app && app.ShouldStayResident())
            {
                args.Cancel = true;
                AppWindow.Hide();
            }
        };

        RootFrame.Navigate(typeof(MainPage));
    }

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
}
