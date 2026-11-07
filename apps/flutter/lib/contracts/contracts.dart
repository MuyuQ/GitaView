// GitaView 契约层（contracts/native/v1 的 Dart 模型与严格解码）。
//
// 规则与 Rust model.rs、C# Dtos.cs 三方一致：
// - JSON 键 camelCase；枚举小写 snake_case；UTF-8。
// - 未知字段、未知枚举值、错误类型一律抛 ContractException，绝不默认填充。
// - schemaVersion = 1（数据契约）、abiVersion = 2（C ABI）各自独立于产品版本。
// - 时间为 Unix 毫秒整数；未知时间显式 null。
// - 五种关系是读取成功的结果；读取失败是独立 readState，不是第六种关系。
library;

/// 数据契约版本。
const int gvSchemaVersion = 1;

/// 契约解码失败：结构、枚举、类型或字段非法。
class ContractException implements Exception {
  final String message;

  ContractException(this.message);

  @override
  String toString() => 'ContractException: $message';
}

// —— 解码工具：严格类型与字段检查 ——

Never _fail(String message) => throw ContractException(message);

Map<String, Object?> _map(Object? value, String context) {
  if (value is Map<String, Object?>) return value;
  if (value is Map) {
    final result = <String, Object?>{};
    for (final entry in value.entries) {
      if (entry.key is! String) _fail('$context: 存在非字符串键');
      result[entry.key as String] = entry.value;
    }
    return result;
  }
  _fail('$context: 期望对象，实际 ${value.runtimeType}');
}

List<Object?> _list(Object? value, String context) {
  if (value is List<Object?>) return value;
  if (value is List) return value;
  _fail('$context: 期望数组，实际 ${value.runtimeType}');
}

/// 校验对象键集合：未知字段拒绝，必需字段缺失拒绝。
void _checkKeys(
  Map<String, Object?> map,
  Set<String> required,
  Set<String> optional,
  String context,
) {
  final keys = map.keys.toSet();
  final unknown = keys.difference({...required, ...optional});
  if (unknown.isNotEmpty) _fail('$context: 未知字段 $unknown');
  final missing = required.difference(keys);
  if (missing.isNotEmpty) _fail('$context: 缺少字段 $missing');
}

String _string(Object? value, String context) {
  if (value is String) return value;
  _fail('$context: 期望字符串，实际 ${value.runtimeType}');
}

int _nonNegativeInt(Object? value, String context) {
  if (value is int && value >= 0) return value;
  _fail('$context: 期望非负整数，实际 $value');
}

bool _bool(Object? value, String context) {
  if (value is bool) return value;
  _fail('$context: 期望布尔值，实际 ${value.runtimeType}');
}

/// 可选字段：缺失或 null 返回 null；出现时按严格类型解码。
String? _optString(Map<String, Object?> map, String key, String context) {
  final value = map[key];
  if (value == null) return null;
  return _string(value, '$context.$key');
}

int? _optNonNegativeInt(Map<String, Object?> map, String key, String context) {
  final value = map[key];
  if (value == null) return null;
  return _nonNegativeInt(value, '$context.$key');
}

bool? _optBool(Map<String, Object?> map, String key, String context) {
  final value = map[key];
  if (value == null) return null;
  return _bool(value, '$context.$key');
}

// —— 枚举 ——

/// 本地与 origin 比较后的五种关系。
enum Relation {
  synced('synced'),
  localAhead('local_ahead'),
  remoteAhead('remote_ahead'),
  diverged('diverged'),
  noRemote('no_remote');

  final String wire;

  const Relation(this.wire);

  static Relation parse(Object? value, String context) {
    for (final candidate in values) {
      if (candidate.wire == value) return candidate;
    }
    _fail('$context: 未知关系 $value');
  }
}

/// 仓库读取状态。pending 表示尚未读取，error 表示读取失败；两者都不是 Git 关系。
enum ReadState {
  pending('pending'),
  ok('ok'),
  error('error');

  final String wire;

  const ReadState(this.wire);

  static ReadState parse(Object? value, String context) {
    for (final candidate in values) {
      if (candidate.wire == value) return candidate;
    }
    _fail('$context: 未知读取状态 $value');
  }
}

/// 全局刷新状态：只描述最近一次完成的尝试。
enum RefreshState {
  pending('pending'),
  ready('ready'),
  failed('failed');

  final String wire;

  const RefreshState(this.wire);

  static RefreshState parse(Object? value, String context) {
    for (final candidate in values) {
      if (candidate.wire == value) return candidate;
    }
    _fail('$context: 未知刷新状态 $value');
  }
}

/// 状态筛选。pending 不是关系：仅 all 包含 pending 行。
enum RelationFilter {
  all('all'),
  error('error'),
  synced('synced'),
  localAhead('local_ahead'),
  remoteAhead('remote_ahead'),
  diverged('diverged'),
  noRemote('no_remote');

  final String wire;

  const RelationFilter(this.wire);

  static RelationFilter parse(Object? value, String context) {
    for (final candidate in values) {
      if (candidate.wire == value) return candidate;
    }
    _fail('$context: 未知状态筛选 $value');
  }
}

/// 打开目标类别。
enum RepoTargetKind {
  directory('directory'),
  remote('remote');

  final String wire;

  const RepoTargetKind(this.wire);

  static RepoTargetKind parse(Object? value, String context) {
    for (final candidate in values) {
      if (candidate.wire == value) return candidate;
    }
    _fail('$context: 未知打开目标 $value');
  }
}

