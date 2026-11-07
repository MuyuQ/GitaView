// 契约层测试：以 contracts/native/v1/fixtures 为唯一期望来源（与 Rust
// contract_fixtures.rs 消费同一组 fixture 与 expectations.json）。
//
// Dart 侧断言范围：
// - 全部 snapshot fixture 严格解码成功，关键字段与 expectations 一致；
// - 未知字段 / 未知 schema 版本 / 未知枚举 / 错误类型一律拒绝；
// - staleness 展示逻辑与 expectations 的边界数据一致；
// - envelope 与输入编码的键形状正确。
import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:gitaview/contracts/contracts.dart';

String _fixturesDir() {
  final candidates = [
    Directory.current.resolveSymbolicLinksSync(),
  ];
  for (final current in candidates) {
    final direct = Directory('$current/contracts/native/v1/fixtures');
    if (direct.existsSync()) return direct.path;
    // flutter test 以包根为 cwd；fixture 在仓库根的 contracts 下。
    final parent = Directory('$current/../../contracts/native/v1/fixtures');
    if (parent.existsSync()) {
      return parent.resolveSymbolicLinksSync();
    }
  }
  fail('未找到 contracts/native/v1/fixtures；cwd=${Directory.current.path}');
}

Map<String, Object?> _expectations() {
  final file = File('${_fixturesDir()}/expectations.json');
  return jsonDecode(file.readAsStringSync()) as Map<String, Object?>;
}

Object? _loadFixture(String name) {
  final file = File('${_fixturesDir()}/$name');
  return jsonDecode(file.readAsStringSync());
}

/// 深拷贝并注入未知字段，用于未知字段拒绝测试。
Map<String, Object?> _withUnknownKey(Object? source, String key, Object? value) {
  final copy = jsonDecode(jsonEncode(source)) as Map<String, Object?>;
  copy[key] = value;
  return copy;
}

