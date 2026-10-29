// 测试可见性：生命周期/租约的内部钩子（如 LeaseDrainTimeout）供真实 FFI 回归测试使用。
using System.Runtime.CompilerServices;

[assembly: InternalsVisibleTo("GitaView.Native.Tests")]
