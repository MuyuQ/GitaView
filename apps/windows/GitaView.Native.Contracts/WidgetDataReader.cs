using System.Text.Json;

namespace GitaView.Native.Contracts;

/// <summary>按宿主提供的路径读取 Widget 缓存，复用契约校验。</summary>
public static class WidgetDataReader
{
    /// <summary>缺失、损坏、未知 schema 或读取 I/O 失败返回 null；其他错误交由调用方处理。</summary>
    public static WidgetSnapshot? TryRead(string path)
    {
        try
        {
            return File.Exists(path)
                ? ContractsJson.Decode<WidgetSnapshot>(File.ReadAllBytes(path))
                : null;
        }
        catch (JsonException)
        {
            return null;
        }
        catch (IOException)
        {
            return null;
        }
    }
}
