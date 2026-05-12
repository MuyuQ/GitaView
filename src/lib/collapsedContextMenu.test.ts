import { describe, expect, it, vi } from "vitest";
import {
  createCollapsedContextMenuItems,
  showCollapsedNativeContextMenu,
  type CollapsedContextMenuActions,
} from "./collapsedContextMenu";

function createActions(overrides: Partial<CollapsedContextMenuActions> = {}): CollapsedContextMenuActions {
  return {
    onRefresh: vi.fn(),
    onExit: vi.fn(),
    ...overrides,
  };
}

describe("createCollapsedContextMenuItems", () => {
  it("builds the collapsed widget menu with refresh and exit items", () => {
    const actions = createActions();

    const items = createCollapsedContextMenuItems(actions);

    expect(items.map((item) => item.text)).toEqual(["刷新", "退出"]);
    expect(items[0]).toMatchObject({ id: "collapsed-refresh", kind: "item" });
    expect(items[1]).toMatchObject({ id: "collapsed-exit", kind: "item" });
  });

  it("wires each menu item to the matching action", () => {
    const actions = createActions();
    const items = createCollapsedContextMenuItems(actions);

    items[0].action();
    items[1].action();

    expect(actions.onRefresh).toHaveBeenCalledTimes(1);
    expect(actions.onExit).toHaveBeenCalledTimes(1);
  });
});

describe("showCollapsedNativeContextMenu", () => {
  it("uses a native popup menu at the right-click position when Tauri is available", async () => {
    const popup = vi.fn().mockResolvedValue(undefined);
    const close = vi.fn().mockResolvedValue(undefined);
    const createMenu = vi.fn().mockResolvedValue({ popup, close });
    const createPosition = vi.fn((x: number, y: number) => ({ x, y }));
    const window = { label: "main" };

    const shown = await showCollapsedNativeContextMenu(
      { x: 21, y: 17 },
      createActions(),
      {
        isAvailable: () => true,
        createMenu,
        createPosition,
        getWindow: () => window,
      },
    );

    expect(shown).toBe(true);
    expect(createMenu).toHaveBeenCalledWith([
      expect.objectContaining({ text: "刷新" }),
      expect.objectContaining({ text: "退出" }),
    ]);
    expect(createPosition).toHaveBeenCalledWith(21, 17);
    expect(popup).toHaveBeenCalledWith({ x: 21, y: 17 }, window);
    expect(close).toHaveBeenCalledTimes(1);
  });

  it("does not try to create a native menu outside Tauri", async () => {
    const createMenu = vi.fn();

    const shown = await showCollapsedNativeContextMenu(
      { x: 0, y: 0 },
      createActions(),
      {
        isAvailable: () => false,
        createMenu,
        createPosition: (x, y) => ({ x, y }),
        getWindow: () => ({}),
      },
    );

    expect(shown).toBe(false);
    expect(createMenu).not.toHaveBeenCalled();
  });
});
