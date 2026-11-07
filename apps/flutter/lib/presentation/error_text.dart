// 稳定错误码 → 中文文案（UI 层唯一映射点；不解析中文决定控制流）。
//
// 语义来源：apps/windows/GitaView.Native/ErrorText.cs（C# 逐条对应移植）。
// - [ErrorText.forCode]：14 个稳定错误码 + 兜底“未知错误”，文案与 C# 完全一致；
// - [ErrorText.relationRow]：关系 → 列表行文案（状态不依赖颜色，始终有文字
//   与计数）；pending 不是关系，error 是独立 readState（契约层规则）；
// - [RepoStatusView]：契约行的轻量投影（绑定友好形状）。
library;

import '../contracts/contracts.dart';

/// 视图行轻量投影：把契约行转为绑定友好形状（C# RepoStatusView record 的
/// 对应物）。C# 的 Ahead/Behind 是 uint?，Dart 契约层是 int?（非负），
/// 语义一致。
class RepoStatusView {
  final String id;
  final String name;
  final String branch;
  final ReadState readState;
  final Relation? relation;
  final int? ahead;
  final int? behind;
  final String? errorCode;

  /// 契约允许缺失（pending 行无 Git 字段）；null 与 false 的分支不同。
  final bool? hasOrigin;
  final bool? hasCommit;

  const RepoStatusView({
    required this.id,
    required this.name,
    required this.branch,
    required this.readState,
    required this.relation,
    required this.ahead,
    required this.behind,
    required this.errorCode,
    this.hasOrigin,
    this.hasCommit,
  });

  /// 从契约行投影（对应 C# `RepoStatusView.From(RepoStatus)`）：
  /// branch 缺省为空串，errorCode 取自 error.code。
  factory RepoStatusView.from(RepoStatus row) => RepoStatusView(
        id: row.id,
        name: row.name,
        branch: row.branch ?? '',
        readState: row.readState,
        relation: row.relation,
        ahead: row.ahead,
        behind: row.behind,
        errorCode: row.error?.code,
        hasOrigin: row.hasOrigin,
        hasCommit: row.hasCommit,
      );
}

/// 错误文案映射（C# 静态类的对应物）。
abstract final class ErrorText {
  /// 稳定错误码 → 中文文案。未知码（含 null）一律“未知错误”，
  /// 绝不把文案用于控制流判断。
  static String forCode(String? code) => switch (code) {
        'INVALID_ARGUMENT' => '参数无效',
        'INVALID_PATH' => '路径无效或不是 Git 仓库',
        'GIT_NOT_FOUND' => '未找到 Git，请确认已安装并在 PATH 中',
        'REPO_NOT_FOUND' => '仓库不存在或已被移除',
        'REPO_BUSY' => '该仓库已有网络操作进行中，请稍后再试',
        'ORIGIN_REQUIRED' => '该仓库没有配置 origin 远端',
        'COMPARISON_REQUIRED' => '没有可比较的 origin 分支',
        'CONFIRMATION_REQUIRED' => '操作需要用户确认',
        'OPERATION_UNAVAILABLE' => '当前状态不允许此操作',
        'SETTINGS_CONFLICT' => '设置已被修改，请重载后重试',
        'SETTINGS_IO' => '设置读写失败',
        'GIT_FAILED' => 'Git 命令执行失败',
        'GIT_TIMEOUT' => 'Git 命令超时',
        'ENGINE_STOPPING' => '应用正在退出',
        _ => '未知错误',
      };

  /// 关系 → 列表行文案。pending → 尚未读取；error → 读取失败 + 错误文案；
  /// 其余按关系分支（no_remote 进一步区分无远端 / 未比较）。
  static String relationRow(RepoStatusView row) => switch (row.readState) {
        ReadState.pending => '尚未读取',
        ReadState.error => '读取失败：${forCode(row.errorCode)}',
        _ => switch (row.relation) {
            Relation.synced => '已同步',
            Relation.localAhead => '本地领先 ${row.ahead} 个提交',
            Relation.remoteAhead => '远端领先 ${row.behind} 个提交',
            Relation.diverged => '已分叉（本地 +${row.ahead} / 远端 +${row.behind}）',
            Relation.noRemote =>
              '${noComparisonLabel(row)}：${noComparisonHint(row)}',
            // relation 为 null（契约不允许出现在 ok 行，防御性兜底与 C# 一致）。
            _ => '未知',
          },
      };

  /// no_remote 行的标签：没有 origin 远端是“无远端”，有 origin 但没有
  /// 可比较结果是“未比较”。
  static String noComparisonLabel(RepoStatusView row) =>
      row.hasOrigin == false ? '无远端' : '未比较';

  /// no_remote 行的原因提示。分支顺序与 C# 属性模式求值顺序一致：
  /// hasOrigin=false 优先，hasCommit=false 次之，分离 HEAD 再次，
  /// 有 origin 但无可比较分支第四，其余兜底。
  static String noComparisonHint(RepoStatusView row) {
    if (row.hasOrigin == false) return '未配置 origin 远端';
    if (row.hasCommit == false) return '当前分支尚无提交';
    if (row.branch == 'HEAD') return '当前处于分离 HEAD 状态';
    if (row.hasOrigin == true) return '没有可比较的 origin 分支';
    return '没有可用的远端比较结果';
  }
}
