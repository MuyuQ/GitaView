// 引擎服务：主应用唯一扫描引擎的 Dart 宿主层。
//
// 架构对应 C# App.xaml.cs：
// - dart:ffi 调用阻塞；所有引擎调用在驻留工作 isolate 中执行，UI isolate
//   永不直接触碰 FFI。
// - 命令 isolate 串行执行业务命令（设置 / 仓库 / Git 操作 / 视图 / Summary /
//   快照读取）；watcher isolate 共享同一引擎地址跑 snapshot_wait 循环，
//   公开状态一变化就推送新快照与 Summary——与 C# 后台快照消费循环同构。
// - ABI 允许并发在途调用；destroy 仅在 Stopped 且无在途调用时成功，
//   BUSY 重试（30 秒排空）由命令 isolate 的 exit 序列负责。
// - 显式退出 = 停止推送 → request_stop → wait_stopped → destroy；
//   退出后扫描停止，无任何组件独立扫描或持久缓存。
//
// 协议：主 isolate 每条请求自带 reply 端口；所有 data 载荷为 JSON 字符串，
// 由契约层在接收端严格解码。
library;

import 'dart:async';
import 'dart:convert';
import 'dart:isolate';

import 'package:flutter/foundation.dart';

import '../contracts/contracts.dart';
import '../ffi/gv_ffi.dart';

/// 服务生命周期状态。
enum EngineServiceState { idle, starting, running, failed, stopping, stopped }

/// 引擎服务：ChangeNotifier，UI 订阅 [snapshot] / [summary] / 状态变化。
final class EngineService extends ChangeNotifier {
  final String dataDirectory;
  final String logDirectory;
  final String? libraryPath;

  EngineServiceState _state = EngineServiceState.idle;
  String? _startupError;
  EngineCreateData? _createData;
  AppSnapshot? _snapshot;
  SummarySnapshot? _summary;

  Isolate? _commandsIsolate;
  Isolate? _watcherIsolate;
  SendPort? _commandsPort;
  ReceivePort? _commandsReceive;
  ReceivePort? _exitReceive;
  ReceivePort? _eventsReceive;
  final Map<int, Completer<Object?>> _pending = {};
  int _nextRequestId = 1;
  bool _exitRequested = false;
  bool _disposed = false;

  EngineService({required this.dataDirectory, required this.logDirectory, this.libraryPath});

  EngineServiceState get state => _state;
  String? get startupError => _startupError;
  EngineCreateData? get createData => _createData;
  AppSnapshot? get snapshot => _snapshot;
  SummarySnapshot? get summary => _summary;
  bool get isRunning => _state == EngineServiceState.running;

  /// 启动引擎与两个驻留 isolate。失败进入 failed（保留错误信息，可重试）。
  Future<void> start() async {
    if (_state != EngineServiceState.idle && _state != EngineServiceState.failed) {
      return;
    }
    _setState(EngineServiceState.starting);
    _startupError = null;
    _exitRequested = false;
    _commandsReceive = ReceivePort();
    _eventsReceive = ReceivePort();
    _exitReceive = ReceivePort();
    _eventsReceive!.listen(_onWatcherEvent);
    try {
      final ready = Completer<SendPort>();
      _commandsReceive!.listen((message) {
        if (message is SendPort) {
          if (!ready.isCompleted && !_exitRequested) {
            ready.complete(message);
          } else {
            // 启动已被取消或已超时放弃：迟到 isolate 里可能刚创建完引擎，
            // 发 exit 命令让它走 stop → wait → destroy，不泄漏原生引擎。
            _sendLateExit(message);
          }
          return;
        }
        if (message is Map && message['startupFailed'] is String) {
          // 引擎创建失败：立即进入 failed，不等待 isolate 退出。
          if (!ready.isCompleted) {
            ready.completeError(GvCallException(GvStatus.error,
                message['startupFailed'] as String));
          }
          return;
        }
        _onCommandReply(message as Map);
      });
      final isolate = await Isolate.spawn(
        _commandsMain,
        _CommandsConfig(
          libraryPath: libraryPath,
          dataDirectory: dataDirectory,
          logDirectory: logDirectory,
          ready: _commandsReceive!.sendPort,
          events: _eventsReceive!.sendPort,
          exit: _exitReceive!.sendPort,
        ),
        errorsAreFatal: true,
        onExit: _exitReceive!.sendPort,
      );
      _commandsIsolate = isolate;
      _commandsPort = await ready.future.timeout(const Duration(seconds: 30));
      final createReply = await _call('create_info', const {});
      _createData = EngineCreateData.decode(jsonDecode(createReply as String));
      _setState(EngineServiceState.running);
    } catch (error) {
      _startupError = error.toString();
      await _teardownIsolates();
      if (_exitRequested) {
        // 启动期退出（requestExit 已把状态置为 stopped）：保持 stopped，
        // 不翻成 failed（用户已经放弃启动）。
        _setState(EngineServiceState.stopped);
      } else {
        _setState(EngineServiceState.failed);
      }
    }
  }

