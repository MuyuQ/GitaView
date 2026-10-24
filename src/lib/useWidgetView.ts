import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { createSnapshotReceiver, type StatusSnapshot } from "./statusSnapshots";
import { flushSync } from "react-dom";
import { createRefreshQueue } from "./refreshQueue";
import { currentMonitor, getCurrentWindow } from "@tauri-apps/api/window";
import { getRepoSnapshot, getSettings, exitApp, saveWindowState, syncDesktopWidgetFrame } from "./commands";
import { subscribeToSettingsUpdates } from "./settingsEvents";
import { hasTauriRuntime } from "./runtime";
import { shouldShowSettingsView } from "./statusModel";
import { resolveRefreshCompletion } from "./refreshGeneration";
import { resolveAnchoredWindowPosition } from "./windowMotion";
import type { WindowSizeValue } from "./windowMotion";
import type { WidgetRenderView, WidgetStableView } from "./widgetTransition";
import type { AppSettings, RepoStatus } from "../types";

const windowSizes: Record<WidgetStableView, WindowSizeValue> = {
  collapsed: { width: 312, height: 40 },
  expanded: { width: 900, height: 560 },
  settings: { width: 760, height: 540 },
} as const;
const resizeGuardBackground = { red: 247, green: 249, blue: 252, alpha: 255 };
const transparentWindowBackground = { red: 0, green: 0, blue: 0, alpha: 0 };
const resizeGuardRestoreMs = 140;

export interface WidgetViewState {
  view: WidgetRenderView;
  windowView: WidgetStableView;
  repos: RepoStatus[];
  initialLoading: boolean;
  refreshing: boolean;
  initialError: string | null;
  refreshError: string | null;
  lastRefreshAt: Date | null;
  allowWidgetDrag: boolean;
  emptySettingsDismissed: boolean;
}

export interface WidgetViewActions {
  expandCollapsedView: () => void;
  collapseExpandedView: () => void;
  handleExit: () => void;
  showView: (nextView: WidgetStableView) => void;
  navigateToSettings: () => void;
  refreshRepos: (opts: { initial: boolean }) => void;
  startDrag: () => void;
  dismissEmptySettings: () => void;
  reloadSettings: () => void;
}

