// GitaView 独立 Widget Provider 进程（N16）。
//
// 生命周期（实施说明 §12 / 官方 implement-widget-provider-cs 模式）：
// - CoRegisterClassObject 注册 COM 类对象（REGCLS_MULTIPLEUSE：一个进程服务全部组件）；
// - 全部组件 Delete 后经空闲确认退出；停止接收新请求后才撤销 COM 工厂；
// - 不创建引擎、不写仓库设置、不访问网络；只读主应用写入的 widget-data.json。

using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using GitaView_Native.WidgetProvider;

[assembly: InternalsVisibleTo("GitaView.Native.Tests")]

[ComImport, ComVisible(false), InterfaceType(ComInterfaceType.InterfaceIsIUnknown),
    Guid("00000001-0000-0000-C000-000000000046")]
internal interface IClassFactory
{
    [PreserveSig]
    int CreateInstance(nint pUnkOuter, ref Guid riid, out nint ppvObject);

    [PreserveSig]
    int LockServer(bool fLock);
}

[ComVisible(true)]
internal sealed class WidgetProviderFactory : IClassFactory
{
    private const int ClassENoAggregation = unchecked((int)0x80040110);
    private const int ENoInterface = unchecked((int)0x80004002);

    public int CreateInstance(nint pUnkOuter, ref Guid riid, out nint ppvObject)
    {
        ppvObject = nint.Zero;
        if (pUnkOuter != nint.Zero)
        {
            return ClassENoAggregation;
        }
        try
        {
            using var callback = GitaViewWidgetProvider.Lifetime.EnterCallback();
            // IWidgetProvider 的接口 GUID 取自 WinRT 投影元数据，不手写。
            var providerGuid = typeof(Microsoft.Windows.Widgets.Providers.IWidgetProvider).GUID;
            if (riid == providerGuid || riid == new Guid("00000000-0000-0000-C000-000000000046"))
            {
                ppvObject = WinRT.MarshalInspectable<Microsoft.Windows.Widgets.Providers.IWidgetProvider>
                    .FromManaged(new GitaViewWidgetProvider());
                return 0;
            }
            return ENoInterface;
        }
        catch (Exception exception) { return Marshal.GetHRForException(exception); }
    }

    public int LockServer(bool fLock) => GitaViewWidgetProvider.Lifetime.LockServer(fLock);
}

internal static class Program
{
    public const string ProviderClsid = "4caa94b6-2e9c-455a-a482-3af61a355f64";

    [DllImport("ole32.dll")]
    private static extern int CoRegisterClassObject(
        [MarshalAs(UnmanagedType.LPStruct)] Guid rclsid,
        [MarshalAs(UnmanagedType.IUnknown)] object pUnk,
        uint dwClsContext,
        uint flags,
        out uint lpdwRegister);

    [DllImport("ole32.dll")]
    private static extern int CoRevokeClassObject(uint dwRegister);

    [DllImport("ole32.dll")]
    private static extern int CoSuspendClassObjects();

    [DllImport("ole32.dll")]
    private static extern int CoResumeClassObjects();

    private const uint ClsctxLocalServer = 0x4;
    private const uint RegclsMultipleuse = 0x2;
    private const uint RegclsSuspended = 0x4;

    [STAThread]
    private static void Main()
    {
        uint cookie;
        int hr = CoRegisterClassObject(
            Guid.Parse(ProviderClsid),
            new WidgetProviderFactory(),
            ClsctxLocalServer,
            RegclsMultipleuse | RegclsSuspended,
            out cookie);
        if (hr != 0)
        {
            return; // 注册失败：宿主下次激活会重新拉起进程。
        }

        bool registered = true;
        try
        {
            Marshal.ThrowExceptionForHR(CoResumeClassObjects());
            while (true)
            {
                GitaViewWidgetProvider.Lifetime.ExitRequested.WaitOne();
                if (GitaViewWidgetProvider.Lifetime.TryShutdown(
                    () => Marshal.ThrowExceptionForHR(CoSuspendClassObjects()),
                    () => Marshal.ThrowExceptionForHR(CoResumeClassObjects()),
                    () =>
                    {
                        Marshal.ThrowExceptionForHR(CoRevokeClassObject(cookie));
                        registered = false;
                    })) break;
            }
        }
        finally
        {
            if (registered) CoRevokeClassObject(cookie);
        }
    }
}
