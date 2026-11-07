// gitaview-native:// 深链协议路由与导航状态机。
//
// 语义来源：apps/windows/GitaView.Native/ProtocolNavigation.cs
// （C# 逐条对应移植）。
// - 路由解析：repoId 为 null 表示未过滤的仓库总列表。ID 只对受管仓库解析；
//   URI 参数永远不会变成路径或 Git 操作。只接受
//   `gitaview-native://repo/<单段 ID>`（scheme / host 大小写不敏感，path 保留
//   原大小写），带 userinfo / 显式端口 / 多段路径 / 空 ID 一律回总列表；
//   百分号编码只解码一次（%252F → %2F，不会把编码斜杠当路径分隔）。
// - 导航状态机：冷启动（SetReady 之前）只缓存最新一次请求；就绪后派发且
//   不重放；热激活每次请求都导航；未知 / 已删除 ID 回总列表。
// Dart 的 Uri 与 C# System.Uri 的端口语义不同（无显式端口时 Dart 返回 0、
// https 返回默认端口、C# 返回 -1），因此显式端口检测通过 authority 字符串
// 实现，行为与 C# 的 `uri.Port != -1` 一致。
library;

/// 深链路由结果（C# ProtocolRoute record struct 的对应物）。
class ProtocolRoute {
  /// null = 未过滤的仓库总列表（open / 未知 / 非法 / 已删 ID）。
  final String? repoId;

  const ProtocolRoute(this.repoId);

  /// 解析深链 URI。非法输入一律返回回总列表的路由，绝不抛异常。
  ///
  /// 与 C# `Uri.TryCreate(value, UriKind.Absolute)` + host/port/userinfo/path
  /// 校验逐条对应：
  /// - scheme 必须是 gitaview-native（大小写不敏感）；
  /// - host 必须是 repo（大小写不敏感），userinfo 必须为空，不得带显式端口；
  /// - path 必须是“/ + 单段 ID”（编码后不含第二个 '/'），解码恰好一次。
  static ProtocolRoute parse(String? value) {
    final Uri? uri = value == null ? null : Uri.tryParse(value);
    if (uri == null ||
        uri.scheme.toLowerCase() != 'gitaview-native' ||
        uri.userInfo.isNotEmpty ||
        _hasExplicitPort(uri) ||
        uri.host.toLowerCase() != 'repo') {
      return const ProtocolRoute(null);
    }
    // uri.path 与 C# AbsolutePath 一致：保持百分号编码、不含 query/fragment。
    final String path = uri.path;
    if (path.length < 2 || !path.startsWith('/') || path.substring(1).contains('/')) {
      return const ProtocolRoute(null);
    }
    final String id = Uri.decodeComponent(path.substring(1));
    return ProtocolRoute(id.isEmpty ? null : id);
  }

  /// 显式端口检测：authority 中 host 段（最后一个 '@' 之后）含 ':' 即带端口。
  /// host 必须是 "repo"（无冒号），因此 IPv6 host 也会在此或 host 校验被拒，
  /// 结果与 C# 一致（null 路由）。
  static bool _hasExplicitPort(Uri uri) {
    final String authority = uri.authority;
    final int at = authority.lastIndexOf('@');
    final String hostPort = at < 0 ? authority : authority.substring(at + 1);
    return hostPort.contains(':');
  }

  @override
  bool operator ==(Object other) =>
      other is ProtocolRoute && other.repoId == repoId;

  @override
  int get hashCode => repoId.hashCode;

  @override
  String toString() => 'ProtocolRoute(${repoId ?? 'null'})';
}

/// 深链导航状态机（C# ProtocolNavigation 类的对应物）。
///
/// C# 中回调在 UI 线程上调用 WinUI 导航；Dart 侧由宿主注入
/// [setReady] 的两个回调（仓库存在性查询 + 导航执行），保持逻辑可测。
class ProtocolNavigation {
  ProtocolRoute? _pending;
  bool Function(String repoId)? _repoExists;
  void Function(ProtocolRoute route)? _navigate;

  /// 接收一次激活请求：总是替换待派发路由（最新请求优先）；
  /// 已就绪则立即派发，未就绪则缓存等待 [setReady]。
  void request(String? uri) {
    _pending = ProtocolRoute.parse(uri);
    _dispatchPending();
  }

  /// 宿主就绪：注册仓库存在性查询与导航执行，并派发可能挂起的冷启动请求。
  /// 重复调用只更新回调，不重放已派发的历史请求。
  void setReady(
    bool Function(String repoId) repoExists,
    void Function(ProtocolRoute route) navigate,
  ) {
    _repoExists = repoExists;
    _navigate = navigate;
    _dispatchPending();
  }

  void _dispatchPending() {
    final ProtocolRoute? route = _pending;
    final exists = _repoExists;
    final navigate = _navigate;
    if (route == null || exists == null || navigate == null) return;
    _pending = null;
    final String? id = route.repoId;
    // 未知 / 已删除的仓库 ID 回总列表（null 路由）。
    final ProtocolRoute effective =
        id != null && !exists(id) ? const ProtocolRoute(null) : route;
    navigate(effective);
  }
}
