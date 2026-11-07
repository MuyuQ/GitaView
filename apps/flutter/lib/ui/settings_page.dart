// 设置页：两分区导航（仓库设置 / 常规设置）+ 固定位置反馈条 + 返回。
//
// 行为规格：apps/windows/GitaView.Native/SettingsPage.xaml(.cs)（语义对齐）：
// - 仓库设置：添加仓库（单仓库 / 扫描目录与候选勾选）、已管理仓库
//   （名称 / 路径 / 归属下拉 / 移除——只删管理记录）、分组管理（添加 / 删除）；
// - 常规设置：自动刷新（开关 + 间隔 + 保存，“已保存”反馈位置固定）、
//   默认分组、开机启动状态展示、操作确认安全规则说明（不可修改）；
// - 全部变更走 SettingsEditRequest 事务；SETTINGS_CONFLICT 显示重载入口；
// - 两个分区常驻组件树（IndexedStack），返回状态页后上下文保留
//   （对应 C# NavigationCacheMode=Required + Visibility 切换）。
// 平台差异：目录选择器与开机启动状态为 Windows 阶段接线（见 AppController 钩子）；
// C# 的“外观”分区依赖 LocalSettings 持久化，随主题接线一并后补，本阶段跟随系统。
library;

import 'package:fluent_ui/fluent_ui.dart';

import '../app_controller.dart';
import '../contracts/contracts.dart';
import '../presentation/settings_group_presentation.dart';
import 'theme/tokens.dart';

/// 设置页。
class SettingsPage extends StatefulWidget {
  final AppController controller;

  /// 返回状态页（C# OnTitleBarBackRequested → NavigateBackToStatus）。
  final VoidCallback onBack;

  const SettingsPage({super.key, required this.controller, required this.onBack});

  @override
  State<SettingsPage> createState() => _SettingsPageState();
}

class _SettingsPageState extends State<SettingsPage> {
  bool _repoSection = true;
  final TextEditingController _newGroupName = TextEditingController();

  @override
  void initState() {
    super.initState();
    widget.controller.loadSettings();
  }

  @override
  void dispose() {
    _newGroupName.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final colors = gvColorsOf(context);
    final controller = widget.controller;
    return AnimatedBuilder(
      animation: controller,
      builder: (context, _) => Column(
        children: [
          // 标题栏内容区：返回 + “设置”（C# TitleBar IsBackButtonVisible）。
          Container(
            height: GvSpacing.toolbarHeight,
            color: colors.surface,
            padding: const EdgeInsets.only(left: 4),
            child: Row(
              children: [
                HoverButton(
                  onPressed: widget.onBack,
                  semanticLabel: '返回',
                  builder: (context, states) => Padding(
                    padding: const EdgeInsets.all(9),
                    child: Icon(
                      FluentIcons.back,
                      size: 16,
                      color: states.contains(WidgetState.hovered) ? colors.ink : colors.muted,
                    ),
                  ),
                ),
                const SizedBox(width: 8),
                Text('设置', style: GvTextStyles.brand(colors.ink)),
              ],
            ),
          ),
          Expanded(
            child: Padding(
              padding: const EdgeInsets.fromLTRB(GvSpacing.content, 8, 24, 20),
              child: Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  // 侧栏导航（同一轻量选择样式）。
                  SizedBox(
                    width: 132,
                    child: Column(
                      children: [
                        const SizedBox(height: GvSpacing.large),
                        _NavButton(
                          label: '仓库设置',
                          selected: _repoSection,
                          onTap: () => setState(() => _repoSection = true),
                        ),
                        const SizedBox(height: 4),
                        _NavButton(
                          label: '常规设置',
                          selected: !_repoSection,
                          onTap: () => setState(() => _repoSection = false),
                        ),
                      ],
                    ),
                  ),
                  Expanded(
                    child: SingleChildScrollView(
                      padding: const EdgeInsetsDirectional.only(start: 24),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          // —— 固定位置反馈区（ui-design 8.4） ——
                          if (controller.settingsConflict)
                            InfoBar(
                              title: const Text('设置已被修改'),
                              content:
                                  const Text('其他入口更新了设置，请重载后重试。'),
                              severity: InfoBarSeverity.warning,
                              action: Button(
                                onPressed: () => controller.reloadSettings(),
                                child: const Text('重新加载'),
                              ),
                            ),
                          if (controller.startupRecovered)
                            const InfoBar(
                              title: Text('配置已恢复为默认值'),
                              content:
                                  Text('原设置文件损坏或无法识别，已隔离并使用默认设置。'),
                              severity: InfoBarSeverity.warning,
                            ),
                          if (controller.settingsMessage != null)
                            InfoBar(
                              title: Text(controller.settingsMessage!.title),
                              content: Text(controller.settingsMessage!.message),
                              severity: controller.settingsMessage!.isError
                                  ? InfoBarSeverity.error
                                  : InfoBarSeverity.success,
                              onClose: () => controller.dismissMessage(),
                            ),
                          // 分区常驻组件树：切换不丢失输入与滚动上下文。
                          IndexedStack(
                            index: _repoSection ? 0 : 1,
                            children: [
                              _RepoSection(controller: controller),
                              _GeneralSection(controller: controller),
                            ],
                          ),
                        ],
                      ),
                    ),
                  ),
                ],
              ),
            ),
          ),
        ],
      ),
    );
  }
}

