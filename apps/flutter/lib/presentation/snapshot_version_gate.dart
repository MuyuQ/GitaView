// 快照版本闸门（展示层纯逻辑）。
//
// 语义来源：apps/windows/GitaView.Native/SnapshotVersionGate.cs（C# 逐条对应移植）。
// 在调度读取前开启会话、读取完成后验收结果：
// - 同一会话内只接受不小于当前已显示 revision 的结果；旧查询晚到不能覆盖新内容；
// - 新会话重置 revision 基线（接受新会话的初始 revision），并拒绝旧会话的迟到结果。
// C# 使用 ulong 计数；Dart 侧沿用契约层的非负 int（契约保证 revision >= 0），
// 比较语义（`revision < 当前基线` 则拒绝）完全一致。
library;

/// 快照验收闸门：UI 线程在调度读取前调用 [beginSession]，读取完成后调用
/// [tryAccept] 决定是否把结果交给展示层。
class SnapshotVersionGate {
  String? _session;
  int _revision = 0;

  /// 开启（或重复声明）一个会话。换会话时把 revision 基线归零；
  /// 同一会话重复调用是空操作，不会清掉已接受的进度。
  void beginSession(String session) {
    if (_session == session) return;
    _session = session;
    _revision = 0;
  }

  /// 尝试接受一次读取结果。会话不匹配或 revision 落后于当前基线时拒绝。
  bool tryAccept(String session, int revision) {
    if (session != _session || revision < _revision) return false;
    _revision = revision;
    return true;
  }
}
