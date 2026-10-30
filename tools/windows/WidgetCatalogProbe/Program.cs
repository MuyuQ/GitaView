using Windows.ApplicationModel.AppExtensions;
using GitaView.Tools.WidgetCatalogProbe;

try
{
    var catalog = AppExtensionCatalog.Open("com.microsoft.windows.widgets");
    var extensions = await catalog.FindAllAsync();
    Console.WriteLine($"catalog count: {extensions.Count}");
    foreach (var extension in extensions)
    {
        Console.WriteLine($"  package={extension.AppInfo.PackageFamilyName} id={extension.Id} display={extension.DisplayName}");
    }
    var state = WidgetCatalogReport.Evaluate(extensions.Select(extension =>
        new WidgetCatalogEntry(extension.AppInfo.PackageFamilyName, extension.Id)).ToArray());
    Console.WriteLine(WidgetCatalogReport.Describe(state));
    return state == WidgetCatalogState.Found ? 0 : 2;
}
catch (Exception error)
{
    Console.Error.WriteLine($"Widget 目录查询失败：{error.Message}");
    return 1;
}
