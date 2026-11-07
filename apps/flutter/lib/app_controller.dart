// 应用控制器：主状态页与设置页共享的 UI 状态机（ChangeNotifier）。
//
// 行为规格：apps/windows/GitaView.Native/MainPage.xaml.cs 与
// SettingsPage.xaml.cs（语义逐条对应，不是像素复刻）。
// - 订阅引擎快照推送（engineEvents = EngineService），stateRevision 变化或
//   筛选 / 搜索变化时用 view_query 重新拉取（C# RequeryAsync：两次查询——
//   组 + 全部状态给筛选条计数，组 + 当前状态给行）；
// - SnapshotVersionGate 保证旧查询晚到不覆盖新内容、新会话重置基线；
// - 行视图模型按仓库身份缓存（同 id 换路径即失效），操作状态（确认、忙态、
//   结果）跨刷新与筛选保留——对应 C# RepoRowVm / _rowCache；
// - Fetch 无确认；Pull / Push 两段式确认，单仓库独立忙态与行内结果反馈；
// - 设置页数据加载与 SettingsEditRequest 事务（SETTINGS_CONFLICT 出重载入口）。
//
// 平台钩子（Windows 阶段接线，见报告）：
// - [launchTarget]：用系统方式打开目录 / 远端 URL（C# Launcher）；
// - [pickFolder]：目录选择器（C# FolderPicker）。
library;

import 'dart:async';

import 'package:flutter/foundation.dart';

import 'contracts/contracts.dart';
import 'engine/engine_gateway.dart';
import 'ffi/gv_ffi.dart';
import 'presentation/error_text.dart';
import 'presentation/repository_filter_presentation.dart';
import 'presentation/settings_edit_request.dart';
import 'presentation/settings_group_presentation.dart';
import 'presentation/snapshot_version_gate.dart';

/// 行内状态色（语义角色；页面映射到主题 token，不携带具体颜色）。
enum GvStatusColor { green, amber, red, slate, ink, muted }

/// 表格行视图模型：按仓库身份保留，投影更新不丢失操作状态（C# RepoRowVm）。
final class RepoRowVm {
  final String id;
  final String path;

  String name = '';
  String groupDisplay = '';
  String branch = '';
  String statusLabel = '';
  String relationLabel = '';
  String changesLabel = '';
  String hint = '';
  GvStatusColor dot = GvStatusColor.slate;
  GvStatusColor relationColor = GvStatusColor.ink;
  bool canOpenDirectory = false;
  bool canOpenRemote = false;
  bool canFetch = false;
  bool canPull = false;
  bool canPush = false;

  bool isExpanded = false;
  bool isFetchBusy = false;
  bool isPullBusy = false;
  bool isPushBusy = false;
  bool confirmPull = false;
  bool confirmPush = false;
  String resultText = '';
  bool resultIsError = false;

  RepoRowVm({required this.id, required this.path});

  bool get isBusy => isFetchBusy || isPullBusy || isPushBusy;

  /// 两段式确认的按钮文案（C# PullLabel / PushLabel）。
  String get pullLabel => confirmPull ? '确认 Pull' : 'Pull';
  String get pushLabel => confirmPush ? '确认 Push' : 'Push';

  /// 确认提示文案（C# ConfirmText）。
  String get confirmText => confirmPull
      ? 'Pull 会修改当前仓库工作区，是否继续？'
      : confirmPush
          ? 'Push 会更新远端分支，是否继续？'
          : '';

  bool get confirmVisible => confirmPull || confirmPush;
  bool get resultVisible => resultText.isNotEmpty;
}

/// 扫描候选（C# _scanCandidates 的 (Path, CheckBox) 对应物）。
final class ScanCandidateVm {
  final String path;
  bool checked;

  ScanCandidateVm({required this.path, this.checked = true});
}

/// 设置页动作消息（C# ActionMessageBar 的 Title / Message / Severity）。
final class GvMessage {
  final String title;
  final String message;
  final bool isError;

  const GvMessage(this.title, this.message, {required this.isError});
}