/// 引擎生命周期。
enum GvLifecycle {
  running('running'),
  stopping('stopping'),
  stopped('stopped');

  final String wire;

  const GvLifecycle(this.wire);

  static GvLifecycle parse(Object? value, String context) {
    for (final candidate in values) {
      if (candidate.wire == value) return candidate;
    }
    _fail('$context: 未知生命周期 $value');
  }
}

// —— 基础对象 ——

/// 错误对象：稳定 code + 可选安全 message。
class GvError {
  final String code;
  final String? message;

  const GvError(this.code, {this.message});

  factory GvError.decode(Object? json, String context) {
    final map = _map(json, context);
    _checkKeys(map, {'code'}, {'message'}, context);
    return GvError(
      _string(map['code'], '$context.code'),
      message: _optString(map, 'message', context),
    );
  }

  Map<String, Object?> toJson() => {
        'code': code,
        if (message != null) 'message': message,
      };
}

/// 操作可用性。UI 以此禁用按钮，核心在执行时再次校验。
class RepoActions {
  final bool canOpenDirectory;
  final bool canOpenRemote;
  final bool canFetch;
  final bool canPull;
  final bool canPush;

  const RepoActions({
    required this.canOpenDirectory,
    required this.canOpenRemote,
    required this.canFetch,
    required this.canPull,
    required this.canPush,
  });

  factory RepoActions.decode(Object? json, String context) {
    final map = _map(json, context);
    _checkKeys(
      map,
      {
        'canOpenDirectory',
        'canOpenRemote',
        'canFetch',
        'canPull',
        'canPush',
      },
      {},
      context,
    );
    return RepoActions(
      canOpenDirectory: _bool(map['canOpenDirectory'], '$context.canOpenDirectory'),
      canOpenRemote: _bool(map['canOpenRemote'], '$context.canOpenRemote'),
      canFetch: _bool(map['canFetch'], '$context.canFetch'),
      canPull: _bool(map['canPull'], '$context.canPull'),
      canPush: _bool(map['canPush'], '$context.canPush'),
    );
  }
}

/// 快照中的仓库行。pending / error 时未知 Git 字段显式为 null 或缺失。
class RepoStatus {
  final String id;
  final String name;
  final String path;
  final String? groupId;
  final ReadState readState;
  final Relation? relation;
  final String? branch;
  final bool? hasCommit;
  final bool? hasOrigin;
  final String? comparisonBranch;
  final int? ahead;
  final int? behind;
  final String? remoteUrl;
  final GvError? error;
  final RepoActions actions;

  const RepoStatus({
    required this.id,
    required this.name,
    required this.path,
    required this.readState,
    required this.actions,
    this.groupId,
    this.relation,
    this.branch,
    this.hasCommit,
    this.hasOrigin,
    this.comparisonBranch,
    this.ahead,
    this.behind,
    this.remoteUrl,
    this.error,
  });

  factory RepoStatus.decode(Object? json, String context) {
    final map = _map(json, context);
    _checkKeys(
      map,
      {'id', 'name', 'path', 'readState', 'actions'},
      {
        'groupId',
        'relation',
        'branch',
        'hasCommit',
        'hasOrigin',
        'comparisonBranch',
        'ahead',
        'behind',
        'remoteUrl',
        'error',
      },
      context,
    );
    return RepoStatus(
      id: _string(map['id'], '$context.id'),
      name: _string(map['name'], '$context.name'),
      path: _string(map['path'], '$context.path'),
      groupId: _optString(map, 'groupId', context),
      readState: ReadState.parse(map['readState'], '$context.readState'),
      relation: map['relation'] == null
          ? null
          : Relation.parse(map['relation'], '$context.relation'),
      branch: _optString(map, 'branch', context),
      hasCommit: _optBool(map, 'hasCommit', context),
      hasOrigin: _optBool(map, 'hasOrigin', context),
      comparisonBranch: _optString(map, 'comparisonBranch', context),
      ahead: _optNonNegativeInt(map, 'ahead', context),
      behind: _optNonNegativeInt(map, 'behind', context),
      remoteUrl: _optString(map, 'remoteUrl', context),
      error: map['error'] == null ? null : GvError.decode(map['error'], '$context.error'),
      actions: RepoActions.decode(map['actions'], '$context.actions'),
    );
  }
}

/// 定时刷新策略。间隔 1–60 分钟；enabled=false 表示手动模式。
class RefreshPolicy {
  final bool enabled;
  final int intervalMinutes;

  const RefreshPolicy({required this.enabled, required this.intervalMinutes});

  factory RefreshPolicy.decode(Object? json, String context) {
    final map = _map(json, context);
    _checkKeys(map, {'enabled', 'intervalMinutes'}, {}, context);
    final interval = _nonNegativeInt(map['intervalMinutes'], '$context.intervalMinutes');
    if (interval < 1 || interval > 60) {
      _fail('$context.intervalMinutes: 超出 1–60 范围 $interval');
    }
    return RefreshPolicy(
      enabled: _bool(map['enabled'], '$context.enabled'),
      intervalMinutes: interval,
    );
  }

  Map<String, Object?> toJson() => {'enabled': enabled, 'intervalMinutes': intervalMinutes};
}

/// 刷新请求进度。等待中的请求用 requested > completed 表示。
class RefreshProgress {
  final bool isRefreshing;
  final int requestedRequestId;
  final int completedRequestId;

