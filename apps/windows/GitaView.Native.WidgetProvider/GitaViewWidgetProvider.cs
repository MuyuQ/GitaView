// GitaView Widget Provider 完整生命周期实现（N16）。
//
// 规则（实施说明 §12）：
// - GetWidgetInfos 恢复已固定实例（Provider 重启 / 系统重启后无需新 Create）；
// - 回调内只复制普通值（ID / 定义 / 尺寸字符串），异步处理不持有 WidgetContext；
// - 更新串行（lock），Delete 从映射移除使后续更新跳过；
// - 活跃期间：FileSystemWatcher 监听 widget-data.json（原子替换 / 通知丢失 →
//   Activate 无条件重读 + 过期边界计时兜底）；全部失活后停止监听与计时；
// - 过期边界：last + 2*interval + 1（等于阈值仍有效）；时钟回拨不调度，有界重查；
// - 数据只来自安全投影（无 path / 凭据 / 原始输出）；损坏 / 未知 schema → 占位卡片；
// - 深链 verb=open → gitaview-native://open（仅导航）。

using GitaView.Native.Contracts;
using Microsoft.Windows.Widgets;
using Microsoft.Windows.Widgets.Providers;
using Windows.System;

namespace GitaView_Native.WidgetProvider;

public sealed class GitaViewWidgetProvider : IWidgetProvider
{
    /// <summary>已固定组件状态。回调内复制普通值后存入。</summary>
    private sealed class WidgetState
    {
        public string Id = "";
        public string DefinitionId = "";
        public WidgetSize Size = WidgetSize.Medium;
        public bool IsActive;
    }

    internal static readonly ManualResetEvent EmptyWidgetListEvent = new(false);
    private static readonly object Gate = new();
    private static readonly Dictionary<string, WidgetState> Widgets = new();

    // —— 空闲退出（review R12）——
    // 构造时"看到空集合"不等于"以后不会有 Create"：宿主可能正处于
    // 创建 Provider → 随后 CreateWidget 的首次添加路径。退出必须走
    // 有界空闲确认：延迟到期时复实例表，仍为空才置退出事件；
    // Create / Activate 到达即取消待退出并 Reset 事件。
    private static Timer? _idleExitTimer;
    private static readonly TimeSpan IdleExitGrace = TimeSpan.FromSeconds(60);
    private static readonly TimeSpan IdleExitAfterDelete = TimeSpan.FromSeconds(10);

    private static FileSystemWatcher? _dataWatcher;
    private static Timer? _stalenessTimer;

    private static readonly DiagFile Log = new(
        Path.Combine(Windows.Storage.ApplicationData.Current.LocalFolder.Path, "logs"),
        "widgetprovider.log");

    public GitaViewWidgetProvider()
    {
        // Provider 重启 / 系统重启恢复：不依赖新的 Create 回调。
        var infos = WidgetManager.GetDefault().GetWidgetInfos();
        Log.Log("INFO", "provider.start", $"restored={(infos?.Length ?? 0)} widgets");
        lock (Gate)
        {
            if (infos is not null)
            {
                foreach (var info in infos)
                {
                    var context = info.WidgetContext;
                    Widgets[context.Id] = new WidgetState
                    {
                        Id = context.Id,
                        DefinitionId = context.DefinitionId,
                        Size = context.Size,
                        IsActive = false,
                    };
                }
            }
            if (Widgets.Count > 0)
            {
                // 已固定实例存在但宿主尚未 Activate：仅恢复映射，不启动监听
                //（Activate 到达时启动）。避免为失活组件空转。
            }
            else
            {
                // 无已固定组件：有界空闲后退出，给首次添加路径留窗口。
                StartIdleExitLocked(IdleExitGrace);
            }
        }
        if (Widgets.Count > 0)
        {
            RefreshAllTracked(update: false);
        }
    }

    /// <summary>调用方必须持有 Gate。到期复实例表：仍为空才置退出事件。</summary>
    private static void StartIdleExitLocked(TimeSpan delay)
    {
        _idleExitTimer?.Dispose();
        _idleExitTimer = new Timer(_ =>
        {
            lock (Gate)
            {
                if (Widgets.Count == 0)
                {
                    Log.Log("INFO", "provider.idle_exit", "no widgets after grace period");
                    EmptyWidgetListEvent.Set();
                }
            }
        }, null, delay, Timeout.InfiniteTimeSpan);
    }

    /// <summary>调用方必须持有 Gate：取消待退出并清除已置位的事件。</summary>
    private static void CancelIdleExitLocked()
    {
        _idleExitTimer?.Dispose();
        _idleExitTimer = null;
        EmptyWidgetListEvent.Reset();
    }

