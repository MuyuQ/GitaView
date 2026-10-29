// N10.Windows 测试：真实 FFI 生命周期 + 契约 fixture 解码。
// 测试进程加载真实 gitaview_ffi.dll（csproj 构建目标自动 cargo build + 复制）。

using System.Diagnostics;
using System.Text.Json;
using GitaView.Native.Contracts;
using GitaView.Native.Interop;

namespace GitaView.Native.Tests;

public static class TestEnvironment
{
    /// <summary>从测试输出目录向上查找仓库根（含 contracts/ 与 Cargo.toml）。</summary>
    public static string FindRepoRoot()
    {
        var dir = new DirectoryInfo(AppContext.BaseDirectory);
        while (dir is not null)
        {
            if (File.Exists(Path.Combine(dir.FullName, "Cargo.toml"))
                && Directory.Exists(Path.Combine(dir.FullName, "contracts")))
            {
                return dir.FullName;
            }
            dir = dir.Parent!;
        }
        throw new InvalidOperationException("repo root not found from " + AppContext.BaseDirectory);
    }

    public static string FixturesDir() => Path.Combine(FindRepoRoot(), "contracts", "native", "v1", "fixtures");

    public static string TempDir(string name)
    {
        var path = Path.Combine(Path.GetTempPath(), name + "_" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(path);
        return path;
    }

    public static void RunGit(string workingDir, params string[] args)
    {
        var info = new ProcessStartInfo("git")
        {
            WorkingDirectory = workingDir,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
            UseShellExecute = false,
        };
        foreach (var arg in args)
        {
            info.ArgumentList.Add(arg);
        }
        using var process = Process.Start(info)!;
        string stderr = process.StandardError.ReadToEnd();
        process.WaitForExit();
        Assert.True(process.ExitCode == 0, $"git {string.Join(' ', args)} failed: {stderr}");
    }

    public static GitaViewEngine CreateEngine(string dataDir) =>
        GitaViewEngine.CreateEngine(new EngineCreateInput
        {
            DataDirectory = dataDir,
            LogDirectory = Path.Combine(dataDir, "logs"),
        });

    public static AppSnapshot PollUntil(Func<AppSnapshot> get, Func<AppSnapshot, bool> check, string what)
    {
        var deadline = DateTime.UtcNow + TimeSpan.FromSeconds(10);
        AppSnapshot last = get();
        while (!check(last))
        {
            Assert.True(DateTime.UtcNow < deadline, $"timeout waiting for: {what}");
            Thread.Sleep(20);
            last = get();
        }
        return last;
    }
}

public class AbiFacts
{
    [Fact]
    public void abi_version_is_one()
    {
        Assert.Equal(1u, GitaViewEngine.AbiVersion());
    }
}

public class EngineLifecycleFacts
{
    [Fact]
    public void create_returns_session_and_startup_defaults()
    {
        string dir = TestEnvironment.TempDir("gitaview_n10_create");
        using var engine = TestEnvironment.CreateEngine(dir);
        Assert.False(string.IsNullOrEmpty(engine.SessionId));
        Assert.Equal(1u, engine.CreateData.AbiVersion);
        Assert.Empty(engine.CreateData.StartupWarnings);

        var settings = engine.SettingsGet();
        Assert.Equal(1ul, settings.Revision);
        Assert.Empty(settings.Repos);
        Assert.Null(settings.DefaultGroupId);
        Assert.True(settings.Refresh.Enabled);
        Assert.Equal(5u, settings.Refresh.IntervalMinutes);
    }

    [Fact]
    public void create_rejects_relative_data_directory()
    {
        var ex = Assert.Throws<GvException>(() =>
            GitaViewEngine.CreateEngine(new EngineCreateInput
            {
                DataDirectory = "relative/dir",
                LogDirectory = "logs",
            }));
        Assert.Equal(GvStatus.InvalidArgument, ex.Status);
    }

    [Fact]
    public void settings_save_conflict_maps_to_error_with_code()
    {
        string dir = TestEnvironment.TempDir("gitaview_n10_conflict");
        using var engine = TestEnvironment.CreateEngine(dir);

        var saved = engine.SettingsSave(new SettingsSaveInput
        {
            ExpectedRevision = 1,
            Settings = new SettingsEditable
            {
                Groups = [new GroupSettings { Id = "g1", Name = "后端" }],
                Refresh = new RefreshPolicy { Enabled = true, IntervalMinutes = 10 },
            },
        });
        Assert.Equal(2ul, saved.Revision);

        var ex = Assert.Throws<GvException>(() =>
            engine.SettingsSave(new SettingsSaveInput
            {
                ExpectedRevision = 1,
                Settings = new SettingsEditable
                {
                    Groups = [],
                    Refresh = new RefreshPolicy { Enabled = true, IntervalMinutes = 5 },
                },
            }));
        Assert.Equal(GvStatus.Error, ex.Status);
        Assert.Equal("SETTINGS_CONFLICT", ex.Code);
    }

