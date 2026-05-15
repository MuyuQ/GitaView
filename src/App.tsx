import { useCallback, useEffect, useRef, useState } from "react";
import { flushSync } from "react-dom";
import { currentMonitor, getCurrentWindow, LogicalSize, PhysicalPosition } from "@tauri-apps/api/window";
import { listRepoStatuses, getSettings, exitApp } from "./lib/commands";
import { subscribeToSettingsUpdates } from "./lib/settingsEvents";
import { hasTauriRuntime } from "./lib/runtime";
import { shouldShowSettingsView } from "./lib/statusModel";
import { resolveAnchoredWindowPosition } from "./lib/windowMotion";
import type { WindowSizeValue } from "./lib/windowMotion";
import { getTransitionMode, isWidgetTransitionView } from "./lib/widgetTransition";
import type { WidgetRenderView, WidgetStableView } from "./lib/widgetTransition";
import type { AppSettings, RepoStatus } from "./types";
import { WidgetCollapsed } from "./components/WidgetCollapsed";
import { WidgetExpanded } from "./components/WidgetExpanded";
import { SettingsShell } from "./components/settings/SettingsShell";

const windowSizes: Record<WidgetStableView, WindowSizeValue> = {
  collapsed: { width: 218, height: 40 },
  expanded: { width: 900, height: 560 },
  settings: { width: 760, height: 540 },
} as const;
const resizeGuardBackground = { red: 247, green: 249, blue: 252, alpha: 255 };
const transparentWindowBackground = { red: 0, green: 0, blue: 0, alpha: 0 };
const resizeGuardRestoreMs = 140;