/// 应用控制器。
class AppController extends ChangeNotifier {
  final EngineGateway gateway;

  /// 引擎事件源（EngineService 的 ChangeNotifier；watcher 推送时通知）。
  final Listenable? engineEvents;

  /// 打开目录 / 远端的平台钩子（kind: directory / remote）。返回是否成功。
  /// 未接线时操作反馈错误文案。
  final Future<bool> Function(String kind, String value)? launchTarget;

  /// 目录选择器钩子（标题 → 选中路径或 null）。未接线时入口无操作。
  final Future<String?> Function(String title)? pickFolder;

  // —— 查询与快照状态 ——
  final SnapshotVersionGate _gate = SnapshotVersionGate();
  String? _sessionId;
  int _requeryGeneration = 0;
  int _loadedRevision = -1;
  bool _loaded = false;
  AppSnapshot? _snapshot;
  ViewResult? _chipsResult;
  ViewResult? _viewResult;
  Map<String, String> _groupNames = const {};
  final Map<String, RepoRowVm> _rowCache = {};
  List<RepoRowVm> _rows = const [];

  // —— 筛选与选择 ——
  String? _selectedGroupId; // null = 全部分组
  RelationFilter _selectedRelation = RelationFilter.all;
  String _query = '';
  String? _selectedRepoId;

  // —— 主页状态 ——
  bool _refreshBusy = false;
  bool _startupRecovered = false;

  // —— 设置页状态 ——
  SettingsData? _settings;
  bool _settingsConflict = false;
  GvMessage? _settingsMessage;
  bool _refreshSavedVisible = false;
  bool _addRepoBusy = false;
  bool _scanBusy = false;
  List<ScanCandidateVm> _scanCandidates = const [];
  final String startupStateText;

  AppController({
    required this.gateway,
    this.engineEvents,
    this.launchTarget,
    this.pickFolder,
    this.startupStateText = '无法读取开机启动状态，返回此页后重试。',
  }) {
    engineEvents?.addListener(_onEngineChanged);
  }

  // —— 只读投影：主页 ——

  bool get loaded => _loaded;
  bool get refreshBusy => _refreshBusy;
  bool get startupRecovered => _startupRecovered;

  /// 当前筛选下的行（列表顺序即核心 view_query 的顺序）。
  List<RepoRowVm> get rows => _rows;

  /// 选中（展开）的仓库 id；null 表示没有展开行。
  String? get selectedRepoId => _selectedRepoId;
  String? get selectedGroupId => _selectedGroupId;
  RelationFilter get selectedRelation => _selectedRelation;
  String get query => _query;

  /// 分组筛选 chips（复用 RepositoryFilterPresentation；未加载为空）。
  List<FilterChip<String?>> get groupChips {
    final chips = _chipsResult;
    if (chips == null) return const [];
    return RepositoryFilterPresentation.groups(
        chips.summaryCounts.total, chips.groups, _selectedGroupId);
  }

  /// 状态筛选 chips（计数为 0 的状态不出现，no_remote 最后）。
  List<FilterChip<RelationFilter>> get statusChips {
    final chips = _chipsResult;
    if (chips == null) return const [];
    return RepositoryFilterPresentation.statuses(
        chips.statusCounts, _selectedRelation);
  }

  /// 刷新失败琥珀横幅（保留历史行，只提示）。
  String? get refreshWarningText {
    final snapshot = _snapshot;
    if (snapshot == null || snapshot.refreshState != RefreshState.failed) {
      return null;
    }
    return '刷新失败：${ErrorText.forCode(snapshot.refreshError?.code)}'
        '（以下为最近一次成功读取的结果）';
  }

