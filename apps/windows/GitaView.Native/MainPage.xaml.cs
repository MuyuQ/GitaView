// 状态明细页（统一设计，升级自旧版 WidgetExpanded / RepoTable / RepoActions）：
// - 分组 / 状态筛选、对齐表格与选中行操作；紧凑摘要由托盘浮窗承载；
// - 筛选、搜索、排序与协调由核心 view_query 统一计算（组筛选 + 状态筛选两次查询）；
// - 行操作走 FFI（Fetch 无确认，Pull / Push 两段式确认），单仓库独立忙态与结果；
// - 刷新失败保留历史行，只出琥珀横幅；状态不依赖颜色（点 + 文字 + 计数并出）。

using System.Collections.ObjectModel;
using System.ComponentModel;
using GitaView.Native.Contracts;
using GitaView.Native.Interop;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Navigation;
using Windows.System;

namespace GitaView_Native;

/// <summary>按仓库身份保留的行视图模型，投影更新不会丢失操作状态。</summary>
public sealed class RepoRowVm : INotifyPropertyChanged
{
    // —— 仓库身份与可更新投影 ——
    public string Id { get; set; } = "";
    public string Name { get; set; } = "";
    public string Path { get; set; } = "";
    public string GroupDisplay { get; set; } = "";
    public string Branch { get; set; } = "";
    public string StatusLabel { get; set; } = "";
    public string RelationLabel { get; set; } = "";
    public string ChangesLabel { get; set; } = "";
    public string Hint { get; set; } = "";
    public Brush Dot { get; set; } = ThemeBrush("GvSlateBrush");
    public Brush RelationBrush { get; set; } = ThemeBrush("GvInkBrush");
    public bool CanOpenDirectory { get; set; }
    public bool CanOpenRemote { get; set; }
    private bool _canFetch;
    public bool CanFetch { get => _canFetch && !IsBusy; set => _canFetch = value; }
    private bool _canPull;
    public bool CanPull { get => _canPull && !IsBusy; set => _canPull = value; }
    private bool _canPush;
    public bool CanPush { get => _canPush && !IsBusy; set => _canPush = value; }

    private Brush _rowBackground = ThemeBrush("GvSurfaceBrush");
    public Brush RowBackground
    {
        get => _rowBackground;
        set { _rowBackground = value; Raise(nameof(RowBackground)); }
    }

    // —— 交互状态（INPC）——
    private bool _isExpanded;
    public bool IsExpanded
    {
        get => _isExpanded;
        set
        {
            if (_isExpanded == value)
            {
                return;
            }
            _isExpanded = value;
            Raise(nameof(IsExpanded));
            Raise(nameof(ActionsVisible));
            Raise(nameof(IndicatorVisible));
        }
    }

    public Visibility ActionsVisible => IsExpanded ? Visibility.Visible : Visibility.Collapsed;
    public Visibility IndicatorVisible => IsExpanded ? Visibility.Visible : Visibility.Collapsed;

    private bool _isFetchBusy;
    public bool IsFetchBusy
    {
        get => _isFetchBusy;
        set { _isFetchBusy = value; RaiseBusyState(); }
    }

    private bool _isPullBusy;
    public bool IsPullBusy
    {
        get => _isPullBusy;
        set { _isPullBusy = value; RaiseBusyState(); }
    }

    private bool _isPushBusy;
    public bool IsPushBusy
    {
        get => _isPushBusy;
        set { _isPushBusy = value; RaiseBusyState(); }
    }

    public bool IsBusy => IsFetchBusy || IsPullBusy || IsPushBusy;
    public Visibility BusyVisible => IsBusy ? Visibility.Visible : Visibility.Collapsed;
    private void RaiseBusyState()
    {
        Raise(nameof(IsBusy)); Raise(nameof(BusyVisible));
        Raise(nameof(CanFetch)); Raise(nameof(CanPull)); Raise(nameof(CanPush));
    }
    public void ProjectionChanged() => Raise("");

    private bool _confirmPull;
    public bool ConfirmPull
    {
        get => _confirmPull;
        set
        {
            _confirmPull = value;
            Raise(nameof(ConfirmPull));
            Raise(nameof(PullLabel));
            Raise(nameof(ConfirmText));
            Raise(nameof(ConfirmVisible));
        }
    }

