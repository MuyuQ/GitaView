// GitaView FFI 层：冻结 C ABI v2（20 个 gv_* 导出）的 dart:ffi 绑定。
//
// 与 C# GitaViewEngine / NativeMethods 一致的语义：
// - GvBuffer = {data: *mut u8, len: usize}；由 gv_buffer_free 释放并清零。
// - 返回 int32 状态：OK=0 ERROR=1 TIMEOUT=2 STOPPING=3 BUSY=4 INVALID_ARGUMENT=5 PANIC=6。
// - 成功 / 业务错误返回 UTF-8 JSON envelope；TIMEOUT / STOPPING 允许空缓冲区。
// - confirmed 仅接受 0 / 1；原始 ID 以 UTF-8 字节传入（非 JSON 字符串）。
// - json 输入非法 / 缓冲区指针为 null → INVALID_ARGUMENT。
//
// dart:ffi 调用是阻塞的：在 UI isolate 直接调用会冻结界面。
// 长任务（快照等待、Git 操作）必须经 EngineService 的工作 isolate 执行。
library;

import 'dart:convert';
import 'dart:ffi';
import 'dart:io' show Platform;

import 'package:ffi/ffi.dart';

import '../contracts/contracts.dart';

/// C ABI 版本；加载后先核对再调用其他函数。
const int gvAbiVersion = 2;

/// FFI 状态码。
abstract final class GvStatus {
  static const int ok = 0;
  static const int error = 1;
  static const int timeout = 2;
  static const int stopping = 3;
  static const int busy = 4;
  static const int invalidArgument = 5;
  static const int panic = 6;

  static String name(int status) => switch (status) {
        ok => 'OK',
        error => 'ERROR',
        timeout => 'TIMEOUT',
        stopping => 'STOPPING',
        busy => 'BUSY',
        invalidArgument => 'INVALID_ARGUMENT',
        panic => 'PANIC',
        _ => 'UNKNOWN($status)',
      };
}

/// FFI 调用失败：状态码与可选的业务错误码。
class GvCallException implements Exception {
  final int status;
  final String? errorCode;

  const GvCallException(this.status, this.errorCode);

  @override
  String toString() =>
      'GvCallException(status: ${GvStatus.name(status)}, code: ${errorCode ?? '—'})';
}

/// 绑定签名（与 include/gitaview.h 一一对应）。
/// `Gv…C` 为 C 侧原生类型；`Gv…D` 为 Dart 侧映射（Int32/Uint32/IntPtr → int）。
typedef GvAbiVersionC = Uint32 Function();
typedef GvAbiVersionD = int Function();
typedef GvEngineCreateC = Int32 Function(
    Pointer<Uint8>, IntPtr, Pointer<Pointer<Void>>, Pointer<GvBufferStruct>);
typedef GvEngineCreateD = int Function(
    Pointer<Uint8>, int, Pointer<Pointer<Void>>, Pointer<GvBufferStruct>);
typedef GvEngineVoidC = Int32 Function(Pointer<Void>, Pointer<GvBufferStruct>);
typedef GvEngineVoidD = int Function(Pointer<Void>, Pointer<GvBufferStruct>);
typedef GvEngineWaitC = Int32 Function(Pointer<Void>, Uint32, Pointer<GvBufferStruct>);
typedef GvEngineWaitD = int Function(Pointer<Void>, int, Pointer<GvBufferStruct>);
typedef GvEngineDestroyC = Int32 Function(Pointer<Pointer<Void>>);
typedef GvEngineDestroyD = int Function(Pointer<Pointer<Void>>);
typedef GvEngineJsonC = Int32 Function(
    Pointer<Void>, Pointer<Uint8>, IntPtr, Pointer<GvBufferStruct>);
typedef GvEngineJsonD = int Function(
    Pointer<Void>, Pointer<Uint8>, int, Pointer<GvBufferStruct>);
// 原始 ID 输入与 JSON 输入在 C 签名上同形（ptr, len）。
typedef GvEngineRawIdC = GvEngineJsonC;
typedef GvEngineRawIdD = GvEngineJsonD;
typedef GvEngineConfirmedC = Int32 Function(
    Pointer<Void>, Pointer<Uint8>, IntPtr, Uint8, Pointer<GvBufferStruct>);
typedef GvEngineConfirmedD = int Function(
    Pointer<Void>, Pointer<Uint8>, int, int, Pointer<GvBufferStruct>);
