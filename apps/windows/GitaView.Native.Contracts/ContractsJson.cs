// 契约 JSON 编解码：camelCase + 拒绝未知字段 + 枚举显式字符串 + schemaVersion 守卫。
// 未知字段 / 未知枚举 / 缺必填 / 未知 schema 版本 → JsonException，
// 不允许解码默认值隐藏不匹配。

using System.Reflection;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace GitaView.Native.Contracts;

/// <summary>携带 schemaVersion 的 DTO：解码后校验版本（镜像 Rust codec 规则）。</summary>
public interface ISchemaVersioned
{
    ulong SchemaVersion { get; }
}

/// <summary>
/// 契约枚举转换器：按 [JsonPropertyName] 显式映射（snake_case 值），
/// 未知值抛 JsonException。不依赖运行时命名策略猜测。
/// </summary>
public sealed class ContractEnumConverterFactory : JsonConverterFactory
{
    public override bool CanConvert(Type typeToConvert) =>
        typeToConvert.IsEnum
        || (Nullable.GetUnderlyingType(typeToConvert)?.IsEnum ?? false);

    public override JsonConverter CreateConverter(Type typeToConvert, JsonSerializerOptions options)
    {
        var underlying = Nullable.GetUnderlyingType(typeToConvert);
        if (underlying is not null)
        {
            // Nullable<T> 需要匹配该类型的转换器；委托给底层枚举转换。
            return (JsonConverter)Activator.CreateInstance(
                typeof(NullableContractEnumConverter<>).MakeGenericType(underlying))!;
        }
        return (JsonConverter)Activator.CreateInstance(
            typeof(ContractEnumConverter<>).MakeGenericType(typeToConvert))!;
    }

    private sealed class NullableContractEnumConverter<T> : JsonConverter<T?>
        where T : struct, Enum
    {
        private readonly ContractEnumConverter<T> _inner = new();

        public override T? Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options) =>
            _inner.Read(ref reader, typeof(T), options);

        public override void Write(Utf8JsonWriter writer, T? value, JsonSerializerOptions options) =>
            _inner.Write(writer, value!.Value, options);
    }

    private sealed class ContractEnumConverter<T> : JsonConverter<T>
        where T : struct, Enum
    {
        private readonly Dictionary<string, T> _byJsonName;
        private readonly Dictionary<T, string> _toJsonName;

        public ContractEnumConverter()
        {
            _byJsonName = new Dictionary<string, T>();
            _toJsonName = new Dictionary<T, string>();
            foreach (string name in Enum.GetNames<T>())
            {
                var member = typeof(T).GetField(name)!;
                var jsonName = member.GetCustomAttribute<JsonPropertyNameAttribute>()?.Name ?? name;
                var value = Enum.Parse<T>(name);
                _byJsonName[jsonName] = value;
                _toJsonName[value] = jsonName;
            }
        }

        public override T Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options)
        {
            string? text = reader.GetString();
            if (text is not null && _byJsonName.TryGetValue(text, out var value))
            {
                return value;
            }
            throw new JsonException(
                $"unknown {typeof(T).Name} value: {text ?? "<non-string>"}");
        }

        public override void Write(Utf8JsonWriter writer, T value, JsonSerializerOptions options)
        {
            writer.WriteStringValue(_toJsonName[value]);
        }
    }
}

public static class ContractsJson
{
    public static readonly JsonSerializerOptions Options = CreateOptions();

    private static JsonSerializerOptions CreateOptions()
    {
        var options = new JsonSerializerOptions(JsonSerializerDefaults.General)
        {
            PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
            // 未知字段必须拒绝（schema additionalProperties: false）。
            UnmappedMemberHandling = JsonUnmappedMemberHandling.Disallow,
            // A required key may still contain null; reject it for non-null DTO members.
            RespectNullableAnnotations = true,
        };
        // 枚举以契约字符串（snake_case）往返；成员映射由 [JsonPropertyName] 提供。
        options.Converters.Add(new ContractEnumConverterFactory());
        return options;
    }

    public static T Decode<T>(ReadOnlySpan<byte> utf8)
    {
        var value = JsonSerializer.Deserialize<T>(utf8, Options);
        if (value is null)
        {
            throw new JsonException($"null is not a valid {typeof(T).Name}");
        }
        // 镜像 Rust codec：未知 schemaVersion 拒绝（如 Widget 缓存文件的 999）。
        if (value is ISchemaVersioned versioned && versioned.SchemaVersion != 1)
        {
            throw new JsonException(
                $"unknown schemaVersion {versioned.SchemaVersion} for {typeof(T).Name}");
        }
        return value;
    }

    public static T Decode<T>(string json) =>
        Decode<T>(System.Text.Encoding.UTF8.GetBytes(json));

    public static string Encode<T>(T value) => JsonSerializer.Serialize(value, Options);
}
