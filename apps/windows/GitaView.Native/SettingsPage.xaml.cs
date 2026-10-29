// 设置页（统一设计，升级自旧版 SettingsShell"仓库设置 / 常规设置"分区）：
// - 仓库设置：添加仓库（FolderPicker）、扫描目录与候选勾选、已管理仓库
//   （路径 / 分组 / 移除——只删管理记录）、分组管理；
// - 常规设置：刷新策略、默认分组、安全确认说明、系统启动状态、小组件说明；
// - 全部变更走 expectedRevision 事务；SETTINGS_CONFLICT 出重载入口；
// - FFI 在后台线程执行，结果回 UI 线程。

using System.Collections.ObjectModel;
using System.ComponentModel;
using GitaView.Native.Contracts;
using GitaView.Native.Interop;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Navigation;

namespace GitaView_Native;

/// <summary>已管理仓库行：名称 / 路径 / 分组下拉 / 移除。</summary>
public sealed class ManagedRepoVm
{
    public string RepoId { get; init; } = "";
    public string Name { get; init; } = "";
    public string Path { get; init; } = "";
    /// <summary>选项序：0=未分组，1..=分组（与设置保存映射保持一致）。</summary>
    public IReadOnlyList<string> GroupOptions { get; init; } = [];
    public int SelectedGroupIndex { get; set; }
}

/// <summary>分组管理行。</summary>
public sealed class GroupRowVm
{
    public string Id { get; init; } = "";
    public string Name { get; init; } = "";
}

public sealed partial class SettingsPage : Page
{
    public TitleBar ActiveTitleBar => PageTitleBar;
    private SettingsData _settings = new()
    {
        SchemaVersion = 1,
        Revision = 1,
        Repos = [],
        Groups = [],
        DefaultGroupId = null,
        Refresh = new RefreshPolicy { Enabled = true, IntervalMinutes = 5 },
    };
    private List<(string? Id, string Display)> _groupBoxes = [];
    private bool _suppressGroupEvents;

