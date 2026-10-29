using GitaView.Native.Contracts;
using GitaView_Native;

namespace GitaView.Native.Tests;

public class ComparisonPresentationFacts
{
    [Theory]
    [InlineData(false, true, "main", "无远端", "未配置 origin 远端")]
    [InlineData(true, true, "native-app", "未比较", "没有可比较的 origin 分支")]
    [InlineData(true, false, "main", "未比较", "当前分支尚无提交")]
    [InlineData(true, true, "HEAD", "未比较", "当前处于分离 HEAD 状态")]
    [InlineData(null, null, "main", "未比较", "没有可用的远端比较结果")]
    public void distinguishes_missing_origin_from_missing_comparison(
        bool? hasOrigin, bool? hasCommit, string branch, string label, string reason)
    {
        var row = RepoStatusView.From(new RepoStatus
        {
            Id = "repo", Name = "test", Path = "test", Branch = branch,
            ReadState = ReadState.Ok, Relation = Relation.NoRemote,
            HasOrigin = hasOrigin, HasCommit = hasCommit,
            Actions = new RepoActions
            {
                CanOpenDirectory = true, CanOpenRemote = hasOrigin == true,
                CanFetch = hasOrigin == true, CanPull = false, CanPush = false,
            },
        });
        Assert.Equal(label, ErrorText.NoComparisonLabel(row));
        Assert.Equal(reason, ErrorText.NoComparisonHint(row));
        Assert.StartsWith(label, ErrorText.RelationRow(row));
    }
}