// —— 侧栏导航按钮（C# NavRepoSection / NavGeneralSection） ——

class _NavButton extends StatelessWidget {
  final String label;
  final bool selected;
  final VoidCallback onTap;

  const _NavButton({
    required this.label,
    required this.selected,
    required this.onTap,
  });

  @override
  Widget build(BuildContext context) {
    final colors = gvColorsOf(context);
    return HoverButton(
      onPressed: onTap,
      builder: (context, states) => Container(
        constraints:
            const BoxConstraints(minHeight: 34),
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 5),
        decoration: BoxDecoration(
          color: selected
              ? colors.rowSelected
              : states.contains(WidgetState.hovered)
                  ? colors.hover
                  : Colors.transparent,
          borderRadius: BorderRadius.circular(GvSpacing.radius),
        ),
        child: Align(
          alignment: AlignmentDirectional.centerStart,
          child: Text(label,
              style: GvTextStyles.body(selected ? colors.ink : colors.muted)),
        ),
      ),
    );
  }
}

// —— 仓库设置分区 ——

class _RepoSection extends StatelessWidget {
  final AppController controller;

  const _RepoSection({required this.controller});

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        _SectionCard(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              _SectionTitle('添加仓库'),
              const SizedBox(height: GvSpacing.small),
              Text('选择单个仓库，或扫描工作目录批量添加。',
                  style: GvTextStyles.body(gvColorsOf(context).muted)),
              const SizedBox(height: GvSpacing.small),
              Row(
                children: [
                  _SmallButton(
                    label: '添加仓库…',
                    onPressed: () => controller.addRepo(),
                  ),
                  if (controller.addRepoBusy)
                    const Padding(
                      padding: EdgeInsets.only(left: GvSpacing.small),
                      child: SizedBox(
                          width: 16, height: 16, child: ProgressRing(strokeWidth: 2.5)),
                    ),
                  const SizedBox(width: GvSpacing.small),
                  _SmallButton(
                    label: '扫描目录…',
                    onPressed: () => controller.scanForRepos(),
                  ),
                  if (controller.scanBusy)
                    const Padding(
                      padding: EdgeInsets.only(left: GvSpacing.small),
                      child: SizedBox(
                          width: 16, height: 16, child: ProgressRing(strokeWidth: 2.5)),
                    ),
                ],
              ),
              if (controller.scanCandidates.isNotEmpty) ...[
                const SizedBox(height: 4),
                for (final candidate in controller.scanCandidates)
                  Checkbox(
                    checked: candidate.checked,
                    onChanged: (value) =>
                        controller.setCandidateChecked(candidate, value ?? false),
                    content: Text(candidate.path,
                        style: GvTextStyles.caption(gvColorsOf(context).muted)),
                  ),
                const SizedBox(height: 4),
                Row(
                  children: [
                    _SmallButton(
                      label: '添加所选',
                      onPressed: () => controller.addSelectedCandidates(),
                    ),
                    const SizedBox(width: GvSpacing.small),
                    _SmallButton(
                      label: '全部选择',
                      onPressed: () => controller.selectAllCandidates(),
                    ),
                  ],
                ),
              ],
            ],
          ),
        ),
        _SectionCard(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              _SectionTitle('仓库列表'),
              const SizedBox(height: GvSpacing.small),
              Text('移除只从管理列表删除记录，不会删除磁盘上的仓库。',
                  style: GvTextStyles.caption(gvColorsOf(context).muted)),
              const SizedBox(height: GvSpacing.small),
              if (controller.settings == null)
                Text('还没有仓库，使用上方入口添加。',
                    style: GvTextStyles.caption(gvColorsOf(context).muted))
              else if (controller.settings!.repos.isEmpty)
                Text('还没有仓库，使用上方入口添加。',
                    style: GvTextStyles.caption(gvColorsOf(context).muted))
              else
                for (final repo in controller.settings!.repos)
                  _ManagedRepoRow(controller: controller, repo: repo),
            ],
          ),
        ),
        _SectionCard(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              _SectionTitle('分组管理'),
              const SizedBox(height: GvSpacing.small),
              _NewGroupRow(controller: controller),
              const SizedBox(height: GvSpacing.small),
              if (controller.settings != null)
                for (final group in controller.settings!.groups)
                  _GroupRow(controller: controller, group: group),
            ],
          ),
        ),
      ],
    );
  }
}

