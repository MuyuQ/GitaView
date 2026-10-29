// GitaViewEngine：C ABI 的受控 owner（N10.Windows；N18 修复锁语义；review R01/R02 修复）。
//
// 锁与租约规则（ABI §9.7 + review R01）：
// - _handleGate 保护 _handle、_closed 与在途租约计数（微秒级临界区）；
//   **不持有跨越 FFI 调用**（修复前 SnapshotWait 持锁最长 15s，UI 全部排队，
//   日志实测 ui.requery.slow duration_ms=15009，而 Rust 层 0ms）。
// - 调用租约：取句柄与计数 +1 在同一临界区完成，调用结束 finally 归还；
//   消除"已取出指针、尚未进入 Rust"期间句柄被销毁的窗口（R01）。
// - 并发安全由 Rust 侧保证：引擎状态锁不在等待期间持有；destroy 与在途调用
//   由 handle.begin_destroy BUSY 协调。
// - 退出路径：RequestStop → WaitStopped → Dispose。Dispose 由唯一关闭 owner
//   串行整个序列（取句柄 → stop → 拒新租约 → 排空 → 等待 → destroy → 清句柄）；
//   只有 destroy 成功才清除句柄，失败时 owner 保留、可重试（R02/S01）。
//   不用 finalizer 等待网络或卸载库。

using System.Text;
using GitaView.Native.Contracts;

namespace GitaView.Native.Interop;

public enum SnapshotWaitKind
{
    Ready,
    Timeout,
    Stopping,
}

public readonly record struct SnapshotWaitResult(SnapshotWaitKind Kind, AppSnapshot? Snapshot)
{
    public static SnapshotWaitResult Ready(AppSnapshot snapshot) =>
        new(SnapshotWaitKind.Ready, snapshot);
}

public sealed unsafe class GitaViewEngine : IDisposable
{
    private IntPtr _handle;
    private readonly object _handleGate = new();

    /// <summary>true = 已进入关闭：拒绝新调用租约；已有租约用完即回收。</summary>
    private bool _closed;

    /// <summary>持有句柄租约的在途调用数（取句柄与计数在同一临界区更新）。</summary>
    private int _activeLeases;

    /// <summary>关闭 owner 互斥：整个关闭序列（stop → 排空 → 等待 → destroy →
    /// 清句柄）串行执行；并发 Dispose 等待同一关闭结果或幂等返回（review S01）。</summary>
    private readonly object _closeLock = new();

    public EngineCreateData CreateData { get; }

    public string SessionId => CreateData.SessionId;

    private GitaViewEngine(IntPtr handle, EngineCreateData createData)
    {
        _handle = handle;
        CreateData = createData;
    }

    public static uint AbiVersion() => NativeMethods.GvAbiVersion();

    /// <summary>创建引擎：失败（含非法参数）抛 GvException 且不产生句柄。</summary>
    public static GitaViewEngine CreateEngine(EngineCreateInput input)
    {
        byte[] json = Encoding.UTF8.GetBytes(ContractsJson.Encode(input));
        IntPtr handle = IntPtr.Zero;
        GvBuffer buffer = default;
        int status;
        byte[]? payload = null;
        fixed (byte* jsonPtr = json)
        {
            try
            {
                status = NativeMethods.GvEngineCreate(jsonPtr, (nuint)json.Length, &handle, &buffer);
                payload = TakeBuffer(ref buffer);
            }
            finally
            {
                FreeBuffer(&buffer);
            }
        }
        if (status != (int)GvStatus.Ok)
        {
            if (handle != IntPtr.Zero)
            {
                throw new InvalidOperationException("failed create must leave engine null");
            }
            throw BuildException("gv_engine_create", status, payload);
        }
        if (handle == IntPtr.Zero)
        {
            throw new InvalidOperationException("successful create returned null engine");
        }
        var envelope = ContractsJson.Decode<Envelope<EngineCreateData>>(payload!);
        return new GitaViewEngine(handle, envelope.Data!);
    }

    // —— 业务调用 ——

    public SettingsData SettingsGet() =>
        Call<SettingsData>("gv_settings_get", &NativeMethods.GvSettingsGet);

    public SettingsData SettingsSave(SettingsSaveInput input) =>
        CallJson<SettingsData>("gv_settings_save", input, &NativeMethods.GvSettingsSave);

    public ScanDirectoryData ScanDirectory(string path) =>
        CallJson<ScanDirectoryData>("gv_scan_directory",
            new ScanDirectoryInput { Path = path }, &NativeMethods.GvScanDirectory);

