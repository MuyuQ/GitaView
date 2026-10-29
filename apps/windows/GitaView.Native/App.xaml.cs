// GitaView 原生主程序（N14：生命周期与托盘）。
//
// 启动顺序（实施说明 §10 / §12）：单实例注册（引擎与窗口创建之前）→ 路径解析 →
// ABI 检查 → 引擎创建 → 快照订阅 → 托盘 → UI 更新。
// - 第二实例把激活重定向给主实例（协议冷 / 热启动都先过单实例关）。
// - 关闭窗口 = 驻留托盘；托盘菜单"退出"才停止引擎并结束进程。
// - 原生恢复通知（WM_POWERBROADCAST）：定时开启则合并请求一次刷新；
//   手动模式不自发扫描（§8.1；宿主按快照策略判断，不需专门 ABI）。
// - 协议 gitaview-native:// 仅导航：未知路由与已删仓库一律回总列表，不执行 Git 操作。

using System.Runtime.InteropServices;
using GitaView.Native.Contracts;
using GitaView.Native.Interop;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.Windows.AppLifecycle;
using Windows.ApplicationModel.Activation;
using Windows.Storage;

namespace GitaView_Native;

public partial class App : Application
{
    private MainWindow? _window;
    private TrayWindow? _tray;
    private bool _exitRequested;
    private AppInstance? _primaryInstance;
    private DispatcherQueue? _uiQueue;
    private readonly ProtocolNavigation _protocolNavigation = new();

    /// <summary>供 FolderPicker 等需要窗口句柄的 API 使用。</summary>
    public static MainWindow? MainWindow { get; private set; }

    public static GitaViewEngine? Engine { get; private set; }

    /// <summary>宿主日志（独立于核心日志；logDirectory 下的 gitaview-host.log）。</summary>
    public static DiagFile? HostLog { get; private set; }

    /// <summary>本次启动的引擎警告（如 SETTINGS_RECOVERED），主页 InfoBar 显示。</summary>
    public static IReadOnlyList<string> StartupWarnings { get; private set; } = [];

    /// <summary>新快照到达（后台线程触发；订阅方自行调度到 UI 线程）。</summary>
    public static event Action<AppSnapshot>? SnapshotArrived;

    public App()
    {
        InitializeComponent();
        UnhandledException += (_, args) =>
            LogStartupFailure("UnhandledException: " + args.Exception);
    }

