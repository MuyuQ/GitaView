// 测试可见性：WidgetDataWriter 的注入钩子（DataPathOverride / WriteHookOverride /
// RetryBaseDelay）供真实回归测试使用，避免测试触碰包目录。
using System.Runtime.CompilerServices;

[assembly: InternalsVisibleTo("GitaView.Native.Tests")]
