// GitaView 原生契约 v1 的 C# 镜像（N10.Windows）。
// 规则与 contracts/native/v1/schema.json 一致：
// - camelCase；枚举小写 snake_case；未知字段拒绝（见 ContractsJson）。
// - 时间为 Unix 毫秒 ulong；不做任何 DateTime 默认解码，UI 侧显式转换。

using System.Text.Json.Serialization;

namespace GitaView.Native.Contracts;

public enum Relation
{
    [JsonPropertyName("synced")] Synced,
    [JsonPropertyName("local_ahead")] LocalAhead,
    [JsonPropertyName("remote_ahead")] RemoteAhead,
    [JsonPropertyName("diverged")] Diverged,
    [JsonPropertyName("no_remote")] NoRemote,
}

public enum ReadState
{
    [JsonPropertyName("pending")] Pending,
    [JsonPropertyName("ok")] Ok,
    [JsonPropertyName("error")] Error,
}

public enum RefreshState
{
    [JsonPropertyName("pending")] Pending,
    [JsonPropertyName("ready")] Ready,
    [JsonPropertyName("failed")] Failed,
}

public enum RelationFilter
{
    [JsonPropertyName("all")] All,
    [JsonPropertyName("error")] Error,
    [JsonPropertyName("synced")] Synced,
    [JsonPropertyName("local_ahead")] LocalAhead,
    [JsonPropertyName("remote_ahead")] RemoteAhead,
    [JsonPropertyName("diverged")] Diverged,
    [JsonPropertyName("no_remote")] NoRemote,
}

public enum RepoTargetKind
{
    [JsonPropertyName("directory")] Directory,
    [JsonPropertyName("remote")] Remote,
}

public enum LifecycleDto
{
    [JsonPropertyName("running")] Running,
    [JsonPropertyName("stopping")] Stopping,
    [JsonPropertyName("stopped")] Stopped,
}

public sealed record ErrorObject
{
    public required string Code { get; init; }
    public string? Message { get; init; }
}

public sealed record RepoActions
{
    public required bool CanOpenDirectory { get; init; }
    public required bool CanOpenRemote { get; init; }
    public required bool CanFetch { get; init; }
    public required bool CanPull { get; init; }
    public required bool CanPush { get; init; }
}

public sealed record RepoStatus
{
    public required string Id { get; init; }
    public required string Name { get; init; }
    public required string Path { get; init; }
    public string? GroupId { get; init; }
    public required ReadState ReadState { get; init; }
    public Relation? Relation { get; init; }
    public string? Branch { get; init; }
    public bool? HasCommit { get; init; }
    public bool? HasOrigin { get; init; }
    public string? ComparisonBranch { get; init; }
    public uint? Ahead { get; init; }
    public uint? Behind { get; init; }
    public string? RemoteUrl { get; init; }
    public ErrorObject? Error { get; init; }
    public required RepoActions Actions { get; init; }
}

public sealed record RefreshPolicy
{
    public required bool Enabled { get; init; }
    public required uint IntervalMinutes { get; init; }
}

public sealed record RefreshProgress
{
    public required bool IsRefreshing { get; init; }
    public required ulong RequestedRequestId { get; init; }
    public required ulong CompletedRequestId { get; init; }
}

public sealed record AppSnapshot : ISchemaVersioned
{
    public required ulong SchemaVersion { get; init; }
    public required string SessionId { get; init; }
    public required ulong Generation { get; init; }
    public required ulong StateRevision { get; init; }
    public required ulong SettingsRevision { get; init; }
    public ulong? AttemptedAtMs { get; init; }
    public ulong? LastSuccessAtMs { get; init; }
    public required RefreshState RefreshState { get; init; }
    public ErrorObject? RefreshError { get; init; }
    public required RefreshPolicy RefreshPolicy { get; init; }
    public required RefreshProgress Refresh { get; init; }
    public required IReadOnlyList<RepoStatus> Repos { get; init; }
}

public sealed record WidgetCounts
{
    public required uint Total { get; init; }
    public required uint Synced { get; init; }
    public required uint LocalAhead { get; init; }
    public required uint RemoteAhead { get; init; }
    public required uint Diverged { get; init; }
    public required uint NoRemote { get; init; }
    public required uint Error { get; init; }
    public required uint Pending { get; init; }
}

public sealed record CompactCounts
{
    public required uint Synced { get; init; }
    public required uint Syncable { get; init; }
    public required uint NeedsAttention { get; init; }
    public required uint NoRemote { get; init; }
}

public sealed record WidgetRepo
{
    public required string Id { get; init; }
    public required string Name { get; init; }
    public required ReadState ReadState { get; init; }
    public Relation? Relation { get; init; }
    public string? ErrorCode { get; init; }
}

