import { LogicalPosition } from "@tauri-apps/api/dpi";
import { Menu } from "@tauri-apps/api/menu";
import { getCurrentWindow, type Window } from "@tauri-apps/api/window";
import { hasTauriRuntime } from "./runtime";

export interface CollapsedContextMenuActions {
  onRefresh: () => void | Promise<void>;
  onExit: () => void | Promise<void>;
}

export type CollapsedContextMenuItem = {
  id: string;
  text: string;
  kind: "check" | "item";
  checked?: boolean;
  action: () => void | Promise<void>;
};

type NativePopupMenu = {
  popup: (position: unknown, window: unknown) => Promise<void>;
  close?: () => Promise<void>;
};

export interface CollapsedContextMenuDeps {
  isAvailable: () => boolean;
  createMenu: (items: CollapsedContextMenuItem[]) => Promise<NativePopupMenu>;
  createPosition: (x: number, y: number) => unknown;
  getWindow: () => unknown;
}

export function createCollapsedContextMenuItems(
  actions: CollapsedContextMenuActions,
): CollapsedContextMenuItem[] {
  return [
    {
      id: "collapsed-refresh",
      text: "刷新",
      kind: "item",
      action: actions.onRefresh,
    },
    {
      id: "collapsed-exit",
      text: "退出",
      kind: "item",
      action: actions.onExit,
    },
  ];
}

const nativeContextMenuDeps: CollapsedContextMenuDeps = {
  isAvailable: hasTauriRuntime,
  createMenu: async (items) => {
    const menu = await Menu.new({
      items: items.map((item) => ({
        id: item.id,
        text: item.text,
        checked: item.kind === "check" ? item.checked : undefined,
        action: () => {
          void item.action();
        },
      })),
    });
    return {
      popup: (position, window) => menu.popup(position as LogicalPosition, window as Window),
      close: () => menu.close(),
    };
  },
  createPosition: (x, y) => new LogicalPosition(x, y),
  getWindow: () => getCurrentWindow(),
};

export async function showCollapsedNativeContextMenu(
  position: { x: number; y: number },
  actions: CollapsedContextMenuActions,
  deps: CollapsedContextMenuDeps = nativeContextMenuDeps,
): Promise<boolean> {
  if (!deps.isAvailable()) return false;
  const menu = await deps.createMenu(createCollapsedContextMenuItems(actions));
  try {
    await menu.popup(deps.createPosition(position.x, position.y), deps.getWindow());
  } finally {
    await menu.close?.();
  }
  return true;
}