    [Fact]
    public void dispose_requires_stopped_and_is_idempotent()
    {
        string dir = TestEnvironment.TempDir("gitaview_n10_dispose");
        var engine = TestEnvironment.CreateEngine(dir);

        var stopping = engine.RequestStop();
        Assert.Equal(LifecycleDto.Stopping, stopping.Lifecycle);

        Assert.True(engine.WaitStopped(30_000));
        engine.Dispose();
        // 二次 Dispose 是 no-op。
        engine.Dispose();

        // 已销毁引擎上的调用 → ObjectDisposedException（不在 finalizer 等待网络）。
        Assert.Throws<ObjectDisposedException>(() => engine.SnapshotGet());
    }

    [Fact]
    public void wait_stopped_without_stop_times_out()
    {
        string dir = TestEnvironment.TempDir("gitaview_n10_wait");
        using var engine = TestEnvironment.CreateEngine(dir);
        Assert.False(engine.WaitStopped(150));
    }
}

public class RepoWorkflowFacts
{
    [Fact]
    public void repo_add_snapshot_wait_view_and_widget_roundtrip()
    {
        string dir = TestEnvironment.TempDir("gitaview_n10_workflow");
        using var engine = TestEnvironment.CreateEngine(dir);

        // 真实 Git 仓库（含中文路径与 https origin）。
        string repoPath = Path.Combine(dir, "仓库-示例");
        Directory.CreateDirectory(repoPath);
        TestEnvironment.RunGit(repoPath, "init", "-b", "main");
        TestEnvironment.RunGit(repoPath, "config", "user.email", "test@example.test");
        TestEnvironment.RunGit(repoPath, "config", "user.name", "Test");
        TestEnvironment.RunGit(repoPath, "commit", "--allow-empty", "-m", "初始提交");
        TestEnvironment.RunGit(repoPath, "remote", "add", "origin", "https://github.com/example/x.git");

        var added = engine.RepoAdd(new RepoAddInput { Path = repoPath });
        Assert.Equal("repo", added.Repo.Id); // 中文目录名折叠为 ASCII 机器 ID
        Assert.Equal(2ul, added.SettingsRevision);

        // 刷新请求返回 requestId；完成与否看 completedRequestId。
        var initial = engine.SnapshotGet();
        var request = engine.RefreshRequest();
        Assert.Equal(engine.SessionId, request.SessionId);

        // 等待读取完成：以 stateRevision 等待，而不是轮询猜。
        var waited = engine.SnapshotWait(initial.StateRevision, 10_000);
        Assert.Equal(SnapshotWaitKind.Ready, waited.Kind);

        var ready = TestEnvironment.PollUntil(
            engine.SnapshotGet,
            snapshot => snapshot.Refresh.CompletedRequestId >= request.RequestId
                && snapshot.Repos.Count == 1 && snapshot.Repos[0].ReadState == ReadState.Ok,
            "repo read completes");
        var row = ready.Repos[0];
        Assert.Equal(ReadState.Ok, row.ReadState);
        Assert.Equal(Relation.NoRemote, row.Relation);
        Assert.Equal("main", row.Branch);
        Assert.True(row.HasOrigin);
        Assert.True(row.Actions.CanFetch);
        Assert.False(row.Actions.CanPull);

        // 视图查询与快照共享同一 stateRevision 语义。
        var view = engine.ViewQuery(new ViewQuery());
        Assert.Equal(ready.StateRevision, view.StateRevision);
        Assert.Single(view.Rows);
        Assert.Equal(1u, view.SummaryCounts.Total);
        Assert.Equal(1u, view.SummaryCounts.NoRemote);

        // Widget 投影：无 path、无远端 URL；计数与摘要一致。
        var widget = engine.WidgetSnapshot();
        Assert.Equal(1u, widget.Counts.Total);
        Assert.Empty(widget.Attention);
        string widgetJson = ContractsJson.Encode(widget);
        Assert.DoesNotContain("\"path\"", widgetJson);
        Assert.DoesNotContain("github.com", widgetJson);

        // 打开目标：remote 返回规范化 URL。
        var target = engine.RepoTarget(new RepoTargetInput { RepoId = added.Repo.Id, Kind = RepoTargetKind.Remote });
        Assert.Equal("https://github.com/example/x", target.Value);
    }

