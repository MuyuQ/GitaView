using Windows.ApplicationModel.AppExtensions;

// N03 诊断：直接查询 widgets app extension 目录（与 Widget 平台发现机制相同）。
var catalog = AppExtensionCatalog.Open("com.microsoft.windows.widgets");
var extensions = await catalog.FindAllAsync();
Console.WriteLine($"catalog count: {extensions.Count}");
foreach (var extension in extensions)
{
    Console.WriteLine($"  package={extension.AppInfo.PackageFamilyName} id={extension.Id} display={extension.DisplayName}");
}
if (extensions.Count == 0)
{
    Console.WriteLine("PLATFORM CATALOG DOES NOT SEE GitaView - service cache is stale.");
}