  /// 底部摘要（C# ResultCountText：N/M 前缀 + 总数 · 待同步 · 需关注 ·
  /// 未比较 [+ 待读] [+ 刷新失败]）。
  String get summaryText {
    final chips = _chipsResult;
    final filtered = _viewResult;
    if (chips == null || filtered == null) return '';
    final summary = chips.summaryCounts;
    final syncable = summary.localAhead + summary.remoteAhead;
    final attention = summary.diverged + summary.error;
    var text = summary.total > 0 && filtered.rows.length != summary.total
        ? '${filtered.rows.length} / ${summary.total} · '
        : '';
    text += '${summary.total} 个仓库 · 待同步 $syncable'
        ' · 需关注 $attention · 未比较 ${summary.noRemote}';
    if (summary.pending > 0) text += ' · 待读 ${summary.pending}';
    if (_snapshot?.refreshState == RefreshState.failed) text += ' · 刷新失败';
    return text;
  }

  /// 刷新失败时底部摘要转琥珀（C# ResultCountText.Foreground）。
  bool get summaryIsWarning =>
      _snapshot?.refreshState == RefreshState.failed;

  /// 底部刷新时间（C# RefreshTimeText）。
  String get refreshTimeText {
    final lastSuccess = _snapshot?.lastSuccessAtMs;
    if (lastSuccess == null) return '尚未刷新';
    return '刷新时间 ${_formatTime(lastSuccess)}';
  }

  /// 仓库总数（空态判断：0 = 显示“从第一个仓库开始”）。
  int get totalRepos => _chipsResult?.summaryCounts.total ?? 0;

  // —— 只读投影：设置页 ——

  SettingsData? get settings => _settings;
  bool get settingsConflict => _settingsConflict;
  GvMessage? get settingsMessage => _settingsMessage;
  bool get refreshSavedVisible => _refreshSavedVisible;
  bool get addRepoBusy => _addRepoBusy;
  bool get scanBusy => _scanBusy;

  /// 扫描候选（未扫描为空；有候选才显示操作行）。
  List<ScanCandidateVm> get scanCandidates => List.unmodifiable(_scanCandidates);

  bool get scanActionsVisible => _scanCandidates.isNotEmpty;

  /// 归属下拉选项（0 = 未分组；C# ManagedRepoVm.GroupOptions）。
  List<String> groupOptions() {
    final groups = _settings?.groups ?? const <GroupSettings>[];
    return <String>[SettingsPageTag.ungrouped, ...groups.map((g) => g.name)];
  }

  /// 默认分组选项：(id, 名称)；0 号位恒为“全部分组”（id null）。
  List<(String?, String)> defaultGroupOptions() {
    final groups = _settings?.groups ?? const <GroupSettings>[];
    return <(String?, String)>[
      (null, SettingsPageTag.allGroups),
      ...groups.map((g) => (g.id as String?, g.name)),
    ];
  }

  // —— 快照推送订阅 ——

  void _onEngineChanged() {
    final pushed = gateway.latestSnapshot;
    if (pushed == null) return;
    // 会话变化（引擎重建）或 revision 变化才重新查询；同 revision 的
    // 状态通知不重复查询。
    if (pushed.sessionId != _sessionId || pushed.stateRevision != _loadedRevision) {
      unawaited(requery());
    }
  }

  // —— 查询（C# RequeryAsync）——

  /// 重新拉取视图。并发调用只有最新一次生效；旧会话 / 旧 revision 的
  /// 迟到结果被闸门拒绝。
  Future<void> requery() async {
    final generation = ++_requeryGeneration;
    try {
      // 两次查询：组 + 全部状态 → 筛选条计数与协调；组 + 当前状态 → 行。
      final chips = await gateway.viewQuery(ViewQuery(
        groupId: _selectedGroupId,
        relationFilter: RelationFilter.all,
        query: _query,
      ));
      final filtered = await gateway.viewQuery(ViewQuery(
        groupId: _selectedGroupId,
        relationFilter: _selectedRelation,
        query: _query,
      ));
      final snapshot = await gateway.snapshotGet();
      final settings = await gateway.settingsGet();
      if (generation != _requeryGeneration) return;
      if (!_acceptSnapshot(snapshot)) return;

      // 核心协调：所选组被删或状态在组内不存在 → 回退（同步本地选择）。
      _selectedGroupId = chips.effective.groupId;
      _selectedRelation = filtered.effective.relationFilter;

      _snapshot = snapshot;
      _chipsResult = chips;
      _viewResult = filtered;
      _loadedRevision = filtered.stateRevision;
      _groupNames = {for (final group in settings.groups) group.id: group.name};
      _startupRecovered =
          gateway.startupWarnings.contains('SETTINGS_RECOVERED');
      _buildRows(filtered);
      _loaded = true;
    } catch (_) {
      // 查询失败（含引擎停止中）：保留既有界面与操作状态，等待下一次推送。
    }
    notifyListeners();
  }