    [Fact]
    public void pull_requires_confirmation_and_origin()
    {
        string dir = TestEnvironment.TempDir("gitaview_n10_pull");
        using var engine = TestEnvironment.CreateEngine(dir);

        string repoPath = Path.Combine(dir, "repo");
        Directory.CreateDirectory(repoPath);
        TestEnvironment.RunGit(repoPath, "init", "-b", "main");
        TestEnvironment.RunGit(repoPath, "config", "user.email", "test@example.test");
        TestEnvironment.RunGit(repoPath, "config", "user.name", "Test");
        TestEnvironment.RunGit(repoPath, "commit", "--allow-empty", "-m", "initial");
        engine.RepoAdd(new RepoAddInput { Path = repoPath });

        var unconfirmed = Assert.Throws<GvException>(() => engine.RepoPull("repo", confirmed: false));
        Assert.Equal(GvStatus.Error, unconfirmed.Status);
        Assert.Equal("CONFIRMATION_REQUIRED", unconfirmed.Code);

        var noOrigin = Assert.Throws<GvException>(() => engine.RepoPull("repo", confirmed: true));
        Assert.Equal("ORIGIN_REQUIRED", noOrigin.Code);

        var missing = Assert.Throws<GvException>(() => engine.RepoFetch("missing-repo"));
        Assert.Equal("REPO_NOT_FOUND", missing.Code);
    }

    [Fact]
    public void repo_remove_uses_raw_utf8_id_bytes()
    {
        string dir = TestEnvironment.TempDir("gitaview_n10_remove");
        using var engine = TestEnvironment.CreateEngine(dir);

        string repoPath = Path.Combine(dir, "repo");
        Directory.CreateDirectory(Path.Combine(repoPath, ".git"));
        engine.RepoAdd(new RepoAddInput { Path = repoPath });

        var removed = engine.RepoRemove("repo");
        Assert.Equal(3ul, removed.SettingsRevision);

        var ex = Assert.Throws<GvException>(() => engine.RepoRemove("repo"));
        Assert.Equal("REPO_NOT_FOUND", ex.Code);
    }
}

public class SnapshotWaitFacts
{
    [Fact]
    public void timeout_and_stopping_are_distinguished_without_payload()
    {
        string dir = TestEnvironment.TempDir("gitaview_n10_wait_kinds");
        var engine = TestEnvironment.CreateEngine(dir);

        // 极高 cursor：无变化可等 → Timeout。
        var timeout = engine.SnapshotWait(ulong.MaxValue, 150);
        Assert.Equal(SnapshotWaitKind.Timeout, timeout.Kind);
        Assert.Null(timeout.Snapshot);

        // 停止 → Stopping；等待者被唤醒。
        engine.RequestStop();
        var stopping = engine.SnapshotWait(ulong.MaxValue, 30_000);
        Assert.Equal(SnapshotWaitKind.Stopping, stopping.Kind);

        Assert.True(engine.WaitStopped(30_000));
        engine.Dispose();
    }

    [Fact]
    public void foreign_session_returns_current_snapshot_immediately()
    {
        string dir = TestEnvironment.TempDir("gitaview_n10_foreign");
        using var engine = TestEnvironment.CreateEngine(dir);
        var foreign = engine.SnapshotWait("different-session", 0, 30_000);
        Assert.Equal(SnapshotWaitKind.Ready, foreign.Kind);
        Assert.NotNull(foreign.Snapshot);
    }

    [Fact]
    public void scan_directory_reports_truncated_false_for_small_tree()
    {
        string dir = TestEnvironment.TempDir("gitaview_n10_scan");
        using var engine = TestEnvironment.CreateEngine(dir);
        Directory.CreateDirectory(Path.Combine(dir, "candidate", ".git"));
        var scan = engine.ScanDirectory(dir);
        Assert.False(scan.Truncated);
        Assert.Single(scan.Paths);

        var bad = Assert.Throws<GvException>(() => engine.ScanDirectory("Z:/definitely/missing"));
        Assert.Equal(GvStatus.Error, bad.Status);
        Assert.Equal("INVALID_PATH", bad.Code);
    }
}

public class ContractFixtureFacts
{
    public static IEnumerable<object[]> SnapshotFixtures()
    {
        foreach (var file in Directory.GetFiles(TestEnvironment.FixturesDir(), "snapshot-*.json"))
        {
            yield return new object[] { Path.GetFileName(file) };
        }
    }

    [Theory]
    [MemberData(nameof(SnapshotFixtures))]
    public void all_snapshot_fixtures_decode_with_contract_rules(string fixture)
    {
        byte[] json = File.ReadAllBytes(Path.Combine(TestEnvironment.FixturesDir(), fixture));
        var snapshot = ContractsJson.Decode<AppSnapshot>(json);
        Assert.Equal(1ul, snapshot.SchemaVersion);
        Assert.False(string.IsNullOrEmpty(snapshot.SessionId));
    }

