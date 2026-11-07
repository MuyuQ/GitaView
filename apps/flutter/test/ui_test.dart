// UI 层 widget 测试：不触碰 FFI，用 FakeGateway（固定数据取自
// contracts/native/v1/fixtures 的形状，见 contracts_test.dart）驱动
// AppController，验证阶段 3c 的页面结构：
// - 筛选 chips 集合与顺序（复用 RepositoryFilterPresentation 的输出）；
// - 表格行渲染与列序（状态 · 仓库 · 分组 · 分支 · 关系 · 差异 · 提示）；
// - 选中行操作面板的两段式确认流（Pull：确认 → 执行 → 结果反馈）；
// - 空态（无仓库 / 无匹配）与全局失败横幅（失败保留历史行）；
// - 设置页两分区渲染与刷新策略保存事务（expectedRevision）。
import 'dart:convert';

import 'package:fluent_ui/fluent_ui.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gitaview/app_controller.dart';
import 'package:gitaview/contracts/contracts.dart';
import 'package:gitaview/engine/engine_gateway.dart';
import 'package:gitaview/ffi/gv_ffi.dart';
import 'package:gitaview/ui/main_page.dart';
import 'package:gitaview/ui/settings_page.dart';

// —— FakeGateway：固定数据，形状与 snapshot-mixed-8 fixture 一致 ——

RepoActions _actions({
  bool openDirectory = true,
  bool openRemote = true,
  bool fetch = true,
  bool pull = false,
  bool push = false,
}) =>
    RepoActions(
      canOpenDirectory: openDirectory,
      canOpenRemote: openRemote,
      canFetch: fetch,
      canPull: pull,
      canPush: push,
    );

RepoStatus _okRow({
  required String id,
  required String name,
  required String path,
  String? groupId,
  required Relation relation,
  required String branch,
  int? ahead,
  int? behind,
  bool hasOrigin = true,
  RepoActions? actions,
}) =>
    RepoStatus(
      id: id,
      name: name,
      path: path,
      groupId: groupId,
      readState: ReadState.ok,
      relation: relation,
      branch: branch,
      hasCommit: true,
      hasOrigin: hasOrigin,
      comparisonBranch: relation == Relation.noRemote ? null : 'main',
      ahead: ahead,
      behind: behind,
      remoteUrl: hasOrigin ? 'https://example.com/$id.git' : null,
      actions: actions ?? _actions(pull: relation != Relation.noRemote, push: false),
    );

/// snapshot-mixed-8 的行（8 个仓库：2 同步 / 1 本地领先 / 1 远端领先 /
/// 1 分叉 / 2 未比较 / 1 错误）。
final List<RepoStatus> _mixedRows = [
  _okRow(
      id: 'repo-api',
      name: 'backend-api',
      path: '/sample/projects/api',
      groupId: 'group-backend',
      relation: Relation.diverged,
      branch: 'main',
      ahead: 3,
      behind: 2,
      actions: _actions(pull: true, push: true)),
  RepoStatus(
    id: 'repo-ci',
    name: 'ci-runner',
    path: '/sample/projects/ci-runner',
    readState: ReadState.error,
    error: const GvError('GIT_FAILED', message: '读取仓库状态失败'),
    actions: _actions(openRemote: false, fetch: false),
  ),
  _okRow(
      id: 'repo-cli',
      name: 'cli-tools',
      path: '/sample/projects/cli-tools',
      relation: Relation.synced,
      branch: 'main',
      ahead: 0,
      behind: 0),
  _okRow(
      id: 'repo-docs',
      name: 'docs-site',
      path: '/sample/projects/docs-site',
      groupId: 'group-docs',
      relation: Relation.remoteAhead,
      branch: 'main',
      ahead: 0,
      behind: 2),
  _okRow(
      id: 'repo-legacy',
      name: 'archived-legacy',
      path: '/sample/projects/archived-legacy',
      relation: Relation.noRemote,
      branch: 'master',
      hasOrigin: false),
  _okRow(
      id: 'repo-mobile',
      name: 'mobile-app',
      path: '/sample/projects/mobile-app',
      relation: Relation.localAhead,
      branch: 'feature/login',
      ahead: 4,
      behind: 0),
  _okRow(
      id: 'repo-notes',
      name: 'notes-vault',
      path: '/sample/projects/notes-vault',
      relation: Relation.noRemote,
      branch: 'main'),
  _okRow(
      id: 'repo-web',
      name: 'website',
      path: '/sample/projects/website',
      groupId: 'group-docs',
      relation: Relation.synced,
      branch: 'main',
      ahead: 0,
      behind: 0),
];