  /// 会话与版本验收：换会话重置基线；旧 revision 迟到结果拒绝。
  bool _acceptSnapshot(AppSnapshot snapshot) {
    if (_sessionId != snapshot.sessionId) {
      _sessionId = snapshot.sessionId;
      _gate.beginSession(snapshot.sessionId);
    }
    return _gate.tryAccept(snapshot.sessionId, snapshot.stateRevision);
  }

  /// 行投影与协调（C# BuildRowsAsync：身份缓存，同 id 换路径失效）。
  void _buildRows(ViewResult result) {
    final settings = _settings;
    final identities = <String, String>{
      if (settings != null)
        for (final repo in settings.repos) repo.id: repo.path,
    };
    _rowCache.removeWhere((id, vm) =>
        !identities.containsKey(id) || identities[id] != vm.path);
    final desired = <RepoRowVm>[];
    for (final row in result.rows) {
      final vm = _rowCache.putIfAbsent(
          row.id, () => RepoRowVm(id: row.id, path: row.path));
      final view = RepoStatusView.from(row);
      final group = row.groupId == null ? null : _groupNames[row.groupId!];
      vm
        ..name = row.name
        ..groupDisplay = group ?? '—'
        ..branch = view.branch.isNotEmpty ? view.branch : '—'
        ..statusLabel = _statusLabelOf(view)
        ..relationLabel = _relationLabelOf(view)
        ..changesLabel = _changesLabelOf(view)
        ..hint = _hintOf(view)
        ..dot = _dotOf(view)
        ..relationColor = _relationColorOf(view)
        ..canOpenDirectory = row.actions.canOpenDirectory
        ..canOpenRemote = row.actions.canOpenRemote
        ..canFetch = row.actions.canFetch
        ..canPull = row.actions.canPull
        ..canPush = row.actions.canPush
        ..isExpanded = row.id == _selectedRepoId;
      desired.add(vm);
    }
    _rows = List.unmodifiable(desired);
  }

  // —— 行标签与颜色（C# MainPage 的 *Of 系列逐条对应） ——

  static String _statusLabelOf(RepoStatusView row) => switch (row.readState) {
        ReadState.pending => '待读',
        ReadState.error => '失败',
        _ => switch (row.relation) {
            Relation.synced => '同步',
            Relation.localAhead => '领先',
            Relation.remoteAhead => '落后',
            Relation.diverged => '分叉',
            Relation.noRemote => ErrorText.noComparisonLabel(row),
            _ => '',
          },
      };

  static String _relationLabelOf(RepoStatusView row) => switch (row.readState) {
        ReadState.pending => '尚未读取',
        ReadState.error => '读取失败',
        _ => switch (row.relation) {
            Relation.synced => '已同步',
            Relation.localAhead => '本地领先',
            Relation.remoteAhead => '远程领先',
            Relation.diverged => '分叉',
            Relation.noRemote => ErrorText.noComparisonLabel(row),
            _ => '',
          },
      };

  static String _changesLabelOf(RepoStatusView row) => switch (row.readState) {
        ReadState.ok when row.relation == Relation.localAhead =>
          '↑${row.ahead ?? 0}',
        ReadState.ok when row.relation == Relation.remoteAhead =>
          '↓${row.behind ?? 0}',
        ReadState.ok when row.relation == Relation.diverged =>
          '↑${row.ahead ?? 0} ↓${row.behind ?? 0}',
        _ => '—',
      };

