// 稳定错误码 → 中文文案（UI 层唯一映射点；不解析中文决定控制流）。

using GitaView.Native.Contracts;

namespace GitaView_Native;

public static class ErrorText
{
    public static string For(string? code) => code switch
    {
        "INVALID_ARGUMENT" => "参数无效",
        "INVALID_PATH" => "路径无效或不是 Git 仓库",
        "GIT_NOT_FOUND" => "未找到 Git，请确认已安装并在 PATH 中",
        "REPO_NOT_FOUND" => "仓库不存在或已被移除",
        "REPO_BUSY" => "该仓库已有网络操作进行中，请稍后再试",
        "ORIGIN_REQUIRED" => "该仓库没有配置 origin 远端",
        "COMPARISON_REQUIRED" => "没有可比较的 origin 分支",
        "CONFIRMATION_REQUIRED" => "操作需要用户确认",
        "OPERATION_UNAVAILABLE" => "当前状态不允许此操作",
        "SETTINGS_CONFLICT" => "设置已被修改，请重载后重试",
        "SETTINGS_IO" => "设置读写失败",
        "GIT_FAILED" => "Git 命令执行失败",
        "GIT_TIMEOUT" => "Git 命令超时",
        "ENGINE_STOPPING" => "应用正在退出",
        _ => "未知错误",
    };

    /// <summary>关系 → 列表行文案（状态不依赖颜色，始终有文字与计数）。</summary>
    public static string RelationRow(RepoStatusView row) => row.ReadState switch
    {
        ReadState.Pending => "尚未读取",
        ReadState.Error => "读取失败：" + For(row.ErrorCode),
        _ => row.Relation switch
        {
            Relation.Synced => "已同步",
            Relation.LocalAhead => $"本地领先 {row.Ahead} 个提交",
            Relation.RemoteAhead => $"远端领先 {row.Behind} 个提交",
            Relation.Diverged => $"已分叉（本地 +{row.Ahead} / 远端 +{row.Behind}）",
            Relation.NoRemote => NoComparisonLabel(row) + "：" + NoComparisonHint(row),
            _ => "未知",
        },
    };

    public static string NoComparisonLabel(RepoStatusView row) =>
        row.HasOrigin == false ? "无远端" : "未比较";

    public static string NoComparisonHint(RepoStatusView row) => row switch
    {
        { HasOrigin: false } => "未配置 origin 远端",
        { HasCommit: false } => "当前分支尚无提交",
        { Branch: "HEAD" } => "当前处于分离 HEAD 状态",
        { HasOrigin: true } => "没有可比较的 origin 分支",
        _ => "没有可用的远端比较结果",
    };

    public static string RefreshStateText(AppSnapshotView snapshot) => snapshot.RefreshState switch
    {
        RefreshState.Pending => "首次读取进行中",
        RefreshState.Ready => "读取完成",
        RefreshState.Failed => "读取失败：" + For(snapshot.RefreshErrorCode),
        _ => "",
    };
}

// 视图行轻量投影：把契约行转为绑定友好形状（RelationRow 等使用）。
public record RepoStatusView(
    string Id,
    string Name,
    string Branch,
    ReadState ReadState,
    Relation? Relation,
    uint? Ahead,
    uint? Behind,
    string? ErrorCode)
{
    public bool? HasOrigin { get; init; }
    public bool? HasCommit { get; init; }

    public static RepoStatusView From(RepoStatus row) => new(
        row.Id, row.Name, row.Branch ?? "", row.ReadState, row.Relation,
        row.Ahead, row.Behind, row.Error?.Code)
    {
        HasOrigin = row.HasOrigin,
        HasCommit = row.HasCommit,
    };
}

public record AppSnapshotView(
    RefreshState RefreshState,
    string? RefreshErrorCode)
{
    public static AppSnapshotView From(AppSnapshot snapshot) => new(
        snapshot.RefreshState, snapshot.RefreshError?.Code);
}