const SummaryCounts _mixedCounts = SummaryCounts(
  total: 8,
  synced: 2,
  localAhead: 1,
  remoteAhead: 1,
  diverged: 1,
  noRemote: 2,
  error: 1,
  pending: 0,
);

const SummaryCounts _emptyCounts = SummaryCounts(
  total: 0, synced: 0, localAhead: 0, remoteAhead: 0,
  diverged: 0, noRemote: 0, error: 0, pending: 0,
);

SettingsData _settingsData({int revision = 2}) => SettingsData(
      schemaVersion: gvSchemaVersion,
      revision: revision,
      repos: const [
        RepoSettings(id: 'repo-api', name: 'backend-api', path: '/sample/projects/api'),
        RepoSettings(
            id: 'repo-docs', name: 'docs-site', path: '/sample/projects/docs-site'),
      ],
      groups: const [
        GroupSettings(id: 'group-backend', name: '后端'),
        GroupSettings(id: 'group-docs', name: '文档'),
      ],
      defaultGroupId: null,
      refresh: const RefreshPolicy(enabled: true, intervalMinutes: 5),
    );

/// 可编程假网关：所有方法返回固定数据，记录调用。
final class FakeGateway implements EngineGateway {
  AppSnapshot snapshot;
  SettingsData settings;
  final List<RepoStatus> rows;
  // 注意：settingsSave 只更新 settings.groups，不更新本列表——两者可能
  // 失同步（与组存在性判断的既有简化同源）；核心永远用当前设置。
  final List<GroupCount> groups;
  final SummaryCounts counts;
  List<ViewQuery> queries = [];
  int saveCalls = 0;
  int? lastExpectedRevision;
  SettingsSaveInput? savedWith;
  int fetchCalls = 0;
  int pullCalls = 0;
  Object? Function()? fetchError;

  FakeGateway({
    List<RepoStatus>? rows,
    this.groups = const [
      GroupCount(id: 'group-backend', name: '后端', count: 1),
      GroupCount(id: 'group-docs', name: '文档', count: 2),
    ],
    this.counts = _mixedCounts,
    SettingsData? settings,
    RefreshState refreshState = RefreshState.ready,
    GvError? refreshError,
  })  : rows = rows ?? _mixedRows,
        snapshot = AppSnapshot(
          schemaVersion: gvSchemaVersion,
          sessionId: 'fx-session-001',
          generation: 4,
          stateRevision: 12,
          settingsRevision: 2,
          attemptedAtMs: 1792800000000,
          lastSuccessAtMs: 1792799700000,
          refreshState: refreshState,
          refreshError: refreshError,
          refreshPolicy: const RefreshPolicy(enabled: true, intervalMinutes: 5),
          refresh: const RefreshProgress(
              isRefreshing: false, requestedRequestId: 7, completedRequestId: 7),
          repos: rows ?? _mixedRows,
        ),
        settings = settings ?? _settingsData();

  @override
  AppSnapshot? get latestSnapshot => snapshot;

  @override
  List<String> get startupWarnings => const [];

  @override
  Future<AppSnapshot> refreshAndWaitComplete({int timeoutMs = 60000}) async =>
      snapshot;

  @override
  Future<AppSnapshot> snapshotGet() async => snapshot;