  /// 显式退出：停止推送 → request_stop → wait_stopped → destroy（BUSY 重试）。
  ///
  /// 退出全程不 kill isolate：watcher 可能正在阻塞原生调用中，kill 会段错误；
  /// 由命令 isolate 先令 watcher 完成当前等待并确认退出，再销毁引擎。
  Future<void> requestExit() async {
    if (_exitRequested) return;
    _exitRequested = true;
    if (_state != EngineServiceState.running) {
      await _teardownIsolates();
      _setState(EngineServiceState.stopped);
      return;
    }
    _setState(EngineServiceState.stopping);
    try {
      // C# RequestExit 按 network_timeout 600s + 余量给 660s：在途
      // Pull / Push 合法耗时可到 600s，超时不能当成功。外层 745s 覆盖
      // 内部最坏路径（watcher 停止 15s + 等待 660s + destroy 重试 60s），
      // 避免外层超时在命令 isolate 阻塞于原生调用时触发 kill。
      await _call('exit', const {}).timeout(const Duration(seconds: 745));
    } catch (_) {
      // isolate 已随引擎销毁退出或回复通道关闭：视为完成。
    }
    await _teardownIsolates();
    _setState(EngineServiceState.stopped);
  }

  // —— 面向 UI 的操作 ——

  /// 手动刷新并等待本次请求完成（对应刷新按钮语义）。
  Future<AppSnapshot> refreshAndWaitComplete({int timeoutMs = 60000}) async {
    final reply = await _call('refresh_request', const {});
    final request = RefreshRequestData.decode(jsonDecode(reply as String));
    final deadline = DateTime.now().add(Duration(milliseconds: timeoutMs));
    var current = await snapshotGet();
    while (current.refresh.completedRequestId < request.requestId) {
      if (DateTime.now().isAfter(deadline)) {
        throw TimeoutException('刷新未在期限内完成', Duration(milliseconds: timeoutMs));
      }
      await Future<void>.delayed(const Duration(milliseconds: 100));
      current = await snapshotGet();
    }
    return current;
  }

  Future<AppSnapshot> snapshotGet() async {
    final reply = await _call('snapshot_get', const {});
    return AppSnapshot.decode(jsonDecode(reply as String), 'snapshotGet');
  }

  Future<SummarySnapshot> summarySnapshot() async {
    final reply = await _call('summary_snapshot', const {});
    return SummarySnapshot.decode(jsonDecode(reply as String), 'summarySnapshot');
  }

  Future<SettingsData> settingsGet() async {
    final reply = await _call('settings_get', const {});
    return SettingsData.decode(jsonDecode(reply as String), 'settingsGet');
  }

  Future<SettingsData> settingsSave(SettingsSaveInput input) async {
    final reply = await _call('settings_save', input.toJson());
    return SettingsData.decode(jsonDecode(reply as String), 'settingsSave');
  }

  Future<ScanDirectoryData> scanDirectory(String path) async {
    final reply = await _call('scan_directory', scanDirectoryInputToJson(path));
    return ScanDirectoryData.decode(jsonDecode(reply as String));
  }

