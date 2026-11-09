// EngineService 集成测试：双 isolate 协议 + 真实引擎 + 真实 Git 仓库。
// 对应 C# EngineConcurrencyFacts / App.xaml.cs 的服务层行为。
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:gitaview/contracts/contracts.dart';
import 'package:gitaview/engine/engine_service.dart';
import 'package:gitaview/ffi/gv_ffi.dart';

String? _resolveLibraryPath() {
  final candidates = <String>[
    if (Platform.environment['GV_FFI_LIB'] != null) Platform.environment['GV_FFI_LIB']!,
    '${Directory.current.path}/../../target/release/libgitaview_ffi.dylib',
    '${Directory.current.path}/../../target/debug/libgitaview_ffi.dylib',
    '${Directory.current.path}/../../target/release/gitaview_ffi.dll',
    '${Directory.current.path}/../../target/debug/gitaview_ffi.dll',
  ];
  for (final path in candidates) {
    if (File(path).existsSync()) return path;
  }
  return null;
}

Future<String> _createUnbornRepo(String root, String name) async {
  final repo = Directory('$root/$name')..createSync(recursive: true);
  Future<void> git(List<String> args) async {
    final result = await Process.run('git', args, workingDirectory: repo.path);
    if (result.exitCode != 0) fail('git ${args.join(" ")} 失败: ${result.stderr}');
  }

  await git(['init', '-q', '-b', 'main']);
  await git(['remote', 'add', 'origin', 'https://example.com/$name.git']);
  return repo.path;
}

void main() {
  late final String libraryPath;

  setUpAll(() {
    final resolved = _resolveLibraryPath();
    if (resolved == null) {
      fail('未找到 gitaview_ffi 动态库；先运行 cargo build -p gitaview-ffi --release');
    }
    libraryPath = resolved;
  });

  test('启动 → watcher 推送 → 设置事务 → 仓库全链路 → 干净退出', () async {
    final base = Directory.systemTemp.createTempSync('gitaview_service');
    addTearDown(() {
      try {
        base.deleteSync(recursive: true);
      } on FileSystemException {
        // isolate 完全退出前目录可能暂被占用。
      }
    });
    final workspace = Directory.systemTemp.createTempSync('gitaview_service_repos');
    addTearDown(() {
      try {
        workspace.deleteSync(recursive: true);
      } on FileSystemException {
        // 同上。
      }
    });

    final service = EngineService(
      dataDirectory: '${base.path}/data',
      logDirectory: '${base.path}/log',
      libraryPath: libraryPath,
    );
    addTearDown(service.dispose);

    await service.start();
    expect(service.state, EngineServiceState.running);
    expect(service.createData?.abiVersion, gvAbiVersion);

    // watcher 在数秒内推送首份快照（generation 0 未就绪也推送）。
    final deadline = DateTime.now().add(const Duration(seconds: 10));
    while (service.snapshot == null) {
      if (DateTime.now().isAfter(deadline)) fail('watcher 未在期限内推送快照');
      await Future<void>.delayed(const Duration(milliseconds: 50));
    }
    expect(service.snapshot!.repos, isEmpty);

    // 设置事务。
    final saved = await service.settingsSave(SettingsSaveInput(
      expectedRevision: 1,
      settings: SettingsEditable(
        groups: [const GroupSettings(id: 'g1', name: '工作')],
        refresh: const RefreshPolicy(enabled: true, intervalMinutes: 5),
      ),
    ));
    expect(saved.revision, 2);
    await expectLater(
      service.settingsSave(SettingsSaveInput(
        expectedRevision: 1,
        settings: SettingsEditable(
          groups: const [],
          refresh: const RefreshPolicy(enabled: true, intervalMinutes: 5),
        ),
      )),
      throwsA(isA<GvCallException>()
          .having((error) => error.errorCode, 'errorCode', 'SETTINGS_CONFLICT')),
    );

    // 仓库添加 + 刷新完成 + watcher 推送新快照。
    final repoPath = await _createUnbornRepo(workspace.path, 'service-demo');
    final added = await service.repoAdd(path: repoPath, groupId: 'g1');
    expect(added.repo.groupId, 'g1');

    final refreshed = await service.refreshAndWaitComplete(timeoutMs: 30000);
    expect(refreshed.repos.single.relation, Relation.noRemote);

    // 视图与 Summary。
    final view = await service.viewQuery(const ViewQuery(groupId: 'g1'));
    expect(view.rows, hasLength(1));
    expect(view.effective.groupId, 'g1');
    final summary = await service.summarySnapshot();
    expect(summary.counts.noRemote, 1);
    expect(summary.counts.closesOverTotal, isTrue);

    // 别名（repoAliases）经双 isolate 管道往返：保存 → 快照行 / 视图行 /
    // Summary 关注行都携带别名（管道 toJson 曾丢失该字段，回归锁定）。
    final withAlias = await service.settingsSave(SettingsSaveInput(
      expectedRevision: saved.revision + 1,
      settings: SettingsEditable(
        groups: [const GroupSettings(id: 'g1', name: '工作')],
        refresh: const RefreshPolicy(enabled: true, intervalMinutes: 5),
        repoAliases: [
          RepoAliasAssignment(repoId: added.repo.id, alias: '管道别名'),
        ],
      ),
    ));
    expect(
        withAlias.repos.single.alias, '管道别名', reason: 'settingsGet 管道');
    final snapAfterAlias = await service.snapshotGet();
    expect(snapAfterAlias.repos.single.alias, '管道别名',
        reason: 'snapshot 管道（曾丢失 alias 的回归点）');
    final viewAfterAlias =
        await service.viewQuery(const ViewQuery(groupId: 'g1'));
    expect(viewAfterAlias.rows.single.alias, '管道别名', reason: '视图行管道');
    // Summary 行显示名 = 别名优先由核心投影（Rust 单测
    // attention_rows_prefer_alias_over_name 覆盖）；本仓库 no_remote
    // 不进关注列表，这里只断言 counts 正常。

    // 未确认 Pull 被拒。
    await expectLater(
      service.repoPull(added.repo.id, confirmed: false),
      throwsA(isA<GvCallException>()
          .having((error) => error.errorCode, 'errorCode', 'CONFIRMATION_REQUIRED')),
    );

    // 干净退出：状态序列 running → stopping → stopped。
    await service.requestExit();
    expect(service.state, EngineServiceState.stopped);
  });

  test('启动失败（非法目录）进入 failed 并保留错误；修复后可重试', () async {
    final service = EngineService(
      dataDirectory: 'relative/not/absolute',
      logDirectory: '${Directory.systemTemp.path}/gitaview-log',
      libraryPath: libraryPath,
    );
    addTearDown(service.dispose);
    await service.start();
    expect(service.state, EngineServiceState.failed);
    expect(service.startupError, isNotNull);

    final base = Directory.systemTemp.createTempSync('gitaview_retry');
    addTearDown(() {
      try {
        base.deleteSync(recursive: true);
      } on FileSystemException {
        // 同上。
      }
    });
    final retry = EngineService(
      dataDirectory: '${base.path}/data',
      logDirectory: '${base.path}/log',
      libraryPath: libraryPath,
    );
    addTearDown(retry.dispose);
    await retry.start();
    expect(retry.state, EngineServiceState.running);
    await retry.requestExit();
    expect(retry.state, EngineServiceState.stopped);
  });
}
