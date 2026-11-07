// 引擎网关：UI 层访问引擎的唯一抽象接口 + EngineService 适配器。
//
// 对应关系：C# 宿主直接持有 GitaViewEngine；Dart 侧 UI（AppController /
// 页面）改经 [EngineGateway] 访问，使 widget 测试可以用固定数据驱动，
// 不触碰 FFI 与 isolate。[EngineServiceGateway] 只做一一转发，
// 不改变 engine_service.dart 的任何语义。
//
// 快照推送：EngineService 是 ChangeNotifier（watcher isolate 推送时
// notifyListeners）；UI 用 `engineEvents`（同一个 Listenable）订阅推送，
// 用 [latestSnapshot] 读取最近一次推送的快照——与 C# App.SnapshotArrived
// 事件 + engine.SnapshotGet() 的组合同构。
library;

import '../contracts/contracts.dart';
import 'engine_service.dart';

/// 引擎异步能力的抽象接口（EngineService 面向 UI 方法的签名）。
abstract interface class EngineGateway {
  /// 最近一次 watcher 推送的快照；尚未推送时为 null。
  AppSnapshot? get latestSnapshot;

  /// 本次启动的引擎警告（C# App.StartupWarnings，如 SETTINGS_RECOVERED）。
  List<String> get startupWarnings;

  /// 手动刷新并等待本次请求完成（刷新按钮语义）。
  Future<AppSnapshot> refreshAndWaitComplete({int timeoutMs});

  /// 读取完整应用快照。
  Future<AppSnapshot> snapshotGet();

  /// 读取 Summary 快照（托盘浮窗投影）。
  Future<SummarySnapshot> summarySnapshot();

  /// 读取设置。
  Future<SettingsData> settingsGet();

  /// 保存设置（expectedRevision 事务）。
  Future<SettingsData> settingsSave(SettingsSaveInput input);

  /// 扫描目录，返回候选仓库路径。
  Future<ScanDirectoryData> scanDirectory(String path);

  /// 添加单个仓库。
  Future<RepoAddData> repoAdd({required String path, String? groupId});

  /// 移除仓库（只删管理记录）。
  Future<RepoRemoveData> repoRemove(String repoId);

  /// Fetch（无确认）。
  Future<RepoOperationData> repoFetch(String repoId);

  /// Pull（核心要求 confirmed；UI 先做两段式确认）。
  Future<RepoOperationData> repoPull(String repoId, {required bool confirmed});

  /// Push（核心要求 confirmed；UI 先做两段式确认）。
  Future<RepoOperationData> repoPush(String repoId, {required bool confirmed});

  /// 视图查询（核心统一计算筛选 / 搜索 / 排序）。
  Future<ViewResult> viewQuery(ViewQuery query);

  /// 解析打开目标（目录路径或浏览器安全的远端 URL）。
  Future<RepoTargetData> repoTarget({
    required String repoId,
    required RepoTargetKind kind,
  });
}

/// [EngineService] 的适配器：方法逐条转发，无任何附加逻辑。
final class EngineServiceGateway implements EngineGateway {
  final EngineService _service;

  EngineServiceGateway(this._service);

  @override
  AppSnapshot? get latestSnapshot => _service.snapshot;

  @override
  List<String> get startupWarnings => _service.createData?.startupWarnings ??
      const <String>[];

  @override
  Future<AppSnapshot> refreshAndWaitComplete({int timeoutMs = 60000}) =>
      _service.refreshAndWaitComplete(timeoutMs: timeoutMs);

  @override
  Future<AppSnapshot> snapshotGet() => _service.snapshotGet();

  @override
  Future<SummarySnapshot> summarySnapshot() => _service.summarySnapshot();

  @override
  Future<SettingsData> settingsGet() => _service.settingsGet();

  @override
  Future<SettingsData> settingsSave(SettingsSaveInput input) =>
      _service.settingsSave(input);

  @override
  Future<ScanDirectoryData> scanDirectory(String path) =>
      _service.scanDirectory(path);

  @override
  Future<RepoAddData> repoAdd({required String path, String? groupId}) =>
      _service.repoAdd(path: path, groupId: groupId);

  @override
  Future<RepoRemoveData> repoRemove(String repoId) =>
      _service.repoRemove(repoId);

  @override
  Future<RepoOperationData> repoFetch(String repoId) =>
      _service.repoFetch(repoId);

  @override
  Future<RepoOperationData> repoPull(String repoId, {required bool confirmed}) =>
      _service.repoPull(repoId, confirmed: confirmed);

  @override
  Future<RepoOperationData> repoPush(String repoId, {required bool confirmed}) =>
      _service.repoPush(repoId, confirmed: confirmed);

  @override
  Future<ViewResult> viewQuery(ViewQuery query) => _service.viewQuery(query);

  @override
  Future<RepoTargetData> repoTarget({
    required String repoId,
    required RepoTargetKind kind,
  }) =>
      _service.repoTarget(repoId: repoId, kind: kind);
}
