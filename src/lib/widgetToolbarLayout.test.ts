import { describe, expect, it } from "vitest";
import { readProjectFile } from "./sourceContract";

function declarationBlock(css: string, selector: string) {
  const escapedSelector = selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  return css.match(new RegExp(`${escapedSelector}\\s*{[^}]*}`))?.[0] ?? "";
}

describe("expanded widget toolbar layout", () => {
  it("groups refresh, settings, and collapse actions on the trailing side", () => {
    const component = readProjectFile("src/components/WidgetExpanded.tsx");
    const actionsMarkup = component.match(/<div className="widget-toolbar-actions">[\s\S]*?<\/div>\s*<\/header>/)?.[0] ?? "";

    expect(actionsMarkup).toContain("onClick={onRefresh}");
    expect(actionsMarkup).toContain("onClick={onOpenSettings}");
    expect(actionsMarkup).toContain("onClick={onCollapse}");
  });

  it("keeps the toolbar action group aligned to the right edge", () => {
    const widgetCss = readProjectFile("src/styles/widget.css");
    const toolbar = declarationBlock(widgetCss, ".widget-toolbar");
    const actions = declarationBlock(widgetCss, ".widget-toolbar-actions");

    expect(toolbar).toContain("justify-content: space-between;");
    expect(actions).toContain("margin-left: auto;");
    expect(actions).toContain("display: flex;");
    expect(actions).toContain("align-items: center;");
  });
});