    public RepoAddData RepoAdd(RepoAddInput input) =>
        CallJson<RepoAddData>("gv_repo_add", input, &NativeMethods.GvRepoAdd);

    public RepoRemoveData RepoRemove(string repoId) =>
        CallRawId<RepoRemoveData>("gv_repo_remove", repoId, &NativeMethods.GvRepoRemove);

    public RepoOperationData RepoFetch(string repoId) =>
        CallRawId<RepoOperationData>("gv_repo_fetch", repoId, &NativeMethods.GvRepoFetch);

    public RepoOperationData RepoPull(string repoId, bool confirmed) =>
        CallConfirmed<RepoOperationData>("gv_repo_pull", repoId, confirmed, &NativeMethods.GvRepoPull);

    public RepoOperationData RepoPush(string repoId, bool confirmed) =>
        CallConfirmed<RepoOperationData>("gv_repo_push", repoId, confirmed, &NativeMethods.GvRepoPush);

    public RefreshRequestData RefreshRequest() =>
        Call<RefreshRequestData>("gv_refresh_request", &NativeMethods.GvRefreshRequest);

    public AppSnapshot SnapshotGet() =>
        Call<AppSnapshot>("gv_snapshot_get", &NativeMethods.GvSnapshotGet);

    /// <summary>等待公开状态变化；cursor 为 sessionId + stateRevision。
    /// 不持引擎级锁：Rust 侧允许等待与读快照 / 设置 / 业务调用并行（§9.7）。</summary>
    public SnapshotWaitResult SnapshotWait(ulong stateRevision, uint timeoutMs) =>
        SnapshotWait(SessionId, stateRevision, timeoutMs);

    public SnapshotWaitResult SnapshotWait(string sessionId, ulong stateRevision, uint timeoutMs)
    {
        var cursor = new SnapshotWaitInput { SessionId = sessionId, StateRevision = stateRevision };
        byte[] json = Encoding.UTF8.GetBytes(ContractsJson.Encode(cursor));
        IntPtr handle = AcquireLease();
        try
        {
            GvBuffer buffer = default;
            int status;
            byte[]? payload = null;
            fixed (byte* jsonPtr = json)
            {
                try
                {
                    status = NativeMethods.GvSnapshotWait(handle, jsonPtr, (nuint)json.Length, timeoutMs, &buffer);
                    payload = TakeBuffer(ref buffer);
                }
                finally
                {
                    FreeBuffer(&buffer);
                }
            }
            return status switch
            {
                (int)GvStatus.Ok => SnapshotWaitResult.Ready(
                    ContractsJson.Decode<Envelope<AppSnapshot>>(payload!).Data!),
                (int)GvStatus.Timeout => new SnapshotWaitResult(SnapshotWaitKind.Timeout, null),
                (int)GvStatus.Stopping => new SnapshotWaitResult(SnapshotWaitKind.Stopping, null),
                _ => throw BuildException("gv_snapshot_wait", status, payload),
            };
        }
        finally
        {
            ReleaseLease();
        }
    }

    /// <summary>刷新请求是否已被完成的扫描覆盖（完成看 completedRequestId）。</summary>
    public bool RefreshCompleted(ulong requestId) =>
        SnapshotGet().Refresh.CompletedRequestId >= requestId;

    public ViewResult ViewQuery(ViewQuery query) =>
        CallJson<ViewResult>("gv_view_query", query, &NativeMethods.GvViewQuery);

    public WidgetSnapshot WidgetSnapshot() =>
        Call<WidgetSnapshot>("gv_widget_snapshot", &NativeMethods.GvWidgetSnapshot);

    public RepoTargetData RepoTarget(RepoTargetInput input) =>
        CallJson<RepoTargetData>("gv_repo_target", input, &NativeMethods.GvRepoTarget);

    // —— 生命周期 ——

    public LifecycleData RequestStop()
    {
        IntPtr handle = AcquireLease();
        try
        {
            (int status, byte[]? payload) = CallSimple(&NativeMethods.GvEngineRequestStop, handle);
            return Unwrap<LifecycleData>("gv_engine_request_stop", status, payload);
        }
        finally
        {
            ReleaseLease();
        }
    }

    /// <summary>等待停止完成；期限到仍清理完返回 false（不代表可以销毁）。</summary>
    public bool WaitStopped(uint timeoutMs)
    {
        IntPtr handle = AcquireLease();
        try
        {
            GvBuffer buffer = default;
            int status;
            try
            {
                status = NativeMethods.GvEngineWaitStopped(handle, timeoutMs, &buffer);
            }
            finally
            {
                FreeBuffer(&buffer);
            }
            return status switch
            {
                (int)GvStatus.Ok => true,
                (int)GvStatus.Timeout => false,
                _ => throw new GvException("gv_engine_wait_stopped failed", (GvStatus)status, null),
            };
        }
        finally
        {
            ReleaseLease();
        }
    }

