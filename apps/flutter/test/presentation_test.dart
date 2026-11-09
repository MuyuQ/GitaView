// 展示层纯逻辑测试：以 apps/windows/GitaView.Native.Tests/*Facts.cs 为唯一
// 断言来源（C# 语义逐条对应移植）。
//
// 覆盖映射（C# Fact → Dart test，可合并场景但不丢断言）：
// - SnapshotVersionGateFacts：旧查询晚到不覆盖新内容；会话切换基线重置；
// - TrayFlyoutPresentationFacts：空态文案优先级 + 4 组定位/DPI/钳制坐标；
// - RepositoryFilterPresentationFacts：分组 chips 身份/计数、状态 chips 顺序；
// - SettingsGroupPresentationFacts：按 ID 选中、容器再生回声忽略、归属归类；
// - SettingsEditRequestFacts：ExpectedRevision 事务语义 + 候选列表快照
//   （引擎侧 SETTINGS_CONFLICT 冲突属 Rust 核心 / FFI 集成行为，不在本层）；
// - MainWindowClosePolicyFacts：托盘/退出路由 + 退出进行中忽略二次关闭；
// - ProtocolNavigationFacts：17 组路由解析 + 冷/热激活状态机；
// - ComparisonPresentationFacts（ErrorText 的既有 Facts 所在文件）：
//   无远端 vs 未比较的区分；另补充 14 个错误码映射与 relationRow 全分支
//   （C# 侧 ErrorTextFacts.cs 不存在，文案表来自 ErrorText.cs 本体）。
import 'dart:async';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:gitaview/contracts/contracts.dart';
import 'package:gitaview/presentation/error_text.dart';
import 'package:gitaview/presentation/main_window_close_policy.dart';
import 'package:gitaview/presentation/protocol_navigation.dart';
import 'package:gitaview/presentation/ui_preferences.dart';
import 'package:gitaview/presentation/repository_filter_presentation.dart';
import 'package:gitaview/presentation/settings_edit_request.dart';
import 'package:gitaview/presentation/settings_group_presentation.dart';
import 'package:gitaview/presentation/snapshot_version_gate.dart';
import 'package:gitaview/presentation/tray_flyout_presentation.dart';

// —— 测试夹具（与各 C# Facts 的私有夹具一一对应） ——

/// TrayFlyoutPresentationFacts.Counts。
SummaryCounts _counts({int synced = 0, int noRemote = 0, int pending = 0}) =>
    SummaryCounts(
      total: synced + noRemote + pending,
      synced: synced,
      localAhead: 0,
      remoteAhead: 0,
      diverged: 0,
      noRemote: noRemote,
      error: 0,
      pending: pending,
    );

/// SettingsGroupPresentationFacts.Groups（注意第二个分组名与占位标签相同）。
const List<GroupSettings> _groups = [
  GroupSettings(id: 'work', name: '工作'),
  GroupSettings(id: 'literal-ungrouped', name: SettingsPageTag.ungrouped),
];

/// SettingsGroupPresentationFacts.Options。
const List<String> _options = [SettingsPageTag.ungrouped, '工作'];

/// SettingsEditRequestFacts 的当前设置快照（revision 7、默认分组 g、
/// 关闭的 30 分钟刷新——与 C# 夹具一致）。
SettingsData _settings({int revision = 7}) => SettingsData(
      schemaVersion: gvSchemaVersion,
      revision: revision,
      repos: const [],
      groups: const [GroupSettings(id: 'g', name: 'Group')],
      defaultGroupId: 'g',
      refresh: const RefreshPolicy(enabled: false, intervalMinutes: 30),
    );

/// RepoStatusView 测试行构造（ErrorText 分支覆盖用）。
RepoStatusView _view({
  ReadState readState = ReadState.ok,
  Relation? relation,
  int? ahead,
  int? behind,
  String? errorCode,
  bool? hasOrigin,
  bool? hasCommit,
  String branch = 'main',
}) =>
    RepoStatusView(
      id: 'repo',
      name: 'test',
      branch: branch,
      readState: readState,
      relation: relation,
      ahead: ahead,
      behind: behind,
      errorCode: errorCode,
      hasOrigin: hasOrigin,
      hasCommit: hasCommit,
    );

