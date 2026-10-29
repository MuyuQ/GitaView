using GitaView_Native;

namespace GitaView.Native.Tests;

public class SnapshotVersionGateFacts
{
    [Fact]
    public async Task older_query_finishing_last_cannot_replace_newer_content()
    {
        var gate = new SnapshotVersionGate();
        gate.BeginSession("s");
        var releaseOld = new TaskCompletionSource<ulong>(TaskCreationOptions.RunContinuationsAsynchronously);
        var oldQuery = releaseOld.Task;
        ulong displayed = 0;
        if (gate.TryAccept("s", 12)) displayed = 12;
        releaseOld.SetResult(11);
        var oldRevision = await oldQuery;
        if (gate.TryAccept("s", oldRevision)) displayed = oldRevision;
        Assert.Equal(12ul, displayed);
        gate.BeginSession("s");
        Assert.False(gate.TryAccept("s", 11));
        Assert.True(gate.TryAccept("s", 12));
    }

    [Fact]
    public void new_session_accepts_its_initial_revision_and_rejects_late_old_session_results()
    {
        var gate = new SnapshotVersionGate();
        gate.BeginSession("old");
        Assert.True(gate.TryAccept("old", 100));
        gate.BeginSession("new");
        Assert.True(gate.TryAccept("new", 0));
        Assert.False(gate.TryAccept("old", 101));
        Assert.True(gate.TryAccept("new", 1));
    }
}