public sealed record WidgetSnapshot : ISchemaVersioned
{
    public required ulong SchemaVersion { get; init; }
    public required string SessionId { get; init; }
    public required ulong Generation { get; init; }
    public required ulong StateRevision { get; init; }
    public required ulong SettingsRevision { get; init; }
    public ulong? AttemptedAtMs { get; init; }
    public ulong? LastSuccessAtMs { get; init; }
    public required RefreshState RefreshState { get; init; }
    public string? RefreshErrorCode { get; init; }
    public required RefreshPolicy RefreshPolicy { get; init; }
    public required WidgetCounts Counts { get; init; }
    public required CompactCounts Compact { get; init; }
    public required IReadOnlyList<WidgetRepo> Attention { get; init; }
}

public sealed record RepoSettings
{
    public required string Id { get; init; }
    public required string Name { get; init; }
    public required string Path { get; init; }
    public string? GroupId { get; init; }
}

public sealed record GroupSettings
{
    public required string Id { get; init; }
    public required string Name { get; init; }
}

public sealed record SettingsData : ISchemaVersioned
{
    public required ulong SchemaVersion { get; init; }
    public required ulong Revision { get; init; }
    public required IReadOnlyList<RepoSettings> Repos { get; init; }
    public required IReadOnlyList<GroupSettings> Groups { get; init; }
    public string? DefaultGroupId { get; init; }
    public required RefreshPolicy Refresh { get; init; }
}

public sealed record RepoGroupAssignment
{
    public required string RepoId { get; init; }
    public string? GroupId { get; init; }
}

public sealed record SettingsEditable
{
    public required IReadOnlyList<GroupSettings> Groups { get; init; }
    public string? DefaultGroupId { get; init; }
    public required RefreshPolicy Refresh { get; init; }
    public IReadOnlyList<RepoGroupAssignment> RepoGroups { get; init; } = [];
}

public sealed record SettingsSaveInput
{
    public required ulong ExpectedRevision { get; init; }
    public required SettingsEditable Settings { get; init; }
}

public sealed record EngineCreateInput
{
    public required string DataDirectory { get; init; }
    public required string LogDirectory { get; init; }
}

public sealed record EngineCreateData
{
    public required string SessionId { get; init; }
    public required uint AbiVersion { get; init; }
    public IReadOnlyList<string> StartupWarnings { get; init; } = [];
}

public sealed record ScanDirectoryInput
{
    public required string Path { get; init; }
}

public sealed record ScanDirectoryData
{
    public required IReadOnlyList<string> Paths { get; init; }
    public required bool Truncated { get; init; }
}

public sealed record RepoAddInput
{
    public required string Path { get; init; }
    public string? GroupId { get; init; }
}

public sealed record RepoAddData
{
    public required RepoSettings Repo { get; init; }
    public required ulong SettingsRevision { get; init; }
}

public sealed record RepoRemoveData
{
    public required ulong SettingsRevision { get; init; }
}

public sealed record RepoOperationData
{
    public required string RepoId { get; init; }
    public required string Operation { get; init; }
}

public sealed record RefreshRequestData
{
    public required string SessionId { get; init; }
    public required ulong RequestId { get; init; }
}

public sealed record SnapshotWaitInput
{
    public required string SessionId { get; init; }
    public required ulong StateRevision { get; init; }
}

public sealed record RepoTargetInput
{
    public required string RepoId { get; init; }
    public required RepoTargetKind Kind { get; init; }
}

public sealed record RepoTargetData
{
    public required RepoTargetKind Kind { get; init; }
    public required string Value { get; init; }
}

public sealed record LifecycleData
{
    public required LifecycleDto Lifecycle { get; init; }
}

public sealed record ViewQuery
{
    public string? GroupId { get; init; }
    public RelationFilter RelationFilter { get; init; } = RelationFilter.All;
    public string Query { get; init; } = "";
}

public sealed record GroupCount
{
    public required string Id { get; init; }
    public required string Name { get; init; }
    public required uint Count { get; init; }
}

public sealed record ViewResult
{
    public required string SessionId { get; init; }
    public required ulong StateRevision { get; init; }
    public required ulong Generation { get; init; }
    public required ulong SettingsRevision { get; init; }
    public required ViewQuery Effective { get; init; }
    public required WidgetCounts SummaryCounts { get; init; }
    public required CompactCounts SummaryCompact { get; init; }
    public required IReadOnlyList<GroupCount> Groups { get; init; }
    public required WidgetCounts StatusCounts { get; init; }
    public required IReadOnlyList<RepoStatus> Rows { get; init; }
}

public sealed record Envelope<T>
{
    public required ulong SchemaVersion { get; init; }
    public required bool Ok { get; init; }
    public T? Data { get; init; }
    public ErrorObject? Error { get; init; }
}

public static class TimeExtensions
{
    /// <summary>显式转换 Unix 毫秒；契约不做任何 DateTime 默认解码。</summary>
    public static DateTimeOffset ToUnixDateTime(this ulong unixMilliseconds) =>
        DateTimeOffset.FromUnixTimeMilliseconds(unchecked((long)unixMilliseconds));
}