    [Fact]
    public void mixed8_fixture_decodes_expected_relations_and_times_as_milliseconds()
    {
        string path = Path.Combine(TestEnvironment.FixturesDir(), "snapshot-mixed-8.json");
        var snapshot = ContractsJson.Decode<AppSnapshot>(File.ReadAllBytes(path));

        Assert.Equal(8, snapshot.Repos.Count);
        Assert.Equal(2, snapshot.Repos.Count(r => r.Relation == Relation.Synced));
        Assert.Equal(1, snapshot.Repos.Count(r => r.Relation == Relation.LocalAhead));
        Assert.Equal(1, snapshot.Repos.Count(r => r.Relation == Relation.RemoteAhead));
        Assert.Equal(1, snapshot.Repos.Count(r => r.Relation == Relation.Diverged));
        Assert.Equal(2, snapshot.Repos.Count(r => r.Relation == Relation.NoRemote));
        Assert.Equal(1, snapshot.Repos.Count(r => r.ReadState == ReadState.Error));

        // 时间是 Unix 毫秒（数字），显式转换可用且不是 ISO 字符串。
        Assert.Equal(1_792_800_000_000ul, snapshot.AttemptedAtMs);
        Assert.Equal(1_792_799_700_000ul, snapshot.LastSuccessAtMs);
        var display = snapshot.LastSuccessAtMs!.Value.ToUnixDateTime();
        Assert.Equal(2026, display.Year);

        string raw = File.ReadAllText(path);
        Assert.DoesNotContain("toISOString", raw);
        Assert.Contains("\"lastSuccessAtMs\": 1792799700000", raw);
    }

    [Fact]
    public void unknown_schema_and_unknown_fields_are_rejected()
    {
        string unknownSchema = Path.Combine(TestEnvironment.FixturesDir(), "widget-unknown-schema.json");
        Assert.Throws<JsonException>(() =>
            ContractsJson.Decode<WidgetSnapshot>(File.ReadAllBytes(unknownSchema)));

        // 未知字段（非契约内容）必须拒绝，不能用默认值隐藏。
        Assert.Throws<JsonException>(() =>
            ContractsJson.Decode<AppSnapshot>("{\"schemaVersion\":1,\"legacy\":true}"));

        // 未知枚举值拒绝。
        Assert.Throws<JsonException>(() =>
            ContractsJson.Decode<AppSnapshot>(
                """
                {"schemaVersion":1,"sessionId":"s","generation":0,"stateRevision":0,
                 "settingsRevision":1,"attemptedAtMs":null,"lastSuccessAtMs":null,
                 "refreshState":"pending","refreshPolicy":{"enabled":true,"intervalMinutes":5},
                 "refresh":{"isRefreshing":false,"requestedRequestId":0,"completedRequestId":0},
                 "repos":[{"id":"x","name":"x","path":"/x","readState":"ok","relation":"conflicted",
                           "branch":"main","hasCommit":true,"hasOrigin":true,"ahead":0,"behind":0,
                           "actions":{"canOpenDirectory":true,"canOpenRemote":false,"canFetch":false,
                                      "canPull":false,"canPush":false}}]}
                """));
    }

    [Fact]
    public void pending_fixture_keeps_git_fields_null_and_actions_disabled()
    {
        string path = Path.Combine(TestEnvironment.FixturesDir(), "snapshot-pending-first.json");
        var snapshot = ContractsJson.Decode<AppSnapshot>(File.ReadAllBytes(path));
        Assert.All(snapshot.Repos, row =>
        {
            Assert.Equal(ReadState.Pending, row.ReadState);
            Assert.Null(row.Relation);
            Assert.Null(row.Branch);
            Assert.Null(row.Ahead);
            Assert.False(row.Actions.CanFetch);
            Assert.True(row.Actions.CanOpenDirectory);
        });
    }

    [Fact]
    public void expectations_counts_match_decoded_mixed8()
    {
        string expectationsPath = Path.Combine(TestEnvironment.FixturesDir(), "expectations.json");
        using var doc = JsonDocument.Parse(File.ReadAllText(expectationsPath));
        var expected = doc.RootElement.GetProperty("snapshots").GetProperty("snapshot-mixed-8").GetProperty("counts");

        string path = Path.Combine(TestEnvironment.FixturesDir(), "snapshot-mixed-8.json");
        var snapshot = ContractsJson.Decode<AppSnapshot>(File.ReadAllBytes(path));

        Assert.Equal((uint)expected.GetProperty("total").GetInt32(), (uint)snapshot.Repos.Count);
        Assert.Equal((uint)expected.GetProperty("synced").GetInt32(),
            (uint)snapshot.Repos.Count(r => r.Relation == Relation.Synced));
        Assert.Equal((uint)expected.GetProperty("error").GetInt32(),
            (uint)snapshot.Repos.Count(r => r.ReadState == ReadState.Error));
        Assert.Equal((uint)expected.GetProperty("noRemote").GetInt32(),
            (uint)snapshot.Repos.Count(r => r.Relation == Relation.NoRemote));
    }
}
