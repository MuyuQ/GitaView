using GitaView.Native.Contracts;

namespace GitaView_Native;

internal readonly record struct FlyoutBounds(int X, int Y, int Width, int Height);

internal static class TrayFlyoutPresentation
{
    internal static string EmptyMessage(WidgetCounts counts, RefreshState refreshState)
    {
        if (refreshState == RefreshState.Failed)
        {
            return "刷新未完成，打开 GitaView 查看详情或重试。";
        }
        if (counts.Total == 0)
        {
            return "点击下方「打开 GitaView」，添加或扫描仓库。";
        }
        if (counts.Pending > 0)
        {
            return "正在读取仓库状态…";
        }
        if (counts.Synced == counts.Total)
        {
            return "所有仓库均已同步。";
        }
        return counts.NoRemote > 0
            ? "暂无待同步或需关注的仓库。部分仓库缺少远端比较条件。"
            : "暂无待同步或需关注的仓库。";
    }

    // 坐标均为屏幕物理像素；工作区由托盘图标所在显示器提供。
    internal static (int X, int Y) Position(
        FlyoutBounds workArea, FlyoutBounds? anchor, int width, int height, double scale)
    {
        int margin = (int)Math.Round(4 * scale);
        int gap = (int)Math.Round(8 * scale);
        int fallbackGap = (int)Math.Round(12 * scale);
        int x;
        int y;
        if (anchor is { } tray)
        {
            x = tray.X + tray.Width - width;
            y = tray.Y - height - gap;
            if (y < workArea.Y && tray.Y + tray.Height + gap + height <= workArea.Y + workArea.Height)
            {
                y = tray.Y + tray.Height + gap;
            }
        }
        else
        {
            x = workArea.X + workArea.Width - width - fallbackGap;
            y = workArea.Y + workArea.Height - height - fallbackGap;
        }
        int minX = workArea.X + margin;
        int minY = workArea.Y + margin;
        return (
            Math.Clamp(x, minX, Math.Max(minX, workArea.X + workArea.Width - width - margin)),
            Math.Clamp(y, minY, Math.Max(minY, workArea.Y + workArea.Height - height - margin)));
    }
}
