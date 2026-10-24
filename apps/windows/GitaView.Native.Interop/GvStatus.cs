// FFI 状态码与稳定业务错误的异常映射。

namespace GitaView.Native.Interop;

public enum GvStatus
{
    Ok = 0,
    Error = 1,
    Timeout = 2,
    Stopping = 3,
    Busy = 4,
    InvalidArgument = 5,
    Panic = 6,
}

/// <summary>
/// FFI 调用失败：状态码 + 稳定业务错误 code（ok=false envelope 的 error.code）。
/// 宿主按 Code 映射文案；TIMEOUT（等待期限）与 GIT_TIMEOUT（Git 命令超时）通过
/// Status 与 Code 明确区分。
/// </summary>
public sealed class GvException(string message, GvStatus status, string? code)
    : Exception($"{message} (status={(int)status}, code={code ?? "none"})")
{
    public GvStatus Status { get; } = status;
    public string? Code { get; } = code;
}