  const RefreshProgress({
    required this.isRefreshing,
    required this.requestedRequestId,
    required this.completedRequestId,
  });

  factory RefreshProgress.decode(Object? json, String context) {
    final map = _map(json, context);
    _checkKeys(map, {'isRefreshing', 'requestedRequestId', 'completedRequestId'}, {}, context);
    return RefreshProgress(
      isRefreshing: _bool(map['isRefreshing'], '$context.isRefreshing'),
      requestedRequestId:
          _nonNegativeInt(map['requestedRequestId'], '$context.requestedRequestId'),
      completedRequestId:
          _nonNegativeInt(map['completedRequestId'], '$context.completedRequestId'),
    );
  }
}

/// 应用快照（宿主主窗口消费）。
class AppSnapshot {
  final int schemaVersion;
  final String sessionId;
  final int generation;
  final int stateRevision;
  final int settingsRevision;
  final int? attemptedAtMs;
  final int? lastSuccessAtMs;
  final RefreshState refreshState;
  final GvError? refreshError;
  final RefreshPolicy refreshPolicy;
  final RefreshProgress refresh;
  final List<RepoStatus> repos;

  const AppSnapshot({
    required this.schemaVersion,
    required this.sessionId,
    required this.generation,
    required this.stateRevision,
    required this.settingsRevision,
    required this.refreshState,
    required this.refreshPolicy,
    required this.refresh,
    required this.repos,
    this.attemptedAtMs,
    this.lastSuccessAtMs,
    this.refreshError,
  });

  factory AppSnapshot.decode(Object? json, String context) {
    final map = _map(json, context);
    _checkKeys(
      map,
      {
        'schemaVersion',
        'sessionId',
        'generation',
        'stateRevision',
        'settingsRevision',
        'refreshState',
        'refreshPolicy',
        'refresh',
        'repos',
      },
      {
        'attemptedAtMs',
        'lastSuccessAtMs',
        'refreshError',
      },
      context,
    );
    final schema = _nonNegativeInt(map['schemaVersion'], '$context.schemaVersion');
    if (schema != gvSchemaVersion) {
      _fail('$context.schemaVersion: 未知 schema 版本 $schema');
    }
    return AppSnapshot(
      schemaVersion: schema,
      sessionId: _string(map['sessionId'], '$context.sessionId'),
      generation: _nonNegativeInt(map['generation'], '$context.generation'),
      stateRevision: _nonNegativeInt(map['stateRevision'], '$context.stateRevision'),
      settingsRevision:
          _nonNegativeInt(map['settingsRevision'], '$context.settingsRevision'),
      attemptedAtMs: _optNonNegativeInt(map, 'attemptedAtMs', context),
      lastSuccessAtMs: _optNonNegativeInt(map, 'lastSuccessAtMs', context),
      refreshState: RefreshState.parse(map['refreshState'], '$context.refreshState'),
      refreshError:
          map['refreshError'] == null ? null : GvError.decode(map['refreshError'], '$context.refreshError'),
      refreshPolicy: RefreshPolicy.decode(map['refreshPolicy'], '$context.refreshPolicy'),
      refresh: RefreshProgress.decode(map['refresh'], '$context.refresh'),
      repos: _list(map['repos'], '$context.repos')
          .asMap()
          .entries
          .map((entry) => RepoStatus.decode(entry.value, '$context.repos[${entry.key}]'))
          .toList(growable: false),
    );
  }
}

// —— Summary 安全投影 ——

/// 全量摘要计数。五种关系 + error + pending 之和等于 total。
class SummaryCounts {
  final int total;
  final int synced;
  final int localAhead;
  final int remoteAhead;
  final int diverged;
  final int noRemote;
  final int error;
  final int pending;

  const SummaryCounts({
    required this.total,
    required this.synced,
    required this.localAhead,
    required this.remoteAhead,
    required this.diverged,
    required this.noRemote,
    required this.error,
    required this.pending,
  });

  factory SummaryCounts.decode(Object? json, String context) {
    final map = _map(json, context);
    _checkKeys(
      map,
      {
        'total',
        'synced',
        'localAhead',
        'remoteAhead',
        'diverged',
        'noRemote',
        'error',
        'pending',
      },
      {},
      context,
    );
    return SummaryCounts(
      total: _nonNegativeInt(map['total'], '$context.total'),
      synced: _nonNegativeInt(map['synced'], '$context.synced'),
      localAhead: _nonNegativeInt(map['localAhead'], '$context.localAhead'),
      remoteAhead: _nonNegativeInt(map['remoteAhead'], '$context.remoteAhead'),
      diverged: _nonNegativeInt(map['diverged'], '$context.diverged'),
      noRemote: _nonNegativeInt(map['noRemote'], '$context.noRemote'),
      error: _nonNegativeInt(map['error'], '$context.error'),
      pending: _nonNegativeInt(map['pending'], '$context.pending'),
    );
  }

  /// 五种关系 + error + pending 之和是否等于 total（投影一致性自检）。
  bool get closesOverTotal =>
      synced + localAhead + remoteAhead + diverged + noRemote + error + pending == total;
}

/// 紧凑四桶摘要：error 计入 needs_attention；pending 单独展示。
class CompactCounts {
  final int synced;
  final int syncable;
  final int needsAttention;
  final int noRemote;

  const CompactCounts({
    required this.synced,
    required this.syncable,
    required this.needsAttention,
    required this.noRemote,
  });