    /// <summary>
    /// 释放（review S01）：整个关闭过程由同一个关闭 owner 串行——读取句柄、
    /// 首次 stop、拒绝新租约、排空在途租约、等待停止、destroy、清空句柄全部
    /// 在 _closeLock 内完成。并发的其他 Dispose 在 _closeLock 上等待，进入后
    /// 看到句柄已清空则幂等返回；关闭失败（租约未排空 / 等待超时）时句柄保留，
    /// 再次调用继续关闭。业务查询与 SnapshotWait 仍并行，不恢复全局串行锁。
    /// </summary>
    public void Dispose()
    {
        lock (_closeLock)
        {
            IntPtr handle;
            lock (_handleGate)
            {
                if (_handle == IntPtr.Zero)
                {
                    return; // 从未创建、已完全释放，或另一方刚完成关闭
                }
                _closed = true; // 阻止新租约进入
                handle = _handle;
            }
            // 先请求停止（幂等）：等待中的 SnapshotWait 立即返回 Stopping 并归还
            // 租约，否则下面的排空会与等待互相等死。stop 在 owner 内执行，此后
            // 本线程外的任何 destroy 都不可能先于它发生（destroy 也在 owner 内）。
            NativeMethods.GvEngineRequestStop(handle, null);
            lock (_handleGate)
            {
                var start = DateTime.UtcNow;
                while (_activeLeases > 0 && DateTime.UtcNow - start < LeaseDrainTimeout)
                {
                    _ = Monitor.Wait(_handleGate, TimeSpan.FromSeconds(1));
                }
                if (_activeLeases > 0)
                {
                    // 仍有业务调用持有租约（如网络操作进行中）：句柄未清空，
                    // 保留 owner 供稍后重试关闭；不能在调用在途时销毁。
                    throw new GvException(
                        "engine dispose: business calls still active", GvStatus.Busy, null);
                }
            }
            CloseNative(handle);
        }
    }

    /// <summary>
    /// 关闭中的租约排空期限。生产 30s（等待中的调用在 stop 后立即归还）；
    /// 测试可缩短以验证"关闭失败可重试"路径。
    /// </summary>
    internal TimeSpan LeaseDrainTimeout { get; set; } = TimeSpan.FromSeconds(30);

    /// <summary>调用方必须持有 _closeLock 且已排空租约：等待 Stopped → destroy → 清句柄。</summary>
    private void CloseNative(IntPtr handle)
    {
        // WaitStopped 分段等待：Stopping 后等待者快速返回（60s 上限；
        // 更长的业务等待由宿主在 Dispose 之前完成）。
        for (var attempt = 0; ; attempt++)
        {
            GvBuffer buffer = default;
            int status;
            try
            {
                status = NativeMethods.GvEngineWaitStopped(handle, 5_000, &buffer);
            }
            finally
            {
                FreeBuffer(&buffer);
            }
            if (status == (int)GvStatus.Ok)
            {
                break;
            }
            if (status == (int)GvStatus.Timeout && attempt < 11)
            {
                continue;
            }
            throw new GvException("gv_engine_wait_stopped failed", (GvStatus)status, null);
        }
        // destroy 与在途调用竞争 → BUSY：短暂重试（在途调用随停止快速返回）。
        for (var attempt = 0; ; attempt++)
        {
            IntPtr target = handle;
            int status = NativeMethods.GvEngineDestroy(&target);
            if (status == (int)GvStatus.Ok)
            {
                lock (_handleGate)
                {
                    if (_handle == handle)
                    {
                        _handle = IntPtr.Zero; // 仅确认释放成功后清除
                    }
                }
                return;
            }
            if (status == (int)GvStatus.Busy && attempt < 20)
            {
                Thread.Sleep(250);
                continue;
            }
            throw new GvException("gv_engine_destroy", (GvStatus)status, null);
        }
    }

    // —— 调用租约（R01：取句柄 + 计数同一临界区；finally 归还）——

    private IntPtr AcquireLease()
    {
        lock (_handleGate)
        {
            if (_closed || _handle == IntPtr.Zero)
            {
                throw new ObjectDisposedException(nameof(GitaViewEngine));
            }
            _activeLeases++;
            return _handle;
        }
    }

    private void ReleaseLease()
    {
        lock (_handleGate)
        {
            _activeLeases--;
            if (_closed && _activeLeases == 0)
            {
                Monitor.PulseAll(_handleGate);
            }
        }
    }