  Future<RepoAddData> repoAdd({required String path, String? groupId}) async {
    final reply = await _call('repo_add', repoAddInputToJson(path: path, groupId: groupId));
    return RepoAddData.decode(jsonDecode(reply as String));
  }

  Future<RepoRemoveData> repoRemove(String repoId) async {
    final reply = await _call('repo_remove', {'repoId': repoId});
    return RepoRemoveData.decode(jsonDecode(reply as String));
  }

  Future<RepoOperationData> repoFetch(String repoId) async {
    final reply = await _call('repo_fetch', {'repoId': repoId});
    return RepoOperationData.decode(jsonDecode(reply as String));
  }

  Future<RepoOperationData> repoPull(String repoId, {required bool confirmed}) async {
    final reply = await _call('repo_pull', {'repoId': repoId, 'confirmed': confirmed ? 1 : 0});
    return RepoOperationData.decode(jsonDecode(reply as String));
  }

  Future<RepoOperationData> repoPush(String repoId, {required bool confirmed}) async {
    final reply = await _call('repo_push', {'repoId': repoId, 'confirmed': confirmed ? 1 : 0});
    return RepoOperationData.decode(jsonDecode(reply as String));
  }

  Future<ViewResult> viewQuery(ViewQuery query) async {
    final reply = await _call('view_query', query.toJson());
    return ViewResult.decode(jsonDecode(reply as String), 'viewQuery');
  }

  Future<RepoTargetData> repoTarget({
    required String repoId,
    required RepoTargetKind kind,
  }) async {
    final reply = await _call('repo_target', repoTargetInputToJson(repoId: repoId, kind: kind));
    return RepoTargetData.decode(jsonDecode(reply as String));
  }

  // —— 内部协议 ——

  /// 迟到完成握手的命令 isolate：发一次 exit 命令令其销毁引擎后自退。
  void _sendLateExit(SendPort port) {
    try {
      port.send({
        'id': -1,
        'method': 'exit',
        'args': const <String, Object?>{},
        'reply': _commandsReceive!.sendPort,
      });
    } catch (_) {
      // 端口已关闭：isolate 已退出。
    }
  }

  Future<Object?> _call(String method, Map<String, Object?> args) {
    final port = _commandsPort;
    if (port == null) throw StateError('引擎服务未运行');
    final id = _nextRequestId++;
    final completer = Completer<Object?>();
    _pending[id] = completer;
    port.send({'id': id, 'method': method, 'args': args, 'reply': _commandsReceive!.sendPort});
    return completer.future;
  }

  void _onCommandReply(Map message) {
    if (_disposed) return;
    final id = message['id'] as int?;
    if (id == null) return;
    final completer = _pending.remove(id);
    if (completer == null || completer.isCompleted) return;
    if (message['ok'] == true) {
      completer.complete(message['payload']);
    } else {
      completer.completeError(GvCallException(
        (message['status'] as num?)?.toInt() ?? GvStatus.panic,
        message['code'] as String?,
      ));
    }
  }

  void _onWatcherEvent(Object? message) {
    // 服务关闭后 watcher 的在途推送仍可能到达：直接丢弃。
    if (_disposed || message is! Map) return;
    if (message['watcherError'] is String) {
      // 推送链路异常已上报：保留最后快照，等待退出序列收尾。
      return;
    }
    final snapshotJson = message['snapshot'];
    final summaryJson = message['summary'];
    if (snapshotJson is String) {
      _snapshot = AppSnapshot.decode(jsonDecode(snapshotJson), 'watcher');
    }
    if (summaryJson is String) {
      _summary = SummarySnapshot.decode(jsonDecode(summaryJson), 'watcher');
    }
    notifyListeners();
  }

  @override
  void dispose() {
    _disposed = true;
    if (_state != EngineServiceState.stopped) {
      // 兜底停机：不 kill 可能正阻塞在原生调用中的 isolate（kill 命中
      // 阻塞原生调用会段错误，见 requestExit 注释），而是发出与
      // requestExit 相同的退出命令后异步等待 isolate 自行退出。
      // 此后不再 notifyListeners（已 disposed）。
      _exitRequested = true;
      unawaited(_gracefulDispose());
    }
    super.dispose();
  }

