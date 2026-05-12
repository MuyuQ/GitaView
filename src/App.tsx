import { useCallback, useEffect, useRef, useState } from "react";
import { currentMonitor, getCurrentWindow, LogicalSize, PhysicalPosition } from "@tauri-apps/api/window";
import { listRepoStatuses, getSettings, exitApp } from "./lib/commands";
import { subscribeToSettingsUpdates } from "./lib/settingsEvents";
import { hasTauriRuntime } from "./lib/runtime";
import { getCompactBodyClassName, shouldShowSettingsView } from "./lib/statusModel";
import { resolveAnchoredWindowPosition } from "./lib/windowMotion";
import type { WindowSizeValue } from "./lib/windowMotion";
import {
  getTransitionMode,
  getTransitionWindowTarget,
  isWidgetTransitionView,
  transitionDurations,
} from "./lib/widgetTransition";
import type { WidgetRenderView, WidgetStableView } from "./lib/widgetTransition";
import type { AppSettings, RepoStatus } from "./types";
import { TransitionSurface } from "./components/TransitionSurface";
import { WidgetCollapsed } from "./components/WidgetCollapsed";
import { WidgetExpanded } from "./components/WidgetExpanded";
import { SettingsShell } from "./components/settings/SettingsShell";

const windowSizes: Record<WidgetStableView, WindowSizeValue> = {
  collapsed: { width: 218, height: 40 },
  expanded: { width: 720, height: 560 },
  settings: { width: 760, height: 540 },
} as const;

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
  const [compactMode, setCompactMode] = useState(false);
  const [allowWidgetDrag, setAllowWidgetDrag] = useState(true);
  const [emptySettingsDismissed, setEmptySettingsDismissed] = useState(false);
  const hasLoadedOnce = useRef(false);
  const transitionTimer = useRef<number | null>(null);

  function applySettings(settings: AppSettings) {
    setRefreshSettings(settings.refresh);
    setCompactMode(settings.appearance.compactMode);
    setAllowWidgetDrag(settings.appearance.allowWidgetDrag);
  }

  function reloadSettings() {
    getSettings()
      .then(applySettings)
      .catch(() => {});
  }

  const syncNativeWindowFrame = useCallback((nextView: WidgetStableView) => {
    if (!hasTauriRuntime()) return;
    const size = windowSizes[nextView];
    const targetLogicalSize = new LogicalSize(size.width, size.height);
    const appWindow = getCurrentWindow();

    Promise.all([
      appWindow.outerPosition(),
      appWindow.outerSize(),
      appWindow.scaleFactor(),
      currentMonitor(),
    ]).then(([position, currentSize, scaleFactor, monitor]) => {
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
    }).catch((err) => {
      console.error("同步窗口位置失败", err);
      appWindow.setSize(targetLogicalSize).catch((sizeErr) => {
        console.error("调整窗口尺寸失败", sizeErr);
      });
    });
  }, []);

  function clearTransitionTimer() {
    if (transitionTimer.current === null) return;
    window.clearTimeout(transitionTimer.current);
    transitionTimer.current = null;
  }

  function showView(nextView: WidgetStableView) {
    clearTransitionTimer();
    syncNativeWindowFrame(nextView);
    setView(nextView);
    setWindowView(nextView);
  }

  function expandCollapsedView() {
    if (!hasTauriRuntime()) {
      showView("expanded");
      return;
    }
    clearTransitionTimer();
    const targetView = getTransitionWindowTarget("opening");
    syncNativeWindowFrame(targetView);
    setWindowView(targetView);
    setView("opening");
    transitionTimer.current = window.setTimeout(() => {
      transitionTimer.current = null;
      setView("expanded");
    }, transitionDurations.openingMs);
  }

  function collapseExpandedView() {
    if (!hasTauriRuntime()) {
      showView("collapsed");
      return;
    }
    clearTransitionTimer();
    const targetView = getTransitionWindowTarget("closing");
    syncNativeWindowFrame(targetView);
    setWindowView(targetView);
    setView("closing");
    transitionTimer.current = window.setTimeout(() => {
      showView("collapsed");
    }, transitionDurations.closingMs);
  }

  function handleExit() {
    exitApp().catch(() => {});
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

  useEffect(() => () => clearTransitionTimer(), []);

  useEffect(() => {
    reloadSettings();
  }, []);

  useEffect(() => subscribeToSettingsUpdates(applySettings), []);

  useEffect(() => {
    const className = getCompactBodyClassName(compactMode);
    if (!className) {
      document.body.classList.remove("gv-compact");
      return;
    }
    document.body.classList.add(className);
    return () => document.body.classList.remove(className);
  }, [compactMode]);

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

  if (transitionMode) {
    return (
      <main className={`app-shell transition-shell transition-shell--${transitionMode}`} aria-hidden="true">
        <TransitionSurface mode={transitionMode} repos={repos} />
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