  factory CompactCounts.decode(Object? json, String context) {
    final map = _map(json, context);
    _checkKeys(map, {'synced', 'syncable', 'needsAttention', 'noRemote'}, {}, context);
    return CompactCounts(
      synced: _nonNegativeInt(map['synced'], '$context.synced'),
      syncable: _nonNegativeInt(map['syncable'], '$context.syncable'),
      needsAttention: _nonNegativeInt(map['needsAttention'], '$context.needsAttention'),
      noRemote: _nonNegativeInt(map['noRemote'], '$context.noRemote'),
    );
  }
}

/// 关注列表条目：不含 path、完整远端 URL 或原始 Git 输出。
class SummaryRepo {
  final String id;
  final String name;
  final ReadState readState;
  final Relation? relation;
  final String? errorCode;

  const SummaryRepo({
    required this.id,
    required this.name,
    required this.readState,
    this.relation,
    this.errorCode,
  });

  factory SummaryRepo.decode(Object? json, String context) {
    final map = _map(json, context);
    _checkKeys(map, {'id', 'name', 'readState'}, {'relation', 'errorCode'}, context);
    return SummaryRepo(
      id: _string(map['id'], '$context.id'),
      name: _string(map['name'], '$context.name'),
      readState: ReadState.parse(map['readState'], '$context.readState'),
      relation: map['relation'] == null
          ? null
          : Relation.parse(map['relation'], '$context.relation'),
      errorCode: _optString(map, 'errorCode', context),
    );
  }
}

/// Summary 快照（托盘浮窗投影）：由 Rust 投影生成，两端消费同一种 JSON。
class SummarySnapshot {
  final int schemaVersion;
  final String sessionId;
  final int generation;
  final int stateRevision;
  final int settingsRevision;
  final int? attemptedAtMs;
  final int? lastSuccessAtMs;
  final RefreshState refreshState;
  final String? refreshErrorCode;
  final RefreshPolicy refreshPolicy;
  final SummaryCounts counts;
  final CompactCounts compact;
  final List<SummaryRepo> attention;

  const SummarySnapshot({
    required this.schemaVersion,
    required this.sessionId,
    required this.generation,
    required this.stateRevision,
    required this.settingsRevision,
    required this.refreshState,
    required this.refreshPolicy,
    required this.counts,
    required this.compact,
    required this.attention,
    this.attemptedAtMs,
    this.lastSuccessAtMs,
    this.refreshErrorCode,
  });

  factory SummarySnapshot.decode(Object? json, String context) {
    final map = _map(json, context);
    _checkKeys(
      map,
      {
        'schemaVersion',
        'sessionId',
        'generation',
        'stateRevision',
        'settingsRevision',
        'refreshState',
        'refreshPolicy',
        'counts',
        'compact',
        'attention',
      },
      {
        'attemptedAtMs',
        'lastSuccessAtMs',
        'refreshErrorCode',
      },
      context,
    );
    final schema = _nonNegativeInt(map['schemaVersion'], '$context.schemaVersion');
    if (schema != gvSchemaVersion) {
      _fail('$context.schemaVersion: 未知 schema 版本 $schema');
    }
    return SummarySnapshot(
      schemaVersion: schema,
      sessionId: _string(map['sessionId'], '$context.sessionId'),
      generation: _nonNegativeInt(map['generation'], '$context.generation'),
      stateRevision: _nonNegativeInt(map['stateRevision'], '$context.stateRevision'),
      settingsRevision:
          _nonNegativeInt(map['settingsRevision'], '$context.settingsRevision'),
      attemptedAtMs: _optNonNegativeInt(map, 'attemptedAtMs', context),
      lastSuccessAtMs: _optNonNegativeInt(map, 'lastSuccessAtMs', context),
      refreshState: RefreshState.parse(map['refreshState'], '$context.refreshState'),
      refreshErrorCode: _optString(map, 'refreshErrorCode', context),
      refreshPolicy: RefreshPolicy.decode(map['refreshPolicy'], '$context.refreshPolicy'),
      counts: SummaryCounts.decode(map['counts'], '$context.counts'),
      compact: CompactCounts.decode(map['compact'], '$context.compact'),
      attention: _list(map['attention'], '$context.attention')
          .asMap()
          .entries
          .map((entry) => SummaryRepo.decode(entry.value, '$context.attention[${entry.key}]'))
          .toList(growable: false),
    );
  }
}

/// 数据新鲜度判断结果（展示层纯逻辑；阈值语义与 Rust summary.rs 一致）。
enum Staleness { manual, fresh, stale, futureTime }

/// 以当前 Unix 毫秒时间计算新鲜度：
/// - 手动模式不作过期判断；
/// - 从未成功过由 refreshState 表达，不算过期；
/// - 成功时间在未来（时钟回拨）显示时间异常，不做无符号减法；
/// - 开启定时刷新时 now > last + 2*interval 为过期（等于阈值仍新鲜）。
Staleness stalenessOf({
  required bool refreshEnabled,
  required int intervalMinutes,
  required int? lastSuccessAtMs,
  required int nowMs,
}) {
  if (!refreshEnabled) return Staleness.manual;
  final last = lastSuccessAtMs;
  if (last == null) return Staleness.fresh;
  if (last > nowMs) return Staleness.futureTime;
  final intervalMs = (intervalMinutes < 1 ? 1 : intervalMinutes) * 60000;
  final threshold = intervalMs * 2;
  if (nowMs - last > threshold) return Staleness.stale;
  return Staleness.fresh;
}