    private bool _confirmPush;
    public bool ConfirmPush
    {
        get => _confirmPush;
        set
        {
            _confirmPush = value;
            Raise(nameof(ConfirmPush));
            Raise(nameof(PushLabel));
            Raise(nameof(ConfirmText));
            Raise(nameof(ConfirmVisible));
        }
    }

    public string PullLabel => ConfirmPull ? "确认 Pull" : "Pull";
    public string PushLabel => ConfirmPush ? "确认 Push" : "Push";
    public string ConfirmText => ConfirmPull
        ? "Pull 会修改当前仓库工作区，是否继续？"
        : ConfirmPush ? "Push 会更新远端分支，是否继续？" : "";
    public Visibility ConfirmVisible => (ConfirmPull || ConfirmPush)
        ? Visibility.Visible : Visibility.Collapsed;

    private string _resultText = "";
    public string ResultText
    {
        get => _resultText;
        set { _resultText = value; Raise(nameof(ResultText)); Raise(nameof(ResultBrush)); Raise(nameof(ResultVisible)); }
    }

    private bool _resultIsError;
    public bool ResultIsError
    {
        get => _resultIsError;
        set { _resultIsError = value; Raise(nameof(ResultIsError)); Raise(nameof(ResultBrush)); }
    }

    public Visibility ResultVisible => string.IsNullOrEmpty(ResultText) ? Visibility.Collapsed : Visibility.Visible;
    public Brush ResultBrush => _resultIsError ? ThemeBrush("GvRedBrush") : ThemeBrush("GvGreenBrush");

    public event PropertyChangedEventHandler? PropertyChanged;

    private void Raise(string name) =>
        PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(name));

    internal static Brush ThemeBrush(string key)
    {
        var theme = App.MainWindow?.ContentTheme
            ?? (Application.Current.RequestedTheme == ApplicationTheme.Dark ? ElementTheme.Dark : ElementTheme.Light);
        var name = new Windows.UI.ViewManagement.AccessibilitySettings().HighContrast
            ? "HighContrast" : theme == ElementTheme.Dark ? "Dark" : "Light";
        var themeResources = Application.Current.Resources.MergedDictionaries.Last();
        return (Brush)((ResourceDictionary)themeResources.ThemeDictionaries[name])[key];
    }
}

public sealed partial class MainPage : Page
{
    public TitleBar ActiveTitleBar => PageTitleBar;
    public ObservableCollection<RepoRowVm> Rows { get; } = [];

    private bool _initialized;
    private readonly Dictionary<string, RepoRowVm> _rowCache = new();
    private GridLength[]? _columnWidths;
    private string? _selectedGroupId; // null = 全部分组
    private RelationFilter _selectedRelation = RelationFilter.All;
    private string? _selectedRepoId;
    private bool _refreshBusy;
    private bool _suppressQueryChanges;
    private int _protocolNavigationVersion;
    private ProtocolRoute? _pendingProtocolReveal;
    private readonly Dictionary<string, string> _groupNames = new();

