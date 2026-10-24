// 卡片模板与数据 JSON（N16）：三种尺寸独立模板；数据只含安全投影字段。

using System.Text.Json;
using GitaView.Native.Contracts;
using Microsoft.Windows.Widgets;

namespace GitaView_Native.WidgetProvider;

internal static class WidgetCardContent
{
    public static string TemplateFor(WidgetSize size) => TemplateForIndex((int)size);

    /// <summary>尺寸 → 模板（0=small 1=medium 2=large；供无 WinRT 的测试使用）。</summary>
    public static string TemplateForIndex(int sizeIndex) => sizeIndex switch
    {
        0 => SmallTemplate,
        2 => LargeTemplate,
        _ => MediumTemplate,
    };

    private const string SmallTemplate =
        """
        {
          "type": "AdaptiveCard",
          "$schema": "http://adaptivecards.io/schemas/adaptive-card.json",
          "version": "1.5",
          "body": [
            { "type": "TextBlock", "text": "GitaView", "weight": "bolder", "wrap": true },
            { "type": "TextBlock", "text": "共 ${total} 仓库", "size": "large", "wrap": true },
            { "type": "TextBlock", "text": "同步 ${synced} · 需处理 ${needsAttention}", "wrap": true },
            { "type": "TextBlock", "text": "${stateText}", "wrap": true, "$when": "${state == 'noData'}" },
            { "type": "TextBlock", "text": "数据已过期", "color": "attention", "wrap": true, "$when": "${stale}" }
          ],
          "actions": [ { "type": "Action.Execute", "title": "打开 GitaView", "verb": "open" } ]
        }
        """;

    private const string MediumTemplate =
        """
        {
          "type": "AdaptiveCard",
          "$schema": "http://adaptivecards.io/schemas/adaptive-card.json",
          "version": "1.5",
          "body": [
            { "type": "TextBlock", "text": "GitaView 仓库状态", "weight": "bolder", "size": "medium", "wrap": true },
            { "type": "TextBlock", "text": "${time}", "isSubtle": true, "spacing": "none", "wrap": true },
            { "type": "TextBlock", "text": "共 ${total} 个仓库", "size": "large", "wrap": true },
            { "type": "TextBlock", "text": "已同步 ${synced} · 需处理 ${needsAttention} · 待读取 ${pending}", "wrap": true },
            { "type": "TextBlock", "text": "${stateText}", "wrap": true, "$when": "${state == 'noData'}" },
            { "type": "TextBlock", "text": "数据已过期 — 打开 GitaView 刷新", "color": "attention", "wrap": true, "$when": "${stale}" }
          ],
          "actions": [ { "type": "Action.Execute", "title": "打开 GitaView", "verb": "open" } ]
        }
        """;

    private const string LargeTemplate =
        """
        {
          "type": "AdaptiveCard",
          "$schema": "http://adaptivecards.io/schemas/adaptive-card.json",
          "version": "1.5",
          "body": [
            { "type": "TextBlock", "text": "GitaView 仓库状态", "weight": "bolder", "size": "medium", "wrap": true },
            { "type": "TextBlock", "text": "${time}", "isSubtle": true, "spacing": "none", "wrap": true },
            { "type": "TextBlock", "text": "共 ${total} 个仓库", "size": "large", "wrap": true },
            { "type": "TextBlock", "text": "已同步 ${synced} · 需处理 ${needsAttention} · 待读取 ${pending}", "wrap": true },
            { "type": "TextBlock", "text": "${stateText}", "wrap": true, "$when": "${state == 'noData'}" },
            { "type": "TextBlock", "text": "数据已过期 — 打开 GitaView 刷新", "color": "attention", "wrap": true, "$when": "${stale}" },
            { "type": "Container", "$when": "${attention.length > 0}", "items": [
              { "type": "TextBlock", "text": "关注：", "isSubtle": true, "wrap": true },
              { "type": "TextBlock", "text": "${name} — ${state}", "wrap": true, "$data": "${attention}" }
            ] }
          ],
          "actions": [ { "type": "Action.Execute", "title": "打开 GitaView", "verb": "open" } ]
        }
        """;

    /// <summary>数据负载：只含安全投影；损坏 / 缺失 → noData 占位。</summary>
    public static string DataJson(WidgetSnapshot? snapshot)
    {
        if (snapshot is null)
        {
            return JsonSerializer.Serialize(new Dictionary<string, object?>
            {
                ["state"] = "noData",
                ["stateText"] = "打开 GitaView 获取仓库状态",
                ["stale"] = false,
                ["total"] = 0,
                ["synced"] = 0,
                ["needsAttention"] = 0,
                ["pending"] = 0,
                ["time"] = "",
                ["attention"] = Array.Empty<object>(),
            });
        }
        var staleness = snapshot.ComputeStaleness(NowMs());
        var stale = staleness == WidgetStaleness.Stale;
        string time = snapshot.LastSuccessAtMs is null
            ? "尚未成功读取"
            : snapshot.LastSuccessAtMs.Value.ToUnixDateTime().ToLocalTime().ToString("yyyy-MM-dd HH:mm");
        string stateText = staleness switch
        {
            WidgetStaleness.FutureTime => "时间异常，等待刷新",
            _ => snapshot.RefreshState.ToString().ToLowerInvariant(),
        };
        var attention = snapshot.Attention
            .Take(6)
            .Select(row => new
            {
                name = JsonSerializer.Serialize(row.Name),
                state = StateText(row),
            });
        return JsonSerializer.Serialize(new Dictionary<string, object?>
        {
            ["state"] = "ready",
            ["stateText"] = stateText,
            ["stale"] = stale,
            ["total"] = snapshot.Counts.Total,
            ["synced"] = snapshot.Counts.Synced,
            ["needsAttention"] = snapshot.Compact.NeedsAttention,
            ["pending"] = snapshot.Counts.Pending,
            ["time"] = time,
            ["attention"] = attention,
        });
    }

    private static string StateText(WidgetRepo row) => row.ReadState switch
    {
        ReadState.Error => "读取失败",
        ReadState.Pending => "未读取",
        _ => row.Relation switch
        {
            Relation.Diverged => "分叉",
            Relation.RemoteAhead => "可拉取",
            Relation.LocalAhead => "可推送",
            _ => "正常",
        },
    };

    private static ulong NowMs() =>
        unchecked((ulong)DateTimeOffset.UtcNow.ToUnixTimeMilliseconds());
}