/// SummarySnapshot 的新鲜度便捷方法。
Staleness summaryStaleness(SummarySnapshot snapshot, int nowMs) => stalenessOf(
      refreshEnabled: snapshot.refreshPolicy.enabled,
      intervalMinutes: snapshot.refreshPolicy.intervalMinutes,
      lastSuccessAtMs: snapshot.lastSuccessAtMs,
      nowMs: nowMs,
    );

// —— 设置 ——

/// 持久化后的仓库记录。
class RepoSettings {
  final String id;
  final String name;
  final String path;
  final String? groupId;

  const RepoSettings({required this.id, required this.name, required this.path, this.groupId});

  factory RepoSettings.decode(Object? json, String context) {
    final map = _map(json, context);
    _checkKeys(map, {'id', 'name', 'path'}, {'groupId'}, context);
    return RepoSettings(
      id: _string(map['id'], '$context.id'),
      name: _string(map['name'], '$context.name'),
      path: _string(map['path'], '$context.path'),
      groupId: _optString(map, 'groupId', context),
    );
  }
}

/// 分组记录。名称唯一且非空。
class GroupSettings {
  final String id;
  final String name;

  const GroupSettings({required this.id, required this.name});

  factory GroupSettings.decode(Object? json, String context) {
    final map = _map(json, context);
    _checkKeys(map, {'id', 'name'}, {}, context);
    return GroupSettings(
      id: _string(map['id'], '$context.id'),
      name: _string(map['name'], '$context.name'),
    );
  }

  Map<String, Object?> toJson() => {'id': id, 'name': name};
}

/// 完整设置（契约 Settings DTO）。
class SettingsData {
  final int schemaVersion;
  final int revision;
  final List<RepoSettings> repos;
  final List<GroupSettings> groups;
  final String? defaultGroupId;
  final RefreshPolicy refresh;

  const SettingsData({
    required this.schemaVersion,
    required this.revision,
    required this.repos,
    required this.groups,
    required this.refresh,
    this.defaultGroupId,
  });

  factory SettingsData.decode(Object? json, String context) {
    final map = _map(json, context);
    _checkKeys(
      map,
      {'schemaVersion', 'revision', 'repos', 'groups', 'refresh'},
      {'defaultGroupId'},
      context,
    );
    return SettingsData(
      schemaVersion: _nonNegativeInt(map['schemaVersion'], '$context.schemaVersion'),
      revision: _nonNegativeInt(map['revision'], '$context.revision'),
      repos: _list(map['repos'], '$context.repos')
          .asMap()
          .entries
          .map((entry) => RepoSettings.decode(entry.value, '$context.repos[${entry.key}]'))
          .toList(growable: false),
      groups: _list(map['groups'], '$context.groups')
          .asMap()
          .entries
          .map((entry) => GroupSettings.decode(entry.value, '$context.groups[${entry.key}]'))
          .toList(growable: false),
      defaultGroupId: _optString(map, 'defaultGroupId', context),
      refresh: RefreshPolicy.decode(map['refresh'], '$context.refresh'),
    );
  }
}

/// 仓库归属覆盖：仅覆盖列出的仓库；groupId null = 未分组。
class RepoGroupAssignment {
  final String repoId;
  final String? groupId;

  const RepoGroupAssignment({required this.repoId, this.groupId});

  Map<String, Object?> toJson() => {
        'repoId': repoId,
        'groupId': groupId,
      };
}

/// settings_save 可编辑字段：分组列表（增删改）、归属覆盖、默认分组与刷新偏好。
class SettingsEditable {
  final List<GroupSettings> groups;
  final String? defaultGroupId;
  final RefreshPolicy refresh;
  final List<RepoGroupAssignment> repoGroups;

  const SettingsEditable({
    required this.groups,
    required this.refresh,
    this.defaultGroupId,
    this.repoGroups = const [],
  });

  Map<String, Object?> toJson() => {
        'groups': groups.map((group) => group.toJson()).toList(),
        'defaultGroupId': defaultGroupId,
        'refresh': refresh.toJson(),
        'repoGroups': repoGroups.map((assignment) => assignment.toJson()).toList(),
      };
}

/// settings_save 输入：`{expectedRevision, settings}`。
class SettingsSaveInput {
  final int expectedRevision;
  final SettingsEditable settings;

  const SettingsSaveInput({required this.expectedRevision, required this.settings});

  Map<String, Object?> toJson() => {
        'expectedRevision': expectedRevision,
        'settings': settings.toJson(),
      };
}

// —— 视图 ——

/// 视图查询输入：groupId null 表示全部仓库。编码为 FFI 输入 JSON。
class ViewQuery {
  final String? groupId;
  final RelationFilter relationFilter;
  final String query;

  const ViewQuery({this.groupId, this.relationFilter = RelationFilter.all, this.query = ''});

  Map<String, Object?> toJson() => {
        'groupId': groupId,
        'relationFilter': relationFilter.wire,
        'query': query,
      };

  factory ViewQuery.decode(Object? json, String context) {
    if (json == null) return const ViewQuery();
    final map = _map(json, context);
    _checkKeys(map, {}, {'groupId', 'relationFilter', 'query'}, context);
    return ViewQuery(
      groupId: _optString(map, 'groupId', context),
      relationFilter: map['relationFilter'] == null
          ? RelationFilter.all
          : RelationFilter.parse(map['relationFilter'], '$context.relationFilter'),
      query: _optString(map, 'query', context) ?? '',
    );
  }
}