    public MainPage()
    {
        InitializeComponent();
        StartupRecoveryBar.IsOpen = App.StartupWarnings.Contains("SETTINGS_RECOVERED");
        Loaded += (_, _) => QueueProtocolReveal();
        ActualThemeChanged += (_, _) => DispatcherQueue.TryEnqueue(() =>
        {
            _ = RequeryAsync();
        });
    }

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        if (!_initialized && App.Engine is { } engine)
        {
            _selectedGroupId = engine.SettingsGet().DefaultGroupId;
            _initialized = true;
        }
        App.SnapshotArrived += OnSnapshotArrived;
        _ = RequeryAsync();
    }

    protected override void OnNavigatedFrom(NavigationEventArgs e)
    {
        base.OnNavigatedFrom(e);
        CancelProtocolReveal();
        App.SnapshotArrived -= OnSnapshotArrived;
    }

    internal async Task NavigateFromProtocolAsync(ProtocolRoute route)
    {
        int version = ++_protocolNavigationVersion;
        _pendingProtocolReveal = null;
        CancelPendingConfirmations();
        _selectedGroupId = null;
        _selectedRelation = RelationFilter.All;
        _selectedRepoId = route.RepoId;
        _suppressQueryChanges = true;
        try { QueryBox.Text = ""; }
        finally { _suppressQueryChanges = false; }
        await RequeryAsync();
        if (version != _protocolNavigationVersion) return;
        // A repository can be removed after activation resolved its ID.
        if (_selectedRepoId is { } id && Rows.All(row => row.Id != id)) _selectedRepoId = null;
        _pendingProtocolReveal = new(_selectedRepoId);
        QueueProtocolReveal();
    }

    private void CancelProtocolReveal()
    {
        _protocolNavigationVersion++;
        _pendingProtocolReveal = null;
    }

    private void QueueProtocolReveal()
    {
        if (!IsLoaded || _pendingProtocolReveal is null) return;
        DispatcherQueue.TryEnqueue(() =>
        {
            if (!IsLoaded || !ReferenceEquals(Frame?.Content, this) || _pendingProtocolReveal is not { } route) return;
            _pendingProtocolReveal = null;
            try
            {
                var row = route.RepoId is { } id ? Rows.FirstOrDefault(item => item.Id == id) : null;
                if (row is null)
                {
                    _selectedRepoId = null;
                    TableScroll.ChangeView(0, 0, null, disableAnimation: true);
                    return;
                }
                TableScroll.ChangeView(0, null, null, disableAnimation: true);
                RepoList.ScrollIntoView(row, ScrollIntoViewAlignment.Leading);
                RepoList.UpdateLayout();
                if (RepoList.ContainerFromItem(row) is FrameworkElement container)
                    container.StartBringIntoView();
            }
            catch (Exception exception)
            {
                App.HostLog?.Log("WARN", "protocol.reveal.failed", exception.Message);
            }
        });
    }

    private void OnSnapshotArrived(AppSnapshot snapshot)
    {
        DispatcherQueue?.TryEnqueue(() => _ = RequeryAsync());
    }

    // —— 查询与渲染 ——

    private async Task RequeryAsync()
    {
        var engine = App.Engine;
        if (engine is null)
        {
            return;
        }
        // 两次查询：组 + 全部状态 → 筛选条计数与协调；
        // 组 + 当前状态 → 行。
        var chips = engine.ViewQuery(new ViewQuery
        {
            GroupId = _selectedGroupId,
            RelationFilter = RelationFilter.All,
            Query = QueryBox.Text,
        });
        var filtered = engine.ViewQuery(new ViewQuery
        {
            GroupId = _selectedGroupId,
            RelationFilter = _selectedRelation,
            Query = QueryBox.Text,
        });

        // 核心协调：所选组被删或状态在组内不存在 → 回退（同步本地选择）。
        if (chips.Effective.GroupId != _selectedGroupId)
        {
            _selectedGroupId = chips.Effective.GroupId;
        }
        _selectedRelation = filtered.Effective.RelationFilter;

        var snapshot = engine.SnapshotGet();
        RefreshTimeText.Text = snapshot.LastSuccessAtMs is { } lastSuccess
            ? $"刷新时间 {FormatTime(lastSuccess)}"
            : "尚未刷新";
        if (snapshot.RefreshState == RefreshState.Failed)
        {
            RefreshWarningText.Text = "刷新失败：" + ErrorText.For(snapshot.RefreshError?.Code)
                + "（以下为最近一次成功读取的结果）";
            RefreshWarningText.Visibility = Visibility.Visible;
        }
        else
        {
            RefreshWarningText.Visibility = Visibility.Collapsed;
        }

        // 底部状态栏承载紧凑摘要语义（收缩窗口模式已移除，浮窗另用同一投影）：
        // 总数 · 待同步 · 需关注 · 无远端（全部仓库口径），筛选时加 N/M 前缀。
        var summary = chips.SummaryCounts;
        var syncable = summary.LocalAhead + summary.RemoteAhead;
        var attention = summary.Diverged + summary.Error;
        var summaryText = summary.Total > 0 && filtered.Rows.Count != summary.Total
            ? $"{filtered.Rows.Count} / {summary.Total} · " : "";
        summaryText += $"{summary.Total} 个仓库 · 待同步 {syncable}"
            + $" · 需关注 {attention} · 未比较 {summary.NoRemote}";
        if (summary.Pending > 0) summaryText += $" · 待读 {summary.Pending}";
        if (snapshot.RefreshState == RefreshState.Failed) summaryText += " · 刷新失败";
        ResultCountText.Text = summaryText;
        ResultCountText.Foreground = RepoRowVm.ThemeBrush(
            snapshot.RefreshState == RefreshState.Failed ? "GvAmberTextBrush" : "GvMutedBrush");

        BuildGroupChips(chips);
        BuildStatusChips(chips.StatusCounts);
        await BuildRowsAsync(filtered);
        UpdateEmptyStates(chips.SummaryCounts.Total);
    }

    private void BuildGroupChips(ViewResult result)
    {
        UpdateChips(GroupChipsPanel,
            RepositoryFilterPresentation.Groups(result.SummaryCounts.Total, result.Groups, _selectedGroupId),
            SelectGroup, group: true);
    }

    private void BuildStatusChips(WidgetCounts counts)
    {
        UpdateChips(StatusChipsPanel,
            RepositoryFilterPresentation.Statuses(counts, _selectedRelation), SelectRelation, group: false);
    }

    private void UpdateChips<T>(StackPanel panel, IReadOnlyList<FilterChip<T>> choices, Action<T> select, bool group)
    {
        var existing = panel.Children.OfType<Button>().ToArray();
        var desired = new List<Button>();
        foreach (var choice in choices)
        {
            var button = existing.FirstOrDefault(candidate =>
                candidate.Tag is FilterChip<T> previous
                && EqualityComparer<T>.Default.Equals(previous.Value, choice.Value));
            if (button is null)
            {
                button = new Button { Style = (Style)Application.Current.Resources["GvChipStyle"] };
                button.Click += (_, _) => select(choice.Value);
            }
            button.Tag = choice;
            button.Content = choice.Label;
            ApplyChipSelection(button, choice.Selected, group);
            Microsoft.UI.Xaml.Automation.AutomationProperties.SetItemStatus(
                button, choice.Selected ? "已选择" : "未选择");
            desired.Add(button);
        }
        // 更新现有按钮，刷新与选中态切换不移除仍然有效的键盘焦点目标。
        foreach (var old in existing.Where(button => !desired.Contains(button)))
        {
            panel.Children.Remove(old);
        }
        for (var i = 0; i < desired.Count; i++)
        {
            int index = panel.Children.IndexOf(desired[i]);
            if (index == i) continue;
            if (index >= 0) panel.Children.RemoveAt(index);
            panel.Children.Insert(i, desired[i]);
        }
    }

    private static void ApplyChipSelection(Button button, bool selected, bool group)
    {
        button.FontWeight = group ? Microsoft.UI.Text.FontWeights.Medium : Microsoft.UI.Text.FontWeights.Normal;
        button.Background = selected ? RepoRowVm.ThemeBrush(group ? "GvSurfaceBrush" : "GvSelectionBrush")
            : new SolidColorBrush(Microsoft.UI.Colors.Transparent);
        button.Foreground = RepoRowVm.ThemeBrush(selected
            ? group ? "GvInkBrush" : "GvOnSelectionBrush" : "GvMutedBrush");
        // 为分组按钮预留相同边框厚度，选中态变化不改变按钮宽度。
        button.BorderThickness = new Thickness(group ? 1 : 0);
        button.BorderBrush = selected && group
            ? RepoRowVm.ThemeBrush("GvLineBrush") : new SolidColorBrush(Microsoft.UI.Colors.Transparent);
    }

    private void SelectGroup(string? groupId)
    {
        CancelProtocolReveal();
        CancelPendingConfirmations();
        _selectedGroupId = groupId;
        _ = RequeryAsync();
    }

    private void SelectRelation(RelationFilter filter)
    {
        CancelProtocolReveal();
        CancelPendingConfirmations();
        _selectedRelation = filter;
        _ = RequeryAsync();
    }

    private Task BuildRowsAsync(ViewResult result)
    {
        if (App.Engine is not { } engine) return Task.CompletedTask;
        var settings = engine.SettingsGet();
        _groupNames.Clear();
        foreach (var group in settings.Groups) _groupNames[group.Id] = group.Name;
        // Keep interaction state across scans and filters, but never across a different path.
        var identities = settings.Repos.ToDictionary(repo => repo.Id, repo => repo.Path);
        foreach (var id in _rowCache.Keys.ToArray())
            if (!identities.TryGetValue(id, out var path) || _rowCache[id].Path != path)
                _rowCache.Remove(id);
        var desired = new List<RepoRowVm>();
        foreach (var row in result.Rows)
        {
            if (!_rowCache.TryGetValue(row.Id, out var vm))
            {
                vm = new RepoRowVm { Id = row.Id, Path = row.Path };
                _rowCache.Add(row.Id, vm);
            }
            var view = RepoStatusView.From(row);
            vm.Name = row.Name;
            vm.GroupDisplay = row.GroupId is { } groupId && _groupNames.TryGetValue(groupId, out var name) ? name : "—";
            vm.Branch = view.Branch is { Length: > 0 } branch ? branch : "—";
            vm.StatusLabel = StatusLabelOf(view);
            vm.RelationLabel = RelationLabelOf(view);
            vm.ChangesLabel = ChangesLabelOf(view);
            vm.Hint = HintOf(view);
            vm.Dot = DotOf(view);
            vm.RelationBrush = RelationBrushOf(view);
            vm.RowBackground = RepoRowVm.ThemeBrush(row.Id == _selectedRepoId ? "GvRowSelectedBrush" : "GvSurfaceBrush");
            vm.CanOpenDirectory = row.Actions.CanOpenDirectory;
            vm.CanOpenRemote = row.Actions.CanOpenRemote;
            vm.CanFetch = row.Actions.CanFetch;
            vm.CanPull = row.Actions.CanPull;
            vm.CanPush = row.Actions.CanPush;
            vm.IsExpanded = row.Id == _selectedRepoId;
            vm.ProjectionChanged();
            desired.Add(vm);
        }
        // Reconcile instead of clearing the collection: keep focus and realized rows stable.
        for (var i = 0; i < desired.Count; i++)
        {
            if (i < Rows.Count && ReferenceEquals(Rows[i], desired[i])) continue;
            var previous = Rows.IndexOf(desired[i]);
            if (previous >= 0) Rows.Move(previous, i);
            else Rows.Insert(i, desired[i]);
        }
        while (Rows.Count > desired.Count) Rows.RemoveAt(Rows.Count - 1);
        return Task.CompletedTask;
    }

    private void OnColumnDrag(object sender, DragDeltaEventArgs e)
    {
        if (sender is not Thumb thumb || !int.TryParse(thumb.Tag?.ToString(), out var index)) return;
        ResizeColumn(index, e.HorizontalChange);
    }

    private void OnColumnKeyDown(object sender, Microsoft.UI.Xaml.Input.KeyRoutedEventArgs e)
    {
        if (sender is not Thumb thumb || !int.TryParse(thumb.Tag?.ToString(), out var index)) return;
        if (e.Key is not (Windows.System.VirtualKey.Left or Windows.System.VirtualKey.Right)) return;
        ResizeColumn(index, e.Key == Windows.System.VirtualKey.Left ? -16 : 16);
        e.Handled = true;
    }

    private void ResizeColumn(int index, double delta)
    {
        _columnWidths ??= TableHeader.ColumnDefinitions.Select(column => new GridLength(column.ActualWidth)).ToArray();
        var minimum = index switch { 1 => 140, 3 => 90, _ => 30 };
        _columnWidths[index] = new GridLength(Math.Max(minimum, _columnWidths[index].Value + delta));
        UpdateTableWidth();
        ApplyColumns(TableHeader);
        ApplyColumnsToChildren(RepoList);
    }

    private void OnTableSizeChanged(object sender, SizeChangedEventArgs e) => UpdateTableWidth();
    private void UpdateTableWidth()
    {
        if (TableContent is null) return;
        TableContent.Width = Math.Max(TableScroll.ActualWidth,
            _columnWidths is null ? 624 : _columnWidths.Sum(column => column.Value) + 48);
    }

    private void ApplyColumnsToChildren(DependencyObject parent)
    {
        for (var i = 0; i < VisualTreeHelper.GetChildrenCount(parent); i++)
        {
            var child = VisualTreeHelper.GetChild(parent, i);
            if (child is Grid { Name: "RepoDataGrid" } grid) ApplyColumns(grid);
            ApplyColumnsToChildren(child);
        }
    }

    private void ApplyColumns(Grid grid)
    {
        if (_columnWidths is null) return;
        for (var i = 0; i < _columnWidths.Length; i++) grid.ColumnDefinitions[i].Width = _columnWidths[i];
    }

    private void OnDataGridLoaded(object sender, RoutedEventArgs e) => ApplyColumns((Grid)sender);
    private void OnRowPointerEntered(object sender, Microsoft.UI.Xaml.Input.PointerRoutedEventArgs e)
    {
        if ((sender as FrameworkElement)?.DataContext is RepoRowVm row && row.Id != _selectedRepoId)
            row.RowBackground = RepoRowVm.ThemeBrush("GvHoverBrush");
    }
    private void OnRowPointerExited(object sender, Microsoft.UI.Xaml.Input.PointerRoutedEventArgs e)
    {
        if ((sender as FrameworkElement)?.DataContext is RepoRowVm row && row.Id != _selectedRepoId)
            row.RowBackground = RepoRowVm.ThemeBrush("GvSurfaceBrush");
    }
    private void OnCancelConfirmClick(object sender, RoutedEventArgs e)
    {
        if (RowOf(sender) is not { } row) return;
        row.ConfirmPull = row.ConfirmPush = false;
    }

    private void UpdateEmptyStates(uint totalRepos)
    {
        NoReposPanel.Visibility = totalRepos == 0 ? Visibility.Visible : Visibility.Collapsed;
        TableScroll.Visibility = totalRepos == 0 ? Visibility.Collapsed : Visibility.Visible;
        NoMatchText.Visibility = totalRepos > 0 && Rows.Count == 0
            ? Visibility.Visible
            : Visibility.Collapsed;
    }

    private static string StatusLabelOf(RepoStatusView row) => row.ReadState switch
    {
        ReadState.Pending => "待读",
        ReadState.Error => "失败",
        _ => row.Relation switch
        {
            Relation.Synced => "同步",
            Relation.LocalAhead => "领先",
            Relation.RemoteAhead => "落后",
            Relation.Diverged => "分叉",
            Relation.NoRemote => ErrorText.NoComparisonLabel(row),
            _ => "",
        },
    };

    private static string RelationLabelOf(RepoStatusView row) => row.ReadState switch
    {
        ReadState.Pending => "尚未读取",
        ReadState.Error => "读取失败",
        _ => row.Relation switch
        {
            Relation.Synced => "已同步",
            Relation.LocalAhead => "本地领先",
            Relation.RemoteAhead => "远程领先",
            Relation.Diverged => "分叉",
            Relation.NoRemote => ErrorText.NoComparisonLabel(row),
            _ => "",
        },
    };

    private static string ChangesLabelOf(RepoStatusView row) => row.ReadState switch
    {
        ReadState.Ok when row.Relation == Relation.LocalAhead => $"↑{row.Ahead ?? 0}",
        ReadState.Ok when row.Relation == Relation.RemoteAhead => $"↓{row.Behind ?? 0}",
        ReadState.Ok when row.Relation == Relation.Diverged =>
            $"↑{row.Ahead ?? 0} ↓{row.Behind ?? 0}",
        _ => "—",
    };

    private static string HintOf(RepoStatusView row) => row.ReadState switch
    {
        ReadState.Pending => "等待首次读取",
        ReadState.Error => ErrorText.For(row.ErrorCode),
        ReadState.Ok when row.Relation == Relation.NoRemote => ErrorText.NoComparisonHint(row),
        _ => "",
    };

    private static Brush DotOf(RepoStatusView row) => row.ReadState switch
    {
        ReadState.Pending => RepoRowVm.ThemeBrush("GvSlateBrush"),
        ReadState.Error => RepoRowVm.ThemeBrush("GvRedBrush"),
        _ => row.Relation switch
        {
            Relation.Synced => RepoRowVm.ThemeBrush("GvGreenBrush"),
            Relation.LocalAhead => RepoRowVm.ThemeBrush("GvAmberTextBrush"),
            Relation.RemoteAhead => RepoRowVm.ThemeBrush("GvAmberTextBrush"),
            Relation.Diverged => RepoRowVm.ThemeBrush("GvRedBrush"),
            Relation.NoRemote => RepoRowVm.ThemeBrush("GvSlateBrush"),
            _ => RepoRowVm.ThemeBrush("GvSlateBrush"),
        },
    };

    private static Brush RelationBrushOf(RepoStatusView row) => row.ReadState switch
    {
        ReadState.Error => RepoRowVm.ThemeBrush("GvRedBrush"),
        ReadState.Pending => RepoRowVm.ThemeBrush("GvMutedBrush"),
        _ => row.Relation switch
        {
            Relation.Diverged => RepoRowVm.ThemeBrush("GvRedBrush"),
            Relation.LocalAhead => RepoRowVm.ThemeBrush("GvAmberTextBrush"),
            Relation.RemoteAhead => RepoRowVm.ThemeBrush("GvAmberTextBrush"),
            _ => RepoRowVm.ThemeBrush("GvInkBrush"),
        },
    };

    private static string FormatTime(ulong unixMs)
    {
        var time = DateTimeOffset.FromUnixTimeMilliseconds((long)unixMs).ToLocalTime();
        return time.ToString("HH:mm:ss");
    }

    // —— 交互 ——

    private void CancelPendingConfirmations()
    {
        foreach (var row in _rowCache.Values) row.ConfirmPull = row.ConfirmPush = false;
    }

    private void OnQueryChanged(object sender, TextChangedEventArgs e)
    {
        if (_suppressQueryChanges) return;
        CancelProtocolReveal();
        CancelPendingConfirmations();
        _ = RequeryAsync();
    }

    private void OnRepoItemClick(object sender, ItemClickEventArgs e)
    {
        if (e.ClickedItem is not RepoRowVm clicked)
        {
            return;
        }
        CancelProtocolReveal();
        // 点击行为与旧版一致：再点同一行收起。
        _selectedRepoId = _selectedRepoId == clicked.Id ? null : clicked.Id;
        foreach (var row in Rows)
        {
            row.IsExpanded = row.Id == _selectedRepoId;
            if (!row.IsExpanded) row.ConfirmPull = row.ConfirmPush = false;
            row.RowBackground = row.Id == _selectedRepoId
                ? RepoRowVm.ThemeBrush("GvRowSelectedBrush")
                : RepoRowVm.ThemeBrush("GvSurfaceBrush");
        }
    }

    private void OnOpenSettingsClick(object sender, RoutedEventArgs e) =>
        App.MainWindow?.OpenSettings();

    private async void OnRefreshClick(object sender, RoutedEventArgs e)
    {
        var engine = App.Engine;
        if (engine is null || _refreshBusy)
        {
            return;
        }
        _refreshBusy = true;
        RefreshButton.Content = "刷新中";
        RefreshButton.IsEnabled = false;
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
            App.HostLog?.Log("WARN", "ui.refresh.failed",
                $"{exception.GetType().Name}: {exception.Message}");
        }
        finally
        {
            _refreshBusy = false;
            RefreshButton.Content = "刷新";
            RefreshButton.IsEnabled = true;
        }
        await RequeryAsync();
    }

    private static RepoRowVm? RowOf(object sender) =>
        (sender as FrameworkElement)?.Tag as RepoRowVm;

    private async void OnOpenDirectoryClick(object sender, RoutedEventArgs e)
    {
        if (RowOf(sender) is not { } row)
        {
            return;
        }
        try
        {
            var engine = App.Engine!;
            var target = await Task.Run(() => engine.RepoTarget(
                new RepoTargetInput { RepoId = row.Id, Kind = RepoTargetKind.Directory }));
            _ = await Launcher.LaunchFolderPathAsync(target.Value);
            row.ResultText = "已打开目录";
            row.ResultIsError = false;
        }
        catch (Exception exception)
        {
            SetActionResult(row, exception);
        }
    }

    private async void OnOpenRemoteClick(object sender, RoutedEventArgs e)
    {
        if (RowOf(sender) is not { } row)
        {
            return;
        }
        try
        {
            var engine = App.Engine!;
            var target = await Task.Run(() => engine.RepoTarget(
                new RepoTargetInput { RepoId = row.Id, Kind = RepoTargetKind.Remote }));
            _ = await Launcher.LaunchUriAsync(new Uri(target.Value));
            row.ResultText = "已打开远端";
            row.ResultIsError = false;
        }
        catch (Exception exception)
        {
            SetActionResult(row, exception);
        }
    }

    private async void OnFetchClick(object sender, RoutedEventArgs e)
    {
        if (RowOf(sender) is not { } row || row.IsBusy)
        {
            return;
        }
        row.IsFetchBusy = true;
        row.ConfirmPull = row.ConfirmPush = false;
        row.ResultIsError = false;
        row.ResultText = "Fetch 执行中…";
        try
        {
            var engine = App.Engine!;
            await Task.Run(() => engine.RepoFetch(row.Id));
            row.ResultText = "Fetch 完成";
            row.ResultIsError = false;
            await RequestRefreshAsync();
        }
        catch (Exception exception)
        {
            SetActionResult(row, exception);
        }
        finally
        {
            row.IsFetchBusy = false;
        }
    }

    private async void OnPullClick(object sender, RoutedEventArgs e)
    {
        if (RowOf(sender) is not { } row || row.IsBusy)
        {
            return;
        }
        if (!row.ConfirmPull)
        {
            row.ConfirmPull = true;
            row.ConfirmPush = false;
            return;
        }
        row.ConfirmPull = false;
        row.IsPullBusy = true;
        row.ConfirmPull = row.ConfirmPush = false;
        row.ResultIsError = false;
        row.ResultText = "Pull 执行中…";
        try
        {
            var engine = App.Engine!;
            await Task.Run(() => engine.RepoPull(row.Id, confirmed: true));
            row.ResultText = "Pull 完成";
            row.ResultIsError = false;
            await RequestRefreshAsync();
        }
        catch (Exception exception)
        {
            SetActionResult(row, exception);
        }
        finally
        {
            row.IsPullBusy = false;
        }
    }

    private async void OnPushClick(object sender, RoutedEventArgs e)
    {
        if (RowOf(sender) is not { } row || row.IsBusy)
        {
            return;
        }
        if (!row.ConfirmPush)
        {
            row.ConfirmPush = true;
            row.ConfirmPull = false;
            return;
        }
        row.ConfirmPush = false;
        row.IsPushBusy = true;
        row.ConfirmPull = row.ConfirmPush = false;
        row.ResultIsError = false;
        row.ResultText = "Push 执行中…";
        try
        {
            var engine = App.Engine!;
            await Task.Run(() => engine.RepoPush(row.Id, confirmed: true));
            row.ResultText = "Push 完成";
            row.ResultIsError = false;
            await RequestRefreshAsync();
        }
        catch (Exception exception)
        {
            SetActionResult(row, exception);
        }
        finally
        {
            row.IsPushBusy = false;
        }
    }

    private static void SetActionResult(RepoRowVm row, Exception exception)
    {
        if (exception is GvException gv)
        {
            row.ResultText = ErrorText.For(gv.Code);
        }
        else
        {
            row.ResultText = ErrorText.For(null);
        }
        row.ResultIsError = true;
        App.HostLog?.Log("WARN", "ui.action.failed",
            $"{exception.GetType().Name}: {exception.Message}");
    }

    private async Task RequestRefreshAsync()
    {
        var engine = App.Engine;
        if (engine is null)
        {
            return;
        }
        try
        {
            await Task.Run(() => engine.RefreshRequest());
        }
        catch (GvException)
        {
            // 停止中：忽略。
        }
    }
}