    protected override void OnLaunched(Microsoft.UI.Xaml.LaunchActivatedEventArgs args)
    {
        // —— 单实例重定向已由 Program.Main 在 XAML 初始化之前完成 ——
        // 本方法只在主实例运行；注册重定向接收（协议冷启动也经此到达）。
        _primaryInstance = AppInstance.FindOrRegisterForKey("GitaView.Native.primary");
        _uiQueue = DispatcherQueue.GetForCurrentThread();
        _primaryInstance.Activated += (_, redirectArgs) =>
            _uiQueue.TryEnqueue(() =>
            {
                try
                {
                    HandleRedirectedActivation(redirectArgs);
                }
                catch (Exception exception)
                {
                    // 分发器回调异常会触发 XAML fail fast（0xc000027b）：
                    // 记录后继续，不让重定向路径击穿进程。
                    HostLog?.Log("ERROR", "redirect.handle.failed",
                        $"{exception.GetType().Name}: {exception.Message}");
                }
            });

        // 冷启动协议参数：Program.Main 判定主实例后，本实例自己的协议激活
        // 也要处理（深链导航），否则冷启动深链参数被丢弃。
        var ownArgs = AppInstance.GetCurrent().GetActivatedEventArgs();
        if (ownArgs.Kind == ExtendedActivationKind.Protocol
            && ownArgs.Data is IProtocolActivatedEventArgs ownProtocol)
        {
            HandleProtocol(ownProtocol);
        }

        // —— 引擎 ——
        HostLog = new DiagFile(
            System.IO.Path.Combine(ApplicationData.Current.LocalFolder.Path, "logs"),
            "gitaview-host.log");
        HostLog.Log("INFO", "app.launch", $"primary={_primaryInstance.IsCurrent}");

        GitaViewEngine engine;
        try
        {
            engine = StartEngine();
        }
        catch (Exception exception)
        {
            // 启动失败：可恢复错误展示 + 退出；不进入"空仓库且启动成功"的主界面，
            // 不发布空 Widget 覆盖历史（§10）。
            HostLog?.Log("ERROR", "engine.create.failed", exception.Message);
            LogStartupFailure("engine create failed: " + exception.Message);
            _ = MessageBoxW(0,
                "GitaView 启动失败：\n" + exception.Message +
                "\n\n请确认 gitaview_ffi.dll 与数据目录可用后重试。",
                "GitaView.Native", 0x10 /* MB_ICONERROR */);
            Environment.Exit(1);
            return;
        }
        Engine = engine;
        StartupWarnings = engine.CreateData.StartupWarnings;
        HostLog.Log("INFO", "engine.created",
            $"session={engine.SessionId} warnings={string.Join(",", engine.CreateData.StartupWarnings)}");

        // —— 窗口与托盘 ——
        _window = new MainWindow();
        MainWindow = _window;
        _window.Activate();

        if (!TrayWindow.TryCreate(
                "GitaView 仓库状态",
                onOpen: ShowMainWindow,
                onToggleFlyout: ToggleTrayFlyout,
                onRefresh: () => _ = RefreshFromTrayAsync(),
                onExit: RequestExit,
                onSystemResume: OnSystemResume,
                out _tray))
        {
            // 托盘不可用时保留主窗口，关闭按钮改走完整退出流程。
            LogStartupFailure("tray icon registration failed");
        }

        // —— 快照消费循环 ——
        // 宿主持有任务引用：退出时观察其结束，失败记录诊断（不再静默丢弃）。
        // Task.Run 包裹（review S04）：整个循环（含 await 之后的续体）在后台
        // 线程执行，WidgetSnapshot FFI 与写入调度绝不回到 UI 线程。
        // 写入器存储位置由宿主注入（Contracts 无 Windows 投影）。
        WidgetDataWriter.DataDirectory = ApplicationData.Current.LocalFolder.Path;
        WidgetDataWriter.Log = HostLog;
        WidgetDataWriter.Start();
        SnapshotLoopTask = Task.Run(() => RunSnapshotLoopAsync(engine));
        _protocolNavigation.SetReady(
            id => engine.SettingsGet().Repos.Any(repo => repo.Id == id),
            route => _ = NavigateFromProtocolAsync(route));
    }

    /// <summary>快照消费循环任务；退出流程等待其结束后才销毁引擎。</summary>
    public static Task SnapshotLoopTask { get; private set; } = Task.CompletedTask;

    /// <summary>
    /// 单一后台快照消费循环：以 sessionId + stateRevision 等待状态变化；
    /// 窗口关闭（驻留）期间循环继续，Widget 数据照常发布。
    /// Widget 写入与订阅解耦（review R05）：写入失败只记录并退避重试，
    /// 绝不终止核心消费循环；循环自身异常记录后退出（可见于日志）。
    /// </summary>
    private static async Task RunSnapshotLoopAsync(GitaViewEngine engine)
    {
        ulong cursor = 0;
        try
        {
            while (true)
            {
                var result = await Task.Run(() => engine.SnapshotWait(cursor, 15_000));
                switch (result.Kind)
                {
                    case SnapshotWaitKind.Ready:
                        var snapshot = result.Snapshot!;
                        cursor = snapshot.StateRevision;
                        try
                        {
                            // Widget 投影由 Rust 统一产生（不重复实现业务规则）。
                            WidgetDataWriter.Write(engine.WidgetSnapshot());
                        }
                        catch (ObjectDisposedException)
                        {
                            // 引擎已在关闭：循环将随下一次 SnapshotWait 退出。
                        }
                        catch (Exception exception)
                        {
                            HostLog?.Log("ERROR", "widget.write.failed",
                                $"{exception.GetType().Name}: {exception.Message}");
                        }
                        try
                        {
                            SnapshotArrived?.Invoke(snapshot);
                        }
                        catch (Exception exception)
                        {
                            HostLog?.Log("ERROR", "snapshot.handler.failed",
                                $"{exception.GetType().Name}: {exception.Message}");
                        }
                        break;
                    case SnapshotWaitKind.Timeout:
                        // 无新快照：N16 实现过期边界重写。
                        try
                        {
                            WidgetDataWriter.RewriteIfStale();
                        }
                        catch (Exception exception)
                        {
                            HostLog?.Log("ERROR", "widget.rewrite.failed",
                                $"{exception.GetType().Name}: {exception.Message}");
                        }
                        break;
                    case SnapshotWaitKind.Stopping:
                        HostLog?.Log("INFO", "snapshot.loop.exit", "stopping");
                        return;
                }
            }
        }
        catch (ObjectDisposedException)
        {
            HostLog?.Log("INFO", "snapshot.loop.exit", "disposed");
        }
        catch (Exception exception)
        {
            HostLog?.Log("ERROR", "snapshot.loop.failed",
                $"{exception.GetType().Name}: {exception.Message}");
        }
    }