void main() {
  test('全部 snapshot fixture 严格解码成功且与 expectations 关键字段一致', () {
    final expectations = _expectations();
    final snapshots = expectations['snapshots']! as Map<String, Object?>;
    expect(snapshots, isNotEmpty);

    snapshots.forEach((name, expectedRaw) {
      final expected = expectedRaw as Map<String, Object?>;
      final snapshot = AppSnapshot.decode(_loadFixture('$name.json'), name);

      expect(snapshot.schemaVersion, 1, reason: name);
      // 行数与 total 一致（total 由投影产生，这里核对行数一致可解码）。
      expect(snapshot.repos.length, (expected['counts'] as Map<String, Object?>)['total'],
          reason: name);

      // pending 行不发明 Git 字段：branch / ahead / behind 均为空。
      for (final repo in snapshot.repos.where((repo) => repo.readState == ReadState.pending)) {
        expect(repo.branch, isNull, reason: '$name/${repo.id}');
        expect(repo.ahead, isNull, reason: '$name/${repo.id}');
        expect(repo.behind, isNull, reason: '$name/${repo.id}');
        expect(repo.relation, isNull, reason: '$name/${repo.id}');
        expect(repo.actions.canFetch, isFalse, reason: '$name/${repo.id}');
      }

      // absentRepoIds：这些 ID 不得出现在快照行中（删除不复活）。
      for (final absent in (expected['absentRepoIds']! as List<Object?>).cast<String>()) {
        expect(snapshot.repos.any((repo) => repo.id == absent), isFalse,
            reason: '$name 不应包含 $absent');
      }

      // 全局失败保留历史：lastSuccessAtMs 与错误码按 expectations 固定。
      final preserved = expected['lastSuccessAtMsPreserved'] as int?;
      if (preserved != null) {
        expect(snapshot.lastSuccessAtMs, preserved, reason: name);
        expect(snapshot.refreshError?.code, expected['refreshErrorCode'], reason: name);
        expect(snapshot.refreshState, RefreshState.failed, reason: name);
      }
    });
  });

  test('未知字段被拒绝而非默认填充', () {
    final source = _loadFixture('snapshot-empty.json');
    // "widgetCounts" 是已随子系统更名退役的定义名：顶层出现即拒绝。
    expect(
      () => AppSnapshot.decode(_withUnknownKey(source, 'widgetCounts', 1), 'test'),
      throwsA(isA<ContractException>()),
    );
  });

  test('未知 schema 版本被拒绝（app 与 summary 两个入口）', () {
    final app = _withUnknownKey(_loadFixture('snapshot-empty.json'), 'schemaVersion', 2);
    expect(() => AppSnapshot.decode(app, 'test'), throwsA(isA<ContractException>()));

    final summary = _loadFixture('summary-unknown-schema.json');
    expect(() => SummarySnapshot.decode(summary, 'test'), throwsA(isA<ContractException>()),
        reason: 'rejects 列表的 summary-unknown-schema 必须被 Dart 解码拒绝');
  });

  test('未知枚举值与错误类型被拒绝', () {
    final source = _withUnknownKey(_loadFixture('snapshot-empty.json'), 'repos', <Object?>[
      {
        'id': 'r1',
        'name': 'r1',
        'path': '/tmp/r1',
        'readState': 'ok',
        'relation': 'error',
        'actions': {
          'canOpenDirectory': true,
          'canOpenRemote': false,
          'canFetch': false,
          'canPull': false,
          'canPush': false,
        },
      },
    ]);
    // 读取失败是 readState，不是第六种关系：relation 枚举不含 error。
    expect(() => AppSnapshot.decode(source, 'test'), throwsA(isA<ContractException>()));

    final badType = _withUnknownKey(_loadFixture('snapshot-empty.json'), 'sessionId', 123);
    expect(() => AppSnapshot.decode(badType, 'test'), throwsA(isA<ContractException>()));

    final badInterval = _withUnknownKey(_loadFixture('snapshot-empty.json'), 'refreshPolicy',
        {'enabled': true, 'intervalMinutes': 61});
    expect(() => AppSnapshot.decode(badInterval, 'test'), throwsA(isA<ContractException>()));
  });

  test('staleness 展示逻辑与 expectations 边界数据一致', () {
    final staleness = _expectations()['staleness']! as Map<String, Object?>;

    final manual = staleness['snapshot-manual']! as Map<String, Object?>;
    final manualSnapshot = AppSnapshot.decode(_loadFixture('snapshot-manual.json'), 'manual');
    expect(
      stalenessOf(
        refreshEnabled: manualSnapshot.refreshPolicy.enabled,
        intervalMinutes: manualSnapshot.refreshPolicy.intervalMinutes,
        lastSuccessAtMs: manualSnapshot.lastSuccessAtMs,
        nowMs: manual['nowMs']! as int,
      ),
      Staleness.manual,
    );

    final boundary = staleness['snapshot-stale-boundary']! as Map<String, Object?>;
    final last = boundary['lastSuccessAtMs']! as int;
    final interval = boundary['intervalMinutes']! as int;
    // 恰好两倍间隔：尚不过期；再过 1 毫秒：过期。
    expect(
      stalenessOf(
        refreshEnabled: true,
        intervalMinutes: interval,
        lastSuccessAtMs: last,
        nowMs: boundary['freshAtMs']! as int,
      ),
      Staleness.fresh,
    );
    expect(
      stalenessOf(
        refreshEnabled: true,
        intervalMinutes: interval,
        lastSuccessAtMs: last,
        nowMs: boundary['staleAtMs']! as int,
      ),
      Staleness.stale,
    );
    // 成功时间在未来（时钟回拨）：显示时间异常，不做减法。
    expect(
      stalenessOf(
        refreshEnabled: true,
        intervalMinutes: interval,
        lastSuccessAtMs: last,
        nowMs: boundary['futureAtMs']! as int,
      ),
      Staleness.futureTime,
    );
    // 从未成功：由 refreshState 表达，不算过期。
    expect(
      stalenessOf(
        refreshEnabled: true,
        intervalMinutes: interval,
        lastSuccessAtMs: null,
        nowMs: boundary['freshAtMs']! as int,
      ),
      Staleness.fresh,
    );
  });

  test('envelope 严格解码：成功与业务错误', () {
    final ok = Envelope.decode({
      'schemaVersion': 1,
      'ok': true,
      'data': {'x': 1},
      'error': null,
    });
    expect(ok.ok, isTrue);
    expect(ok.data, {'x': 1});

    final error = Envelope.decode({
      'schemaVersion': 1,
      'ok': false,
      'data': null,
      'error': {'code': 'SETTINGS_CONFLICT'},
    });
    expect(error.ok, isFalse);
    expect(error.errorCode, 'SETTINGS_CONFLICT');

    expect(
      () => Envelope.decode({
        'schemaVersion': 1,
        'ok': false,
        'data': null,
        'error': {'code': 'X', 'extra': 1},
      }),
      throwsA(isA<ContractException>()),
    );
  });

  test('SummaryCounts 一致性自检与输入编码形状', () {
    const counts = SummaryCounts(
      total: 8,
      synced: 2,
      localAhead: 1,
      remoteAhead: 1,
      diverged: 1,
      noRemote: 2,
      error: 1,
      pending: 0,
    );
    expect(counts.closesOverTotal, isTrue);
    const broken = SummaryCounts(
      total: 9,
      synced: 2,
      localAhead: 1,
      remoteAhead: 1,
      diverged: 1,
      noRemote: 2,
      error: 1,
      pending: 0,
    );
    expect(broken.closesOverTotal, isFalse);

    const query = ViewQuery(groupId: 'g1', relationFilter: RelationFilter.diverged, query: 'api');
    expect(query.toJson(), {
      'groupId': 'g1',
      'relationFilter': 'diverged',
      'query': 'api',
    });

    final save = SettingsSaveInput(
      expectedRevision: 3,
      settings: SettingsEditable(
        groups: [GroupSettings(id: 'g1', name: '工作')],
        defaultGroupId: null,
        refresh: const RefreshPolicy(enabled: true, intervalMinutes: 10),
        repoGroups: [RepoGroupAssignment(repoId: 'r1', groupId: 'g1')],
      ),
    );
    expect(save.toJson()['expectedRevision'], 3);
    expect((save.toJson()['settings']! as Map<String, Object?>)['defaultGroupId'], isNull);
  });
}