  Future<void> _gracefulDispose() async {
    try {
      _commandsPort?.send({
        'id': -1,
        'method': 'exit',
        'args': const <String, Object?>{},
        'reply': _commandsReceive!.sendPort,
      });
    } catch (_) {
      // 端口已关闭 / isolate 已退出：直接进入收尾。
    }
    await _teardownIsolates();
  }

  void _setState(EngineServiceState state) {
    // dispose 后的迟到状态转换（如启动取消）不再通知已销毁的监听者。
    if (_disposed) return;
    _state = state;
    notifyListeners();
  }

  Future<void> _teardownIsolates() async {
    _commandsPort = null;
    // 等待 isolate 自行退出：watcher 单次等待 ≤1s、命令调用为有限时长，
    // 6 秒宽限足够；超时才 kill（兜底，此时不应有长阻塞原生调用）。
    final exitPort = _exitReceive;
    if (exitPort != null) {
      try {
        await exitPort.first.timeout(const Duration(seconds: 6));
      } catch (_) {
        // 超时 / 端口关闭：走下方 kill 兜底。
      }
    }
    _commandsIsolate?.kill(priority: Isolate.immediate);
    _watcherIsolate?.kill(priority: Isolate.immediate);
    _commandsIsolate = null;
    _watcherIsolate = null;
    _commandsReceive?.close();
    _eventsReceive?.close();
    _exitReceive?.close();
    _commandsReceive = null;
    _eventsReceive = null;
    _exitReceive = null;
    for (final completer in _pending.values) {
      if (!completer.isCompleted) {
        completer.completeError(StateError('引擎服务已关闭'));
      }
    }
    _pending.clear();
  }
}

class _CommandsConfig {
  final String? libraryPath;
  final String dataDirectory;
  final String logDirectory;
  final SendPort ready;
  final SendPort events;
  final SendPort exit;

  const _CommandsConfig({
    required this.libraryPath,
    required this.dataDirectory,
    required this.logDirectory,
    required this.ready,
    required this.events,
    required this.exit,
  });
}

/// 命令 isolate 入口：创建引擎、启动 watcher、串行应答业务命令。
Future<void> _commandsMain(_CommandsConfig config) async {
  final native = GvNative.load(path: config.libraryPath);
  // 与 C# App.xaml.cs 启动门禁一致：ABI 不匹配立即失败，不创建引擎。
  final loadedAbiVersion = native.abiVersion();
  if (loadedAbiVersion != gvAbiVersion) {
    config.ready.send({
      'startupFailed':
          'gitaview_ffi ABI 版本不匹配（期望 $gvAbiVersion，实际 $loadedAbiVersion）',
    });
    return;
  }
  final GvEngine engine;
  try {
    engine = GvEngine.create(
      native,
      dataDirectory: config.dataDirectory,
      logDirectory: config.logDirectory,
    );
  } catch (error) {
    // 启动失败立即回报主 isolate（否则 ready 握手会一直等到超时）。
    config.ready.send({'startupFailed': error.toString()});
    return;
  }

  late final SendPort watcherControl;
  final watcherReady = Completer<void>();
  final watcherStopped = Completer<void>();
  final control = ReceivePort()
    ..listen((message) {
      if (message is SendPort) {
        watcherControl = message;
        if (!watcherReady.isCompleted) watcherReady.complete();
      } else if (message == 'stopped') {
        if (!watcherStopped.isCompleted) watcherStopped.complete();
      } else if (message is Map) {
        // watcher 推送透传到主 isolate。
        config.events.send(message);
      }
    });

  await Isolate.spawn(
    _watcherMain,
    _WatcherConfig(
      libraryPath: config.libraryPath,
      engineAddress: engine.address,
      sessionId: engine.createData?.sessionId ?? '',
      startRevision: 0,
      control: control.sendPort,
    ),
    errorsAreFatal: false,
  );
  await watcherReady.future.timeout(const Duration(seconds: 30));

  // ready 端口第一条消息必须是命令端口（主 isolate 据此完成握手）。
  final commandPort = ReceivePort();
  config.ready.send(commandPort.sendPort);

  await for (final message in commandPort) {
    final request = message as Map;
    final reply = request['reply'] as SendPort?;
    final id = request['id'] as int?;
    final method = request['method'] as String?;
    if (method == 'exit') {
      // 退出序列：watcher 完成当前等待并自退（≤ 等待期限）→ 引擎停止 → 销毁。
      try {
        watcherControl.send('stop');
        await watcherStopped.future.timeout(const Duration(seconds: 15));
        await _stopAndDestroy(engine);
        reply?.send({'id': id, 'ok': true, 'payload': jsonEncode({'exited': true})});
      } catch (error) {
        // 任何退出路径都必须回复，否则主 isolate 等到超时。
        reply?.send({'id': id, 'ok': false, 'status': GvStatus.panic, 'code': error.toString()});
      }
      control.close();
      commandPort.close();
      return;
    }
    try {
      final args = ((request['args'] as Map?) ?? const {}).cast<String, Object?>();
      final payload = await _execute(engine, method!, args);
      reply?.send({'id': id, 'ok': true, 'payload': payload});
    } on GvCallException catch (error) {
      reply?.send({'id': id, 'ok': false, 'status': error.status, 'code': error.errorCode});
    } catch (error) {
      reply?.send({'id': id, 'ok': false, 'status': GvStatus.panic, 'code': error.toString()});
    }
  }
}