    private static GitaViewEngine StartEngine()
    {
        // 固定 DLL 加载：按安装位置绝对路径预加载，不依赖工作目录或 PATH（§12）。
        GitaView.Native.Interop.NativeMethods.PreloadNativeLibrary();
        string localFolder = ApplicationData.Current.LocalFolder.Path;
        var input = new EngineCreateInput
        {
            DataDirectory = System.IO.Path.Combine(localFolder, "data"),
            LogDirectory = System.IO.Path.Combine(localFolder, "logs"),
        };
        if (GitaViewEngine.AbiVersion() != 1)
        {
            throw new InvalidOperationException("gitaview_ffi.dll ABI 版本不匹配");
        }
        return GitaViewEngine.CreateEngine(input);
    }

    // —— 激活 ——

    private void HandleRedirectedActivation(AppActivationArguments args)
    {
        if (_exitRequested) return;
        if (args.Kind == ExtendedActivationKind.Protocol
            && args.Data is IProtocolActivatedEventArgs protocol)
        {
            HandleProtocol(protocol);
        }
        ShowMainWindow();
    }

    private void HandleProtocol(IProtocolActivatedEventArgs protocol)
    {
        if (!_exitRequested) _protocolNavigation.Request(protocol.Uri.OriginalString);
    }

    private async Task NavigateFromProtocolAsync(ProtocolRoute route)
    {
        if (_window is null || _exitRequested) return;
        try
        {
            await _window.NavigateFromProtocolAsync(route);
            HostLog?.Log("INFO", "protocol.navigation", route.RepoId is null ? "all" : "repository");
        }
        catch (Exception exception)
        {
            HostLog?.Log("ERROR", "protocol.navigation.failed", exception.Message);
        }
    }

    private void ShowMainWindow()
    {
        _flyout?.Dismiss();
        if (_window is null)
        {
            return;
        }
        _window.AppWindow.Show();
        _window.Activate();
    }

    private TrayFlyoutWindow? _flyout;

    /// <summary>托盘左键：动效浮窗开关（Magpie 式；收缩窗口模式已按用户决策移除）。
    /// 可见状态由浮窗自己维护（失焦自关后 App 侧标记不会失效）。</summary>
    private void ToggleTrayFlyout()
    {
        if (_flyout is { } flyout && flyout.IsFlyoutVisible)
        {
            flyout.Dismiss();
            return;
        }
        _flyout ??= new TrayFlyoutWindow();
        var trayRect = default(Windows.Graphics.RectInt32);
        if (_tray is { } tray)
        {
            tray.TryGetIconRect(out trayRect);
        }
        _flyout.ShowNearTray(trayRect);
    }

    /// <summary>浮窗"退出"入口（与托盘菜单同一退出流）。</summary>
    internal void TriggerExit() => RequestExit();

#if DEBUG
    internal void PreviewTrayFlyout()
    {
        _flyout ??= new TrayFlyoutWindow();
        // The independent debug preview must be discoverable by native UI tools.
        _flyout.AppWindow.IsShownInSwitchers = true;
        ToggleTrayFlyout();
    }
#endif

    private async Task RefreshFromTrayAsync()
    {
        if (Engine is null)
        {
            return;
        }
        var request = await Task.Run(() => Engine.RefreshRequest());
        await Task.Run(async () =>
        {
            while (!Engine.RefreshCompleted(request.RequestId))
            {
                await Task.Delay(50);
            }
        });
        ShowMainWindow();
    }

    // —— 唤醒 / 退出 ——

    private void OnSystemResume()
    {
        // §8.1：定时开启则合并请求一次刷新；手动模式不自发扫描。
        // 宿主按当前快照策略判断（与引擎 notify_system_resume 同语义，无需专门 ABI）。
        var engine = Engine;
        if (engine is null)
        {
            return;
        }
        var snapshot = engine.SnapshotGet();
        if (snapshot.RefreshPolicy.Enabled)
        {
            _ = Task.Run(() =>
            {
                try
                {
                    engine.RefreshRequest();
                }
                catch (GvException)
                {
                    // 停止中：忽略。
                }
            });
        }
    }