/// 组计数（基于全部仓库）。
class GroupCount {
  final String id;
  final String name;
  final int count;

  const GroupCount({required this.id, required this.name, required this.count});

  factory GroupCount.decode(Object? json, String context) {
    final map = _map(json, context);
    _checkKeys(map, {'id', 'name', 'count'}, {}, context);
    return GroupCount(
      id: _string(map['id'], '$context.id'),
      name: _string(map['name'], '$context.name'),
      count: _nonNegativeInt(map['count'], '$context.count'),
    );
  }
}

/// 视图查询结果。同一 stateRevision 的两次查询可以有不同筛选（有效筛选随输入协调）。
class ViewResult {
  final String sessionId;
  final int stateRevision;
  final int generation;
  final int settingsRevision;
  final ViewQuery effective;
  final SummaryCounts summaryCounts;
  final CompactCounts summaryCompact;
  final List<GroupCount> groups;
  final SummaryCounts statusCounts;
  final List<RepoStatus> rows;

  const ViewResult({
    required this.sessionId,
    required this.stateRevision,
    required this.generation,
    required this.settingsRevision,
    required this.effective,
    required this.summaryCounts,
    required this.summaryCompact,
    required this.groups,
    required this.statusCounts,
    required this.rows,
  });

  factory ViewResult.decode(Object? json, String context) {
    final map = _map(json, context);
    _checkKeys(
      map,
      {
        'sessionId',
        'stateRevision',
        'generation',
        'settingsRevision',
        'effective',
        'summaryCounts',
        'summaryCompact',
        'groups',
        'statusCounts',
        'rows',
      },
      {},
      context,
    );
    return ViewResult(
      sessionId: _string(map['sessionId'], '$context.sessionId'),
      stateRevision: _nonNegativeInt(map['stateRevision'], '$context.stateRevision'),
      generation: _nonNegativeInt(map['generation'], '$context.generation'),
      settingsRevision:
          _nonNegativeInt(map['settingsRevision'], '$context.settingsRevision'),
      effective: ViewQuery.decode(map['effective'], '$context.effective'),
      summaryCounts: SummaryCounts.decode(map['summaryCounts'], '$context.summaryCounts'),
      summaryCompact: CompactCounts.decode(map['summaryCompact'], '$context.summaryCompact'),
      groups: _list(map['groups'], '$context.groups')
          .asMap()
          .entries
          .map((entry) => GroupCount.decode(entry.value, '$context.groups[${entry.key}]'))
          .toList(growable: false),
      statusCounts: SummaryCounts.decode(map['statusCounts'], '$context.statusCounts'),
      rows: _list(map['rows'], '$context.rows')
          .asMap()
          .entries
          .map((entry) => RepoStatus.decode(entry.value, '$context.rows[${entry.key}]'))
          .toList(growable: false),
    );
  }
}

// —— FFI envelope 与结果对象 ——

/// FFI envelope：`{"schemaVersion":1,"ok":true,"data":…,"error":null}`。
class Envelope {
  final bool ok;
  final Object? data;
  final String? errorCode;
  final String? errorMessage;

  const Envelope({required this.ok, this.data, this.errorCode, this.errorMessage});

  factory Envelope.decode(Object? json) {
    final map = _map(json, 'envelope');
    _checkKeys(map, {'schemaVersion', 'ok', 'data', 'error'}, {}, 'envelope');
    final schema = _nonNegativeInt(map['schemaVersion'], 'envelope.schemaVersion');
    if (schema != gvSchemaVersion) {
      _fail('envelope.schemaVersion: 未知 schema 版本 $schema');
    }
    final ok = _bool(map['ok'], 'envelope.ok');
    if (ok) {
      return Envelope(ok: true, data: map['data']);
    }
    final error = map['error'];
    if (error == null) _fail('envelope: 失败 envelope 缺少 error 对象');
    final decoded = GvError.decode(error, 'envelope.error');
    return Envelope(ok: false, errorCode: decoded.code, errorMessage: decoded.message);
  }
}

/// gv_engine_create 成功 data：`{sessionId, abiVersion, startupWarnings}`。
class EngineCreateData {
  final String sessionId;
  final int abiVersion;
  final List<String> startupWarnings;

  const EngineCreateData({
    required this.sessionId,
    required this.abiVersion,
    required this.startupWarnings,
  });

  factory EngineCreateData.decode(Object? json) {
    final map = _map(json, 'engineCreateData');
    _checkKeys(map, {'sessionId', 'abiVersion', 'startupWarnings'}, {}, 'engineCreateData');
    return EngineCreateData(
      sessionId: _string(map['sessionId'], 'engineCreateData.sessionId'),
      abiVersion: _nonNegativeInt(map['abiVersion'], 'engineCreateData.abiVersion'),
      startupWarnings: _list(map['startupWarnings'], 'engineCreateData.startupWarnings')
          .map((value) => _string(value, 'engineCreateData.startupWarnings[]'))
          .toList(growable: false),
    );
  }
}

/// gv_scan_directory 成功 data：`{paths, truncated}`。
class ScanDirectoryData {
  final List<String> paths;
  final bool truncated;

  const ScanDirectoryData({required this.paths, required this.truncated});

  factory ScanDirectoryData.decode(Object? json) {
    final map = _map(json, 'scanDirectoryData');
    _checkKeys(map, {'paths', 'truncated'}, {}, 'scanDirectoryData');
    return ScanDirectoryData(
      paths: _list(map['paths'], 'scanDirectoryData.paths')
          .map((value) => _string(value, 'scanDirectoryData.paths[]'))
          .toList(growable: false),
      truncated: _bool(map['truncated'], 'scanDirectoryData.truncated'),
    );
  }
}