// —— 已管理仓库行（名称 / 路径 / 归属 / 移除） ——

class _ManagedRepoRow extends StatelessWidget {
  final AppController controller;
  final RepoSettings repo;

  const _ManagedRepoRow({required this.controller, required this.repo});

  @override
  Widget build(BuildContext context) {
    final colors = gvColorsOf(context);
    final options = controller.groupOptions();
    final settings = controller.settings!;
    final selectedIndex =
        SettingsGroupPresentation.selectedIndex(settings.groups, repo.groupId);
    return Container(
      padding: const EdgeInsets.symmetric(vertical: 6),
      decoration:
          BoxDecoration(border: Border(bottom: BorderSide(color: colors.line))),
      child: Row(
        children: [
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(repo.name,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: GvTextStyles.body(colors.ink, weight: GvTextStyles.medium)),
                const SizedBox(height: 3),
                Text(repo.path,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: GvTextStyles.caption(colors.muted)),
              ],
            ),
          ),
          // 归属下拉：值 = 选项索引（0 = 未分组），与纯逻辑层约定一致。
          SizedBox(
            width: 130,
            child: ComboBox<int>(
              value: selectedIndex,
              items: [
                for (var i = 0; i < options.length; i++)
                  ComboBoxItem<int>(value: i, child: Text(options[i])),
              ],
              onChanged: (index) {
                if (index == null) return;
                controller.setRepoGroup(
                    repoId: repo.id, selectedIndex: index, options: options);
              },
            ),
          ),
          const SizedBox(width: GvSpacing.small),
          _SmallButton(
            label: '移除',
            onPressed: () => _confirmRemove(context),
          ),
        ],
      ),
    );
  }

  /// 移除确认（C# ContentDialog：磁盘上的仓库文件不会被删除）。
  Future<void> _confirmRemove(BuildContext context) async {
    final result = await showDialog<bool>(
      context: context,
      builder: (context) => ContentDialog(
        title: const Text('移除仓库'),
        content: Text(
            '从管理列表移除“${repo.name}”？\n\n${repo.path}\n\n磁盘上的仓库文件不会被删除。'),
        actions: [
          Button(
            onPressed: () => Navigator.pop(context, true),
            child: const Text('移除'),
          ),
          Button(
            onPressed: () => Navigator.pop(context, false),
            child: const Text('取消'),
          ),
        ],
      ),
    );
    if (result == true) {
      await controller.removeRepo(repo);
    }
  }
}

// —— 分组管理 ——

class _NewGroupRow extends StatefulWidget {
  final AppController controller;