    // —— 调用基础设施（函数指针：指针类型不能作泛型参数）——

    private T Call<T>(string name, delegate*<IntPtr, GvBuffer*, int> call)
    {
        IntPtr handle = AcquireLease();
        try
        {
            (int status, byte[]? payload) = CallSimple(call, handle);
            return Unwrap<T>(name, status, payload);
        }
        finally
        {
            ReleaseLease();
        }
    }

    private static (int status, byte[]? payload) CallSimple(
        delegate*<IntPtr, GvBuffer*, int> call, IntPtr handle)
    {
        GvBuffer buffer = default;
        int status;
        byte[]? payload = null;
        try
        {
            status = call(handle, &buffer);
            payload = TakeBuffer(ref buffer);
        }
        finally
        {
            FreeBuffer(&buffer);
        }
        return (status, payload);
    }

    private T CallJson<T>(string name, object input, delegate*<IntPtr, byte*, nuint, GvBuffer*, int> call)
    {
        byte[] json = Encoding.UTF8.GetBytes(ContractsJson.Encode(input));
        IntPtr handle = AcquireLease();
        try
        {
            GvBuffer buffer = default;
            int status;
            byte[]? payload = null;
            fixed (byte* jsonPtr = json)
            {
                try
                {
                    status = call(handle, jsonPtr, (nuint)json.Length, &buffer);
                    payload = TakeBuffer(ref buffer);
                }
                finally
                {
                    FreeBuffer(&buffer);
                }
            }
            return Unwrap<T>(name, status, payload);
        }
        finally
        {
            ReleaseLease();
        }
    }

    private T CallRawId<T>(string name, string repoId, delegate*<IntPtr, byte*, nuint, GvBuffer*, int> call)
    {
        byte[] id = Encoding.UTF8.GetBytes(repoId);
        IntPtr handle = AcquireLease();
        try
        {
            GvBuffer buffer = default;
            int status;
            byte[]? payload = null;
            fixed (byte* idPtr = id)
            {
                try
                {
                    status = call(handle, idPtr, (nuint)id.Length, &buffer);
                    payload = TakeBuffer(ref buffer);
                }
                finally
                {
                    FreeBuffer(&buffer);
                }
            }
            return Unwrap<T>(name, status, payload);
        }
        finally
        {
            ReleaseLease();
        }
    }

    private T CallConfirmed<T>(string name, string repoId, bool confirmed,
        delegate*<IntPtr, byte*, nuint, byte, GvBuffer*, int> call)
    {
        byte[] id = Encoding.UTF8.GetBytes(repoId);
        byte confirmedByte = confirmed ? (byte)1 : (byte)0;
        IntPtr handle = AcquireLease();
        try
        {
            GvBuffer buffer = default;
            int status;
            byte[]? payload = null;
            fixed (byte* idPtr = id)
            {
                try
                {
                    status = call(handle, idPtr, (nuint)id.Length, confirmedByte, &buffer);
                    payload = TakeBuffer(ref buffer);
                }
                finally
                {
                    FreeBuffer(&buffer);
                }
            }
            return Unwrap<T>(name, status, payload);
        }
        finally
        {
            ReleaseLease();
        }
    }

    private static T Unwrap<T>(string name, int status, byte[]? payload)
    {
        if (status == (int)GvStatus.Ok)
        {
            return ContractsJson.Decode<Envelope<T>>(payload!).Data
                ?? throw new GvException($"{name}: ok envelope without data", GvStatus.Error, null);
        }
        throw BuildException(name, status, payload);
    }

    private static GvException BuildException(string name, int status, byte[]? payload)
    {
        string? code = null;
        if (payload is { Length: > 0 })
        {
            try
            {
                code = ContractsJson.Decode<Envelope<object>>(payload).Error?.Code;
            }
            catch (System.Text.Json.JsonException)
            {
                // envelope 之外的负载（不应发生）：保留 code=null。
            }
        }
        return new GvException($"{name} failed", (GvStatus)status, code);
    }

    private static byte[]? TakeBuffer(ref GvBuffer buffer) =>
        buffer.Data == IntPtr.Zero ? null : CopyBuffer(buffer);

    private static byte[] CopyBuffer(GvBuffer buffer)
    {
        var payload = new byte[(int)buffer.Len];
        System.Runtime.InteropServices.Marshal.Copy(buffer.Data, payload, 0, payload.Length);
        return payload;
    }

    private static void FreeBuffer(GvBuffer* buffer)
    {
        if (buffer->Data != IntPtr.Zero)
        {
            NativeMethods.GvBufferFree(buffer);
        }
    }
}
