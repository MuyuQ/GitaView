// 状态明细页（统一设计，升级自旧版 WidgetExpanded / RepoTable / RepoActions）：
// - 紧凑摘要 + 分组 / 状态筛选条 + 对齐表格 + 选中行下方展开操作（不常驻按钮）；
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
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Navigation;
using Windows.System;

namespace GitaView_Native;

/// <summary>表格行视图模型：静态字段构造时定型；交互状态走 INPC。</summary>
public sealed class RepoRowVm : INotifyPropertyChanged
{
    // —— 静态投影（重建时定型）——
    public string Id { get; set; } = "";
    public string Name { get; set; } = "";
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
    public bool CanFetch { get; set; }
    public bool CanPull { get; set; }
    public bool CanPush { get; set; }

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
        set { _isFetchBusy = value; Raise(nameof(IsFetchBusy)); }
    }

    private bool _isPullBusy;
    public bool IsPullBusy
    {
        get => _isPullBusy;
        set { _isPullBusy = value; Raise(nameof(IsPullBusy)); }
    }

    private bool _isPushBusy;
    public bool IsPushBusy
    {
        get => _isPushBusy;
        set { _isPushBusy = value; Raise(nameof(IsPushBusy)); }
    }

    private bool _confirmPull;
    public bool ConfirmPull
    {
        get => _confirmPull;
        set
        {
            _confirmPull = value;
            Raise(nameof(ConfirmPull));
            Raise(nameof(PullLabel));
            Raise(nameof(PullBackground));
            Raise(nameof(PullForeground));
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
            Raise(nameof(PushBackground));
            Raise(nameof(PushForeground));
            Raise(nameof(ConfirmText));
            Raise(nameof(ConfirmVisible));
        }
    }

    public string PullLabel => ConfirmPull ? "确认 Pull" : "Pull";
    public string PushLabel => ConfirmPush ? "确认 Push" : "Push";
    public Brush PullBackground => ConfirmPull ? ThemeBrush("GvAmberBrush") : ThemeBrush("GvSurfaceBrush");
    public Brush PullForeground => ConfirmPull ? ThemeBrush("GvSurfaceBrush") : ThemeBrush("GvAmberTextBrush");
    public Brush PushBackground => ConfirmPush ? ThemeBrush("GvAmberBrush") : ThemeBrush("GvSurfaceBrush");
    public Brush PushForeground => ConfirmPush ? ThemeBrush("GvSurfaceBrush") : ThemeBrush("GvAmberTextBrush");
    public string ConfirmText => ConfirmPull
        ? "Pull 会修改当前仓库工作区，是否继续？"
        : ConfirmPush ? "Push 会更新远端分支，是否继续？" : "";
    public Visibility ConfirmVisible => (ConfirmPull || ConfirmPush)
        ? Visibility.Visible : Visibility.Collapsed;

    private string _resultText = "";
    public string ResultText
    {
        get => _resultText;
        set { _resultText = value; Raise(nameof(ResultText)); Raise(nameof(ResultBrush)); }
    }

    private bool _resultIsError;
    public bool ResultIsError
    {
        get => _resultIsError;
        set { _resultIsError = value; Raise(nameof(ResultIsError)); Raise(nameof(ResultBrush)); }
    }

    public Brush ResultBrush => _resultIsError ? ThemeBrush("GvRedBrush") : ThemeBrush("GvGreenBrush");

    public event PropertyChangedEventHandler? PropertyChanged;

    private void Raise(string name) =>
        PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(name));

    internal static Brush ThemeBrush(string key) =>
        (Application.Current.Resources[key] as Brush)!;
}

public sealed partial class MainPage : Page
{
    public ObservableCollection<RepoRowVm> Rows { get; } = [];

    /// <summary>状态筛选条顺序与文案（与旧版 StatusFilters 一致；0 计数不显示）。</summary>
    private static readonly (string Label, RelationFilter Filter)[] StatusChipOrder =
    [
        ("读取失败", RelationFilter.Error),
        ("分叉", RelationFilter.Diverged),
        ("远端领先", RelationFilter.RemoteAhead),
        ("本地领先", RelationFilter.LocalAhead),
        ("同步", RelationFilter.Synced),
        ("无远端", RelationFilter.NoRemote),
    ];

