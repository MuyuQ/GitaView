// FFI 层测试：加载真实 gitaview_ffi 动态库（macOS 上为 target/release 的
// dylib），走完整引擎生命周期与核心调用面，对应 Rust tests/abi.rs 的覆盖。
//
// Windows（gitaview_ffi.dll）由 CI / 实机执行；本文件按平台解析库名，
// 找不到库时整组失败并给出明确原因，不伪造结果。
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:gitaview/contracts/contracts.dart';
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

/// 独立临时目录（createTempSync 自带随机后缀，天然不冲突）。
Directory _tempDir(String tag) {
  final base = Directory.systemTemp.createTempSync('gitaview_ffi_$tag');
  addTearDown(() {
    try {
      base.deleteSync(recursive: true);
    } on FileSystemException {
      // Windows 上引擎句柄完全释放前目录可能仍被占用；macOS/Linux 不走此分支。
    }
  });
  return base;
}

/// 建一个带 origin 的未出生仓库（无首提交）：确定性映射为 no_remote 且可 Fetch。
Future<String> _createUnbornRepo(String root, String name) async {
  final repo = Directory('$root/$name')..createSync(recursive: true);
  await _git(repo.path, ['init', '-q', '-b', 'main']);
  await _git(repo.path, ['remote', 'add', 'origin', 'https://example.com/$name.git']);
  return repo.path;
}

Future<void> _git(String cwd, List<String> args) async {
  final result = await Process.run('git', args, workingDirectory: cwd);
  if (result.exitCode != 0) {
    fail('git ${args.join(" ")} 失败: ${result.stderr}');
  }
}

/// 创建引擎并注册清理：退出前 stop → wait → destroy。
GvEngine _createEngine(GvNative native, Directory base) {
  final engine = GvEngine.create(
    native,
    dataDirectory: '${base.path}/data',
    logDirectory: '${base.path}/log',
  );
  addTearDown(() {
    if (!engine.isDestroyed) {
      engine.requestStop();
      engine.waitStopped(timeoutMs: 10000);
      engine.destroy();
    }
  });
  return engine;
}

