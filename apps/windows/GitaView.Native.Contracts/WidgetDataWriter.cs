// Widget 数据写入器（N03；review S04 后台化；branch review B03 锁外写盘）。
//
// - 唯一后台写入循环：Write() 只合并"最新待发布投影"（内存操作，任意线程
//   立即返回），磁盘 I/O 全部在写入器任务内、且在 Gate 之外执行；多代快照
//   合并为最后一次。Gate 只保护待写状态与任务生命周期，慢盘不阻塞调用方。
// - 可等待的停止 / flush：StopAsync() 让循环写出最新投影后退出；写盘期间
//   到来的新投影保留 dirty，退出前补写。退出流程先停写入器再销毁引擎。
// - 失败隔离：写入失败记录诊断 + 有界退避重试（默认 2s 步进 ×3；新投影到达
//   即开始新一轮），绝不向调用方抛出、不终止消费循环。
// - 首刷前有效缓存保护（实施说明 §12"尚未首刷"规则）：generation 0 且现有
//   文件是同 schema、同 settingsRevision 的有效投影时保留缓存；首轮完成或
//   配置变化后正常替换。
// - 该文件只供 Widget 展示，不回灌核心为成功状态（§8.1）。

using System.Text.Json;

namespace GitaView.Native.Contracts;

public static class WidgetDataWriter
{
    private const string FileName = "widget-data.json";
    private const int MaxRetryAttempts = 3;
    private static readonly object Gate = new();
    private static WidgetSnapshot? _pending;
    private static bool _dirty;
    private static bool _stopping;
    private static Task? _writerTask;

    /// <summary>宿主注入的诊断器（可选；App 启动时设置）。</summary>
    public static DiagFile? Log { get; set; }

    /// <summary>
    /// 宿主注入的数据目录（生产 = 包 LocalState 根；Contracts 不依赖
    /// Windows 投影，与 DiagFile 同模式）。必须在 Start() 前设置。
    /// </summary>
    public static string? DataDirectory { get; set; }

    /// <summary>测试注入：替代真实磁盘写的钩子（参数 = 目标路径与内容）。</summary>
    internal static Action<string, string>? WriteHookOverride;

    /// <summary>测试注入：退避重试的基准延迟（生产 2s）。</summary>
    internal static TimeSpan RetryBaseDelay { get; set; } = TimeSpan.FromSeconds(2);

    private static string DataPath
    {
        get
        {
            if (string.IsNullOrEmpty(DataDirectory))
            {
                throw new InvalidOperationException(
                    "WidgetDataWriter.DataDirectory must be set before use");
            }
            return Path.Combine(DataDirectory, FileName);
        }
    }

    /// <summary>启动唯一写入循环（幂等）。宿主在创建引擎前调用。</summary>
    public static void Start()
    {
        lock (Gate)
        {
            if (_writerTask is not null)
            {
                return;
            }
            _stopping = false;
            // A new lifetime may target another file, or the old file may have
            // been removed. Only deduplicate writes within this running worker.
            _lastWritten = null;
            _writerTask = Task.Run(WriterLoop);
        }
    }

    /// <summary>可等待停止：写出最新待发布投影（flush）后退出循环。</summary>
    public static async Task StopAsync()
    {
        Task? task;
        lock (Gate)
        {
            if (_writerTask is null)
            {
                return;
            }
            _stopping = true;
            Monitor.PulseAll(Gate);
            task = _writerTask;
        }
        await task.ConfigureAwait(false);
        lock (Gate)
        {
            _writerTask = null;
            _stopping = false;
        }
    }

    /// <summary>记录最新待发布投影；立即返回，磁盘 I/O 在写入器任务内执行。</summary>
    public static void Write(WidgetSnapshot snapshot)
    {
        lock (Gate)
        {
            _pending = snapshot;
            _dirty = true;
            Monitor.PulseAll(Gate);
        }
    }