  @override
  Future<SummarySnapshot> summarySnapshot() async => SummarySnapshot(
        schemaVersion: gvSchemaVersion,
        sessionId: 'fx-session-001',
        generation: 4,
        stateRevision: 12,
        settingsRevision: 2,
        lastSuccessAtMs: 1792799700000,
        refreshState: RefreshState.ready,
        refreshPolicy: const RefreshPolicy(enabled: true, intervalMinutes: 5),
        counts: counts,
        compact: const CompactCounts(
            synced: 2, syncable: 2, needsAttention: 2, noRemote: 2),
        attention: const [],
      );

  @override
  Future<SettingsData> settingsGet() async => settings;

  @override
  Future<SettingsData> settingsSave(SettingsSaveInput input) async {
    saveCalls++;
    lastExpectedRevision = input.expectedRevision;
    // 与核心一致：expectedRevision 不匹配 → SETTINGS_CONFLICT。
    if (input.expectedRevision != settings.revision) {
      throw const GvCallException(GvStatus.error, 'SETTINGS_CONFLICT');
    }
    savedWith = input;
    settings = SettingsData(
      schemaVersion: settings.schemaVersion,
      revision: settings.revision + 1,
      repos: settings.repos,
      groups: input.settings.groups,
      defaultGroupId: input.settings.defaultGroupId,
      refresh: input.settings.refresh,
    );
    return settings;
  }

  @override
  Future<ScanDirectoryData> scanDirectory(String path) async =>
      const ScanDirectoryData(paths: [], truncated: false);

  @override
  Future<RepoAddData> repoAdd({required String path, String? groupId}) async =>
      RepoAddData(
        repo: RepoSettings(id: 'new', name: 'new', path: path, groupId: groupId),
        settingsRevision: settings.revision + 1,
      );

  @override
  Future<RepoRemoveData> repoRemove(String repoId) async =>
      RepoRemoveData(settingsRevision: settings.revision + 1);

  @override
  Future<RepoOperationData> repoFetch(String repoId) async {
    fetchCalls++;
    final error = fetchError?.call();
    if (error != null) throw error;
    return RepoOperationData(repoId: repoId, operation: 'fetch');
  }

  @override
  Future<RepoOperationData> repoPull(String repoId, {required bool confirmed}) async {
    pullCalls++;
    if (!confirmed) {
      throw const GvCallException(GvStatus.error, 'CONFIRMATION_REQUIRED');
    }
    return RepoOperationData(repoId: repoId, operation: 'pull');
  }

  @override
  Future<RepoOperationData> repoPush(String repoId, {required bool confirmed}) async {
    if (!confirmed) {
      throw const GvCallException(GvStatus.error, 'CONFIRMATION_REQUIRED');
    }
    return RepoOperationData(repoId: repoId, operation: 'push');
  }

  @override
  Future<ViewResult> viewQuery(ViewQuery query) async {
    queries.add(query);
    // 模拟核心协调语义（gitaview-core view.rs）：
    // - 所选组不在 groups（被删）→ effective.groupId 回 null；
    // - 所选状态在组内无行 → effective.relationFilter 回 all；
    // - 搜索 name / path / branch，lowercase 包含；搜索零结果不清空搜索。
    final groupExists = query.groupId == null ||
        groups.any((group) => group.id == query.groupId);
    final effectiveGroupId = groupExists ? query.groupId : null;
    Iterable<RepoStatus> scoped = effectiveGroupId == null
        ? rows
        : rows.where((row) => row.groupId == effectiveGroupId);
    var effectiveFilter = query.relationFilter;
    if (effectiveFilter != RelationFilter.all) {
      final matches =
          scoped.where((row) => _matchesFilter(row, effectiveFilter));
      if (matches.isEmpty) {
        effectiveFilter = RelationFilter.all;
      } else {
        scoped = matches;
      }
    }
    final needle = query.query.trim().toLowerCase();
    if (needle.isNotEmpty) {
      // 核心 haystack 含组名（view.rs group_name_of）。
      String? groupNameOf(RepoStatus row) {
        final id = row.groupId;
        if (id == null) return null;
        for (final group in groups) {
          if (group.id == id) return group.name;
        }
        return null;
      }

      scoped = scoped.where((row) =>
          row.name.toLowerCase().contains(needle) ||
          row.path.toLowerCase().contains(needle) ||
          (row.branch ?? '').toLowerCase().contains(needle) ||
          (groupNameOf(row) ?? '').toLowerCase().contains(needle));
    }
    return ViewResult(
      sessionId: 'fx-session-001',
      stateRevision: 12,
      generation: 4,
      settingsRevision: 2,
      effective: ViewQuery(
        groupId: effectiveGroupId,
        relationFilter: effectiveFilter,
        query: query.query,
      ),
      summaryCounts: counts,
      summaryCompact: const CompactCounts(
          synced: 2, syncable: 2, needsAttention: 2, noRemote: 2),
      groups: groups,
      statusCounts: counts,
      rows: scoped.toList(growable: false),
    );
  }

