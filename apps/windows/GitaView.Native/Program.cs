// 自定义入口（N15/N18 排障结论）：单实例重定向在任何 WinUI / XAML 初始化之前
// 完成——次实例只启动 .NET 即重定向退出，不完整引导 XAML（秒级）。
//
// 微软官方单实例样例的完整模式（缺一即 CoreMessagingXP fail fast 0xc000027b，
// 实测）：① RedirectActivationToAsync 需要调用线程有 DispatcherQueue → 显式
// CreateDispatcherQueueController；② STA 主线程不能 .Wait()（COM 泵）→
// Task.Run + WaitForSingleObject 非泵式等待。
using System.Runtime.InteropServices;
using GitaView_Native;
using Microsoft.UI.Dispatching;
using Microsoft.Windows.AppLifecycle;
using WinRT;

public static class Program
{
    [STAThread]
    private static int Main(string[] args)
    {
        global::WinRT.ComWrappersSupport.InitializeComWrappers();

        var primary = AppInstance.FindOrRegisterForKey("GitaView.Native.primary");
        if (!primary.IsCurrent)
        {
            var current = AppInstance.GetCurrent();
            var currentArgs = current.GetActivatedEventArgs();
            // 重定向需要当前线程的 DispatcherQueue（CoreMessagingXP）。
            IntPtr controller = CreateDispatcherQueueControllerForCurrentThread();
            var redirectDone = new ManualResetEvent(false);
            _ = Task.Run(async () =>
            {
                try
                {
                    await primary.RedirectActivationToAsync(currentArgs);
                }
                catch
                {
                    // 主实例退出竞争：本次启动直接结束。
                }
                finally
                {
                    redirectDone.Set();
                }
            });
            // STA 线程非泵式等待（避免 COM 消息泵死锁）。
            _ = WaitForSingleObject(redirectDone.SafeWaitHandle.DangerousGetHandle(), 10_000);
            redirectDone.Dispose();
            _ = controller;
            return 0;
        }

        Microsoft.UI.Xaml.Application.Start(callbackParams =>
        {
            var context = new DispatcherQueueSynchronizationContext(
                DispatcherQueue.GetForCurrentThread());
            SynchronizationContext.SetSynchronizationContext(context);
            new App();
        });
        return 0;
    }

    private const uint Infinite = 0xFFFFFFFF;

    [StructLayout(LayoutKind.Sequential)]
    private struct DispatcherQueueOptions
    {
        public uint dwSize;
        public int threadType;     // DQTYPE_THREAD_CURRENT = 2
        public int apartmentType;  // DQTAT_COM_STA = 2
    }

    [DllImport("coremessaging.dll", EntryPoint = "CreateDispatcherQueueController",
        SetLastError = true, CharSet = CharSet.Unicode)]
    private static extern int CreateDispatcherQueueController(
        DispatcherQueueOptions options, out IntPtr dispatcherQueueController);

    private static IntPtr CreateDispatcherQueueControllerForCurrentThread()
    {
        var options = new DispatcherQueueOptions
        {
            dwSize = (uint)Marshal.SizeOf<DispatcherQueueOptions>(),
            threadType = 2,   // DQTYPE_THREAD_CURRENT
            apartmentType = 2, // DQTAT_COM_STA
        };
        _ = CreateDispatcherQueueController(options, out IntPtr controller);
        return controller;
    }

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern uint WaitForSingleObject(nint handle, uint milliseconds);
}
