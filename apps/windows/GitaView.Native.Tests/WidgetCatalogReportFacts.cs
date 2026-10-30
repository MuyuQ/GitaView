using GitaView.Tools.WidgetCatalogProbe;

namespace GitaView.Native.Tests;

public class WidgetCatalogReportFacts
{
    [Fact]
    public void empty_catalog_has_a_distinct_result()
    {
        Assert.Equal(WidgetCatalogState.Empty,
            WidgetCatalogReport.Evaluate(Array.Empty<WidgetCatalogEntry>()));
    }

    [Theory]
    [InlineData("OtherApp_publisher", "GitaViewWidget", WidgetCatalogState.Missing)]
    [InlineData("GitaView.Native.Other_publisher", "GitaViewWidget", WidgetCatalogState.Missing)]
    [InlineData("GitaView.Native_publisher", "OtherWidget", WidgetCatalogState.Missing)]
    [InlineData("GitaView.Native_publisher", "GitaViewWidget", WidgetCatalogState.Found)]
    [InlineData("gitaview.native_publisher", "GitaViewWidget", WidgetCatalogState.Found)]
    public void matches_package_identity_and_extension_id(
        string packageFamilyName, string extensionId, WidgetCatalogState expected)
    {
        Assert.Equal(expected, WidgetCatalogReport.Evaluate(
            new[] { new WidgetCatalogEntry(packageFamilyName, extensionId) }));
    }

    [Fact]
    public void finds_gitaview_among_unrelated_extensions()
    {
        Assert.Equal(WidgetCatalogState.Found, WidgetCatalogReport.Evaluate(new[]
        {
            new WidgetCatalogEntry("OtherApp_publisher", "OtherWidget"),
            new WidgetCatalogEntry("GitaView.Native_publisher", "GitaViewWidget"),
        }));
    }
}