  static bool _matchesFilter(RepoStatus row, RelationFilter filter) {
    if (filter == RelationFilter.error) {
      return row.readState == ReadState.error;
    }
    final Relation? wanted = switch (filter) {
      RelationFilter.synced => Relation.synced,
      RelationFilter.localAhead => Relation.localAhead,
      RelationFilter.remoteAhead => Relation.remoteAhead,
      RelationFilter.diverged => Relation.diverged,
      RelationFilter.noRemote => Relation.noRemote,
      _ => null,
    };
    if (wanted == null) return true;
    return row.readState == ReadState.ok && row.relation == wanted;
  }

  @override
  Future<RepoTargetData> repoTarget({
    required String repoId,
    required RepoTargetKind kind,
  }) async =>
      kind == RepoTargetKind.directory
          ? const RepoTargetData(kind: RepoTargetKind.directory, value: '/sample/projects/api')
          : const RepoTargetData(
              kind: RepoTargetKind.remote, value: 'https://example.com/api');
}


/// 测试专用占位：启动项状态文案在控制器默认值之外显式传入。
const String _startupStateTextForTest = '无法读取开机启动状态，返回此页后重试。';

// —— 测试工具 ——

AppController _controller(FakeGateway gateway,
    {Future<bool> Function(String, String)? launchTarget}) {
  final controller = AppController(
    gateway: gateway,
    launchTarget: launchTarget,
    pickFolder: null,
  );
  return controller;
}

/// 泵主页面（FluentApp 提供 FluentTheme / 本地化）。
Widget _wrap(Widget child) {
  return FluentApp(
    theme: FluentThemeData(brightness: Brightness.light),
    home: ScaffoldPage(content: child),
  );
}

Future<void> _pumpMainPage(WidgetTester tester, FakeGateway gateway,
    {Future<bool> Function(String, String)? launchTarget}) async {
  final controller = _controller(gateway, launchTarget: launchTarget);
  addTearDown(controller.dispose);
  await controller.requery();
  await tester.pumpWidget(_wrap(MainPage(controller: controller, onOpenSettings: () {})));
  await tester.pump();
}

// —— 测试 ——