typedef GvEngineWaitCursorC = Int32 Function(
    Pointer<Void>, Pointer<Uint8>, IntPtr, Uint32, Pointer<GvBufferStruct>);
typedef GvEngineWaitCursorD = int Function(
    Pointer<Void>, Pointer<Uint8>, int, int, Pointer<GvBufferStruct>);
typedef GvBufferFreeC = Void Function(Pointer<GvBufferStruct>);
typedef GvBufferFreeD = void Function(Pointer<GvBufferStruct>);

/// `repr(C)` 缓冲区：与 Rust `GvBuffer` 布局一致。
///
/// `len` 对应 Rust `usize`（指针宽度）：dart:ffi 结构体整数字段以注解标宽，
/// 本项目支持目标（x64 Windows、arm64 / x86_64 macOS）全为 64 位，
/// usize 与 Uint64 布局一致。
final class GvBufferStruct extends Struct {
  /// 对应 Rust `*mut u8` 数据指针。
  external Pointer<Uint8> data;

  /// 对应 Rust `usize` 长度。
  @Uint64()
  external int len;

  /// 是否携带已分配字节。
  bool get hasBytes => data != nullptr && len > 0;
}

/// 引擎句柄包装：`Pointer<Void>` 在 Dart 层不透明，销毁后指针归 null。
final class GvEngineHandle {
  Pointer<Void> _pointer;

  GvEngineHandle._(this._pointer);

  Pointer<Void>? get pointer => _pointer;

  bool get isDestroyed => _pointer == nullptr;
}

/// 本地库入口：解析平台动态库并绑定全部导出。
final class GvNative {
  final DynamicLibrary lib;

  GvNative(this.lib);

  /// 按平台默认名称加载本地库。测试可用 [path] 显式指定 dylib / DLL。
  static GvNative load({String? path}) {
    final library =
        path != null ? DynamicLibrary.open(path) : DynamicLibrary.open(_defaultName());
    return GvNative(library);
  }

  static String _defaultName() => switch (Platform.operatingSystem) {
        'windows' => 'gitaview_ffi.dll',
        'macos' => 'libgitaview_ffi.dylib',
        _ => 'libgitaview_ffi.so',
      };

  late final GvAbiVersionD abiVersion =
      lib.lookupFunction<GvAbiVersionC, GvAbiVersionD>('gv_abi_version');
  late final GvEngineCreateD engineCreate =
      lib.lookupFunction<GvEngineCreateC, GvEngineCreateD>('gv_engine_create');
  late final GvEngineVoidD engineRequestStop =
      lib.lookupFunction<GvEngineVoidC, GvEngineVoidD>('gv_engine_request_stop');
  late final GvEngineWaitD engineWaitStopped =
      lib.lookupFunction<GvEngineWaitC, GvEngineWaitD>('gv_engine_wait_stopped');
  late final GvEngineDestroyD engineDestroy =
      lib.lookupFunction<GvEngineDestroyC, GvEngineDestroyD>('gv_engine_destroy');
  late final GvEngineVoidD settingsGet =
      lib.lookupFunction<GvEngineVoidC, GvEngineVoidD>('gv_settings_get');
  late final GvEngineJsonD settingsSave =
      lib.lookupFunction<GvEngineJsonC, GvEngineJsonD>('gv_settings_save');
  late final GvEngineJsonD scanDirectory =
      lib.lookupFunction<GvEngineJsonC, GvEngineJsonD>('gv_scan_directory');
  late final GvEngineJsonD repoAdd =
      lib.lookupFunction<GvEngineJsonC, GvEngineJsonD>('gv_repo_add');
  late final GvEngineRawIdD repoRemove =
      lib.lookupFunction<GvEngineRawIdC, GvEngineRawIdD>('gv_repo_remove');
  late final GvEngineRawIdD repoFetch =
      lib.lookupFunction<GvEngineRawIdC, GvEngineRawIdD>('gv_repo_fetch');
  late final GvEngineConfirmedD repoPull =
      lib.lookupFunction<GvEngineConfirmedC, GvEngineConfirmedD>('gv_repo_pull');
  late final GvEngineConfirmedD repoPush =
      lib.lookupFunction<GvEngineConfirmedC, GvEngineConfirmedD>('gv_repo_push');
  late final GvEngineVoidD refreshRequest =
      lib.lookupFunction<GvEngineVoidC, GvEngineVoidD>('gv_refresh_request');
  late final GvEngineVoidD snapshotGet =
      lib.lookupFunction<GvEngineVoidC, GvEngineVoidD>('gv_snapshot_get');
  late final GvEngineWaitCursorD snapshotWait =
      lib.lookupFunction<GvEngineWaitCursorC, GvEngineWaitCursorD>('gv_snapshot_wait');
  late final GvEngineJsonD viewQuery =
      lib.lookupFunction<GvEngineJsonC, GvEngineJsonD>('gv_view_query');
  late final GvEngineVoidD summarySnapshot =
      lib.lookupFunction<GvEngineVoidC, GvEngineVoidD>('gv_summary_snapshot');
  late final GvEngineJsonD repoTarget =
      lib.lookupFunction<GvEngineJsonC, GvEngineJsonD>('gv_repo_target');
  late final GvBufferFreeD bufferFree =
      lib.lookupFunction<GvBufferFreeC, GvBufferFreeD>('gv_buffer_free');
}

