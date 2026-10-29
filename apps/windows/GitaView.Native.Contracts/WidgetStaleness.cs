// Widget 数据新鲜度（Provider 侧；与 Rust widget.staleness 同语义，因 Provider
// 不加载引擎而在此镜像实现。契约变更时两侧同步——测试以同一 fixture 边界锁定）。
//
// 规则（实施说明 §6.5 / §12）：
// - 手动模式（enabled=false）：不过期判断；
// - 开启定时刷新：now 越过 last + 2*interval（严格大于）为过期；等于阈值仍有效；
// - 成功时间在未来（时钟回拨）：FutureTime——显示等待刷新，不做无符号减法。

namespace GitaView.Native.Contracts;

public enum WidgetStaleness
{
    Manual,
    Fresh,
    Stale,
    FutureTime,
}

public static class WidgetStalenessExtensions
{
    public static WidgetStaleness ComputeStaleness(this WidgetSnapshot snapshot, ulong nowMs)
    {
        if (!snapshot.RefreshPolicy.Enabled)
        {
            return WidgetStaleness.Manual;
        }
        if (snapshot.LastSuccessAtMs is not { } last)
        {
            // 从未成功过：由 refreshState 表达，不是过期。
            return WidgetStaleness.Fresh;
        }
        if (last > nowMs)
        {
            return WidgetStaleness.FutureTime;
        }
        var intervalMs = Math.Max(snapshot.RefreshPolicy.IntervalMinutes, 1) * 60_000UL;
        var threshold = intervalMs * 2;
        return nowMs - last > threshold ? WidgetStaleness.Stale : WidgetStaleness.Fresh;
    }

    /// <summary>
    /// 过期边界触发时刻：last + 2*interval + 1（等于阈值仍有效，触发须越过边界）。
    /// 时钟回拨（成功时间晚于 now）返回 null——不调度，等待新数据。
    /// </summary>
    public static ulong? StalenessBoundaryMs(this WidgetSnapshot snapshot, ulong nowMs) =>
        snapshot.LastSuccessAtMs is { } last && last <= nowMs
            ? last + Math.Max(snapshot.RefreshPolicy.IntervalMinutes, 1) * 60_000UL * 2 + 1
            : null;
}