    /// <summary>
    /// 主循环收到退出事件后的最终确认：实例表非空 → 返回 false（撤销退出，
    /// 调用方应 Reset 事件继续等待）；为空 → 真正空闲，允许撤销 COM 服务。
    /// </summary>
    public static bool ConfirmIdleExit()
    {
        lock (Gate)
        {
            return Widgets.Count == 0;
        }
    }

    // —— IWidgetProvider ——

    public void CreateWidget(WidgetContext widgetContext)
    {
        Log.Log("INFO", "widget.create", $"id={widgetContext.Id} size={widgetContext.Size}");
        // 回调内复制普通值；不保存 WidgetContext 引用（回调外行为未定义）。
        // 先取消待退出（review R12）：进程要接单，不能被空闲退出带走。
        lock (Gate)
        {
            CancelIdleExitLocked();
            Widgets[widgetContext.Id] = new WidgetState
            {
                Id = widgetContext.Id,
                DefinitionId = widgetContext.DefinitionId,
                Size = widgetContext.Size,
                IsActive = true,
            };
        }
        EnsureDataWatcher();
        PushUpdate(widgetContext.Id, widgetContext.Size);
    }

    public void DeleteWidget(string widgetId, string customState)
    {
        Log.Log("INFO", "widget.delete", $"id={widgetId}");
        lock (Gate)
        {
            Widgets.Remove(widgetId);
            if (Widgets.Count == 0)
            {
                StopDataWatcher();
                StopStalenessTimer();
                // 最后一个组件删除后确认稳定空闲再退出；期间新 Create 可取消。
                StartIdleExitLocked(IdleExitAfterDelete);
            }
        }
    }

    public void OnActionInvoked(WidgetActionInvokedArgs args)
    {
        var verb = args.Verb; // 回调内复制
        if (verb == "open")
        {
            _ = Launcher.LaunchUriAsync(new Uri("gitaview-native://open"));
        }
    }

    public void OnWidgetContextChanged(WidgetContextChangedArgs args)
    {
        var context = args.WidgetContext;
        var id = context.Id;
        var size = context.Size; // 尺寸枚举值复制
        lock (Gate)
        {
            if (Widgets.TryGetValue(id, out var state))
            {
                state.Size = size;
            }
            else
            {
                return;
            }
        }
        PushUpdate(id, size);
    }

    public void Activate(WidgetContext widgetContext)
    {
        Log.Log("INFO", "widget.activate", $"id={widgetContext.Id}");
        var id = widgetContext.Id;
        // 恢复路径（GetWidgetInfos → Activate）同样取消待退出（review R12）。
        lock (Gate)
        {
            CancelIdleExitLocked();
            if (Widgets.TryGetValue(id, out var state))
            {
                state.IsActive = true;
            }
            else
            {
                Widgets[id] = new WidgetState
                {
                    Id = id,
                    DefinitionId = widgetContext.DefinitionId,
                    Size = widgetContext.Size,
                    IsActive = true,
                };
            }
        }
        // 重新激活必须无条件重读（通知可能丢失）。
        EnsureDataWatcher();
        EnsureStalenessTimer();
        PushUpdate(id, widgetContext.Size);
    }

    public void Deactivate(string widgetId)
    {
        Log.Log("INFO", "widget.deactivate", $"id={widgetId}");
        lock (Gate)
        {
            if (Widgets.TryGetValue(widgetId, out var state))
            {
                state.IsActive = false;
            }
            if (Widgets.Values.All(state => !state.IsActive))
            {
                StopDataWatcher();
                StopStalenessTimer();
            }
        }
    }

    // —— 更新（串行）——

    private static void PushUpdate(string widgetId, WidgetSize size)
    {
        var state = new { Id = widgetId, Size = size }; // 普通值快照
        Task.Run(() =>
        {
            lock (Gate)
            {
                if (!Widgets.ContainsKey(state.Id))
                {
                    return; // Delete 已清理：取消该实例的待更新
                }
            }
            SendUpdate(state.Id, state.Size);
            RescheduleStalenessTimer();
        });
    }

    private static void RefreshAllTracked(bool update)
    {
        KeyValuePair<string, WidgetState>[] snapshot;
        lock (Gate)
        {
            snapshot = Widgets.ToArray();
        }
        foreach (var pair in snapshot)
        {
            if (update)
            {
                PushUpdate(pair.Key, pair.Value.Size);
            }
        }
    }

    private static void SendUpdate(string widgetId, WidgetSize size)
    {
        try
        {
            var options = new WidgetUpdateRequestOptions(widgetId)
            {
                Template = WidgetCardContent.TemplateFor(size),
                Data = WidgetCardContent.DataJson(ReadLatestSnapshot()),
            };
            WidgetManager.GetDefault().UpdateWidget(options);
        }
        catch (Exception exception)
        {
            // 宿主回收 / 组件已删除等：下一次 Activate 或数据写入会重试。
            Log.Log("ERROR", "widget.update.failed",
                $"id={widgetId} {exception.GetType().Name}: {exception.Message}");
        }
    }