/// gv_repo_add 成功 data：`{repo, settingsRevision}`。
class RepoAddData {
  final RepoSettings repo;
  final int settingsRevision;

  const RepoAddData({required this.repo, required this.settingsRevision});

  factory RepoAddData.decode(Object? json) {
    final map = _map(json, 'repoAddData');
    _checkKeys(map, {'repo', 'settingsRevision'}, {}, 'repoAddData');
    return RepoAddData(
      repo: RepoSettings.decode(map['repo'], 'repoAddData.repo'),
      settingsRevision:
          _nonNegativeInt(map['settingsRevision'], 'repoAddData.settingsRevision'),
    );
  }
}

/// gv_repo_remove 成功 data：`{settingsRevision}`。
class RepoRemoveData {
  final int settingsRevision;

  const RepoRemoveData({required this.settingsRevision});

  factory RepoRemoveData.decode(Object? json) {
    final map = _map(json, 'repoRemoveData');
    _checkKeys(map, {'settingsRevision'}, {}, 'repoRemoveData');
    return RepoRemoveData(
      settingsRevision:
          _nonNegativeInt(map['settingsRevision'], 'repoRemoveData.settingsRevision'),
    );
  }
}

/// Git 操作成功 data：`{repoId, operation}`。
class RepoOperationData {
  final String repoId;
  final String operation;

  const RepoOperationData({required this.repoId, required this.operation});

  factory RepoOperationData.decode(Object? json) {
    final map = _map(json, 'repoOperationData');
    _checkKeys(map, {'repoId', 'operation'}, {}, 'repoOperationData');
    return RepoOperationData(
      repoId: _string(map['repoId'], 'repoOperationData.repoId'),
      operation: _string(map['operation'], 'repoOperationData.operation'),
    );
  }
}

/// gv_refresh_request 成功 data：`{sessionId, requestId}`。
class RefreshRequestData {
  final String sessionId;
  final int requestId;

  const RefreshRequestData({required this.sessionId, required this.requestId});

  factory RefreshRequestData.decode(Object? json) {
    final map = _map(json, 'refreshRequestData');
    _checkKeys(map, {'sessionId', 'requestId'}, {}, 'refreshRequestData');
    return RefreshRequestData(
      sessionId: _string(map['sessionId'], 'refreshRequestData.sessionId'),
      requestId: _nonNegativeInt(map['requestId'], 'refreshRequestData.requestId'),
    );
  }
}

/// gv_repo_target 成功 data：`{kind, value}`。
class RepoTargetData {
  final RepoTargetKind kind;
  final String value;

  const RepoTargetData({required this.kind, required this.value});

  factory RepoTargetData.decode(Object? json) {
    final map = _map(json, 'repoTargetData');
    _checkKeys(map, {'kind', 'value'}, {}, 'repoTargetData');
    return RepoTargetData(
      kind: RepoTargetKind.parse(map['kind'], 'repoTargetData.kind'),
      value: _string(map['value'], 'repoTargetData.value'),
    );
  }
}

/// 生命周期 data：`{lifecycle}`。
class LifecycleData {
  final GvLifecycle lifecycle;

  const LifecycleData({required this.lifecycle});

  factory LifecycleData.decode(Object? json) {
    final map = _map(json, 'lifecycleData');
    _checkKeys(map, {'lifecycle'}, {}, 'lifecycleData');
    return LifecycleData(
      lifecycle: GvLifecycle.parse(map['lifecycle'], 'lifecycleData.lifecycle'),
    );
  }
}

// —— FFI 输入编码 ——

/// gv_engine_create 输入：`{dataDirectory, logDirectory}`（绝对私有路径）。
Map<String, Object?> engineCreateInputToJson({
  required String dataDirectory,
  required String logDirectory,
}) =>
    {'dataDirectory': dataDirectory, 'logDirectory': logDirectory};

/// gv_scan_directory 输入：`{path}`。
Map<String, Object?> scanDirectoryInputToJson(String path) => {'path': path};

/// gv_repo_add 输入：`{path, groupId?}`。
Map<String, Object?> repoAddInputToJson({required String path, String? groupId}) =>
    {'path': path, 'groupId': groupId};

/// gv_snapshot_wait cursor：`{sessionId, stateRevision}`。
Map<String, Object?> snapshotWaitInputToJson({
  required String sessionId,
  required int stateRevision,
}) =>
    {'sessionId': sessionId, 'stateRevision': stateRevision};

/// gv_repo_target 输入：`{repoId, kind}`。
Map<String, Object?> repoTargetInputToJson({
  required String repoId,
  required RepoTargetKind kind,
}) =>
    {'repoId': repoId, 'kind': kind.wire};

// —— 序列化：与 decode 严格互逆（跨 isolate 传输与调试用）——
// GvError / RefreshPolicy / GroupSettings / RepoGroupAssignment /
// SettingsEditable / SettingsSaveInput / ViewQuery 的 toJson 在类内定义，
// 不再重复提供扩展。

extension RepoActionsJson on RepoActions {
  Map<String, Object?> toJson() => {
        'canOpenDirectory': canOpenDirectory,
        'canOpenRemote': canOpenRemote,
        'canFetch': canFetch,
        'canPull': canPull,
        'canPush': canPush,
      };
}