export default function App() {
  const [view, setView] = useState<WidgetRenderView>("collapsed");
  const [windowView, setWindowView] = useState<WidgetStableView>("collapsed");
  const [repos, setRepos] = useState<RepoStatus[]>([]);
  const [initialLoading, setInitialLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [initialError, setInitialError] = useState<string | null>(null);
  const [refreshError, setRefreshError] = useState<string | null>(null);
  const [lastRefreshAt, setLastRefreshAt] = useState<Date | null>(null);
  const [refreshSettings, setRefreshSettings] = useState<AppSettings["refresh"] | null>(null);
  const [allowWidgetDrag, setAllowWidgetDrag] = useState(true);
  const [emptySettingsDismissed, setEmptySettingsDismissed] = useState(false);
  const hasLoadedOnce = useRef(false);
  const resizeGuardTimer = useRef<number | null>(null);

  function applySettings(settings: AppSettings) {
    setRefreshSettings(settings.refresh);
    setAllowWidgetDrag(settings.appearance.allowWidgetDrag);
  }

  function reloadSettings() {
    getSettings()
      .then(applySettings)
      .catch((err) => {
        console.error("加载设置失败", err);
      });
  }

  const syncNativeWindowFrame = useCallback((nextView: WidgetStableView) => {
    if (!hasTauriRuntime()) return;
    const size = windowSizes[nextView];
    const targetLogicalSize = new LogicalSize(size.width, size.height);
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

    // 设置窗口置顶状态：settings 视图置顶，其他不置顶
    appWindow.setAlwaysOnTop(nextView === "settings").catch((err) => {
      console.error("设置窗口置顶状态失败", err);
    });

    const prepareResizeBackground = shouldUseResizeGuard
      ? appWindow.setBackgroundColor(resizeGuardBackground).catch((err) => {
        console.error("设置窗口 resize 背景保护失败", err);
      })
      : appWindow.setBackgroundColor(transparentWindowBackground).catch((err) => {
        console.error("恢复透明窗口背景失败", err);
      });

    prepareResizeBackground.then(() => Promise.all([
      appWindow.outerPosition(),
      appWindow.outerSize(),
      appWindow.scaleFactor(),
      currentMonitor(),
    ])).then(([position, currentSize, scaleFactor, monitor]) => {
      if (!monitor) {
        return appWindow.setSize(targetLogicalSize);
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

      return appWindow
        .setPosition(new PhysicalPosition(nextPosition.x, nextPosition.y))
        .then(() => appWindow.setSize(targetLogicalSize));
    }).then(() => {
      restoreTransparentBackground();
    }).catch((err) => {
      console.error("同步窗口位置失败", err);
      appWindow.setSize(targetLogicalSize).catch((sizeErr) => {
        console.error("调整窗口尺寸失败", sizeErr);
      }).finally(() => {
        restoreTransparentBackground();
      });
    });
  }, []);

  function showView(nextView: WidgetStableView) {
    flushSync(() => {
      setView(nextView);
      setWindowView(nextView);
    });
    syncNativeWindowFrame(nextView);
  }

  function expandCollapsedView() {
    showView("expanded");
  }

  function collapseExpandedView() {
    showView("collapsed");
  }

  function handleExit() {
    exitApp().catch((err) => {
      console.error("退出应用失败", err);
    });
  }

  const refreshRepos = useCallback((opts: { initial: boolean }) => {
    if (opts.initial) {
      setInitialLoading(true);
    }
    setRefreshing(true);
    setRefreshError(null);
    listRepoStatuses()
      .then((data) => {
        setRepos(data);
        if (data.length > 0) {
          setEmptySettingsDismissed(false);
        }
        setLastRefreshAt(new Date());
        setInitialError(null);
      })
      .catch((err) => {
        const message = String(err);
        if (opts.initial && !hasLoadedOnce.current) {
          setInitialError(message);
        } else {
          setRefreshError(message);
        }
      })
      .finally(() => {
        setInitialLoading(false);
        setRefreshing(false);
        hasLoadedOnce.current = true;
      });
  }, []);

  useEffect(() => {
    refreshRepos({ initial: true });
  }, [refreshRepos]);

  useEffect(() => {
    if (!hasTauriRuntime()) return;
    const targetView = shouldShowSettingsView(windowView, repos.length, initialError, emptySettingsDismissed) ? "settings" : windowView;
    syncNativeWindowFrame(targetView);
  }, [windowView, repos.length, initialError, emptySettingsDismissed, syncNativeWindowFrame]);

  useEffect(() => {
    reloadSettings();
  }, []);

  useEffect(() => () => {
    if (resizeGuardTimer.current === null) return;
    window.clearTimeout(resizeGuardTimer.current);
  }, []);

  useEffect(() => subscribeToSettingsUpdates(applySettings), []);

  useEffect(() => {
    const transparent = (view === "collapsed" || isWidgetTransitionView(view))
      && !shouldShowSettingsView(windowView, repos.length, initialError, emptySettingsDismissed);
    document.body.classList.toggle("gv-collapsed-view", transparent);
    return () => document.body.classList.remove("gv-collapsed-view");
  }, [view, windowView, repos.length, initialError, emptySettingsDismissed]);

  useEffect(() => {
    if (!refreshSettings?.lightweightRefreshEnabled) return;
    const intervalMinutes = Math.min(Math.max(refreshSettings.intervalMinutes, 1), 60);
    const id = window.setInterval(() => refreshRepos({ initial: false }), intervalMinutes * 60_000);
    return () => window.clearInterval(id);
  }, [refreshSettings, refreshRepos]);

  if (initialLoading) return <main className="app-shell">正在刷新仓库状态...</main>;
  const transitionMode = getTransitionMode(view);
  const stableView = isWidgetTransitionView(view) ? windowView : view;
  const shouldRenderSettings = !transitionMode && shouldShowSettingsView(stableView, repos.length, initialError, emptySettingsDismissed);

  if (shouldRenderSettings) {
    return (
      <main className="app-shell settings-shell">
        <SettingsShell
          onClose={() => {
            setEmptySettingsDismissed(true);
            refreshRepos({ initial: false });
            showView("collapsed");
            reloadSettings();
          }}
        />
      </main>
    );
  }
  if (initialError && repos.length === 0) {
    return (
      <main className="app-shell error-shell" role="alert">
        <p>加载仓库失败：{initialError}</p>
        <button onClick={() => refreshRepos({ initial: true })}>重试</button>
        <button
          onClick={() => {
            setInitialError(null);
            showView("settings");
          }}
        >
          打开设置
        </button>
      </main>
    );
  }

  return view === "expanded" ? (
    <WidgetExpanded
      repos={repos}
      lastRefreshAt={lastRefreshAt}
      refreshing={refreshing}
      refreshError={refreshError}
      onRefresh={() => refreshRepos({ initial: false })}
      onCollapse={collapseExpandedView}
      onOpenSettings={() => showView("settings")}
      allowDrag={allowWidgetDrag}
      onStartDrag={() => {
        if (!hasTauriRuntime()) return;
        getCurrentWindow().startDragging().catch((err) => {
          console.error("启动展开浮窗拖动失败", err);
        });
      }}
    />
  ) : (
    <main className="app-shell collapsed-shell">
      <WidgetCollapsed
        repos={repos}
        allowDrag={allowWidgetDrag}
        onExpand={expandCollapsedView}
        onStartDrag={() => {
          if (!hasTauriRuntime()) return;
          getCurrentWindow().startDragging().catch((err) => {
            console.error("启动收起浮窗拖动失败", err);
          });
        }}
        onRefresh={() => refreshRepos({ initial: false })}
        onExit={handleExit}
      />
    </main>
  );
}