    private string? _selectedGroupId; // null = 全部分组
    private RelationFilter _selectedRelation = RelationFilter.All;
    private string? _selectedRepoId;
    private bool _refreshBusy;
    private string _lastGroupSignature = "";
    private readonly Dictionary<string, string> _groupNames = new();

    public MainPage()
    {
        InitializeComponent();
    }

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        App.SnapshotArrived += OnSnapshotArrived;
        _ = RequeryAsync();
    }

    protected override void OnNavigatedFrom(NavigationEventArgs e)
    {
        base.OnNavigatedFrom(e);
        App.SnapshotArrived -= OnSnapshotArrived;
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
        // 两次轻量查询（亚毫秒）：组 + 全部状态 → 筛选条计数与协调；
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
            _lastGroupSignature = ""; // 强制重建组筛选条以更新选中态
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

        // 紧凑摘要（共享组件语义）：总数 · 待同步 · 需关注 · 无远端（全部仓库口径）。
        var summary = chips.SummaryCounts;
        var syncable = summary.LocalAhead + summary.RemoteAhead;
        var attention = summary.Diverged + summary.Error;
        SummaryText.Text = $"GitaView · {summary.Total} 个仓库 · 待同步 {syncable}"
            + $" · 需关注 {attention} · 无远端 {summary.NoRemote}";

        BuildGroupChips(chips);
        BuildStatusChips(chips.StatusCounts);
        await BuildRowsAsync(filtered);
        UpdateEmptyStates(chips.SummaryCounts.Total);
    }

    private void BuildGroupChips(ViewResult result)
    {
        // 选项集合只在组集合变化时重建（计数变化也要重建，旧版如此）。
        var signature = string.Join("|",
            result.Groups.Select(group => $"{group.Id}:{group.Name}:{group.Count}"));
        if (signature == _lastGroupSignature)
        {
            return;
        }
        _lastGroupSignature = signature;
        GroupChipsPanel.Children.Clear();
        GroupChipsPanel.Children.Add(MakeChip(
            $"全部分组 {result.SummaryCounts.Total}",
            _selectedGroupId is null,
            () => SelectGroup(null)));
        foreach (var group in result.Groups)
        {
            GroupChipsPanel.Children.Add(MakeChip(
                $"{group.Name} {group.Count}",
                _selectedGroupId == group.Id,
                () => SelectGroup(group.Id)));
        }
    }

    private void BuildStatusChips(WidgetCounts counts)
    {
        StatusChipsPanel.Children.Clear();
        StatusChipsPanel.Children.Add(MakeChip(
            $"全部 {counts.Total}",
            _selectedRelation == RelationFilter.All,
            () => SelectRelation(RelationFilter.All)));
        foreach (var (label, filter) in StatusChipOrder)
        {
            var count = CountOf(counts, filter);
            if (count == 0)
            {
                continue;
            }
            StatusChipsPanel.Children.Add(MakeChip(
                $"{label} {count}",
                _selectedRelation == filter,
                () => SelectRelation(filter)));
        }
    }

    private static uint CountOf(WidgetCounts counts, RelationFilter filter) => filter switch
    {
        RelationFilter.Error => counts.Error,
        RelationFilter.Synced => counts.Synced,
        RelationFilter.LocalAhead => counts.LocalAhead,
        RelationFilter.RemoteAhead => counts.RemoteAhead,
        RelationFilter.Diverged => counts.Diverged,
        RelationFilter.NoRemote => counts.NoRemote,
        _ => counts.Total,
    };

    private Button MakeChip(string text, bool selected, Action onClick)
    {
        var button = new Button
        {
            Content = text,
            Style = (Style)Resources["GvChipStyle"],
        };
        if (selected)
        {
            // 选中态：ink 底 + 白字（旧版 .filter-btn.active）。
            button.Background = RepoRowVm.ThemeBrush("GvSelectionBrush");
            button.Foreground = RepoRowVm.ThemeBrush("GvSurfaceBrush");
            button.BorderBrush = RepoRowVm.ThemeBrush("GvSelectionBrush");
        }
        button.Click += (_, _) => onClick();
        return button;
    }

    private void SelectGroup(string? groupId)
    {
        _selectedGroupId = groupId;
        _lastGroupSignature = "";
        _ = RequeryAsync();
    }

    private void SelectRelation(RelationFilter filter)
    {
        _selectedRelation = filter;
        _ = RequeryAsync();
    }

    private async Task BuildRowsAsync(ViewResult result)
    {
        var engine = App.Engine;
        if (engine is null)
        {
            return;
        }
        var settings = engine.SettingsGet();
        _groupNames.Clear();
        foreach (var group in settings.Groups)
        {
            _groupNames[group.Id] = group.Name;
        }

        Rows.Clear();
        foreach (var row in result.Rows)
        {
            var view = RepoStatusView.From(row);
            var groupName = row.GroupId is { } groupId
                && _groupNames.TryGetValue(groupId, out var name) ? name : "";
            var vm = new RepoRowVm
            {
                Id = row.Id,
                Name = row.Name,
                GroupDisplay = groupName,
                Branch = view.Branch is { Length: > 0 } branch ? branch : "—",
                StatusLabel = StatusLabelOf(view),
                RelationLabel = RelationLabelOf(view),
                ChangesLabel = ChangesLabelOf(view),
                Hint = HintOf(view),
                Dot = DotOf(view),
                RelationBrush = RelationBrushOf(view),
                RowBackground = row.Id == _selectedRepoId
                    ? RepoRowVm.ThemeBrush("GvRowSelectedBrush")
                    : RepoRowVm.ThemeBrush("GvSurfaceBrush"),
                CanOpenDirectory = row.Actions.CanOpenDirectory,
                CanOpenRemote = row.Actions.CanOpenRemote,
                CanFetch = row.Actions.CanFetch,
                CanPull = row.Actions.CanPull,
                CanPush = row.Actions.CanPush,
            };
            if (row.Id == _selectedRepoId)
            {
                vm.IsExpanded = true;
            }
            Rows.Add(vm);
        }
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
            Relation.NoRemote => "无远端",
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
            Relation.NoRemote => "无远端",
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
        _ => "",
    };

    private static Brush DotOf(RepoStatusView row) => row.ReadState switch
    {
        ReadState.Pending => RepoRowVm.ThemeBrush("GvSlateBrush"),
        ReadState.Error => RepoRowVm.ThemeBrush("GvRedBrush"),
        _ => row.Relation switch
        {
            Relation.Synced => RepoRowVm.ThemeBrush("GvDotSyncedBrush"),
            Relation.LocalAhead => RepoRowVm.ThemeBrush("GvDotSyncableBrush"),
            Relation.RemoteAhead => RepoRowVm.ThemeBrush("GvDotSyncableBrush"),
            Relation.Diverged => RepoRowVm.ThemeBrush("GvDotAttentionBrush"),
            Relation.NoRemote => RepoRowVm.ThemeBrush("GvDotNoRemoteBrush"),
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

    private void OnQueryChanged(object sender, TextChangedEventArgs e) => _ = RequeryAsync();

    private void OnRepoItemClick(object sender, ItemClickEventArgs e)
    {
        if (e.ClickedItem is not RepoRowVm clicked)
        {
            return;
        }
        // 点击行为与旧版一致：再点同一行收起。
        _selectedRepoId = _selectedRepoId == clicked.Id ? null : clicked.Id;
        foreach (var row in Rows)
        {
            row.IsExpanded = row.Id == _selectedRepoId;
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
        if (RowOf(sender) is not { } row || row.IsFetchBusy)
        {
            return;
        }
        row.IsFetchBusy = true;
        row.ResultText = "";
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
        if (RowOf(sender) is not { } row || row.IsPullBusy)
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
        row.ResultText = "";
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
        if (RowOf(sender) is not { } row || row.IsPushBusy)
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
        row.ResultText = "";
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