extension RepoStatusJson on RepoStatus {
  Map<String, Object?> toJson() => {
        'id': id,
        'name': name,
        'path': path,
        if (groupId != null) 'groupId': groupId,
        'readState': readState.wire,
        'relation': relation?.wire,
        'branch': branch,
        'hasCommit': hasCommit,
        'hasOrigin': hasOrigin,
        'comparisonBranch': comparisonBranch,
        'ahead': ahead,
        'behind': behind,
        'remoteUrl': remoteUrl,
        'error': error?.toJson(),
        'actions': actions.toJson(),
      };
}

extension RefreshProgressJson on RefreshProgress {
  Map<String, Object?> toJson() => {
        'isRefreshing': isRefreshing,
        'requestedRequestId': requestedRequestId,
        'completedRequestId': completedRequestId,
      };
}

extension AppSnapshotJson on AppSnapshot {
  Map<String, Object?> toJson() => {
        'schemaVersion': schemaVersion,
        'sessionId': sessionId,
        'generation': generation,
        'stateRevision': stateRevision,
        'settingsRevision': settingsRevision,
        'attemptedAtMs': attemptedAtMs,
        'lastSuccessAtMs': lastSuccessAtMs,
        'refreshState': refreshState.wire,
        'refreshError': refreshError?.toJson(),
        'refreshPolicy': refreshPolicy.toJson(),
        'refresh': refresh.toJson(),
        'repos': repos.map((repo) => repo.toJson()).toList(),
      };
}

extension SummaryCountsJson on SummaryCounts {
  Map<String, Object?> toJson() => {
        'total': total,
        'synced': synced,
        'localAhead': localAhead,
        'remoteAhead': remoteAhead,
        'diverged': diverged,
        'noRemote': noRemote,
        'error': error,
        'pending': pending,
      };
}

extension CompactCountsJson on CompactCounts {
  Map<String, Object?> toJson() => {
        'synced': synced,
        'syncable': syncable,
        'needsAttention': needsAttention,
        'noRemote': noRemote,
      };
}

extension SummaryRepoJson on SummaryRepo {
  Map<String, Object?> toJson() => {
        'id': id,
        'name': name,
        'readState': readState.wire,
        'relation': relation?.wire,
        'errorCode': errorCode,
      };
}

extension SummarySnapshotJson on SummarySnapshot {
  Map<String, Object?> toJson() => {
        'schemaVersion': schemaVersion,
        'sessionId': sessionId,
        'generation': generation,
        'stateRevision': stateRevision,
        'settingsRevision': settingsRevision,
        'attemptedAtMs': attemptedAtMs,
        'lastSuccessAtMs': lastSuccessAtMs,
        'refreshState': refreshState.wire,
        'refreshErrorCode': refreshErrorCode,
        'refreshPolicy': refreshPolicy.toJson(),
        'counts': counts.toJson(),
        'compact': compact.toJson(),
        'attention': attention.map((item) => item.toJson()).toList(),
      };
}

extension RepoSettingsJson on RepoSettings {
  Map<String, Object?> toJson() => {
        'id': id,
        'name': name,
        'path': path,
        'groupId': groupId,
      };
}

extension SettingsDataJson on SettingsData {
  Map<String, Object?> toJson() => {
        'schemaVersion': schemaVersion,
        'revision': revision,
        'repos': repos.map((repo) => repo.toJson()).toList(),
        'groups': groups.map((group) => group.toJson()).toList(),
        'defaultGroupId': defaultGroupId,
        'refresh': refresh.toJson(),
      };
}

extension GroupCountJson on GroupCount {
  Map<String, Object?> toJson() => {'id': id, 'name': name, 'count': count};
}

extension ViewResultJson on ViewResult {
  Map<String, Object?> toJson() => {
        'sessionId': sessionId,
        'stateRevision': stateRevision,
        'generation': generation,
        'settingsRevision': settingsRevision,
        'effective': effective.toJson(),
        'summaryCounts': summaryCounts.toJson(),
        'summaryCompact': summaryCompact.toJson(),
        'groups': groups.map((group) => group.toJson()).toList(),
        'statusCounts': statusCounts.toJson(),
        'rows': rows.map((row) => row.toJson()).toList(),
      };
}

extension EngineCreateDataJson on EngineCreateData {
  Map<String, Object?> toJson() => {
        'sessionId': sessionId,
        'abiVersion': abiVersion,
        'startupWarnings': startupWarnings,
      };
}

extension ScanDirectoryDataJson on ScanDirectoryData {
  Map<String, Object?> toJson() => {'paths': paths, 'truncated': truncated};
}

extension RepoAddDataJson on RepoAddData {
  Map<String, Object?> toJson() => {'repo': repo.toJson(), 'settingsRevision': settingsRevision};
}

extension RepoRemoveDataJson on RepoRemoveData {
  Map<String, Object?> toJson() => {'settingsRevision': settingsRevision};
}

extension RepoOperationDataJson on RepoOperationData {
  Map<String, Object?> toJson() => {'repoId': repoId, 'operation': operation};
}

extension RefreshRequestDataJson on RefreshRequestData {
  Map<String, Object?> toJson() => {'sessionId': sessionId, 'requestId': requestId};
}

extension RepoTargetDataJson on RepoTargetData {
  Map<String, Object?> toJson() => {'kind': kind.wire, 'value': value};
}

extension LifecycleDataJson on LifecycleData {
  Map<String, Object?> toJson() => {'lifecycle': lifecycle.wire};
}
