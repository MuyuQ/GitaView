using GitaView.Native.Contracts;
using GitaView_Native;

namespace GitaView.Native.Tests;

public class TrayFlyoutPresentationFacts
{
    private static WidgetCounts Counts(uint synced = 0, uint noRemote = 0, uint pending = 0) => new()
    {
        Total = synced + noRemote + pending,
        Synced = synced,
        LocalAhead = 0,
        RemoteAhead = 0,
        Diverged = 0,
        NoRemote = noRemote,
        Error = 0,
        Pending = pending,
    };

    [Fact]
    public void adding_synced_repositories_replaces_the_add_repository_hint()
    {
        Assert.Contains("添加或扫描", TrayFlyoutPresentation.EmptyMessage(Counts(), RefreshState.Ready));
        Assert.Equal("所有仓库均已同步。",
            TrayFlyoutPresentation.EmptyMessage(Counts(synced: 3), RefreshState.Ready));
    }

    [Fact]
    public void repositories_without_remotes_are_not_reported_as_all_synced()
    {
        string message = TrayFlyoutPresentation.EmptyMessage(Counts(synced: 2, noRemote: 1), RefreshState.Ready);
        Assert.Contains("部分仓库缺少远端比较条件", message);
        Assert.DoesNotContain("所有仓库均已同步", message);
    }

    [Fact]
    public void pending_or_failed_refresh_does_not_report_a_successful_empty_state()
    {
        Assert.Equal("正在读取仓库状态…",
            TrayFlyoutPresentation.EmptyMessage(Counts(pending: 2), RefreshState.Ready));
        Assert.Contains("刷新未完成",
            TrayFlyoutPresentation.EmptyMessage(Counts(synced: 2), RefreshState.Failed));
        Assert.Contains("刷新未完成",
            TrayFlyoutPresentation.EmptyMessage(Counts(), RefreshState.Failed));
    }

    [Fact]
    public void bottom_taskbar_aligns_to_the_icon_and_respects_dpi()
    {
        var point = TrayFlyoutPresentation.Position(
            new(0, 0, 1920, 1040), new(1800, 1040, 24, 40), 500, 300, 1.25);
        Assert.Equal((1324, 730), point);
    }

    [Fact]
    public void secondary_monitor_with_negative_coordinates_keeps_its_work_area()
    {
        var point = TrayFlyoutPresentation.Position(
            new(-1920, 0, 1920, 1040), new(-120, 1040, 24, 40), 400, 250, 1);
        Assert.Equal((-496, 782), point);
    }

    [Fact]
    public void top_taskbar_places_the_flyout_below_the_icon()
    {
        var point = TrayFlyoutPresentation.Position(
            new(0, 40, 1920, 1040), new(1800, 0, 24, 40), 400, 250, 1);
        Assert.Equal((1424, 48), point);
    }

    [Fact]
    public void missing_icon_uses_the_work_area_corner_and_edge_icons_are_clamped()
    {
        var workArea = new FlyoutBounds(1920, 0, 1920, 1040);
        Assert.Equal((3428, 778), TrayFlyoutPresentation.Position(workArea, null, 400, 250, 1));
        Assert.Equal((1924, 88), TrayFlyoutPresentation.Position(
            workArea, new(1920, 40, 24, 40), 400, 250, 1));
    }
}