  static String _hintOf(RepoStatusView row) => switch (row.readState) {
        ReadState.pending => '等待首次读取',
        ReadState.error => ErrorText.forCode(row.errorCode),
        ReadState.ok when row.relation == Relation.noRemote =>
          ErrorText.noComparisonHint(row),
        _ => '',
      };

  static GvStatusColor _dotOf(RepoStatusView row) => switch (row.readState) {
        ReadState.pending => GvStatusColor.slate,
        ReadState.error => GvStatusColor.red,
        _ => switch (row.relation) {
            Relation.synced => GvStatusColor.green,
            Relation.localAhead => GvStatusColor.amber,
            Relation.remoteAhead => GvStatusColor.amber,
            Relation.diverged => GvStatusColor.red,
            _ => GvStatusColor.slate,
          },
      };

  static GvStatusColor _relationColorOf(RepoStatusView row) =>
      switch (row.readState) {
        ReadState.error => GvStatusColor.red,
        ReadState.pending => GvStatusColor.muted,
        _ => switch (row.relation) {
            Relation.diverged => GvStatusColor.red,
            Relation.localAhead => GvStatusColor.amber,
            Relation.remoteAhead => GvStatusColor.amber,
            _ => GvStatusColor.ink,
          },
      };

  static String _formatTime(int unixMs) {
    final time =
        DateTime.fromMillisecondsSinceEpoch(unixMs).toLocal();
    String two(int value) => value.toString().padLeft(2, '0');
    return '${two(time.hour)}:${two(time.minute)}:${two(time.second)}';
  }

  // —— 主页交互 ——

  /// 选择分组（null = 全部分组）。
  void selectGroup(String? groupId) {
    _cancelConfirmations();
    _selectedGroupId = groupId;
    unawaited(requery());
  }

  /// 选择状态筛选。
  void selectRelation(RelationFilter filter) {
    _cancelConfirmations();
    _selectedRelation = filter;
    unawaited(requery());
  }

  /// 搜索词变化（C# OnQueryChanged）。
  void setQuery(String text) {
    _cancelConfirmations();
    _query = text;
    unawaited(requery());
  }

  /// 点击行：再点同一行收起（C# OnRepoItemClick）；收起时清除确认态。
  void toggleRow(String id) {
    _selectedRepoId = _selectedRepoId == id ? null : id;
    for (final vm in _rows) {
      vm.isExpanded = vm.id == _selectedRepoId;
      if (!vm.isExpanded) {
        vm.confirmPull = false;
        vm.confirmPush = false;
      }
    }
    notifyListeners();
  }

  /// 取消某行的两段式确认（C# OnCancelConfirmClick）。
  void cancelConfirmation(RepoRowVm row) {
    row.confirmPull = false;
    row.confirmPush = false;
    notifyListeners();
  }

  void _cancelConfirmations() {
    for (final vm in _rowCache.values) {
      vm.confirmPull = false;
      vm.confirmPush = false;
    }
  }

  /// 手动刷新（C# OnRefreshClick：按钮进入“刷新中”禁用态）。
  Future<void> refresh() async {
    if (_refreshBusy) return;
    _refreshBusy = true;
    notifyListeners();
    try {
      await gateway.refreshAndWaitComplete();
    } catch (_) {
      // C# 记录日志后忽略：刷新失败由快照的 refreshState 横幅表达。
    }
    _refreshBusy = false;
    await requery();
  }

  /// 打开目录。
  Future<void> openDirectory(RepoRowVm row) =>
      _openTarget(row, RepoTargetKind.directory, '已打开目录');

  /// 打开远端。
  Future<void> openRemote(RepoRowVm row) =>
      _openTarget(row, RepoTargetKind.remote, '已打开远端');

  Future<void> _openTarget(
    RepoRowVm row,
    RepoTargetKind kind,
    String successText,
  ) async {
    try {
      final target = await gateway.repoTarget(repoId: row.id, kind: kind);
      // 平台打开器在 Windows 阶段接线；未接线时如实反馈失败。
      final launched = launchTarget == null
          ? false
          : await launchTarget!(target.kind.wire, target.value);
      if (launched) {
        row
          ..resultText = successText
          ..resultIsError = false;
      } else {
        row
          ..resultText = ErrorText.forCode(null)
          ..resultIsError = true;
      }
    } catch (error) {
      _setActionResult(row, error);
    }
    notifyListeners();
  }