/// 停止 → 等待（660 秒：在途网络操作合法耗时上限 + 余量，对齐 C#）→
/// destroy（BUSY 重试：等在途调用排空）。
Future<void> _stopAndDestroy(GvEngine engine) async {
  engine.requestStop();
  var stopped = false;
  final waitDeadline = DateTime.now().add(const Duration(seconds: 660));
  while (!stopped && DateTime.now().isBefore(waitDeadline)) {
    stopped = engine.waitStopped(timeoutMs: 500);
  }
  if (!stopped) {
    throw const GvCallException(GvStatus.timeout, 'ENGINE_STOPPING');
  }
  final destroyDeadline = DateTime.now().add(const Duration(seconds: 60));
  while (true) {
    try {
      engine.destroy();
      return;
    } on GvCallException catch (error) {
      if (error.status != GvStatus.busy || DateTime.now().isAfter(destroyDeadline)) {
        rethrow;
      }
      await Future<void>.delayed(const Duration(milliseconds: 50));
    }
  }
}

Future<Object?> _execute(
  GvEngine engine,
  String method,
  Map<String, Object?> args,
) async {
  switch (method) {
    case 'create_info':
      return jsonEncode(engine.createData?.toJson() ?? {});
    case 'refresh_request':
      return jsonEncode(engine.refreshRequest().toJson());
    case 'snapshot_get':
      return jsonEncode(engine.snapshotGet().toJson());
    case 'summary_snapshot':
      return jsonEncode(engine.summarySnapshot().toJson());
    case 'settings_get':
      return jsonEncode(engine.settingsGet().toJson());
    case 'settings_save':
      return jsonEncode(engine.settingsSave(_parseSettingsSave(args)).toJson());
    case 'scan_directory':
      return jsonEncode(engine.scanDirectory(args['path']! as String).toJson());
    case 'repo_add':
      return jsonEncode(engine
          .repoAdd(path: args['path']! as String, groupId: args['groupId'] as String?)
          .toJson());
    case 'repo_remove':
      return jsonEncode(engine.repoRemove(args['repoId']! as String).toJson());
    case 'repo_fetch':
      return jsonEncode(engine.repoFetch(args['repoId']! as String).toJson());
    case 'repo_pull':
      return jsonEncode(engine
          .repoPull(args['repoId']! as String, confirmed: args['confirmed'] == 1)
          .toJson());
    case 'repo_push':
      return jsonEncode(engine
          .repoPush(args['repoId']! as String, confirmed: args['confirmed'] == 1)
          .toJson());
    case 'view_query':
      return jsonEncode(engine.viewQuery(ViewQuery.decode(args, 'viewQuery')).toJson());
    case 'repo_target':
      return jsonEncode(engine
          .repoTarget(
              repoId: args['repoId']! as String,
              kind: RepoTargetKind.parse(args['kind'], 'repo_target'))
          .toJson());
    default:
      throw ArgumentError('未知命令 $method');
  }
}

