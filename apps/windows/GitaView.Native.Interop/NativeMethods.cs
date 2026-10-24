// C ABI 的 P/Invoke 声明（LibraryImport 源生成）。
// 签名与 crates/gitaview-ffi/include/gitaview.h 一一对应；
// 指针参数原样传递，编解码与生命周期由 GitaViewEngine 受控 owner 管理。

using System.Runtime.InteropServices;

namespace GitaView.Native.Interop;

/// <summary>Rust 分配的缓冲区：无 NUL 终止；只能由 gv_buffer_free 释放一次。</summary>
[StructLayout(LayoutKind.Sequential)]
internal struct GvBuffer
{
    public IntPtr Data;
    public nuint Len;
}

public static unsafe partial class NativeMethods
{
    private const string Library = "gitaview_ffi";

    /// <summary>
    /// 固定 DLL 加载（§12）：按安装位置绝对路径预加载，后续 "gitaview_ffi"
    /// 绑定解析到已加载模块；不依赖工作目录或用户 PATH。
    /// </summary>
    public static void PreloadNativeLibrary()
    {
        string path = System.IO.Path.Combine(AppContext.BaseDirectory, "gitaview_ffi.dll");
        if (!LoadLibraryExW(path, 0, LoadWithAlteredSearchPath))
        {
            throw new DllNotFoundException($"gitaview_ffi.dll not found at {path}");
        }
    }

    private const uint LoadWithAlteredSearchPath = 0x00000008;

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool LoadLibraryExW(string fileName, nint handle, uint flags);

    [LibraryImport(Library, EntryPoint = "gv_abi_version")]
    internal static partial uint GvAbiVersion();

    [LibraryImport(Library, EntryPoint = "gv_engine_create")]
    internal static partial int GvEngineCreate(byte* json, nuint len, IntPtr* engine, GvBuffer* reply);

    [LibraryImport(Library, EntryPoint = "gv_engine_request_stop")]
    internal static partial int GvEngineRequestStop(IntPtr engine, GvBuffer* reply);

    [LibraryImport(Library, EntryPoint = "gv_engine_wait_stopped")]
    internal static partial int GvEngineWaitStopped(IntPtr engine, uint timeoutMs, GvBuffer* reply);

    [LibraryImport(Library, EntryPoint = "gv_engine_destroy")]
    internal static partial int GvEngineDestroy(IntPtr* engine);

    [LibraryImport(Library, EntryPoint = "gv_settings_get")]
    internal static partial int GvSettingsGet(IntPtr engine, GvBuffer* reply);

    [LibraryImport(Library, EntryPoint = "gv_settings_save")]
    internal static partial int GvSettingsSave(IntPtr engine, byte* json, nuint len, GvBuffer* reply);

    [LibraryImport(Library, EntryPoint = "gv_scan_directory")]
    internal static partial int GvScanDirectory(IntPtr engine, byte* json, nuint len, GvBuffer* reply);

    [LibraryImport(Library, EntryPoint = "gv_repo_add")]
    internal static partial int GvRepoAdd(IntPtr engine, byte* json, nuint len, GvBuffer* reply);

    [LibraryImport(Library, EntryPoint = "gv_repo_remove")]
    internal static partial int GvRepoRemove(IntPtr engine, byte* id, nuint len, GvBuffer* reply);

    [LibraryImport(Library, EntryPoint = "gv_repo_fetch")]
    internal static partial int GvRepoFetch(IntPtr engine, byte* id, nuint len, GvBuffer* reply);

    [LibraryImport(Library, EntryPoint = "gv_repo_pull")]
    internal static partial int GvRepoPull(IntPtr engine, byte* id, nuint len, byte confirmed, GvBuffer* reply);

    [LibraryImport(Library, EntryPoint = "gv_repo_push")]
    internal static partial int GvRepoPush(IntPtr engine, byte* id, nuint len, byte confirmed, GvBuffer* reply);

    [LibraryImport(Library, EntryPoint = "gv_refresh_request")]
    internal static partial int GvRefreshRequest(IntPtr engine, GvBuffer* reply);

    [LibraryImport(Library, EntryPoint = "gv_snapshot_get")]
    internal static partial int GvSnapshotGet(IntPtr engine, GvBuffer* reply);

    [LibraryImport(Library, EntryPoint = "gv_snapshot_wait")]
    internal static partial int GvSnapshotWait(IntPtr engine, byte* cursorJson, nuint len, uint timeoutMs, GvBuffer* reply);

    [LibraryImport(Library, EntryPoint = "gv_view_query")]
    internal static partial int GvViewQuery(IntPtr engine, byte* json, nuint len, GvBuffer* reply);

    [LibraryImport(Library, EntryPoint = "gv_widget_snapshot")]
    internal static partial int GvWidgetSnapshot(IntPtr engine, GvBuffer* reply);

    [LibraryImport(Library, EntryPoint = "gv_repo_target")]
    internal static partial int GvRepoTarget(IntPtr engine, byte* json, nuint len, GvBuffer* reply);

    [LibraryImport(Library, EntryPoint = "gv_buffer_free")]
    internal static partial void GvBufferFree(GvBuffer* buffer);
}