  const _NewGroupRow({required this.controller});

  @override
  State<_NewGroupRow> createState() => _NewGroupRowState();
}

class _NewGroupRowState extends State<_NewGroupRow> {
  final TextEditingController _name = TextEditingController();

  @override
  void dispose() {
    _name.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        SizedBox(
          width: 180,
          child: TextBox(
            controller: _name,
            placeholder: '新分组名称',
          ),
        ),
        const SizedBox(width: GvSpacing.small),
        _SmallButton(
          label: '添加分组',
          onPressed: () async {
            final ok = await widget.controller.addGroup(_name.text);
            if (ok && mounted) setState(() => _name.clear());
          },
        ),
      ],
    );
  }
}

class _GroupRow extends StatelessWidget {
  final AppController controller;
  final GroupSettings group;

  const _GroupRow({required this.controller, required this.group});

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 4),
      child: Row(
        children: [
          Expanded(child: Text(group.name, style: GvTextStyles.caption(gvColorsOf(context).ink))),
          _SmallButton(
            label: '删除',
            onPressed: () => controller.deleteGroup(group.id),
          ),
        ],
      ),
    );
  }
}

// —— 常规设置分区 ——

class _GeneralSection extends StatefulWidget {
  final AppController controller;

  const _GeneralSection({required this.controller});

  @override
  State<_GeneralSection> createState() => _GeneralSectionState();
}

class _GeneralSectionState extends State<_GeneralSection> {
  // 刷新策略是“编辑后按保存提交”的本地草稿（C# RefreshEnabled / RefreshInterval）。
  bool _refreshEnabled = true;
  int _refreshInterval = 5;
  int _adoptedRevision = -1;

  /// 设置快照变化（加载 / 保存成功）时采纳引擎值，覆盖本地草稿。
  void _adoptFromEngine() {
    final settings = widget.controller.settings;
    if (settings == null) return;
    if (_adoptedRevision == settings.revision) return;
    _adoptedRevision = settings.revision;
    _refreshEnabled = settings.refresh.enabled;
    _refreshInterval = settings.refresh.intervalMinutes;
  }