export function useWidgetView(): WidgetViewState & WidgetViewActions {
  const [view, setView] = useState<WidgetRenderView>("collapsed");
  const [windowView, setWindowView] = useState<WidgetStableView>("collapsed");
  const [repos, setRepos] = useState<RepoStatus[]>([]);
  const [initialLoading, setInitialLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [initialError, setInitialError] = useState<string | null>(null);
  const [refreshError, setRefreshError] = useState<string | null>(null);
  const [lastRefreshAt, setLastRefreshAt] = useState<Date | null>(null);
  const [allowWidgetDrag, setAllowWidgetDrag] = useState(true);
  const [emptySettingsDismissed, setEmptySettingsDismissed] = useState(false);
  const hasLoadedOnce = useRef(false);
  const resizeGuardTimer = useRef<number | null>(null);
  const latestRefreshGeneration = useRef(0);
  const refreshQueueRef = useRef<ReturnType<typeof createRefreshQueue> | null>(null);
  const initialRefreshPending = useRef(false);

  const applySettings = useCallback((settings: AppSettings) => {
    setAllowWidgetDrag(settings.appearance.allowWidgetDrag);
  }, []);

  const reloadSettings = useCallback(() => {
    getSettings()
      .then(applySettings)
      .catch((err) => {
        console.error("加载设置失败", err);
      });
  }, [applySettings]);

  const syncNativeWindowFrame = useCallback((nextView: WidgetStableView) => {
    if (!hasTauriRuntime()) return;
    const size = windowSizes[nextView];
    const appWindow = getCurrentWindow();
    const shouldUseResizeGuard = nextView !== "collapsed";

    if (resizeGuardTimer.current !== null) {
      window.clearTimeout(resizeGuardTimer.current);
      resizeGuardTimer.current = null;
    }

    const restoreTransparentBackground = () => {
      if (!shouldUseResizeGuard) return;
      resizeGuardTimer.current = window.setTimeout(() => {
        resizeGuardTimer.current = null;
        appWindow.setBackgroundColor(transparentWindowBackground).catch((err) => {
          console.error("恢复透明窗口背景失败", err);
        });
      }, resizeGuardRestoreMs);
    };

    const prepareResizeBackground = shouldUseResizeGuard
      ? appWindow.setBackgroundColor(resizeGuardBackground).catch((err) => {
        console.error("设置窗口 resize 背景保护失败", err);
      })
      : appWindow.setBackgroundColor(transparentWindowBackground).catch((err) => {
        console.error("恢复透明窗口背景失败", err);
      });

    const syncSizeOnly = () => appWindow.scaleFactor().then((scaleFactor) => syncDesktopWidgetFrame({
      width: Math.round(size.width * scaleFactor),
      height: Math.round(size.height * scaleFactor),
    }));

    prepareResizeBackground.then(() => Promise.all([
      appWindow.outerPosition(),
      appWindow.outerSize(),
      appWindow.scaleFactor(),
      currentMonitor(),
    ])).then(([position, currentSize, scaleFactor, monitor]) => {
      const targetSize = {
        width: Math.round(size.width * scaleFactor),
        height: Math.round(size.height * scaleFactor),
      };
      if (!monitor) {
        return syncDesktopWidgetFrame(targetSize);
      }

      const nextPosition = resolveAnchoredWindowPosition(
        {
          x: position.x,
          y: position.y,
          width: currentSize.width,
          height: currentSize.height,
        },
        {
          width: Math.round(size.width * scaleFactor),
          height: Math.round(size.height * scaleFactor),
        },
        {
          x: monitor.workArea.position.x,
          y: monitor.workArea.position.y,
          width: monitor.workArea.size.width,
          height: monitor.workArea.size.height,
        },
      );

      return syncDesktopWidgetFrame({ ...nextPosition, ...targetSize });
    }).then(() => {
      restoreTransparentBackground();
    }).catch((err) => {
      console.error("同步窗口位置失败", err);
      syncSizeOnly().catch((sizeErr) => {
        console.error("调整窗口尺寸失败", sizeErr);
      }).finally(() => {
        restoreTransparentBackground();
      });
    });
  }, []);

  const showView = useCallback((nextView: WidgetStableView) => {
    flushSync(() => {
      setView(nextView);
      setWindowView(nextView);
    });
    syncNativeWindowFrame(nextView);
  }, [syncNativeWindowFrame]);

  const expandCollapsedView = useCallback(() => {
    showView("expanded");
  }, [showView]);

  const collapseExpandedView = useCallback(() => {
    showView("collapsed");
  }, [showView]);

  const handleExit = useCallback(() => {
    exitApp().catch((err) => {
      console.error("退出应用失败", err);
    });
  }, []);

  const snapshotReceiver = useRef<ReturnType<typeof createSnapshotReceiver> | null>(null);
  if (!snapshotReceiver.current) {
    snapshotReceiver.current = createSnapshotReceiver((snapshot) => {
      if (snapshot.error) {
        if (hasLoadedOnce.current) setRefreshError(snapshot.error);
        else setInitialError(snapshot.error);
      } else {
        setRepos(snapshot.repos);
        if (snapshot.repos.length > 0) setEmptySettingsDismissed(false);
        setLastRefreshAt(new Date(snapshot.completedAt));
        setInitialError(null);
        setRefreshError(null);
        hasLoadedOnce.current = true;
      }
      setInitialLoading(false);
    });
  }

  const runRefresh = useCallback(() => {
    const opts = { initial: initialRefreshPending.current };
    initialRefreshPending.current = false;
    const requestGeneration = ++latestRefreshGeneration.current;
    if (opts.initial) {
      setInitialLoading(true);
    }
    setRefreshing(true);
    setRefreshError(null);
    return getRepoSnapshot(!opts.initial)
      .then((snapshot) => snapshotReceiver.current!(snapshot))
      .catch((err) => {
        if (!resolveRefreshCompletion(requestGeneration, latestRefreshGeneration.current)) return;
        const message = String(err);
        if (opts.initial && !hasLoadedOnce.current) {
          setInitialError(message);
        } else {
          setRefreshError(message);
        }
      })
      .finally(() => {
        if (!resolveRefreshCompletion(requestGeneration, latestRefreshGeneration.current)) return;
        setInitialLoading(false);
        setRefreshing(false);
      });
  }, []);

  if (!refreshQueueRef.current) refreshQueueRef.current = createRefreshQueue(runRefresh);
  const refreshRepos = useCallback((opts: { initial: boolean }) => {
    initialRefreshPending.current ||= opts.initial;
    void refreshQueueRef.current!();
  }, []);

  const startDrag = useCallback(() => {
    if (!hasTauriRuntime()) return;
    const contextLabel = view === "expanded" ? "展开" : "收起";
    getCurrentWindow().startDragging().catch((err) => {
      console.error(`启动${contextLabel}浮窗拖动失败`, err);
    });
  }, [view]);

  const navigateToSettings = useCallback(() => {
    setInitialError(null);
    showView("settings");
  }, [showView]);

  const dismissEmptySettings = useCallback(() => {
    setEmptySettingsDismissed(true);
    refreshRepos({ initial: false });
    showView("collapsed");
    reloadSettings();
  }, [refreshRepos, showView, reloadSettings]);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    const subscribe = hasTauriRuntime()
      ? listen<StatusSnapshot>("repo-status-snapshot", ({ payload }) => {
        if (!disposed) snapshotReceiver.current!(payload);
      })
      : Promise.resolve(() => {});
    subscribe.then((dispose) => {
      if (disposed) { dispose(); return; }
      unlisten = dispose;
      refreshRepos({ initial: true });
    }).catch((error) => {
      if (disposed) return;
      console.error("订阅仓库状态失败", error);
      refreshRepos({ initial: true });
    });
    return () => { disposed = true; unlisten?.(); };
  }, [refreshRepos]);

  useEffect(() => {
    if (!hasTauriRuntime()) return;
    if (initialLoading) return;
    const targetView = initialError && repos.length === 0
      ? "expanded"
      : shouldShowSettingsView(windowView, repos.length, initialError, emptySettingsDismissed) ? "settings" : windowView;
    syncNativeWindowFrame(targetView);
  }, [windowView, repos.length, initialError, emptySettingsDismissed, initialLoading, syncNativeWindowFrame]);

  useEffect(() => {
    reloadSettings();
  }, [reloadSettings]);

  useEffect(() => () => {
    if (resizeGuardTimer.current === null) return;
    window.clearTimeout(resizeGuardTimer.current);
  }, []);

  // 窗口位置持久化（规格 §8）：移动后防抖 800ms 保存物理坐标，
  // 恢复在 Rust setup 阶段完成（先于前端首次帧同步）
  useEffect(() => {
    if (!hasTauriRuntime()) return;
    const appWindow = getCurrentWindow();
    let saveTimer: number | null = null;
    let unlisten: (() => void) | null = null;
    let disposed = false;
    appWindow
      .onMoved(({ payload }) => {
        if (saveTimer !== null) window.clearTimeout(saveTimer);
        saveTimer = window.setTimeout(() => {
          saveTimer = null;
          saveWindowState(payload.x, payload.y).catch((err) => {
            console.error("保存窗口位置失败", err);
          });
        }, 800);
      })
      .then((dispose) => {
        if (disposed) dispose();
        else unlisten = dispose;
      });
    return () => {
      disposed = true;
      unlisten?.();
      if (saveTimer !== null) window.clearTimeout(saveTimer);
    };
  }, []);

  useEffect(() => subscribeToSettingsUpdates(applySettings), [applySettings]);

  useEffect(() => {
    const transparent = view === "collapsed"
      && !shouldShowSettingsView(windowView, repos.length, initialError, emptySettingsDismissed);
    document.body.classList.toggle("gv-collapsed-view", transparent);
    return () => document.body.classList.remove("gv-collapsed-view");
  }, [view, windowView, repos.length, initialError, emptySettingsDismissed]);

  return {
    view,
    windowView,
    repos,
    initialLoading,
    refreshing,
    initialError,
    refreshError,
    lastRefreshAt,
    allowWidgetDrag,
    emptySettingsDismissed,
    expandCollapsedView,
    collapseExpandedView,
    handleExit,
    showView,
    refreshRepos,
    startDrag,
    navigateToSettings,
    dismissEmptySettings,
    reloadSettings,
  };
}