SettingsSaveInput _parseSettingsSave(Map<String, Object?> args) {
  final expectedRevision = args['expectedRevision'];
  final settings = args['settings'];
  if (expectedRevision is! int || settings is! Map<String, Object?>) {
    throw const GvCallException(GvStatus.invalidArgument, 'INVALID_ARGUMENT');
  }
  final groups = (settings['groups'] as List<Object?>? ?? const [])
      .map((group) => GroupSettings.decode(group, 'settings.groups'))
      .toList();
  final repoGroups = (settings['repoGroups'] as List<Object?>? ?? const [])
      .map((assignment) => RepoGroupAssignment(
            repoId: (assignment as Map<String, Object?>)['repoId']! as String,
            groupId: assignment['groupId'] as String?,
          ))
      .toList();
  final repoAliases = (settings['repoAliases'] as List<Object?>? ?? const [])
      .map((assignment) => RepoAliasAssignment(
            repoId: (assignment as Map<String, Object?>)['repoId']! as String,
            alias: assignment['alias'] as String?,
          ))
      .toList();
  final refreshRaw = settings['refresh']! as Map<String, Object?>;
  return SettingsSaveInput(
    expectedRevision: expectedRevision,
    settings: SettingsEditable(
      groups: groups,
      defaultGroupId: settings['defaultGroupId'] as String?,
      refresh: RefreshPolicy(
        enabled: refreshRaw['enabled']! as bool,
        intervalMinutes: refreshRaw['intervalMinutes']! as int,
      ),
      repoGroups: repoGroups,
      repoAliases: repoAliases,
    ),
  );
}

class _WatcherConfig {
  final String? libraryPath;
  final int engineAddress;
  final String sessionId;
  final int startRevision;
  final SendPort control;

  const _WatcherConfig({
    required this.libraryPath,
    required this.engineAddress,
    required this.sessionId,
    required this.startRevision,
    required this.control,
  });
}

/// watcher 入口：snapshot_wait 循环。变化即推送快照与 Summary；
/// TIMEOUT 静默续等；收到 'stop' 后完成当前等待即自退并发回确认——
/// 全程不发生 kill（kill 命中阻塞原生调用会段错误），所有权在命令 isolate。
Future<void> _watcherMain(_WatcherConfig config) async {
  final native = GvNative.load(path: config.libraryPath);
  final engine = GvEngine.viewAt(native, config.engineAddress);

  var stopRequested = false;
  final control = ReceivePort();
  control.listen((message) {
    if (message == 'stop') stopRequested = true;
  });
  config.control.send(control.sendPort);

  var revision = config.startRevision;
  try {
    while (!stopRequested) {
      // 让渡事件循环：同步 FFI 调用之间必须处理控制消息（stop），
      // 否则纯同步循环会永久饿死 ReceivePort 监听。
      await Future<void>.delayed(const Duration(milliseconds: 1));
      try {
        final (result, status) = engine.snapshotWait(
          sessionId: config.sessionId,
          stateRevision: revision,
          timeoutMs: 1000,
        );
        if (stopRequested) break;
        if (status == GvStatus.ok && result.ok) {
          final snapshot =
              result.expectData((json) => AppSnapshot.decode(json, 'watcher'));
          revision = snapshot.stateRevision;
          final summary = engine.summarySnapshot();
          config.control.send({
            'snapshot': jsonEncode(snapshot.toJson()),
            'summary': jsonEncode(summary.toJson()),
          });
        }
        // TIMEOUT：无变化继续等；STOPPING：引擎退出中，等待终态或确认退出。
      } catch (error) {
        // 推送链路异常（解码失败等）：上报后终止推送，UI 保留最后快照；
        // 必须仍发 'stopped'，否则退出序列要白等 15 秒并跳过引擎清理。
        config.control.send({'watcherError': error.toString()});
        break;
      }
    }
  } finally {
    config.control.send('stopped');
    control.close();
  }
}