void main() {
  late final GvNative native;
  late final String libraryPath;

  setUpAll(() {
    final resolved = _resolveLibraryPath();
    if (resolved == null) {
      fail('未找到 gitaview_ffi 动态库；先运行 cargo build -p gitaview-ffi --release');
    }
    libraryPath = resolved;
    native = GvNative.load(path: libraryPath);
  });

  test('ABI 版本为 2', () {
    expect(native.abiVersion(), gvAbiVersion);
    expect(gvAbiVersion, 2);
  });

  test('引擎创建返回会话与默认设置；销毁后不可再用', () {
    final base = _tempDir('create');
    final engine = _createEngine(native, base);

    expect(engine.createData, isNotNull);
    expect(engine.createData!.abiVersion, gvAbiVersion);
    expect(engine.createData!.sessionId, isNotEmpty);
    expect(engine.createData!.startupWarnings, isEmpty);

    final settings = engine.settingsGet();
    expect(settings.revision, 1);
    expect(settings.repos, isEmpty);
    expect(settings.groups, isEmpty);
    expect(settings.refresh.enabled, isTrue);
    expect(settings.refresh.intervalMinutes, 5);

    engine.requestStop();
    expect(engine.waitStopped(timeoutMs: 10000), isTrue);
    engine.destroy();
    expect(engine.isDestroyed, isTrue);
    expect(() => engine.settingsGet(), throwsA(isA<StateError>()));
  });

  test('settings_save 保存可编辑字段并拒绝过期 revision', () {
    final base = _tempDir('settings');
    final engine = _createEngine(native, base);

    final saved = engine.settingsSave(SettingsSaveInput(
      expectedRevision: 1,
      settings: SettingsEditable(
        groups: [const GroupSettings(id: 'g1', name: '工作')],
        defaultGroupId: 'g1',
        refresh: const RefreshPolicy(enabled: true, intervalMinutes: 15),
      ),
    ));
    expect(saved.revision, 2);
    expect(saved.groups.single.id, 'g1');
    expect(saved.defaultGroupId, 'g1');
    expect(saved.refresh.intervalMinutes, 15);

    // 过期 revision → SETTINGS_CONFLICT。
    expect(
      () => engine.settingsSave(SettingsSaveInput(
        expectedRevision: 1,
        settings: SettingsEditable(
          groups: const [],
          refresh: const RefreshPolicy(enabled: false, intervalMinutes: 5),
        ),
      )),
      throwsA(
        isA<GvCallException>().having(
          (error) => error.errorCode,
          'errorCode',
          'SETTINGS_CONFLICT',
        ),
      ),
    );
  });

  test('非法数据目录路径被拒绝（INVALID_ARGUMENT）', () {
    expect(
      () => GvEngine.create(
        native,
        dataDirectory: 'relative/not/absolute',
        logDirectory: '${Directory.systemTemp.path}/gitaview-log',
      ),
      throwsA(
        isA<GvCallException>()
            .having((error) => error.status, 'status', GvStatus.invalidArgument),
      ),
    );
  });

  test('仓库添加 / 刷新等待 / 快照 / 视图 / Summary / 移除全链路', () async {
    final base = _tempDir('roundtrip');
    final workspace = _tempDir('repos');
    final repoPath = await _createUnbornRepo(workspace.path, 'flutter-ffi-demo');
    final engine = _createEngine(native, base);

    // 添加：返回仓库记录；重复添加返回同一条记录。
    final added = engine.repoAdd(path: repoPath);
    expect(added.repo.name, 'flutter-ffi-demo');
    expect(added.settingsRevision, 2);
    final duplicated = engine.repoAdd(path: repoPath);
    expect(duplicated.repo.id, added.repo.id);

    // 手动刷新请求 + 等待完成（snapshotWait 阻塞至变化或期限）。
    final request = engine.refreshRequest();
    expect(request.sessionId, engine.createData!.sessionId);
    var snapshot = engine.snapshotGet();
    final deadline = DateTime.now().add(const Duration(seconds: 20));
    while (snapshot.refresh.completedRequestId < request.requestId) {
      if (DateTime.now().isAfter(deadline)) {
        fail('刷新未在期限内完成（completedRequestId=${snapshot.refresh.completedRequestId}）');
      }
      final (result, status) = engine.snapshotWait(
        sessionId: snapshot.sessionId,
        stateRevision: snapshot.stateRevision,
        timeoutMs: 2000,
      );
      expect(status, anyOf(GvStatus.ok, GvStatus.timeout, GvStatus.stopping),
          reason: '等待返回状态合法');
      if (status == GvStatus.ok && result.ok) {
        snapshot = result.expectData((json) => AppSnapshot.decode(json, 'snapshotWait'));
      }
    }

    // 未出生仓库 + origin：no_remote，网络入口按规则表开放 / 禁用。
    expect(snapshot.repos, hasLength(1));
    final row = snapshot.repos.single;
    expect(row.readState, ReadState.ok);
    expect(row.relation, Relation.noRemote);
    expect(row.hasOrigin, isTrue);
    expect(row.actions.canFetch, isTrue);
    expect(row.actions.canPull, isFalse);
    expect(row.actions.canPush, isFalse);
    expect(row.actions.canOpenDirectory, isTrue);

    // 视图查询：no_remote 筛选命中；计数闭合；有效筛选回显。
    final view = engine.viewQuery(const ViewQuery(relationFilter: RelationFilter.noRemote));
    expect(view.rows, hasLength(1));
    expect(view.summaryCounts.total, 1);
    expect(view.statusCounts.noRemote, 1);
    expect(view.summaryCounts.closesOverTotal, isTrue);
    expect(view.effective.relationFilter, RelationFilter.noRemote);

    // Summary 投影：no_remote 不进入关注列表；DTO 结构上不含 path 字段。
    final summary = engine.summarySnapshot();
    expect(summary.counts.total, 1);
    expect(summary.counts.noRemote, 1);
    expect(summary.counts.closesOverTotal, isTrue);
    expect(summary.attention, isEmpty);
    expect(summary.refreshPolicy.enabled, isTrue);

    // 移除：设置前移，快照不再包含该仓库。
    final removed = engine.repoRemove(added.repo.id);
    expect(removed.settingsRevision, greaterThan(2));
    expect(engine.snapshotGet().repos, isEmpty);
  });

  test('Pull 缺少确认在启动 Git 前被拒绝（CONFIRMATION_REQUIRED）', () async {
    final base = _tempDir('confirm');
    final workspace = _tempDir('repos-confirm');
    final repoPath = await _createUnbornRepo(workspace.path, 'confirm-demo');
    final engine = _createEngine(native, base);

    final added = engine.repoAdd(path: repoPath);
    // 确认校验先于状态校验与 Git 执行：false 恒定返回 CONFIRMATION_REQUIRED。
    expect(
      () => engine.repoPull(added.repo.id, confirmed: false),
      throwsA(
        isA<GvCallException>().having(
          (error) => error.errorCode,
          'errorCode',
          'CONFIRMATION_REQUIRED',
        ),
      ),
    );
    expect(
      () => engine.repoPush(added.repo.id, confirmed: false),
      throwsA(
        isA<GvCallException>().having(
          (error) => error.errorCode,
          'errorCode',
          'CONFIRMATION_REQUIRED',
        ),
      ),
    );
  });

  test('快照等待超时返回 TIMEOUT 且不携带 envelope', () async {
    final base = _tempDir('wait');
    final engine = _createEngine(native, base);

    // 静置：等初始扫描完成（CI 慢机上扫描完成可能落在等待窗口内，
    // 先归位 revision 再测超时，消除竞态）。
    var snapshot = engine.snapshotGet();
    final deadline = DateTime.now().add(const Duration(seconds: 20));
    while (snapshot.refreshState == RefreshState.pending ||
        snapshot.generation == 0) {
      if (DateTime.now().isAfter(deadline)) fail('初始扫描未在期限内完成');
      await Future<void>.delayed(const Duration(milliseconds: 100));
      snapshot = engine.snapshotGet();
    }

    final (result, status) = engine.snapshotWait(
      sessionId: snapshot.sessionId,
      stateRevision: snapshot.stateRevision,
      timeoutMs: 300,
    );
    expect(status, GvStatus.timeout);
    expect(result.ok, isFalse);
  });

  test('group 筛选：添加归属后按组计数', () async {
    final base = _tempDir('groups');
    final workspace = _tempDir('repos-groups');
    final repoPath = await _createUnbornRepo(workspace.path, 'grouped-demo');
    final engine = _createEngine(native, base);

    final saved = engine.settingsSave(SettingsSaveInput(
      expectedRevision: 1,
      settings: SettingsEditable(
        groups: [const GroupSettings(id: 'g1', name: '工作')],
        refresh: const RefreshPolicy(enabled: true, intervalMinutes: 5),
      ),
    ));
    expect(saved.revision, 2);

    final added = engine.repoAdd(path: repoPath, groupId: 'g1');
    expect(added.repo.groupId, 'g1');
    expect(added.settingsRevision, 3);

    final view = engine.viewQuery(const ViewQuery(groupId: 'g1'));
    expect(view.groups.single, isA<GroupCount>());
    expect(view.groups.single.count, 1);
    expect(view.effective.groupId, 'g1');

    // 所选组消失 → 有效筛选协调回全部。
    engine.settingsSave(SettingsSaveInput(
      expectedRevision: 3,
      settings: SettingsEditable(
        groups: const [],
        refresh: const RefreshPolicy(enabled: true, intervalMinutes: 5),
      ),
    ));
    final fallback = engine.viewQuery(const ViewQuery(groupId: 'g1'));
    expect(fallback.effective.groupId, isNull);
  });
}