    public SettingsPage()
    {
        InitializeComponent();
        ActualThemeChanged += (_, _) => DispatcherQueue.TryEnqueue(() =>
            SelectSection(RepoSection.Visibility == Visibility.Visible));
    }

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        LoadFromEngine();
        SelectSection(repo: RepoSection.Visibility == Visibility.Visible);
    }

    private GitaViewEngine? Engine => App.Engine;

    private void LoadFromEngine()
    {
        var engine = Engine;
        if (engine is null)
        {
            return;
        }
        _settings = engine.SettingsGet();
        ConflictBar.IsOpen = false;
        RefreshSavedText.Visibility = Visibility.Collapsed;

        // —— 分组管理列表 ——
        _suppressGroupEvents = true;
        GroupList.ItemsSource = _settings.Groups
            .Select(group => new GroupRowVm { Id = group.Id, Name = group.Name })
            .ToList();

        // —— 已管理仓库（路径 / 分组 / 移除）——
        var groupOptions = new List<string> { SettingsPageTag.Ungrouped };
        groupOptions.AddRange(_settings.Groups.Select(group => group.Name));
        ManagedRepoList.ItemsSource = _settings.Repos
            .Select(repo =>
            {
                // 选项序 0=未分组；归属指向已删分组（异常数据）→ 未分组。
                var selected = SettingsGroupPresentation.SelectedIndex(_settings.Groups, repo.GroupId);
                return new ManagedRepoVm
                {
                    RepoId = repo.Id,
                    Name = repo.Name,
                    Path = repo.Path,
                    GroupOptions = groupOptions,
                    SelectedGroupIndex = selected,
                };
            })
            .ToList();
        NoManagedReposText.Visibility = _settings.Repos.Count == 0
            ? Visibility.Visible
            : Visibility.Collapsed;

        // —— 默认分组 ——
        _groupBoxes = [new(null, SettingsPageTag.AllGroups)];
        _groupBoxes.AddRange(_settings.Groups.Select(group => ((string?)group.Id, group.Name)));
        DefaultGroupBox.Items.Clear();
        foreach (var box in _groupBoxes)
        {
            DefaultGroupBox.Items.Add(box.Display);
        }
        var defaultIndex = _settings.DefaultGroupId is { } def
            ? _groupBoxes.FindIndex(box => box.Id == def)
            : 0;
        DefaultGroupBox.SelectedIndex = defaultIndex;

        // —— 刷新策略 ——
        RefreshEnabled.IsOn = _settings.Refresh.Enabled;
        RefreshInterval.Value = _settings.Refresh.IntervalMinutes;

        if (App.StartupWarnings.Count > 0)
        {
            StartupWarningsBar.IsOpen = true;
        }

        AppearanceBox.SelectedIndex = App.MainWindow?.Appearance switch
        {
            ElementTheme.Light => 1,
            ElementTheme.Dark => 2,
            _ => 0,
        };
        _suppressGroupEvents = false;
        _ = RefreshStartupStateAsync();
    }

    // —— 导航 ——

    private void OnBackClick(object sender, RoutedEventArgs e) =>
        App.MainWindow?.NavigateBackToStatus();

    private void OnTitleBarBackRequested(TitleBar sender, object args) =>
        App.MainWindow?.NavigateBackToStatus();

    private void OnNavRepoClick(object sender, RoutedEventArgs e) => SelectSection(repo: true);

    private void OnNavGeneralClick(object sender, RoutedEventArgs e) => SelectSection(repo: false);

    private void SelectSection(bool repo)
    {
        RepoSection.Visibility = repo ? Visibility.Visible : Visibility.Collapsed;
        GeneralSection.Visibility = repo ? Visibility.Collapsed : Visibility.Visible;
        NavRepoSection.Background = repo
            ? RepoRowVm.ThemeBrush("GvRowSelectedBrush") : null;
        NavGeneralSection.Background = !repo
            ? RepoRowVm.ThemeBrush("GvRowSelectedBrush") : null;
    }

    private void ShowMessage(string title, string message, bool isError)
    {
        ActionMessageBar.Title = title;
        ActionMessageBar.Message = message;
        ActionMessageBar.Severity = isError ? Microsoft.UI.Xaml.Controls.InfoBarSeverity.Error
            : Microsoft.UI.Xaml.Controls.InfoBarSeverity.Success;
        ActionMessageBar.IsOpen = true;
    }

    // —— 添加仓库（单个） ——

    private async void OnPickRepoClick(object sender, RoutedEventArgs e)
    {
        var folder = await PickFolderAsync("选择仓库目录");
        if (folder is null)
        {
            return;
        }
        AddRepoBusy.IsActive = true;
        AddRepoBusy.Visibility = Visibility.Visible;
        try
        {
            var engine = Engine!;
            await Task.Run(() => engine.RepoAdd(new RepoAddInput { Path = folder }));
            ShowMessage("已添加仓库", folder, isError: false);
            LoadFromEngine();
        }
        catch (Exception exception)
        {
            ShowMessage("添加仓库失败", MessageOf(exception), isError: true);
        }
        finally
        {
            AddRepoBusy.IsActive = false;
            AddRepoBusy.Visibility = Visibility.Collapsed;
        }
    }

    // —— 扫描目录与候选 ——

    private readonly List<(string Path, CheckBox Box)> _scanCandidates = [];

    private async void OnScanDirectoryClick(object sender, RoutedEventArgs e)
    {
        var folder = await PickFolderAsync("选择扫描根目录");
        if (folder is null)
        {
            return;
        }
        ScanBusy.IsActive = true;
        ScanBusy.Visibility = Visibility.Visible;
        ScanCandidatesPanel.Children.Clear();
        _scanCandidates.Clear();
        ScanActionsPanel.Visibility = Visibility.Collapsed;
        try
        {
            var engine = Engine!;
            var scan = await Task.Run(() => engine.ScanDirectory(folder));
            if (scan.Paths.Count == 0)
            {
                ShowMessage("扫描完成", "该目录下没有找到 Git 仓库。", isError: false);
            }
            foreach (var path in scan.Paths)
            {
                var box = new CheckBox
                {
                    Content = path,
                    IsChecked = true,
                    FontSize = 11,
                };
                _scanCandidates.Add((path, box));
                ScanCandidatesPanel.Children.Add(box);
            }
            ScanActionsPanel.Visibility = scan.Paths.Count > 0 ? Visibility.Visible : Visibility.Collapsed;
            if (scan.Truncated)
            {
                ShowMessage("扫描完成（结果被截断）", "候选过多，仅显示部分结果。", isError: false);
            }
        }
        catch (Exception exception)
        {
            ShowMessage("扫描失败", MessageOf(exception), isError: true);
        }
        finally
        {
            ScanBusy.IsActive = false;
            ScanBusy.Visibility = Visibility.Collapsed;
        }
    }

    private void OnSelectAllCandidatesClick(object sender, RoutedEventArgs e)
    {
        foreach (var (_, box) in _scanCandidates)
        {
            box.IsChecked = true;
        }
    }

    private async void OnAddCandidatesClick(object sender, RoutedEventArgs e)
    {
        var selected = _scanCandidates
            .Where(candidate => candidate.Box.IsChecked == true)
            .Select(candidate => candidate.Path)
            .ToList();
        if (selected.Count == 0)
        {
            return;
        }
        ScanBusy.IsActive = true;
        ScanBusy.Visibility = Visibility.Visible;
        var added = 0;
        var failures = new List<string>();
        try
        {
            var engine = Engine!;
            foreach (var path in selected)
            {
                try
                {
                    await Task.Run(() => engine.RepoAdd(new RepoAddInput { Path = path }));
                    added++;
                }
                catch (Exception)
                {
                    failures.Add(path);
                }
            }
            ShowMessage(added > 0 ? "已添加候选仓库" : "没有添加任何仓库",
                added > 0
                    ? $"成功 {added} 个{(failures.Count > 0 ? $"；失败 {failures.Count} 个（无效路径或重复）" : "")}"
                    : "所选候选均无效（不是 Git 仓库或已存在）",
                isError: added == 0);
            LoadFromEngine();
        }
        finally
        {
            ScanBusy.IsActive = false;
            ScanBusy.Visibility = Visibility.Collapsed;
        }
    }

    // —— 已管理仓库：分组与移除 ——

    private async void OnRepoGroupChanged(object sender, SelectionChangedEventArgs e)
    {
        if (_suppressGroupEvents)
        {
            return;
        }
        if ((sender as FrameworkElement)?.Tag as ManagedRepoVm is not { } row
            || sender is not ComboBox comboBox)
        {
            return;
        }
        // 与加载映射一致：0=未分组，1..=分组；按名称稳定映射回 ID。
        string? groupId = null;
        if (comboBox.SelectedIndex > 0 && comboBox.SelectedIndex < row.GroupOptions.Count)
        {
            groupId = _settings.Groups
                .FirstOrDefault(group => group.Name == row.GroupOptions[comboBox.SelectedIndex])?.Id;
        }
        if (_settings.Repos.FirstOrDefault(repo => repo.Id == row.RepoId)?.GroupId == groupId) return;
        var engine = Engine;
        if (engine is null) return;
        var input = SettingsEditRequest.ForRepoGroup(_settings, row.RepoId, groupId);
        try
        {
            await Task.Run(() => engine.SettingsSave(input));
            LoadFromEngine();
        }
        catch (GvException exception) when (exception.Code == "SETTINGS_CONFLICT")
        {
            ConflictBar.IsOpen = true;
        }
        catch (Exception exception)
        {
            ShowMessage("修改归属失败", MessageOf(exception), isError: true);
        }
    }

    private async void OnRemoveRepoClick(object sender, RoutedEventArgs e)
    {
        if ((sender as FrameworkElement)?.Tag as ManagedRepoVm is not { } row)
        {
            return;
        }
        // 移除只删除管理记录，不删除磁盘仓库；确认框走平台机制。
        var dialog = new ContentDialog
        {
            Title = "移除仓库",
            Content = $"从管理列表移除“{row.Name}”？\n\n{row.Path}\n\n磁盘上的仓库文件不会被删除。",
            PrimaryButtonText = "移除",
            CloseButtonText = "取消",
            DefaultButton = ContentDialogButton.Close,
            XamlRoot = XamlRoot,
        };
        if (await dialog.ShowAsync() != ContentDialogResult.Primary)
        {
            return;
        }
        try
        {
            var engine = Engine!;
            await Task.Run(() => engine.RepoRemove(row.RepoId));
            ShowMessage("已移除", row.Path, isError: false);
            LoadFromEngine();
        }
        catch (Exception exception)
        {
            ShowMessage("移除失败", MessageOf(exception), isError: true);
        }
    }

    // —— 分组管理 ——

    private async void OnAddGroupClick(object sender, RoutedEventArgs e)
    {
        var name = NewGroupNameBox.Text.Trim();
        if (name.Length == 0)
        {
            return;
        }
        if (_settings.Groups.Any(group => group.Name == name))
        {
            ShowMessage("分组已存在", name, isError: true);
            return;
        }
        NewGroupNameBox.Text = "";
        await SaveGroupsAsync(new List<GroupSettings>(_settings.Groups)
        {
            new() { Id = NewGroupId(), Name = name },
        });
    }

    private async void OnDeleteGroupClick(object sender, RoutedEventArgs e)
    {
        if ((sender as FrameworkElement)?.Tag as GroupRowVm is not { } row)
        {
            return;
        }
        var groups = _settings.Groups
            .Where(group => group.Id != row.Id)
            .ToList();
        await SaveGroupsAsync(groups);
    }

    private async Task SaveGroupsAsync(List<GroupSettings> groups)
    {
        var engine = Engine;
        if (engine is null)
        {
            return;
        }
        var input = SettingsEditRequest.ForGroups(_settings, groups);
        try
        {
            // 删除分组时核心对全部仓库清悬挂归属（空 repoGroups 路径）。
            await Task.Run(() => engine.SettingsSave(input));
            LoadFromEngine();
        }
        catch (GvException exception) when (exception.Code == "SETTINGS_CONFLICT")
        {
            ConflictBar.IsOpen = true;
        }
        catch (Exception exception)
        {
            ShowMessage("修改分组失败", MessageOf(exception), isError: true);
        }
    }

    private string NewGroupId()
    {
        var existing = _settings.Groups.Select(group => group.Id).ToHashSet();
        var index = 1;
        while (true)
        {
            var candidate = $"group-{index}";
            if (existing.Add(candidate))
            {
                return candidate;
            }
            index++;
        }
    }

    // —— 常规设置 ——

    private async void OnSaveRefreshClick(object sender, RoutedEventArgs e)
    {
        var engine = Engine;
        if (engine is null)
        {
            return;
        }
        var input = new SettingsSaveInput
        {
            ExpectedRevision = _settings.Revision,
            Settings = new SettingsEditable
            {
                Groups = [.. _settings.Groups],
                DefaultGroupId = _settings.DefaultGroupId,
                Refresh = new RefreshPolicy
                {
                    Enabled = RefreshEnabled.IsOn,
                    IntervalMinutes = (uint)Math.Clamp(double.IsNaN(RefreshInterval.Value) ? 5 : RefreshInterval.Value, 1, 60),
                },
                RepoGroups = [],
            },
        };
        if (sender is Button button) button.IsEnabled = false;
        try
        {
            await Task.Run(() => engine.SettingsSave(input));
            LoadFromEngine();
            RefreshSavedText.Visibility = Visibility.Visible;
        }
        catch (GvException exception) when (exception.Code == "SETTINGS_CONFLICT")
        {
            ConflictBar.IsOpen = true;
        }
        catch (Exception exception)
        {
            ShowMessage("保存刷新设置失败", MessageOf(exception), isError: true);
        }
        finally
        {
            if (sender is Button savedButton) savedButton.IsEnabled = true;
        }
    }

    private async void OnDefaultGroupChanged(object sender, SelectionChangedEventArgs e)
    {
        if (_suppressGroupEvents || DefaultGroupBox.SelectedIndex < 0)
        {
            return;
        }
        var engine = Engine;
        if (engine is null)
        {
            return;
        }
        var selectedDefault = DefaultGroupBox.SelectedIndex > 0
            ? _groupBoxes[DefaultGroupBox.SelectedIndex].Id
            : null;
        if (selectedDefault == _settings.DefaultGroupId)
        {
            return;
        }
        var input = SettingsEditRequest.ForDefaultGroup(_settings, selectedDefault);
        try
        {
            await Task.Run(() => engine.SettingsSave(input));
            LoadFromEngine();
        }
        catch (GvException exception) when (exception.Code == "SETTINGS_CONFLICT")
        {
            ConflictBar.IsOpen = true;
        }
        catch (Exception exception)
        {
            ShowMessage("修改默认分组失败", MessageOf(exception), isError: true);
        }
    }

    private void OnReloadClick(object sender, RoutedEventArgs e) => LoadFromEngine();

    private void OnAppearanceChanged(object sender, SelectionChangedEventArgs e)
    {
        if (_suppressGroupEvents || AppearanceBox.SelectedIndex < 0) return;
        App.MainWindow?.SetAppearance(AppearanceBox.SelectedIndex switch
        {
            1 => ElementTheme.Light,
            2 => ElementTheme.Dark,
            _ => ElementTheme.Default,
        });
    }

    // —— 开机启动（平台能力；显示系统实际状态）——

    private int _startupReadGeneration;

    private async Task RefreshStartupStateAsync()
    {
        int generation = ++_startupReadGeneration;
        StartupStateText.Text = "正在读取系统状态…";
        StartupEnableButton.IsEnabled = false;
        StartupDisableButton.IsEnabled = false;
        try
        {
            var task = await Windows.ApplicationModel.StartupTask.GetAsync("GitaViewStartup")
                .AsTask().WaitAsync(TimeSpan.FromSeconds(5));
            if (generation != _startupReadGeneration) return;
            if (task is null) throw new InvalidOperationException("Startup task was not found");
            StartupStateText.Text = task.State switch
            {
                Windows.ApplicationModel.StartupTaskState.Enabled => "已启用（系统）",
                Windows.ApplicationModel.StartupTaskState.Disabled => "未启用",
                Windows.ApplicationModel.StartupTaskState.DisabledByUser => "已被用户在系统设置中禁用",
                Windows.ApplicationModel.StartupTaskState.DisabledByPolicy => "已被策略禁用",
                Windows.ApplicationModel.StartupTaskState.EnabledByPolicy => "已由策略启用",
                _ => task.State.ToString(),
            };
            StartupEnableButton.IsEnabled = task.State == Windows.ApplicationModel.StartupTaskState.Disabled;
            StartupDisableButton.IsEnabled = task.State == Windows.ApplicationModel.StartupTaskState.Enabled;
        }
        catch (Exception exception)
        {
            if (generation != _startupReadGeneration) return;
            StartupStateText.Text = "无法读取开机启动状态，返回此页后重试。";
            StartupEnableButton.IsEnabled = false;
            StartupDisableButton.IsEnabled = false;
            App.HostLog?.Log("WARN", "ui.startup_state.unavailable", exception.Message);
        }
    }

    private async void OnStartupEnableClick(object sender, RoutedEventArgs e)
    {
        try
        {
            var task = await Windows.ApplicationModel.StartupTask.GetAsync("GitaViewStartup");
            _ = await task.RequestEnableAsync();
            await RefreshStartupStateAsync();
        }
        catch (InvalidOperationException)
        {
            StartupStateText.Text = "当前构建未包含开机启动声明";
        }
    }

    private async void OnStartupDisableClick(object sender, RoutedEventArgs e)
    {
        try
        {
            var task = await Windows.ApplicationModel.StartupTask.GetAsync("GitaViewStartup");
            task.Disable();
            await RefreshStartupStateAsync();
        }
        catch (InvalidOperationException)
        {
        }
    }

    // —— 工具 ——

    private static async Task<string?> PickFolderAsync(string title)
    {
        var picker = new Windows.Storage.Pickers.FolderPicker();
        picker.FileTypeFilter.Add("*");
        if (App.MainWindow is { } window)
        {
            // FolderPicker 需要前台窗口句柄（WinUI 桌面桥接）。
            var hwnd = WinRT.Interop.WindowNative.GetWindowHandle(window);
            WinRT.Interop.InitializeWithWindow.Initialize(picker, hwnd);
        }
        return (await picker.PickSingleFolderAsync())?.Path;
    }

    private static string MessageOf(Exception exception) => exception is GvException gv
        ? ErrorText.For(gv.Code)
        : ErrorText.For(null) + "：" + exception.Message;
}