    // —— 数据读取 ——

    private static string DataPath()
    {
        // 同包身份 → 与主应用共享 LocalState。
        return Path.Combine(
            Windows.Storage.ApplicationData.Current.LocalFolder.Path, "widget-data.json");
    }

    private static WidgetSnapshot? ReadLatestSnapshot()
    {
        try
        {
            string path = DataPath();
            if (!File.Exists(path))
            {
                return null;
            }
            return ContractsJson.Decode<WidgetSnapshot>(File.ReadAllBytes(path));
        }
        catch (System.Text.Json.JsonException)
        {
            return null; // 损坏 / 未知 schema：占位卡片提示重新打开应用
        }
        catch (IOException)
        {
            return null; // 与原子替换竞争：下一次 Activate / Watcher 重读
        }
    }

    // —— 目录监听（原子替换 = rename，需同时监听 Changed/Renamed/Created）——

    private static void EnsureDataWatcher()
    {
        lock (Gate)
        {
            if (_dataWatcher is not null)
            {
                return;
            }
            var directory = Path.GetDirectoryName(DataPath());
            if (directory is null || !Directory.Exists(directory))
            {
                return;
            }
            var watcher = new FileSystemWatcher(directory, "widget-data.json")
            {
                NotifyFilter = NotifyFilters.FileName | NotifyFilters.LastWrite,
                EnableRaisingEvents = true,
            };
            watcher.Created += OnDataFileEvent;
            watcher.Changed += OnDataFileEvent;
            watcher.Renamed += OnDataFileEvent;
            _dataWatcher = watcher;
        }
    }

    private static void StopDataWatcher()
    {
        lock (Gate)
        {
            _dataWatcher?.Dispose();
            _dataWatcher = null;
        }
    }

    private static void OnDataFileEvent(object sender, FileSystemEventArgs e)
    {
        // 原子替换可能产生多个事件：防抖后统一刷新全部活跃组件。
        Task.Run(() =>
        {
            Thread.Sleep(200); // 防抖
            RefreshAllActive();
            RescheduleStalenessTimer();
        });
    }

    private static void RefreshAllActive()
    {
        lock (Gate)
        {
            foreach (var pair in Widgets.Where(pair => pair.Value.IsActive))
            {
                SendUpdate(pair.Key, pair.Value.Size);
            }
        }
    }

    // —— 过期边界计时（活跃期间；无新文件也跨越边界提交过期卡片）——

    private static void EnsureStalenessTimer() => RescheduleStalenessTimer();

    private static void StopStalenessTimer()
    {
        lock (Gate)
        {
            _stalenessTimer?.Dispose();
            _stalenessTimer = null;
        }
    }

    private static void RescheduleStalenessTimer()
    {
        lock (Gate)
        {
            if (Widgets.Values.All(state => !state.IsActive))
            {
                StopStalenessTimer();
                return;
            }
            var snapshot = ReadLatestSnapshot();
            if (snapshot is null)
            {
                return; // 无数据卡片：不需要过期边界
            }
            var staleness = snapshot.ComputeStaleness(UxNowMs());
            if (staleness == WidgetStaleness.FutureTime)
            {
                // 时钟回拨：不调度边界；有界重查（60s），不忙循环。
                ScheduleStalenessTimer(TimeSpan.FromSeconds(60));
                return;
            }
            if (staleness == WidgetStaleness.Stale)
            {
                RefreshAllActive(); // 已过期：立即提交过期卡片
                return;
            }
            var boundary = snapshot.StalenessBoundaryMs(UxNowMs());
            if (boundary is { } dueMs)
            {
                var delayMs = Math.Clamp(dueMs - UxNowMs(), 1, int.MaxValue);
                ScheduleStalenessTimer(TimeSpan.FromMilliseconds(delayMs));
            }
        }
    }

    private static void ScheduleStalenessTimer(TimeSpan delay)
    {
        _stalenessTimer?.Dispose();
        _stalenessTimer = new Timer(
            _ => Task.Run(() =>
            {
                RefreshAllActive(); // 到点：重算并提交（过期 → 过期卡片；仍新鲜 → 重新调度）
                RescheduleStalenessTimer();
            }),
            null, delay, Timeout.InfiniteTimeSpan);
    }

    private static ulong UxNowMs() =>
        unchecked((ulong)DateTimeOffset.UtcNow.ToUnixTimeMilliseconds());
}