    /// <summary>
    /// 托盘"退出"（review R02）：RequestStop → 后台分段等待引擎真正 Stopped →
    /// 等快照循环结束 → Dispose → 清托盘 → Exit。网络操作（Pull/Push）合法耗时
    /// 可达 600s，超时不能当成功：日志记录实际阶段，等待总期限 660s（核心
    /// network_timeout 600s + 余量）后仍退出，但以 ERROR 标明未干净停止。
    /// </summary>
    private async void RequestExit()
    {
        if (_exitRequested)
        {
            return;
        }
        _exitRequested = true;
        HostLog?.Log("INFO", "app.exit.begin", "");
        var engine = Engine;
        var stoppedCleanly = engine is null;
        if (engine is not null)
        {
            try
            {
                engine.RequestStop();
                HostLog?.Log("INFO", "app.exit.stop_requested", "");
            }
            catch (ObjectDisposedException)
            {
                stoppedCleanly = true;
            }
            catch (GvException exception)
            {
                HostLog?.Log("ERROR", "app.exit.stop_failed", exception.Message);
            }
            // 分段等待真停止（每段 2s，UI 可响应；SnapshotWait 在停止后立即返回）。
            stoppedCleanly = await Task.Run(() =>
            {
                for (var waitedMs = 0; waitedMs < 660_000; waitedMs += 2_000)
                {
                    try
                    {
                        if (engine.WaitStopped(2_000))
                        {
                            return true;
                        }
                    }
                    catch (ObjectDisposedException)
                    {
                        return true;
                    }
                    catch (GvException exception)
                    {
                        HostLog?.Log("ERROR", "app.exit.wait_failed", exception.Message);
                        return false;
                    }
                }
                return false;
            });
            if (!stoppedCleanly)
            {
                // 引擎长期未停（异常情况）：记录后仍继续退出，不再伪装成功。
                HostLog?.Log("ERROR", "app.exit.wait_timeout",
                    "engine not stopped within 660s; exiting anyway");
            }
            try
            {
                // 快照循环结束后不再有新投影；停写入器（flush 最新投影）再销毁引擎。
                await SnapshotLoopTask;
            }
            catch (Exception exception)
            {
                HostLog?.Log("WARN", "app.exit.snapshot_task",
                    $"{exception.GetType().Name}: {exception.Message}");
            }
            try
            {
                await Task.Run(() => WidgetDataWriter.StopAsync());
                HostLog?.Log("INFO", "app.exit.writer_flushed", "");
            }
            catch (Exception exception)
            {
                HostLog?.Log("WARN", "app.exit.writer_stop",
                    $"{exception.GetType().Name}: {exception.Message}");
            }
            try
            {
                // Dispose 的租约排空可能等待在途调用（review S04）：留在后台，
                // 不阻塞 UI 线程。
                await Task.Run(engine.Dispose);
            }
            catch (Exception exception)
            {
                // Dispose 失败保留 owner 可重试；退出路径尽力而为并如实记录。
                HostLog?.Log("ERROR", "app.exit.dispose",
                    $"{exception.GetType().Name}: {exception.Message}");
            }
        }
        _tray?.Dispose();
        HostLog?.Log("INFO", "app.exit.done",
            stoppedCleanly ? "engine stopped and disposed" : "exited without clean engine stop");
        Environment.Exit(0);
    }

    /// <summary>托盘可用时关闭驻留；否则请求完整退出，等待期间保留主窗口。</summary>
    internal void HandleMainWindowClose() => MainWindowClosePolicy.Handle(
        _exitRequested, _tray?.IsAvailable == true,
        () => _window?.AppWindow.Hide(), RequestExit);

    private static void LogStartupFailure(string message)
    {
        // 启动期尚无 UI / 日志模块：写包临时目录供排障（N17 引入正式诊断模块后替换）。
        try
        {
            string dir = ApplicationData.Current.LocalFolder.Path;
            File.AppendAllText(Path.Combine(dir, "startup-errors.log"),
                $"[{DateTime.Now:HH:mm:ss.fff}] {message}" + Environment.NewLine);
        }
        catch
        {
        }
    }

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    private static extern int MessageBoxW(nint hwnd, string text, string caption, uint type);
}
