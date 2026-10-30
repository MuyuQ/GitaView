namespace GitaView.Tools.WidgetCatalogProbe;

public enum WidgetCatalogState { Empty, Missing, Found }

internal readonly record struct WidgetCatalogEntry(string PackageFamilyName, string ExtensionId);

internal static class WidgetCatalogReport
{
    internal static WidgetCatalogState Evaluate(IReadOnlyCollection<WidgetCatalogEntry> entries)
    {
        if (entries.Count == 0) return WidgetCatalogState.Empty;
        return entries.Any(entry =>
            entry.PackageFamilyName.StartsWith("GitaView.Native_", StringComparison.OrdinalIgnoreCase)
            && entry.ExtensionId == "GitaViewWidget")
            ? WidgetCatalogState.Found : WidgetCatalogState.Missing;
    }

    internal static string Describe(WidgetCatalogState state) => state switch
    {
        WidgetCatalogState.Found =>
            "已在平台目录发现 GitaView.Native / GitaViewWidget；这只确认目录注册，不代表 Widgets Board 已显示或 Provider 激活成功。",
        WidgetCatalogState.Empty =>
            "平台目录未返回任何 Widget 扩展，原因尚未确定。请检查当前用户下的应用包安装和扩展注册。",
        _ =>
            "平台目录包含其他扩展，但未发现 GitaView.Native / GitaViewWidget，原因尚未确定。请核对包身份和扩展 ID。",
    };
}
