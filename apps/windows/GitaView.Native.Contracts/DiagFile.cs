// C# 诊断文件（宿主 / Provider 共用）：UTC 时间戳、大小轮转（5MB × 2 代）、
// 写入绝不抛出（诊断不能反向影响业务）。与 Rust diagnostics 同一套规则。

using System.Text;

namespace GitaView.Native.Contracts;

public sealed class DiagFile
{
    private readonly string _path;
    private readonly object _gate = new();
    private const long MaxBytes = 5 * 1024 * 1024;

    public DiagFile(string directory, string name)
    {
        Directory.CreateDirectory(directory);
        _path = Path.Combine(directory, name);
    }

    public void Log(string level, string eventName, string detail)
    {
        try
        {
            var now = DateTimeOffset.UtcNow;
            var line = $"{now:yyyy-MM-ddTHH:mm:ss.fffZ} {level} {eventName} {detail}{Environment.NewLine}";
            lock (_gate)
            {
                RotateIfNeeded();
                File.AppendAllText(_path, line, Encoding.UTF8);
            }
        }
        catch
        {
            // 诊断失败静默。
        }
    }

    private void RotateIfNeeded()
    {
        if (!File.Exists(_path) || new FileInfo(_path).Length < MaxBytes)
        {
            return;
        }
        for (var generation = 1; generation < 2; generation++)
        {
            var from = _path + "." + generation;
            var to = _path + "." + (generation + 1);
            if (File.Exists(from))
            {
                File.Move(from, to, overwrite: true);
            }
        }
        File.Move(_path, _path + ".1", overwrite: true);
    }
}