/// 一次 FFI 调用的结果。
final class GvCallResult {
  final bool ok;
  final Object? data;
  final int status;
  final String? errorCode;

  const GvCallResult._({
    required this.ok,
    required this.status,
    this.data,
    this.errorCode,
  });

  factory GvCallResult.ok(Object? data) =>
      GvCallResult._(ok: true, status: GvStatus.ok, data: data);

  factory GvCallResult.businessError(String? code) =>
      GvCallResult._(ok: false, status: GvStatus.error, errorCode: code);

  factory GvCallResult.statusError(int status, String? code) =>
      GvCallResult._(ok: false, status: status, errorCode: code);

  /// 成功 data 解码；业务错误 / 状态错误抛 GvCallException。
  T expectData<T>(T Function(Object? json) decode) {
    if (!ok) throw GvCallException(status, errorCode);
    return decode(data);
  }
}

/// 阻塞调用的统一出口：执行 [invoke]，取回字节、解析 envelope、释放缓冲区。
///
/// reply 缓冲区在调用前清零、调用后无条件释放（含异常路径）；
/// TIMEOUT / STOPPING 的空缓冲区合法，由调用方凭状态码分支。
(GvCallResult, int) _runCall(
  GvNative native,
  int Function(Pointer<GvBufferStruct>) invoke,
) {
  final reply = malloc<GvBufferStruct>();
  _zeroBuffer(reply.ref);
  var status = GvStatus.panic;
  try {
    status = invoke(reply);
    final ref = reply.ref;
    String? payload;
    if (ref.hasBytes) {
      payload = utf8.decode(ref.data.asTypedList(ref.len), allowMalformed: false);
    }
    return (_decodeResult(status, payload), status);
  } finally {
    // 释放并把结构体清零：与 Rust buffer::free 的清零约定一致。
    native.bufferFree(reply);
    _zeroBuffer(reply.ref);
    malloc.free(reply);
  }
}

void _zeroBuffer(GvBufferStruct ref) {
  ref.data = nullptr;
  ref.len = 0;
}

GvCallResult _decodeResult(int status, String? payload) {
  if (status == GvStatus.ok || status == GvStatus.error) {
    if (payload == null) {
      // 非 OK 状态允许空缓冲区；OK / ERROR 缺 envelope 属于绑定错误。
      throw GvCallException(status, null);
    }
    final envelope = Envelope.decode(jsonDecode(payload));
    if (envelope.ok) {
      return GvCallResult.ok(envelope.data);
    }
    // error.message 属于契约可选字段：解码保证严格性，控制流只看 code。
    return GvCallResult.businessError(envelope.errorCode);
  }
  // TIMEOUT / STOPPING：空缓冲区合法；其余非 OK 状态仍解析稳定错误 envelope。
  if (payload != null) {
    final envelope = Envelope.decode(jsonDecode(payload));
    return GvCallResult.statusError(status, envelope.errorCode);
  }
  return GvCallResult.statusError(status, null);
}

/// 引擎级封装：阻塞调用 + 原始指针生命周期。
///
/// 实例方法只做参数编码、调用与结果解码；调用是否放行、并发与
/// 驻留 isolate 的调度由上层 EngineService 负责。
final class GvEngine {
  final GvNative native;
  final GvEngineHandle _handle;

  GvEngine._(this.native, this._handle);

  /// gv_engine_create 成功 data（会话与启动警告）。
  EngineCreateData? createData;

