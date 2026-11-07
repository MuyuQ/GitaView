// 设置编辑请求构造（ExpectedRevision 事务语义）。
//
// 语义来源：apps/windows/GitaView.Native/SettingsEditRequest.cs
// （C# 逐条对应移植）。
// 在 UI 线程上、调度保存之前构造；候选数据与其 expectedRevision 永远来自
// 同一次加载的设置快照。核心在保存时用 expectedRevision 做 CAS 校验，
// 冲突返回 SETTINGS_CONFLICT（该冲突行为由 Rust 核心 / FFI 测试覆盖，
// 不属于本纯逻辑层）。
library;

import '../contracts/contracts.dart';

/// 设置编辑请求构造（C# 静态类的对应物）。
abstract final class SettingsEditRequest {
  /// 修改分组列表（增删改）：候选列表完整替换 groups，其余字段保持当前值。
  static SettingsSaveInput forGroups(
    SettingsData current,
    List<GroupSettings> groups,
  ) =>
      _capture(current, groups, current.defaultGroupId, const []);

  /// 修改默认分组：groups 保持当前值，仅 defaultGroupId 变化。
  static SettingsSaveInput forDefaultGroup(SettingsData current, String? groupId) =>
      _capture(current, current.groups, groupId, const []);

  /// 修改单个仓库归属：groups / defaultGroupId 保持当前值，
  /// 仅携带这一条归属覆盖（null groupId = 未分组）。
  static SettingsSaveInput forRepoGroup(
    SettingsData current,
    String repoId,
    String? groupId,
  ) =>
      _capture(current, current.groups, current.defaultGroupId,
          [RepoGroupAssignment(repoId: repoId, groupId: groupId)]);

  /// 组装保存输入。groups 与 assignments 都做快照复制（对应 C# 的 `[.. groups]`）：
  /// 请求拥有自己的候选列表，调用方随后修改原列表不影响已构造的请求。
  static SettingsSaveInput _capture(
    SettingsData current,
    List<GroupSettings> groups,
    String? defaultGroupId,
    List<RepoGroupAssignment> assignments,
  ) =>
      SettingsSaveInput(
        expectedRevision: current.revision,
        settings: SettingsEditable(
          groups: List.of(groups),
          defaultGroupId: defaultGroupId,
          refresh: current.refresh,
          repoGroups: List.of(assignments),
        ),
      );
}