/// ComparisonPresentationFacts 的 no_remote 契约行。
RepoStatus _noRemoteRow({bool? hasOrigin, bool? hasCommit, String branch = 'main'}) =>
    RepoStatus(
      id: 'repo',
      name: 'test',
      path: 'test',
      branch: branch,
      readState: ReadState.ok,
      relation: Relation.noRemote,
      hasOrigin: hasOrigin,
      hasCommit: hasCommit,
      actions: RepoActions(
        canOpenDirectory: true,
        canOpenRemote: hasOrigin == true,
        canFetch: hasOrigin == true,
        canPull: false,
        canPush: false,
      ),
    );

void main() {
  group('SnapshotVersionGate（SnapshotVersionGateFacts）', () {
    test('旧查询最后完成不能覆盖新内容', () async {
      final gate = SnapshotVersionGate();
      gate.beginSession('s');
      // 用 Completer 复现 C# TaskCompletionSource 的交错时序。
      final releaseOld = Completer<int>();
      final oldQuery = releaseOld.future;
      var displayed = 0;
      if (gate.tryAccept('s', 12)) displayed = 12;
      releaseOld.complete(11);
      final oldRevision = await oldQuery;
      if (gate.tryAccept('s', oldRevision)) displayed = oldRevision;
      expect(displayed, 12);
      gate.beginSession('s');
      expect(gate.tryAccept('s', 11), isFalse);
      expect(gate.tryAccept('s', 12), isTrue);
    });

    test('新会话接受初始 revision 并拒绝旧会话迟到结果', () {
      final gate = SnapshotVersionGate();
      gate.beginSession('old');
      expect(gate.tryAccept('old', 100), isTrue);
      gate.beginSession('new');
      expect(gate.tryAccept('new', 0), isTrue);
      expect(gate.tryAccept('old', 101), isFalse);
      expect(gate.tryAccept('new', 1), isTrue);
    });
  });

  group('TrayFlyoutPresentation（TrayFlyoutPresentationFacts）', () {
    test('添加同步仓库后替换“添加或扫描”提示', () {
      expect(
        TrayFlyoutPresentation.emptyMessage(_counts(), RefreshState.ready),
        contains('添加或扫描'),
      );
      expect(
        TrayFlyoutPresentation.emptyMessage(_counts(synced: 3), RefreshState.ready),
        '所有仓库均已同步。',
      );
    });

    test('无远端仓库不会被报告为全部同步', () {
      final message = TrayFlyoutPresentation.emptyMessage(
          _counts(synced: 2, noRemote: 1), RefreshState.ready);
      expect(message, contains('部分仓库缺少远端比较条件'));
      expect(message, isNot(contains('所有仓库均已同步')));
    });

    test('pending 或 failed 刷新不报告成功空态', () {
      expect(
        TrayFlyoutPresentation.emptyMessage(_counts(pending: 2), RefreshState.ready),
        '正在读取仓库状态…',
      );
      expect(
        TrayFlyoutPresentation.emptyMessage(_counts(synced: 2), RefreshState.failed),
        contains('刷新未完成'),
      );
      expect(
        TrayFlyoutPresentation.emptyMessage(_counts(), RefreshState.failed),
        contains('刷新未完成'),
      );
    });

    test('底部任务栏对齐图标并尊重 DPI', () {
      expect(
        TrayFlyoutPresentation.position(
          const FlyoutBounds(0, 0, 1920, 1040),
          const FlyoutBounds(1800, 1040, 24, 40),
          500,
          300,
          1.25,
        ),
        (x: 1324, y: 730),
      );
    });

    test('负坐标副显示器保持自己的工作区', () {
      expect(
        TrayFlyoutPresentation.position(
          const FlyoutBounds(-1920, 0, 1920, 1040),
          const FlyoutBounds(-120, 1040, 24, 40),
          400,
          250,
          1,
        ),
        (x: -496, y: 782),
      );
    });

    test('顶部任务栏把浮窗放到图标下方', () {
      expect(
        TrayFlyoutPresentation.position(
          const FlyoutBounds(0, 40, 1920, 1040),
          const FlyoutBounds(1800, 0, 24, 40),
          400,
          250,
          1,
        ),
        (x: 1424, y: 48),
      );
    });

    test('无图标回退工作区角落，边缘图标被钳制在工作区内', () {
      const workArea = FlyoutBounds(1920, 0, 1920, 1040);
      expect(
        TrayFlyoutPresentation.position(workArea, null, 400, 250, 1),
        (x: 3428, y: 778),
      );
      expect(
        TrayFlyoutPresentation.position(
          workArea,
          const FlyoutBounds(1920, 40, 24, 40),
          400,
          250,
          1,
        ),
        (x: 1924, y: 88),
      );
    });

    test('nativeFrame：Windows 物理像素原样，macOS 按缩放除回逻辑点', () {
      const position = (x: 1424, y: 48);
      expect(
        TrayFlyoutPresentation.nativeFrame(
          position: position,
          width: 500,
          height: 300,
          scale: 1.25,
          logicalUnits: false,
        ),
        (x: 1424.0, y: 48.0, width: 500.0, height: 300.0),
      );
      expect(
        TrayFlyoutPresentation.nativeFrame(
          position: position,
          width: 704,
          height: 420,
          scale: 2.0,
          logicalUnits: true,
        ),
        (x: 712.0, y: 24.0, width: 352.0, height: 210.0),
      );
    });
  });

  group('RepositoryFilterPresentation（RepositoryFilterPresentationFacts）', () {
    test('无分组时仍提供全部分组选项', () {
      for (final total in [0, 3]) {
        final chips = RepositoryFilterPresentation.groups(total, const [], null);
        expect(chips, hasLength(1), reason: 'total=$total');
        expect(chips.single.value, isNull, reason: 'total=$total');
        expect(chips.single.label, '全部分组 $total', reason: 'total=$total');
        expect(chips.single.selected, isTrue, reason: 'total=$total');
      }
    });

    test('仓库数变化只更新全部分组计数，不改选项集合', () {
      final before = RepositoryFilterPresentation.groups(1, const [], null);
      final after = RepositoryFilterPresentation.groups(2, const [], null);
      expect(before, hasLength(1));
      expect(after, hasLength(1));
      expect(before.single.value, after.single.value);
      expect(after.single.label, '全部分组 2');
    });

    test('选中按分组 ID，即使名称含分隔符', () {
      const groups = [
        GroupCount(id: 'work', name: '工作:项目|1', count: 2),
        GroupCount(id: 'tools', name: '工具', count: 1),
      ];
      final chips = RepositoryFilterPresentation.groups(3, groups, 'tools');
      expect(chips[1].label, '工作:项目|1 2');
      final selected = chips.where((chip) => chip.selected).toList();
      expect(selected, hasLength(1));
      expect(selected.single.value, 'tools');
    });

    test('状态选项隐藏零计数且 no_remote 永远最后', () {
      const counts = SummaryCounts(
        total: 4,
        synced: 1,
        localAhead: 0,
        remoteAhead: 0,
        diverged: 1,
        noRemote: 1,
        error: 1,
        pending: 0,
      );
      final chips = RepositoryFilterPresentation.statuses(counts, RelationFilter.diverged);
      expect(
        chips.map((chip) => chip.value),
        [
          RelationFilter.all,
          RelationFilter.error,
          RelationFilter.diverged,
          RelationFilter.synced,
          RelationFilter.noRemote,
        ],
      );
      final selected = chips.where((chip) => chip.selected).toList();
      expect(selected, hasLength(1));
      expect(selected.single.value, RelationFilter.diverged);
      expect(chips.first.label, '全部 4');
      expect(chips.last.label, '未比较 1');
    });
  });

  group('SettingsGroupPresentation（SettingsGroupPresentationFacts）', () {
    test('仓库归属按 ID 选中身份（名称可与占位相同）', () {
      const cases = <(String?, int)>[
        (null, 0),
        ('deleted', 0),
        ('work', 1),
        ('literal-ungrouped', 2),
      ];
      for (final (id, expected) in cases) {
        expect(SettingsGroupPresentation.selectedIndex(_groups, id), expected,
            reason: 'id=$id');
      }
    });

    test('显示名与占位相同时选中仍跟随分组重排', () {
      final reordered = _groups.reversed.toList();
      expect(SettingsGroupPresentation.selectedIndex(reordered, 'literal-ungrouped'), 1);
      expect(SettingsGroupPresentation.selectedIndex(reordered, 'work'), 2);
      expect(SettingsGroupPresentation.selectedIndex(const [], 'literal-ungrouped'), 0);
    });

    test('容器再生回声（索引重置为 -1）被忽略', () {
      // 回归：LoadFromEngine 重挂 ItemsSource 时 SelectedIndex 瞬时重置为 -1，
      // 回声事件若按“未分组”发送空归属，会把刚保存的归属在 ~230ms 内覆盖掉。
      expect(
        SettingsGroupPresentation.assignmentFromSelection(
            -1, _options, _groups, 'repo-1', 'work'),
        isNull,
      );
    });

    test('越界选择被忽略', () {
      expect(
        SettingsGroupPresentation.assignmentFromSelection(
            _options.length, _options, _groups, 'repo-1', null),
        isNull,
      );
    });

    test('重复选择当前分组被忽略', () {
      expect(
        SettingsGroupPresentation.assignmentFromSelection(
            1, _options, _groups, 'repo-1', 'work'),
        isNull,
      );
    });

    test('重复选择未分组被忽略', () {
      expect(
        SettingsGroupPresentation.assignmentFromSelection(
            0, _options, _groups, 'repo-1', null),
        isNull,
      );
    });

    test('选择分组发出携带分组身份的归属', () {
      final assignment = SettingsGroupPresentation.assignmentFromSelection(
          1, _options, _groups, 'repo-1', null);
      expect(assignment, isNotNull);
      expect(assignment!.repoId, 'repo-1');
      expect(assignment.groupId, 'work');
    });

    test('已有分组时选择未分组发出空归属', () {
      final assignment = SettingsGroupPresentation.assignmentFromSelection(
          0, _options, _groups, 'repo-1', 'work');
      expect(assignment, isNotNull);
      expect(assignment!.repoId, 'repo-1');
      expect(assignment.groupId, isNull);
    });

    test('陈旧选项标签按未分组解析，且当前未归属时被忽略', () {
      expect(
        SettingsGroupPresentation.assignmentFromSelection(
            1, [SettingsPageTag.ungrouped, '已删除的组'], _groups, 'repo-1', null),
        isNull,
      );
    });
  });

  group('SettingsEditRequest（SettingsEditRequestFacts）', () {
    test('延迟编辑保持构造时的原始 revision，不能覆盖已完成的编辑', () {
      // C# Facts 用真实引擎验证交错保存被 SETTINGS_CONFLICT 拒绝；该冲突
      // 判定在 Rust 核心（expectedRevision CAS），属 FFI 集成行为。本层断言
      // 其纯逻辑前提：三种操作构造的请求永远携带构造时快照的 revision，
      // 即使随后同一快照上又提交了另一次编辑。
      for (final operation in ['groups', 'default', 'assignment']) {
        final original = _settings(revision: 7);
        final pending = switch (operation) {
          'groups' => SettingsEditRequest.forGroups(original, const [
              GroupSettings(id: 'late', name: 'Late edit'),
            ]),
          'default' => SettingsEditRequest.forDefaultGroup(original, null),
          _ => SettingsEditRequest.forRepoGroup(original, 'missing-repo', null),
        };
        // 模拟交错：先提交一次编辑（核心成功后 revision 推进到 8），
        // 再让延迟编辑落盘。
        final committed = SettingsEditRequest.forGroups(original, const [
          GroupSettings(id: 'first', name: 'Completed edit'),
        ]);
        final committedRevision = original.revision + 1;
        expect(pending.expectedRevision, original.revision, reason: operation);
        expect(committed.expectedRevision, original.revision, reason: operation);
        expect(pending.expectedRevision, isNot(committedRevision),
            reason: 'operation=$operation：延迟编辑必须以陈旧 revision 被核心拒绝');
        // 请求载荷：候选数据与 revision 来自同一快照，其余字段保持当前值。
        expect(pending.settings.groups.single.id,
            operation == 'groups' ? 'late' : 'g', reason: operation);
        expect(pending.settings.defaultGroupId,
            operation == 'default' ? null : 'g', reason: operation);
        if (operation == 'assignment') {
          expect(pending.settings.repoGroups, hasLength(1), reason: operation);
          expect(pending.settings.repoGroups.single.repoId, 'missing-repo',
              reason: operation);
          expect(pending.settings.repoGroups.single.groupId, isNull,
              reason: operation);
        } else {
          expect(pending.settings.repoGroups, isEmpty, reason: operation);
        }
      }
    });

    test('分组请求拥有自己的候选列表并保持其余设置', () {
      final candidates = <GroupSettings>[const GroupSettings(id: 'g', name: 'Group')];
      final current = _settings(revision: 7);
      final request = SettingsEditRequest.forGroups(current, candidates);
      candidates.clear(); // 调用方随后清空原列表，不得影响已构造的请求。
      expect(request.expectedRevision, 7);
      expect(request.settings.groups, hasLength(1));
      expect(request.settings.groups.single.id, 'g');
      expect(request.settings.defaultGroupId, 'g');
      expect(request.settings.refresh, same(current.refresh));
      expect(request.settings.repoGroups, isEmpty);
    });
  });

  group('MainWindowClosePolicy（MainWindowClosePolicyFacts）', () {
    test('关闭路由到托盘隐藏或优雅退出，且不打断进行中的退出', () {
      const cases = <(bool, bool, int, int)>[
        (false, true, 1, 0),
        (false, false, 0, 1),
        (true, true, 0, 0),
        (true, false, 0, 0),
      ];
      for (final (exitRequested, trayAvailable, expectedHides, expectedExits) in cases) {
        var hides = 0;
        var exits = 0;
        MainWindowClosePolicy.handle(
          exitRequested: exitRequested,
          trayAvailable: trayAvailable,
          hide: () => hides++,
          requestExit: () => exits++,
        );
        expect(hides, expectedHides,
            reason: 'exitRequested=$exitRequested trayAvailable=$trayAvailable');
        expect(exits, expectedExits,
            reason: 'exitRequested=$exitRequested trayAvailable=$trayAvailable');
      }
    });

    test('无托盘时重复关闭只启动一次清理且窗口保持可见', () {
      var exitRequested = false;
      var hides = 0;
      var cleanupRequests = 0;
      void requestExit() {
        exitRequested = true;
        cleanupRequests++;
      }

      MainWindowClosePolicy.handle(
        exitRequested: exitRequested,
        trayAvailable: false,
        hide: () => hides++,
        requestExit: requestExit,
      );
      MainWindowClosePolicy.handle(
        exitRequested: exitRequested,
        trayAvailable: false,
        hide: () => hides++,
        requestExit: requestExit,
      );
      expect(cleanupRequests, 1);
      expect(hides, 0);
    });
  });

  group('ProtocolNavigation（ProtocolNavigationFacts）', () {
    test('只接受导航路由且仓库 ID 只解码一次', () {
      const cases = <(String?, String?)>[
        ('gitaview-native://open', null),
        ('gitaview-native://open?confirmed=true&command=push', null),
        ('gitaview-native://repo/my-repo', 'my-repo'),
        ('GITAVIEW-NATIVE://REPO/My-Repo', 'My-Repo'),
        ('gitaview-native://repo/%E8%AE%BE%E8%AE%A1%E7%B3%BB%E7%BB%9F', '设计系统'),
        ('gitaview-native://repo/repo%20name?confirmed=true#ignored', 'repo name'),
        ('gitaview-native://repo/name%252Fpart', 'name%2Fpart'),
        ('gitaview-native://repo', null),
        ('gitaview-native://repo/', null),
        ('gitaview-native://repo/id/extra', null),
        ('gitaview-native://push/my-repo?confirmed=true', null),
        ('gitaview-native://unknown', null),
        ('gitaview-native://user@repo/my-repo', null),
        ('gitaview-native://repo:123/my-repo', null),
        ('https://repo/my-repo', null),
        // 合法十六进制的非法 UTF-8 百分号序列：回总列表，绝不抛异常。
        ('gitaview-native://repo/%FF', null),
        ('gitaview-native://repo/%E4%B8', null),
        ('file:///C:/private/repo', null),
        ('not a URI', null),
        (null, null),
      ];
      for (final (uri, expected) in cases) {
        expect(ProtocolRoute.parse(uri).repoId, expected, reason: 'uri=$uri');
      }
    });

    test('冷启动等待就绪并按当前仓库解析，且不重放', () {
      final navigation = ProtocolNavigation();
      final targets = <ProtocolRoute>[];
      var lookups = 0;
      navigation.request('gitaview-native://repo/pending-repo');
      expect(targets, isEmpty);
      navigation.setReady((id) {
        lookups++;
        return id == 'pending-repo';
      }, targets.add);
      expect(targets, hasLength(1));
      expect(targets.single.repoId, 'pending-repo');
      expect(lookups, 1);
      navigation.setReady((_) => throw StateError('must not replay'), targets.add);
      expect(targets, hasLength(1));
    });

    test('最新的冷启动请求替换更早的请求', () {
      final navigation = ProtocolNavigation();
      final targets = <ProtocolRoute>[];
      navigation.request('gitaview-native://repo/old-target');
      navigation.request('gitaview-native://open');
      navigation.setReady(
          (_) => throw StateError('open needs no repo lookup'), targets.add);
      expect(targets, hasLength(1));
      expect(targets.single.repoId, isNull);
    });

    test('热激活每次都导航，未知或已删除仓库回总列表', () {
      final navigation = ProtocolNavigation();
      final targets = <ProtocolRoute>[];
      final ids = <String>{'repo'};
      navigation.setReady(ids.contains, targets.add);
      navigation.request('gitaview-native://repo/repo?confirmed=true');
      navigation.request('gitaview-native://open');
      navigation.request('gitaview-native://unknown');
      ids.remove('repo');
      navigation.request('gitaview-native://repo/repo');
      expect(targets.map((target) => target.repoId), ['repo', null, null, null]);
    });

    test('初始化完成前仓库被移除则不选中', () {
      final navigation = ProtocolNavigation();
      final targets = <ProtocolRoute>[];
      navigation.request('gitaview-native://repo/removed');
      navigation.setReady((_) => false, targets.add);
      expect(targets, hasLength(1));
      expect(targets.single.repoId, isNull);
    });
  });

  group('ErrorText（ComparisonPresentationFacts + ErrorText.cs 文案表）', () {
    test('区分缺少 origin 远端与缺少远端比较', () {
      const cases = <(bool?, bool?, String, String, String)>[
        (false, true, 'main', '无远端', '未配置 origin 远端'),
        (true, true, 'native-app', '未比较', '没有可比较的 origin 分支'),
        (true, false, 'main', '未比较', '当前分支尚无提交'),
        (true, true, 'HEAD', '未比较', '当前处于分离 HEAD 状态'),
        (null, null, 'main', '未比较', '没有可用的远端比较结果'),
      ];
      for (final (hasOrigin, hasCommit, branch, label, reason) in cases) {
        final row = RepoStatusView.from(
            _noRemoteRow(hasOrigin: hasOrigin, hasCommit: hasCommit, branch: branch));
        expect(ErrorText.noComparisonLabel(row), label, reason: 'branch=$branch');
        expect(ErrorText.noComparisonHint(row), reason, reason: 'branch=$branch');
        expect(ErrorText.relationRow(row), startsWith(label), reason: 'branch=$branch');
      }
    });

    test('14 个稳定错误码与兜底文案', () {
      const cases = <String, String>{
        'INVALID_ARGUMENT': '参数无效',
        'INVALID_PATH': '路径无效或不是 Git 仓库',
        'GIT_NOT_FOUND': '未找到 Git，请确认已安装并在 PATH 中',
        'REPO_NOT_FOUND': '仓库不存在或已被移除',
        'REPO_BUSY': '该仓库已有网络操作进行中，请稍后再试',
        'ORIGIN_REQUIRED': '该仓库没有配置 origin 远端',
        'COMPARISON_REQUIRED': '没有可比较的 origin 分支',
        'CONFIRMATION_REQUIRED': '操作需要用户确认',
        'OPERATION_UNAVAILABLE': '当前状态不允许此操作',
        'SETTINGS_CONFLICT': '设置已被修改，请重载后重试',
        'SETTINGS_IO': '设置读写失败',
        'GIT_FAILED': 'Git 命令执行失败',
        'GIT_TIMEOUT': 'Git 命令超时',
        'ENGINE_STOPPING': '应用正在退出',
      };
      cases.forEach((code, text) {
        expect(ErrorText.forCode(code), text, reason: code);
      });
      expect(ErrorText.forCode('SOMETHING_NEW'), '未知错误');
      expect(ErrorText.forCode(null), '未知错误');
    });

    test('relationRow 全分支文案', () {
      expect(ErrorText.relationRow(_view(readState: ReadState.pending)), '尚未读取');
      expect(
        ErrorText.relationRow(
            _view(readState: ReadState.error, errorCode: 'GIT_TIMEOUT')),
        '读取失败：Git 命令超时',
      );
      expect(
        ErrorText.relationRow(_view(relation: Relation.synced, ahead: 0, behind: 0)),
        '已同步',
      );
      expect(
        ErrorText.relationRow(_view(relation: Relation.localAhead, ahead: 2, behind: 0)),
        '本地领先 2 个提交',
      );
      expect(
        ErrorText.relationRow(_view(relation: Relation.remoteAhead, ahead: 0, behind: 3)),
        '远端领先 3 个提交',
      );
      expect(
        ErrorText.relationRow(_view(relation: Relation.diverged, ahead: 1, behind: 2)),
        '已分叉（本地 +1 / 远端 +2）',
      );
      expect(
        ErrorText.relationRow(_view(relation: Relation.noRemote, hasOrigin: true)),
        '未比较：没有可比较的 origin 分支',
      );
      expect(
        ErrorText.relationRow(_view(relation: Relation.noRemote, hasOrigin: false)),
        '无远端：未配置 origin 远端',
      );
    });

    test('RepoStatusView.from 投影契约行', () {
      final row = RepoStatus(
        id: 'r1',
        name: 'demo',
        path: 'p',
        readState: ReadState.error,
        error: const GvError('REPO_BUSY', message: 'busy'),
        actions: const RepoActions(
          canOpenDirectory: true,
          canOpenRemote: false,
          canFetch: false,
          canPull: false,
          canPush: false,
        ),
      );
      final view = RepoStatusView.from(row);
      expect(view.id, 'r1');
      expect(view.name, 'demo');
      expect(view.branch, ''); // 契约行 branch 缺省为空串
      expect(view.readState, ReadState.error);
      expect(view.errorCode, 'REPO_BUSY'); // errorCode 取自 error.code
      expect(view.relation, isNull);
      expect(view.ahead, isNull);
      expect(view.behind, isNull);
      expect(view.hasOrigin, isNull);
      expect(view.hasCommit, isNull);
    });
  });

  group('UiPreferences（宿主本地主题偏好）', () {
    test('默认 system；保存后可读回；损坏文件回退 system', () async {
      final dir = Directory.systemTemp.createTempSync('gv_ui_prefs');
      addTearDown(() {
        try {
          dir.deleteSync(recursive: true);
        } on FileSystemException {
          // isolate 退出前目录占用可忽略。
        }
      });
      final prefs = UiPreferences(dir);
      expect(await prefs.loadThemeMode(), 'system', reason: '无文件回退 system');

      await prefs.saveThemeMode('dark');
      expect(await prefs.loadThemeMode(), 'dark');
      await prefs.saveThemeMode('light');
      expect(await prefs.loadThemeMode(), 'light');

      // 损坏文件（非法 JSON / 未知取值）→ system。
      File('${dir.path}/ui-preferences.json').writeAsStringSync('{broken');
      expect(await prefs.loadThemeMode(), 'system');
      File('${dir.path}/ui-preferences.json')
          .writeAsStringSync('{"themeMode":"pink"}');
      expect(await prefs.loadThemeMode(), 'system');
    });
  });
}