void main() {
  testWidgets('筛选 chips 集合与顺序：分组与状态两层（RepositoryFilterPresentation 输出）',
      (tester) async {
    final gateway = FakeGateway();
    final controller = _controller(gateway);
    addTearDown(controller.dispose);
    await controller.requery();
    // 纯逻辑层输出即 UI 的数据源：先断言顺序。
    expect(
      controller.groupChips.map((chip) => chip.label).toList(),
      ['全部分组 8', '后端 1', '文档 2'],
    );
    expect(
      controller.statusChips.map((chip) => chip.label).toList(),
      // 错误最前、no_remote 最后，计数为 0 的状态不出现。
      ['全部 8', '读取失败 1', '分叉 1', '远端领先 1', '本地领先 1', '同步 2', '未比较 2'],
    );
    expect(controller.statusChips.first.selected, isTrue);

    // 页面渲染出相同文案与“分组 / 状态”标签（“分组”同时是表格列头）。
    await tester.pumpWidget(_wrap(MainPage(controller: controller, onOpenSettings: () {})));
    await tester.pump();
    expect(find.text('分组'), findsNWidgets(2)); // 筛选条标签 + 表格列头
    expect(find.text('状态'), findsOneWidget);
    expect(find.text('全部分组 8'), findsOneWidget);
    expect(find.text('读取失败 1'), findsOneWidget);
    expect(find.text('未比较 2'), findsOneWidget);
  });

  testWidgets('表格行渲染与列序：表头 状态(空)·仓库·分组·分支·关系·差异·提示',
      (tester) async {
    final gateway = FakeGateway();
    await _pumpMainPage(tester, gateway);
    // 表头列序（第一列状态点无文字标签；“分组”另出现在筛选条标签）。
    expect(find.text('仓库'), findsOneWidget);
    expect(find.text('分组'), findsNWidgets(2));
    expect(find.text('分支'), findsOneWidget);
    expect(find.text('关系'), findsOneWidget);
    expect(find.text('差异'), findsOneWidget);
    expect(find.text('提示'), findsOneWidget);
    // 首行 = backend-api：分叉 + 双向计数（文字 + 数字并用）。
    expect(find.text('backend-api'), findsOneWidget);
    expect(find.text('分叉'), findsOneWidget);
    expect(find.text('↑3 ↓2'), findsOneWidget);
    // 错误行的提示列（ErrorText.forCode 映射）。
    expect(find.text('读取失败'), findsOneWidget);
    expect(find.text('Git 命令执行失败'), findsOneWidget);
    // no_remote 行区分“无远端”。
    expect(find.text('无远端'), findsNWidgets(1));
    // 底部摘要与刷新时间（N/M 前缀 + 紧凑口径）。
    expect(find.text('8 个仓库 · 待同步 2 · 需关注 2 · 未比较 2'), findsOneWidget);
    expect(find.textContaining('刷新时间'), findsOneWidget);
  });

  testWidgets('选中行操作面板与 Pull 两段式确认流', (tester) async {
    final gateway = FakeGateway();
    await _pumpMainPage(tester, gateway);

    // 点击首行展开操作面板（打开目录 / 远端 / Fetch / Pull / Push）。
    // fluent_ui 的 HoverButton 在 tap-up 里安排 100ms 的按压态复位定时器，
    // 测试用 pump(150ms) 让它走完（否则遗留 pending Timer）。
    await tester.tap(find.text('backend-api'));
    await tester.pump(const Duration(milliseconds: 150));
    expect(find.text('打开目录'), findsOneWidget);
    expect(find.text('远端'), findsOneWidget);
    expect(find.text('Fetch'), findsOneWidget);
    expect(find.text('Push'), findsOneWidget);

    // 第一段：只出确认，不调用 repoPull。
    await tester.tap(find.text('Pull'));
    await tester.pump(const Duration(milliseconds: 150));
    expect(gateway.pullCalls, 0);
    expect(find.text('Pull 会修改当前仓库工作区，是否继续？'), findsOneWidget);
    expect(find.text('取消确认'), findsOneWidget);
    expect(find.text('确认 Pull'), findsOneWidget);

    // 第二段：执行（confirmed=true），结果行反馈完成。
    await tester.tap(find.text('确认 Pull'));
    await tester.pump(const Duration(milliseconds: 150));
    expect(gateway.pullCalls, 1);
    expect(find.text('Pull 完成'), findsOneWidget);
    expect(find.text('Pull 会修改当前仓库工作区，是否继续？'), findsNothing);

    // 再点同一行收起；再点重新展开（C# 点击切换语义）。
    await tester.tap(find.text('backend-api'));
    await tester.pump(const Duration(milliseconds: 150));
    expect(find.text('打开目录'), findsNothing);
    await tester.tap(find.text('backend-api'));
    await tester.pump(const Duration(milliseconds: 150));
    expect(find.text('打开目录'), findsOneWidget);
  });

  testWidgets('打开目录走平台钩子；未接线时如实反馈错误文案', (tester) async {
    final gateway = FakeGateway();
    final opened = <String>[];
    await _pumpMainPage(tester, gateway, launchTarget: (kind, value) async {
      opened.add('$kind:$value');
      return true;
    });
    await tester.tap(find.text('backend-api'));
    await tester.pump();
    await tester.tap(find.text('打开目录'));
    await tester.pumpAndSettle();
    expect(opened, ['directory:/sample/projects/api']);
    expect(find.text('已打开目录'), findsOneWidget);

    // 未接线（launchTarget = null）：展示稳定错误文案“未知错误”。
    final plain = _controller(FakeGateway());
    addTearDown(plain.dispose);
    await plain.requery();
    await tester.pumpWidget(_wrap(MainPage(controller: plain, onOpenSettings: () {})));
    await tester.pump();
    await tester.tap(find.text('backend-api'));
    await tester.pump();
    await tester.tap(find.text('打开目录'));
    await tester.pumpAndSettle();
    expect(find.text('未知错误'), findsOneWidget);
  });

  testWidgets('空态：无仓库显示引导；筛选无结果显示无匹配', (tester) async {
    // 无仓库 → “从第一个仓库开始” + 添加仓库按钮，表格不出现。
    final empty = FakeGateway(rows: const [], counts: _emptyCounts);
    final emptyController = _controller(empty);
    addTearDown(emptyController.dispose);
    await emptyController.requery();
    await tester.pumpWidget(_wrap(MainPage(controller: emptyController, onOpenSettings: () {})));
    await tester.pump();
    expect(find.text('从第一个仓库开始'), findsOneWidget);
    expect(find.text('添加仓库目录，或扫描一个工作目录。'), findsOneWidget);
    expect(find.text('添加仓库'), findsOneWidget);
    expect(find.text('backend-api'), findsNothing);

    // 有仓库但组合筛选无结果 → 无匹配提示（不出空仓库引导）。
    final gateway = FakeGateway();
    final controller = _controller(gateway);
    addTearDown(controller.dispose);
    await controller.requery();
    await tester.pumpWidget(_wrap(MainPage(controller: controller, onOpenSettings: () {})));
    await tester.pump();
    controller.selectGroup('group-backend'); // 组内只有 diverged 行
    await tester.pumpAndSettle();
    expect(find.text('backend-api'), findsOneWidget); // 组内行可见
    // 组内无此关系：核心协调把状态筛选回退为全部（行仍可见、chip 同步），
    // 不产生空结果（C# RequeryAsync 消费 effective 的实际语义）。
    controller.selectRelation(RelationFilter.remoteAhead);
    await tester.pumpAndSettle();
    expect(find.text('backend-api'), findsOneWidget);
    expect(controller.selectedRelation, RelationFilter.all);
    // 无匹配提示由搜索零结果触发（搜索零结果不清空搜索词）。
    controller.setQuery('不存在的关键词');
    await tester.pumpAndSettle();
    expect(find.text('没有匹配的仓库，请调整筛选或搜索条件。'), findsOneWidget);
    expect(find.text('从第一个仓库开始'), findsNothing);
  });

  testWidgets('全局刷新失败：琥珀横幅出现且历史行保留（不依赖颜色）', (tester) async {
    final gateway = FakeGateway(
      refreshState: RefreshState.failed,
      refreshError: const GvError('SETTINGS_IO', message: '无法读取设置'),
    );
    final controller = _controller(gateway);
    addTearDown(controller.dispose);
    await controller.requery();
    await tester.pumpWidget(_wrap(MainPage(controller: controller, onOpenSettings: () {})));
    await tester.pump();
    // 横幅文案（错误码映射自 ErrorText）。
    expect(
      find.text('刷新失败：设置读写失败（以下为最近一次成功读取的结果）'),
      findsOneWidget,
    );
    // 历史行保留 + 底部摘要附加“刷新失败”。
    expect(find.text('backend-api'), findsOneWidget);
    expect(find.text('8 个仓库 · 待同步 2 · 需关注 2 · 未比较 2 · 刷新失败'),
        findsOneWidget);
  });

  testWidgets('设置页：两分区导航与仓库设置内容渲染', (tester) async {
    final gateway = FakeGateway();
    final controller = _controller(gateway);
    addTearDown(controller.dispose);
    await controller.loadSettings();
    await tester.pumpWidget(_wrap(SettingsPage(controller: controller, onBack: () {})));
    await tester.pump();

    // 分区导航 + 仓库设置内容。
    expect(find.text('仓库设置'), findsOneWidget);
    expect(find.text('常规设置'), findsOneWidget);
    expect(find.text('添加仓库'), findsOneWidget);
    expect(find.text('仓库列表'), findsOneWidget);
    expect(find.text('分组管理'), findsOneWidget);
    // 已管理仓库行：名称 / 路径 / 归属 / 移除。
    expect(find.text('backend-api'), findsOneWidget);
    expect(find.text('/sample/projects/api'), findsOneWidget);
    expect(find.text('移除'), findsNWidgets(2)); // 每行一个“移除”按钮
    // 分组管理行。
    expect(find.text('后端'), findsWidgets);
    expect(find.text('文档'), findsWidgets);

    // 切换到常规设置：刷新策略 / 默认分组 / 开机启动 / 安全规则。
    await tester.tap(find.text('常规设置'));
    await tester.pump(const Duration(milliseconds: 150));
    expect(find.text('自动刷新'), findsOneWidget);
    expect(find.text('每隔'), findsOneWidget);
    expect(find.text('保存'), findsOneWidget);
    expect(find.text('默认分组'), findsOneWidget);
    expect(find.text('开机启动'), findsOneWidget);
    expect(find.text('操作确认'), findsOneWidget);
    expect(find.text('Pull 会修改工作区，Push 会更新远端分支。每次执行前都需要确认。'),
        findsOneWidget);
  });

  testWidgets('设置页：刷新策略保存走 expectedRevision 事务；冲突出重载入口',
      (tester) async {
    final gateway = FakeGateway();
    final controller = _controller(gateway);
    addTearDown(controller.dispose);
    await controller.loadSettings();
    await tester.pumpWidget(_wrap(SettingsPage(controller: controller, onBack: () {})));
    await tester.pump();
    await tester.tap(find.text('常规设置'));
    await tester.pump();

    // 保存刷新策略：事务携带当前 revision，成功后“已保存”出现在固定位置。
    await tester.tap(find.text('保存'));
    await tester.pumpAndSettle();
    expect(gateway.saveCalls, 1);
    expect(gateway.lastExpectedRevision, 2);
    expect(find.text('已保存'), findsOneWidget);

    // 冲突：其他入口把 revision 推进（模拟外部修改）→ 重载横幅。
    gateway.settings = SettingsData(
      schemaVersion: gvSchemaVersion,
      revision: 9,
      repos: gateway.settings.repos,
      groups: gateway.settings.groups,
      refresh: gateway.settings.refresh,
    );
    final result =
        await controller.saveRefresh(enabled: false, intervalMinutes: 10);
    expect(result, isFalse);
    await tester.pump();
    expect(find.text('设置已被修改'), findsOneWidget);
    expect(find.text('重新加载'), findsOneWidget);

    // 重新加载成功后横幅消失。
    await tester.tap(find.text('重新加载'));
    await tester.pumpAndSettle();
    expect(find.text('设置已被修改'), findsNothing);
  });
  test('组被删除：核心协调回退同步控制器选择', () async {
    final gateway = FakeGateway(groups: [
      const GroupCount(id: 'group-backend', name: '后端', count: 1),
      const GroupCount(id: 'group-docs', name: '文档', count: 2),
    ]);
    final controller = _controller(gateway);
    addTearDown(controller.dispose);
    await controller.requery();
    controller.selectGroup('group-docs');
    await Future<void>.delayed(Duration.zero);
    expect(controller.selectedGroupId, 'group-docs');

    // 组随后被删除：核心 effective.groupId 回 null，控制器同步回全部。
    gateway.groups.removeWhere((group) => group.id == 'group-docs');
    await controller.requery();
    expect(controller.selectedGroupId, isNull,
        reason: '核心协调（组消失切回全部）应同步到控制器');
  });

  test('搜索词进入 viewQuery 并按结果过滤行', () async {
    final gateway = FakeGateway();
    final controller = _controller(gateway);
    addTearDown(controller.dispose);
    await controller.requery();
    final baseline = gateway.queries.length;

    controller.setQuery('backend');
    await Future<void>.delayed(Duration.zero);
    expect(gateway.queries.length, greaterThan(baseline));
    expect(gateway.queries.last.query, 'backend');
    expect(controller.rows.map((row) => row.id), ['repo-api'],
        reason: '搜索命中 backend-api 一行（name lowercase 包含）');

    // 组名 haystack（view.rs group_name_of）：repo-api 属"后端"组。
    controller.setQuery('后端');
    await Future<void>.delayed(Duration.zero);
    expect(controller.rows.map((row) => row.id), ['repo-api'],
        reason: '按组名搜索命中组内仓库');

    // 搜索零结果不清空搜索词（核心协调语义）。
    controller.setQuery('不存在的关键词');
    await Future<void>.delayed(Duration.zero);
    expect(gateway.queries.last.query, '不存在的关键词');
    expect(controller.rows, isEmpty);
    expect(controller.query, '不存在的关键词');
  });

  test('推送驱动 requery：revision 变化触发，同 revision 不重复', () async {
    final events = ChangeNotifier();
    final gateway = FakeGateway();
    final controller = AppController(
      gateway: gateway,
      engineEvents: events,
      startupStateText: _startupStateTextForTest,
    );
    addTearDown(controller.dispose);
    await controller.requery();
    final baseline = gateway.queries.length;

    // 同 revision 的状态通知不重复查询。
    events.notifyListeners();
    await Future<void>.delayed(Duration.zero);
    expect(gateway.queries.length, baseline);

    // revision 变化 → 重新查询。
    final newer =
        AppSnapshot.decode((jsonDecode(jsonEncode(gateway.snapshot.toJson()))
              ..['stateRevision'] = 13)
            .cast<String, Object?>(), 'test');
    gateway.snapshot = newer;
    events.notifyListeners();
    await Future<void>.delayed(Duration.zero);
    expect(gateway.queries.length, greaterThan(baseline));
  });

  test('会话切换：新引擎重新播种默认分组；非空选择跨会话保留', () async {
    final gateway = FakeGateway();
    final controller = _controller(gateway);
    addTearDown(controller.dispose);
    await controller.requery();
    expect(controller.selectedGroupId, isNull,
        reason: '首会话无默认分组');

    // 引擎重建（sessionId 变化）+ 新设置带默认分组 → 重新播种。
    gateway.snapshot = AppSnapshot.decode(
        (jsonDecode(jsonEncode(gateway.snapshot.toJson()))
              ..['sessionId'] = 'fx-session-002')
            .cast<String, Object?>(), 'test');
    gateway.settings = SettingsData.decode(
        (jsonDecode(jsonEncode(gateway.settings.toJson()))
              ..['defaultGroupId'] = 'group-docs')
            .cast<String, Object?>(), 'test');
    await controller.requery();
    expect(controller.selectedGroupId, 'group-docs',
        reason: '会话切换重置播种旗标，新默认分组生效');

    // 反向语义（收敛复审固化）：播种值与用户显式选择共用同一字段，
    // 非空选择跨会话经 ??= 保留，不被新引擎默认组覆盖。
    gateway.snapshot = AppSnapshot.decode(
        (jsonDecode(jsonEncode(gateway.snapshot.toJson()))
              ..['sessionId'] = 'fx-session-003')
            .cast<String, Object?>(), 'test');
    await controller.requery();
    expect(controller.selectedGroupId, 'group-docs',
        reason: '??= 保留非空选择（设计取舍，见进度第 63 轮收敛记录）');
  });
}