  @override
  Widget build(BuildContext context) {
    final colors = gvColorsOf(context);
    _adoptFromEngine();
    final options = widget.controller.defaultGroupOptions();
    final settings = widget.controller.settings;
    final defaultIndex = settings == null
        ? 0
        : options.indexWhere((option) => option.$1 == settings.defaultGroupId);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        _SectionCard(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Row(
                children: [
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        _SectionTitle('自动刷新'),
                        const SizedBox(height: 5),
                        Text('自动刷新只读取本地状态，不会执行 Fetch。',
                            style: GvTextStyles.body(colors.muted)),
                      ],
                    ),
                  ),
                  Column(
                    crossAxisAlignment: CrossAxisAlignment.end,
                    children: [
                      Text(_refreshEnabled ? '已开启' : '手动',
                          style: GvTextStyles.caption(colors.muted)),
                      ToggleSwitch(
                        checked: _refreshEnabled,
                        onChanged: (value) => setState(() => _refreshEnabled = value),
                      ),
                    ],
                  ),
                ],
              ),
              const SizedBox(height: GvSpacing.section),
              // 同一任务区域：间隔 + 保存 + 固定位置的“已保存”反馈。
              Row(
                children: [
                  Text('每隔', style: GvTextStyles.body(colors.muted)),
                  const SizedBox(width: GvSpacing.small),
                  SizedBox(
                    width: 96,
                    child: NumberBox<int>(
                      value: _refreshInterval,
                      min: 1,
                      max: 60,
                      smallChange: 1,
                      mode: SpinButtonPlacementMode.compact,
                      clearButton: false,
                      onChanged: (value) => setState(() {
                        _refreshInterval =
                            ((value ?? 5).clamp(1, 60)).toInt();
                      }),
                    ),
                  ),
                  const SizedBox(width: GvSpacing.small),
                  Text('分钟', style: GvTextStyles.body(colors.muted)),
                  const SizedBox(width: GvSpacing.small),
                  _SmallButton(
                    label: '保存',
                    onPressed: () => widget.controller.saveRefresh(
                        enabled: _refreshEnabled, intervalMinutes: _refreshInterval),
                  ),
                  if (widget.controller.refreshSavedVisible) ...[
                    const SizedBox(width: GvSpacing.small),
                    Text('已保存', style: GvTextStyles.caption(colors.green)),
                  ],
                ],
              ),
            ],
          ),
        ),
        _SectionCard(
          child: Row(
            children: [
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    _SectionTitle('默认分组'),
                    const SizedBox(height: 5),
                    Text('启动时优先显示的仓库分组。',
                        style: GvTextStyles.body(colors.muted)),
                  ],
                ),
              ),
              SizedBox(
                width: 170,
                child: ComboBox<int>(
                  value: defaultIndex < 0 ? 0 : defaultIndex,
                  items: [
                    for (var i = 0; i < options.length; i++)
                      ComboBoxItem<int>(value: i, child: Text(options[i].$2)),
                  ],
                  onChanged: (index) {
                    if (index == null || settings == null) return;
                    widget.controller.setDefaultGroup(options[index].$1);
                  },
                ),
              ),
            ],
          ),
        ),
        _SectionCard(
          child: Row(
            children: [
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    _SectionTitle('开机启动'),
                    const SizedBox(height: 5),
                    // 状态展示（Windows 阶段接线系统 StartupTask 后取实际状态）。
                    Text(widget.controller.startupStateText,
                        style: GvTextStyles.body(colors.muted)),
                  ],
                ),
              ),
            ],
          ),
        ),
        // 不可修改的安全规则：辅助说明，不与操作争抢层级。
        Padding(
          padding: const EdgeInsets.only(top: GvSpacing.large),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              _SectionTitle('操作确认'),
              const SizedBox(height: 6),
              Text('Pull 会修改工作区，Push 会更新远端分支。每次执行前都需要确认。',
                  style: GvTextStyles.body(colors.muted)),
            ],
          ),
        ),
      ],
    );
  }
}

// —— 共享小组件 ——

/// 分区卡（C# GvSectionCard：底部分隔线 + 0,18,0,20 内边距）。
class _SectionCard extends StatelessWidget {
  final Widget child;

  const _SectionCard({required this.child});

  @override
  Widget build(BuildContext context) {
    final colors = gvColorsOf(context);
    return Container(
      width: double.infinity,
      padding: const EdgeInsets.fromLTRB(0, 18, 0, 20),
      decoration:
          BoxDecoration(border: Border(bottom: BorderSide(color: colors.line))),
      child: child,
    );
  }
}

class _SectionTitle extends StatelessWidget {
  final String text;

  const _SectionTitle(this.text);

  @override
  Widget build(BuildContext context) {
    return Text(text,
        style: GvTextStyles.body(gvColorsOf(context).ink,
            weight: GvTextStyles.semiBold));
  }
}

/// 小按钮（C# GvSmallButton）。
class _SmallButton extends StatelessWidget {
  final String label;
  final VoidCallback? onPressed;

  const _SmallButton({required this.label, this.onPressed});

  @override
  Widget build(BuildContext context) {
    final colors = gvColorsOf(context);
    return Button(
      onPressed: onPressed,
      style: ButtonStyle(
        padding: WidgetStateProperty.all(
            const EdgeInsets.symmetric(horizontal: 12, vertical: 5)),
        backgroundColor: WidgetStateProperty.resolveWith((states) =>
            states.contains(WidgetState.disabled)
                ? colors.surface.withValues(alpha: 0.4)
                : colors.surface),
        foregroundColor: WidgetStateProperty.resolveWith((states) =>
            states.contains(WidgetState.disabled)
                ? colors.muted.withValues(alpha: 0.4)
                : colors.ink),
        shape: WidgetStateProperty.all(RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(GvSpacing.radius),
          side: BorderSide(color: colors.line),
        )),
        textStyle: WidgetStateProperty.all(GvTextStyles.body(colors.ink)),
      ),
      child: Text(label, style: GvTextStyles.body(colors.ink)),
    );
  }
}