  /// Fetch（无确认；独立忙态）。
  Future<void> fetch(RepoRowVm row) async {
    if (row.isBusy) return;
    row
      ..isFetchBusy = true
      ..confirmPull = false
      ..confirmPush = false
      ..resultIsError = false
      ..resultText = 'Fetch 执行中…';
    notifyListeners();
    try {
      await gateway.repoFetch(row.id);
      row
        ..resultText = 'Fetch 完成'
        ..resultIsError = false;
      unawaited(_requestRefresh());
    } catch (error) {
      _setActionResult(row, error);
    }
    row.isFetchBusy = false;
    notifyListeners();
  }

  /// Pull（两段式确认；第二段才真正执行 confirmed=true）。
  Future<void> pull(RepoRowVm row) async {
    if (row.isBusy) return;
    if (!row.confirmPull) {
      row
        ..confirmPull = true
        ..confirmPush = false;
      notifyListeners();
      return;
    }
    row
      ..confirmPull = false
      ..confirmPush = false
      ..isPullBusy = true
      ..resultIsError = false
      ..resultText = 'Pull 执行中…';
    notifyListeners();
    try {
      await gateway.repoPull(row.id, confirmed: true);
      row
        ..resultText = 'Pull 完成'
        ..resultIsError = false;
      unawaited(_requestRefresh());
    } catch (error) {
      _setActionResult(row, error);
    }
    row.isPullBusy = false;
    notifyListeners();
  }

  /// Push（两段式确认；第二段才真正执行 confirmed=true）。
  Future<void> push(RepoRowVm row) async {
    if (row.isBusy) return;
    if (!row.confirmPush) {
      row
        ..confirmPush = true
        ..confirmPull = false;
      notifyListeners();
      return;
    }
    row
      ..confirmPull = false
      ..confirmPush = false
      ..isPushBusy = true
      ..resultIsError = false
      ..resultText = 'Push 执行中…';
    notifyListeners();
    try {
      await gateway.repoPush(row.id, confirmed: true);
      row
        ..resultText = 'Push 完成'
        ..resultIsError = false;
      unawaited(_requestRefresh());
    } catch (error) {
      _setActionResult(row, error);
    }
    row.isPushBusy = false;
    notifyListeners();
  }

  void _setActionResult(RepoRowVm row, Object error) {
    row.resultText = ErrorText.forCode(
        error is GvCallException ? error.errorCode : null);
    row.resultIsError = true;
  }

  /// 操作成功后请求刷新（C# RequestRefreshAsync：失败忽略，等下次推送）。
  Future<void> _requestRefresh() async {
    try {
      await gateway.refreshAndWaitComplete();
    } catch (_) {
      // 停止中：忽略。
    }
  }

  // —— 设置页（C# SettingsPage.xaml.cs）——

  /// 加载设置并复位反馈（C# LoadFromEngine 的反馈复位部分）。
  Future<void> loadSettings() async {
    try {
      _settings = await gateway.settingsGet();
      _settingsConflict = false;
      _refreshSavedVisible = false;
    } catch (_) {
      // 读取失败保留旧数据；下次进入页面或手动重载再试。
    }
    notifyListeners();
  }

  /// 保存成功后的静默重载（C# LoadFromEngine；不清除动作消息）。
  Future<void> _reloadSettingsQuiet() async {
    try {
      _settings = await gateway.settingsGet();
    } catch (_) {
      // 保留旧数据。
    }
  }

  /// 重载入口（冲突横幅 / 重新加载按钮）。
  Future<void> reloadSettings() => loadSettings();

  /// 关闭动作消息（InfoBar 可关闭）。
  void dismissMessage() {
    _settingsMessage = null;
    notifyListeners();
  }