    /// <summary>
    /// 无新快照时跨过过期边界的重写：把最新投影标记为待写（N03 简化；
    /// 完整的过期边界计时与时间回拨处理在 N16 实现）。
    /// </summary>
    public static void RewriteIfStale()
    {
        lock (Gate)
        {
            if (_pending is not null)
            {
                _dirty = true;
                Monitor.PulseAll(Gate);
            }
        }
    }

    // Start 在旧任务退出后重置；运行期间只有唯一 writer 任务访问。
    private static string? _lastWritten;

    private static async Task WriterLoop()
    {
        while (true)
        {
            WidgetSnapshot? toWrite;
            lock (Gate)
            {
                while (!_dirty && !_stopping)
                {
                    _ = Monitor.Wait(Gate, TimeSpan.FromSeconds(1));
                }
                if (_stopping && !_dirty)
                {
                    return; // flush 已完成（无待写投影）
                }
                toWrite = _pending;
                _dirty = false;
            }
            if (toWrite is not null)
            {
                // 磁盘 I/O 在 Gate 外执行（B03）：编码、读缓存、写盘期间到来的
                // 新投影只置 dirty，绝不阻塞 Write / RewriteIfStale / StopAsync
                // 的调用方；下一轮循环会把最新投影补写出去（flush 语义不变）。
                await WriteWithBoundedRetryAsync(toWrite).ConfigureAwait(false);
            }
        }
    }

    private static async Task WriteWithBoundedRetryAsync(WidgetSnapshot snapshot)
    {
        for (var attempt = 0; ; attempt++)
        {
            try
            {
                WriteSnapshot(snapshot);
                return;
            }
            catch (Exception exception)
            {
                Log?.Log("ERROR", "widget.data.write_failed",
                    $"attempt={attempt + 1} {exception.GetType().Name}: {exception.Message}");
                if (attempt >= MaxRetryAttempts)
                {
                    return; // 重试耗尽：保留 _pending，等待下一次投影或过期重写
                }
                await Task.Delay(RetryBaseDelay * (attempt + 1)).ConfigureAwait(false);
            }
        }
    }

    /// <summary>调用方 = 唯一 writer 任务（锁外执行；B03）。</summary>
    private static void WriteSnapshot(WidgetSnapshot snapshot)
    {
        if (snapshot.Generation == 0 && HasValidCache(snapshot.SettingsRevision))
        {
            // 尚未首刷且现有缓存有效（同 schema、同 settingsRevision）：保留。
            return;
        }
        string json = ContractsJson.Encode(snapshot);
        if (json == _lastWritten)
        {
            return;
        }
        string path = DataPath;
        string temp = path + ".tmp";
        if (WriteHookOverride is { } hook)
        {
            hook(path, json);
        }
        else
        {
            File.WriteAllText(temp, json);
            File.Move(temp, path, overwrite: true);
        }
        _lastWritten = json;
    }

    /// <summary>首刷保护判断：现有文件可解码、schema 一致且配置版本一致。</summary>
    private static bool HasValidCache(ulong settingsRevision)
    {
        try
        {
            string path = DataPath;
            if (!File.Exists(path))
            {
                return false;
            }
            var cached = ContractsJson.Decode<WidgetSnapshot>(File.ReadAllBytes(path));
            return cached is not null
                && cached.SchemaVersion == 1
                && cached.SettingsRevision == settingsRevision;
        }
        catch (Exception exception) when (
            exception is JsonException or IOException or UnauthorizedAccessException)
        {
            return false; // 缺失 / 损坏 / 与读取竞争：没有可保留的有效缓存
        }
    }

    /// <summary>Provider 读取入口；损坏 / 缺失返回 null（Provider 显示占位状态）。</summary>
    public static WidgetSnapshot? TryRead()
    {
        try
        {
            string path = DataPath;
            if (!File.Exists(path))
            {
                return null;
            }
            return ContractsJson.Decode<WidgetSnapshot>(File.ReadAllBytes(path));
        }
        catch (JsonException)
        {
            return null; // 损坏 / 未知 schema：Provider 显示"重新打开应用"占位
        }
        catch (IOException)
        {
            return null; // 与写入竞争：下次 Activate 会重读
        }
    }
}