  /// gv_engine_create 输入绝对数据 / 日志目录；失败抛 GvCallException，指针保证为 null。
  factory GvEngine.create(
    GvNative native, {
    required String dataDirectory,
    required String logDirectory,
  }) {
    final json = utf8.encode(jsonEncode(engineCreateInputToJson(
        dataDirectory: dataDirectory, logDirectory: logDirectory)));
    final input = malloc<Uint8>(json.length);
    input.asTypedList(json.length).setAll(0, json);
    final engineOut = malloc<Pointer<Void>>();
    engineOut.value = nullptr;
    try {
      final (result, status) = _runCall(
        native,
        (reply) => native.engineCreate(input, json.length, engineOut, reply),
      );
      final pointer = engineOut.value;
      if (status != GvStatus.ok || pointer == nullptr) {
        throw GvCallException(status, result.errorCode);
      }
      final engine = GvEngine._(native, GvEngineHandle._(pointer));
      try {
        engine.createData = result.expectData(EngineCreateData.decode);
        return engine;
      } catch (_) {
        // 指针已到手但 envelope 解码失败：排空引擎（stop → wait → destroy），
        // 不把已创建的原生引擎随异常丢弃。
        try {
          engine.requestStop();
          if (engine.waitStopped(timeoutMs: 10000)) {
            engine.destroy();
          }
        } catch (_) {
          // 排空失败只能交给进程退出兜底；不能吞掉原始异常。
        }
        rethrow;
      }
    } finally {
      malloc.free(input);
      malloc.free(engineOut);
    }
  }

  bool get isDestroyed => _handle.isDestroyed;

  /// 引擎指针地址：跨 isolate 共享同一引擎时以地址传递（watcher isolate）。
  int get address => _handle.pointer!.address;

  /// 以已知地址构造引擎视图（watcher isolate 用）：共享同一原生句柄，
  /// 不得调用 [destroy]——所有权在创建方。
  factory GvEngine.viewAt(GvNative native, int address) =>
      GvEngine._(native, GvEngineHandle._(Pointer<Void>.fromAddress(address)));

  Pointer<Void> get _engine {
    if (isDestroyed) {
      throw StateError('引擎已销毁');
    }
    return _handle.pointer!;
  }

  // —— JSON / 原始 ID / confirmed 输入的统一编码 ——

  (GvCallResult, int) _callJson(GvEngineJsonD fn, Map<String, Object?> input) =>
      _callBytes(fn, utf8.encode(jsonEncode(input)));

  (GvCallResult, int) _callRawId(GvEngineRawIdD fn, String repoId) =>
      _callBytes(fn, utf8.encode(repoId));

  (GvCallResult, int) _callBytes(GvEngineJsonD fn, List<int> bytes) {
    final Pointer<Uint8> buffer;
    if (bytes.isEmpty) {
      buffer = nullptr;
    } else {
      buffer = malloc<Uint8>(bytes.length);
      buffer.asTypedList(bytes.length).setAll(0, bytes);
    }
    try {
      return _runCall(native, (reply) => fn(_engine, buffer, bytes.length, reply));
    } finally {
      if (buffer != nullptr) malloc.free(buffer);
    }
  }

  (GvCallResult, int) _callConfirmed(
    GvEngineConfirmedD fn,
    String repoId,
    bool confirmed,
  ) {
    final bytes = utf8.encode(repoId);
    final Pointer<Uint8> buffer;
    if (bytes.isEmpty) {
      buffer = nullptr;
    } else {
      buffer = malloc<Uint8>(bytes.length);
      buffer.asTypedList(bytes.length).setAll(0, bytes);
    }
    try {
      return _runCall(
        native,
        (reply) => fn(_engine, buffer, bytes.length, confirmed ? 1 : 0, reply),
      );
    } finally {
      if (buffer != nullptr) malloc.free(buffer);
    }
  }

  // —— 设置 ——

  SettingsData settingsGet() => _runCall(native, (reply) => native.settingsGet(_engine, reply))
      .$1
      .expectData((json) => SettingsData.decode(json, 'settingsGet'));

  SettingsData settingsSave(SettingsSaveInput input) => _callJson(native.settingsSave, input.toJson())
      .$1
      .expectData((json) => SettingsData.decode(json, 'settingsSave'));

  // —— 仓库管理 ——

  ScanDirectoryData scanDirectory(String path) =>
      _callJson(native.scanDirectory, scanDirectoryInputToJson(path))
          .$1
          .expectData(ScanDirectoryData.decode);