  Future<bool> _saveSettings(
    SettingsSaveInput input, {
    required String failureTitle,
    void Function()? onSuccess,
  }) async {
    try {
      _settings = await gateway.settingsSave(input);
      onSuccess?.call();
      notifyListeners();
      return true;
    } on GvCallException catch (error) {
      if (error.errorCode == 'SETTINGS_CONFLICT') {
        _settingsConflict = true;
      } else {
        _settingsMessage = GvMessage(failureTitle, _messageOf(error),
            isError: true);
      }
    } catch (error) {
      _settingsMessage =
          GvMessage(failureTitle, _messageOf(error), isError: true);
    }
    notifyListeners();
    return false;
  }

  static String _messageOf(Object error) => error is GvCallException
      ? ErrorText.forCode(error.errorCode)
      : '${ErrorText.forCode(null)}：$error';

  /// 添加单个仓库（C# OnPickRepoClick；选择器未接线时无操作）。
  Future<void> addRepo() async {
    final pick = pickFolder;
    if (pick == null) return;
    final folder = await pick('选择仓库目录');
    if (folder == null) return;
    _addRepoBusy = true;
    notifyListeners();
    try {
      await gateway.repoAdd(path: folder);
      _settingsMessage = GvMessage('已添加仓库', folder, isError: false);
      await _reloadSettingsQuiet();
    } catch (error) {
      _settingsMessage =
          GvMessage('添加仓库失败', _messageOf(error), isError: true);
    }
    _addRepoBusy = false;
    notifyListeners();
  }

  /// 扫描目录并展示候选（C# OnScanDirectoryClick）。
  Future<void> scanForRepos() async {
    final pick = pickFolder;
    if (pick == null) return;
    final folder = await pick('选择扫描根目录');
    if (folder == null) return;
    _scanBusy = true;
    _scanCandidates = const [];
    notifyListeners();
    try {
      final scan = await gateway.scanDirectory(folder);
      _scanCandidates = List.unmodifiable(
          scan.paths.map((path) => ScanCandidateVm(path: path)));
      if (scan.paths.isEmpty) {
        _settingsMessage =
            GvMessage('扫描完成', '该目录下没有找到 Git 仓库。', isError: false);
      }
      if (scan.truncated) {
        _settingsMessage = GvMessage(
            '扫描完成（结果被截断）', '候选过多，仅显示部分结果。', isError: false);
      }
    } catch (error) {
      _settingsMessage =
          GvMessage('扫描失败', _messageOf(error), isError: true);
    }
    _scanBusy = false;
    notifyListeners();
  }

  /// 全部选择（C# OnSelectAllCandidatesClick）。
  void selectAllCandidates() {
    for (final candidate in _scanCandidates) {
      candidate.checked = true;
    }
    notifyListeners();
  }

  /// 切换单个候选勾选。
  void setCandidateChecked(ScanCandidateVm candidate, bool checked) {
    candidate.checked = checked;
    notifyListeners();
  }

  /// 添加所选候选（C# OnAddCandidatesClick：逐个添加，报告成功 / 失败数量）。
  Future<void> addSelectedCandidates() async {
    final selected = <String>[
      for (final candidate in _scanCandidates)
        if (candidate.checked) candidate.path,
    ];
    if (selected.isEmpty) return;
    _scanBusy = true;
    notifyListeners();
    var added = 0;
    final failures = <String>[];
    for (final path in selected) {
      try {
        await gateway.repoAdd(path: path);
        added++;
      } catch (_) {
        failures.add(path);
      }
    }
    _settingsMessage = GvMessage(
      added > 0 ? '已添加候选仓库' : '没有添加任何仓库',
      added > 0
          ? '成功 $added 个${failures.isNotEmpty ? '；失败 ${failures.length} 个（无效路径或重复）' : ''}'
          : '所选候选均无效（不是 Git 仓库或已存在）',
      isError: added == 0,
    );
    _scanBusy = false;
    await _reloadSettingsQuiet();
    notifyListeners();
  }

  /// 移除仓库（只删管理记录；确认对话框由页面负责）。
  Future<void> removeRepo(RepoSettings repo) async {
    try {
      await gateway.repoRemove(repo.id);
      _settingsMessage = GvMessage('已移除', repo.path, isError: false);
      await _reloadSettingsQuiet();
    } catch (error) {
      _settingsMessage =
          GvMessage('移除失败', _messageOf(error), isError: true);
    }
    notifyListeners();
  }

  /// 修改仓库归属（C# OnRepoGroupChanged：回声事件被纯逻辑层丢弃）。
  Future<void> setRepoGroup({
    required String repoId,
    required int selectedIndex,
    required List<String> options,
  }) async {
    final settings = _settings;
    if (settings == null) return;
    String? currentGroupId;
    for (final repo in settings.repos) {
      if (repo.id == repoId) {
        currentGroupId = repo.groupId;
        break;
      }
    }
    final assignment = SettingsGroupPresentation.assignmentFromSelection(
      selectedIndex,
      options,
      settings.groups,
      repoId,
      currentGroupId,
    );
    if (assignment == null) return;
    await _saveSettings(
      SettingsEditRequest.forRepoGroup(settings, assignment.repoId, assignment.groupId),
      failureTitle: '修改归属失败',
    );
  }

  /// 添加分组（重名拒绝；成功返回 true 供输入框清空）。
  Future<bool> addGroup(String rawName) async {
    final name = rawName.trim();
    final settings = _settings;
    if (name.isEmpty || settings == null) return false;
    if (settings.groups.any((group) => group.name == name)) {
      _settingsMessage = GvMessage('分组已存在', name, isError: true);
      notifyListeners();
      return false;
    }
    return _saveGroups(<GroupSettings>[
      ...settings.groups,
      GroupSettings(id: _newGroupId(settings), name: name),
    ]);
  }

  /// 删除分组（核心对全部仓库清悬挂归属）。
  Future<bool> deleteGroup(String groupId) async {
    final settings = _settings;
    if (settings == null) return false;
    return _saveGroups(<GroupSettings>[
      for (final group in settings.groups)
        if (group.id != groupId) group,
    ]);
  }

  Future<bool> _saveGroups(List<GroupSettings> groups) async {
    final settings = _settings;
    if (settings == null) return false;
    return _saveSettings(
      SettingsEditRequest.forGroups(settings, groups),
      failureTitle: '修改分组失败',
    );
  }

  /// 新分组 id：group-1、group-2 … 跳过已存在（C# NewGroupId）。
  static String _newGroupId(SettingsData settings) {
    final existing = settings.groups.map((group) => group.id).toSet();
    var index = 1;
    while (true) {
      final candidate = 'group-$index';
      if (existing.add(candidate)) return candidate;
      index++;
    }
  }

  /// 保存刷新策略（间隔钳制 1–60；成功后显示“已保存”）。
  Future<bool> saveRefresh({
    required bool enabled,
    required int intervalMinutes,
  }) async {
    final settings = _settings;
    if (settings == null) return false;
    final clamped = intervalMinutes < 1
        ? 1
        : intervalMinutes > 60
            ? 60
            : intervalMinutes;
    return _saveSettings(
      SettingsSaveInput(
        expectedRevision: settings.revision,
        settings: SettingsEditable(
          groups: List.of(settings.groups),
          defaultGroupId: settings.defaultGroupId,
          refresh: RefreshPolicy(enabled: enabled, intervalMinutes: clamped),
          repoGroups: const [],
        ),
      ),
      failureTitle: '保存刷新设置失败',
      onSuccess: () => _refreshSavedVisible = true,
    );
  }

  /// 修改默认分组（同值不发送）。
  Future<void> setDefaultGroup(String? groupId) async {
    final settings = _settings;
    if (settings == null || groupId == settings.defaultGroupId) return;
    await _saveSettings(
      SettingsEditRequest.forDefaultGroup(settings, groupId),
      failureTitle: '修改默认分组失败',
    );
  }

  @override
  void dispose() {
    engineEvents?.removeListener(_onEngineChanged);
    super.dispose();
  }
}