  RepoAddData repoAdd({required String path, String? groupId}) =>
      _callJson(native.repoAdd, repoAddInputToJson(path: path, groupId: groupId))
          .$1
          .expectData(RepoAddData.decode);

  RepoRemoveData repoRemove(String repoId) =>
      _callRawId(native.repoRemove, repoId).$1.expectData(RepoRemoveData.decode);

  // —— Git 操作 ——

  RepoOperationData repoFetch(String repoId) =>
      _callRawId(native.repoFetch, repoId).$1.expectData(RepoOperationData.decode);

  RepoOperationData repoPull(String repoId, {required bool confirmed}) =>
      _callConfirmed(native.repoPull, repoId, confirmed).$1.expectData(RepoOperationData.decode);

  RepoOperationData repoPush(String repoId, {required bool confirmed}) =>
      _callConfirmed(native.repoPush, repoId, confirmed).$1.expectData(RepoOperationData.decode);

  // —— 刷新与快照 ——

  RefreshRequestData refreshRequest() =>
      _runCall(native, (reply) => native.refreshRequest(_engine, reply))
          .$1
          .expectData(RefreshRequestData.decode);

  AppSnapshot snapshotGet() =>
      _runCall(native, (reply) => native.snapshotGet(_engine, reply))
          .$1
          .expectData((json) => AppSnapshot.decode(json, 'snapshotGet'));

  /// 等待公开状态变化。返回 (result, status)：TIMEOUT / STOPPING 时
  /// result.ok 为 false，由调用方凭状态分支。
  (GvCallResult, int) snapshotWait({
    required String sessionId,
    required int stateRevision,
    required int timeoutMs,
  }) {
    final bytes = utf8.encode(jsonEncode(
        snapshotWaitInputToJson(sessionId: sessionId, stateRevision: stateRevision)));
    final buffer = malloc<Uint8>(bytes.length);
    buffer.asTypedList(bytes.length).setAll(0, bytes);
    try {
      return _runCall(
        native,
        (reply) => native.snapshotWait(_engine, buffer, bytes.length, timeoutMs, reply),
      );
    } finally {
      malloc.free(buffer);
    }
  }

  // —— 视图与 Summary ——

  ViewResult viewQuery(ViewQuery query) => _callJson(native.viewQuery, query.toJson())
      .$1
      .expectData((json) => ViewResult.decode(json, 'viewQuery'));

  SummarySnapshot summarySnapshot() =>
      _runCall(native, (reply) => native.summarySnapshot(_engine, reply))
          .$1
          .expectData((json) => SummarySnapshot.decode(json, 'summarySnapshot'));

  // —— 打开目标 ——

  RepoTargetData repoTarget({required String repoId, required RepoTargetKind kind}) =>
      _callJson(native.repoTarget, repoTargetInputToJson(repoId: repoId, kind: kind))
          .$1
          .expectData(RepoTargetData.decode);

  // —— 生命周期 ——

  /// 请求停止（幂等）；成功返回 lifecycle data。
  LifecycleData requestStop() {
    final (result, status) =
        _runCall(native, (reply) => native.engineRequestStop(_engine, reply));
    if (status != GvStatus.ok) throw GvCallException(status, result.errorCode);
    return result.expectData(LifecycleData.decode);
  }

  /// 等待停止完成。返回是否已 stopped；TIMEOUT（清理未完成）返回 false。
  bool waitStopped({required int timeoutMs}) {
    final (result, status) = _runCall(
      native,
      (reply) => native.engineWaitStopped(_engine, timeoutMs, reply),
    );
    if (status == GvStatus.timeout) return false;
    if (status != GvStatus.ok) throw GvCallException(status, result.errorCode);
    return result.expectData(LifecycleData.decode).lifecycle == GvLifecycle.stopped;
  }

  /// 销毁引擎：仅在 Stopped 且无在途调用时成功。成功后本实例不可再用。
  void destroy() {
    if (isDestroyed) throw StateError('引擎已销毁');
    final engineOut = malloc<Pointer<Void>>();
    engineOut.value = _handle.pointer!;
    try {
      final status = native.engineDestroy(engineOut);
      if (status == GvStatus.ok) {
        _handle._pointer = nullptr;
        return;
      }
      throw GvCallException(status, status == GvStatus.busy ? 'REPO_BUSY' : 'INVALID_ARGUMENT');
    } finally {
      malloc.free(engineOut);
    }
  }
}
