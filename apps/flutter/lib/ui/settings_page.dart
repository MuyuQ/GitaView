// 设置页：标题栏（返回 + 设置）+ 侧栏导航（196，分组标签 + 选中指示条）+
// 平铺分区内容（仓库设置 / 常规设置）+ 固定位置反馈条。
//
// 视觉规格：design/preview/index.html（docs/flutter-ui-redesign-plan.md §4）：
// - 分区为“平铺行”（下边框分隔），不再是卡片；内容区最大宽 760 居中；
// - 仓库设置：添加仓库（单仓库 / 扫描目录与候选块）、已管理仓库
//   （名称 / 路径 / 归属下拉 / 移除——悬停变红，只删管理记录）、分组管理；
// - 常规设置：自动刷新拆两行（启用开关 / 间隔步进器 + 保存 + 固定位置
//   “已保存”）、默认分组（分区级下拉）、开机启动（只读状态行）、
//   操作确认安全规则（辅助说明）；
// - 外观分区：主题（跟随系统 / 浅色 / 深色）为宿主本地偏好
//   （ui-preferences.json，与 C# LocalSettings 方向一致），不入 settings.json
//   契约、不触发引擎重扫；切换由宿主状态驱动 FluentApp.themeMode；
// - 全部变更走 SettingsEditRequest 事务；SETTINGS_CONFLICT 显示重载入口；
// - 两个分区常驻组件树（IndexedStack），返回状态页后上下文保留。
library;

import 'dart:io' show Platform;

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

  /// 当前主题模式（宿主状态；宿主本地持久化，不入 settings.json 契约）。
  final ThemeMode themeMode;

  /// 主题切换回调（null = 只读展示，不可切换）。
  final ValueChanged<ThemeMode>? onThemeModeChanged;

  const SettingsPage({
    super.key,
    required this.controller,
    required this.onBack,
    this.themeMode = ThemeMode.system,
    this.onThemeModeChanged,
  });

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
          // 标题栏：返回 + “设置” + 辅助说明（不重复品牌）。
          Container(
            height: GvSpacing.toolbarHeight,
            color: colors.surface,
            // macOS 沉浸式标题栏：为系统红绿灯保留前导空间（同主页面 68）。
            padding: EdgeInsets.only(
                left: Platform.isMacOS
                    ? GvSpacing.macTrafficLightsClearance
                    : 12,
                right: 18),
            child: Row(
              children: [
                _IconActionButton(
                  icon: FluentIcons.chevron_left,
                  semanticLabel: '返回',
                  onPressed: widget.onBack,
                ),
                const SizedBox(width: 10),
                Text('设置',
                    style: GvTextStyles.body(colors.ink,
                        weight: GvTextStyles.semiBold)
                    .copyWith(fontSize: GvFontSizes.title)),
                const SizedBox(width: 10),
                Text('仓库与常规设置', style: GvTextStyles.caption(colors.muted).copyWith(fontSize: 12)),
                const Spacer(),
                // 设计稿 head-badge；文案按实际事务语义改写——仓库/分组/开关
                // 提交即生效，只有刷新间隔走“保存”按钮（ui-design §7.2 曾纠正
                // “三者统称变更即保存”不准确）。
                Container(
                  padding:
                      const EdgeInsets.symmetric(horizontal: 10, vertical: 4),
                  decoration: BoxDecoration(
                    borderRadius: BorderRadius.circular(999),
                    border: Border.all(color: colors.line),
                  ),
                  child: Text('设置随改随存 · 刷新间隔需保存',
                      style: GvTextStyles.subtitle(colors.muted)
                          .copyWith(fontSize: 11)),
                ),
              ],
            ),
          ),
          Expanded(
            // 内容区实心表面底（同主页面：非 Windows 的 Mica 不铺底）。
            child: ColoredBox(
              color: colors.surface,
              child: Row(
                crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                _SettingsNav(
                  repoSelected: _repoSection,
                  onSelectRepo: () => setState(() => _repoSection = true),
                  onSelectGeneral: () => setState(() => _repoSection = false),
                ),
                Expanded(
                  child: SingleChildScrollView(
                    padding: const EdgeInsets.fromLTRB(22, 22, 34, 36),
                    child: Align(
                      alignment: Alignment.topCenter,
                      child: ConstrainedBox(
                        constraints:
                            const BoxConstraints(maxWidth: 760),
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
                                _GeneralSection(
                                  controller: controller,
                                  themeMode: widget.themeMode,
                                  onThemeModeChanged: widget.onThemeModeChanged,
                                ),
                              ],
                            ),
                          ],
                        ),
                      ),
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

// —— 侧栏导航（设计稿 settings-nav：宽 196、分组标签 10.5 + 0.09em、
// nav 项高 34、选中 = surface 底 + line 边 + tint 指示条） ——

class _SettingsNav extends StatelessWidget {
  final bool repoSelected;
  final VoidCallback onSelectRepo;
  final VoidCallback onSelectGeneral;

  const _SettingsNav({
    required this.repoSelected,
    required this.onSelectRepo,
    required this.onSelectGeneral,
  });

  @override
  Widget build(BuildContext context) {
    final colors = gvColorsOf(context);
    return Container(
      width: 196,
      decoration: BoxDecoration(
        color: colors.filterSurface,
        border: Border(right: BorderSide(color: colors.lineSoft)),
      ),
      padding: const EdgeInsets.fromLTRB(10, 12, 10, 14),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          const _NavLabel('仓库'),
          _NavItem(
            label: '仓库设置',
            selected: repoSelected,
            onTap: onSelectRepo,
          ),
          const _NavLabel('应用'),
          _NavItem(
            label: '常规设置',
            selected: !repoSelected,
            onTap: onSelectGeneral,
          ),
          const Spacer(),
          // 侧栏脚注（设计稿 nav-foot）。
          Container(
            padding: const EdgeInsets.all(12),
            decoration: BoxDecoration(
                border: Border(top: BorderSide(color: colors.lineSoft))),
            child: Row(
              children: [
                const GvBrandMark(size: 22),
                const SizedBox(width: 9),
                Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text('本地优先', style: GvTextStyles.subtitle(colors.muted)),
                    Text('无云同步', style: GvTextStyles.subtitle(colors.muted)),
                  ],
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

class _NavLabel extends StatelessWidget {
  final String text;

  const _NavLabel(this.text);

  @override
  Widget build(BuildContext context) {
    final colors = gvColorsOf(context);
    return Padding(
      padding: const EdgeInsets.fromLTRB(12, 10, 12, 6),
      child: Text(
        text,
        style: GvTextStyles.subtitle(colors.muted).copyWith(fontSize: 10.5, letterSpacing: 0.95),
      ),
    );
  }
}

class _NavItem extends StatelessWidget {
  final String label;
  final bool selected;
  final VoidCallback onTap;

  const _NavItem({
    required this.label,
    required this.selected,
    required this.onTap,
  });

  @override
  Widget build(BuildContext context) {
    final colors = gvColorsOf(context);
    return HoverButton(
      onPressed: onTap,
      builder: (context, states) {
        final hovered = states.contains(WidgetState.hovered);
        return Container(
          height: 34,
          padding: const EdgeInsets.symmetric(horizontal: 12),
          alignment: AlignmentDirectional.centerStart,
          decoration: BoxDecoration(
            // 指示条常驻占位（3px + 9px 间距），选中态不移动文字。
            color: selected
                ? colors.surface
                : hovered
                    ? colors.hover
                    : null,
            borderRadius: BorderRadius.circular(GvSpacing.radius),
            border: Border.all(
              color: selected ? colors.line : Colors.transparent,
            ),
          ),
          child: Row(
            children: [
              Container(
                width: 3,
                height: 14,
                margin: const EdgeInsets.only(right: 9),
                decoration: BoxDecoration(
                  color: selected ? colors.tint : Colors.transparent,
                  borderRadius: BorderRadius.circular(2),
                ),
              ),
              Text(
                label,
                style: GvTextStyles.caption(
                  selected ? colors.ink : colors.ink2,
                  // caption 默认 13 与设计稿一致；选中 w500。
                ).copyWith(
                    fontWeight:
                        selected ? GvTextStyles.medium : FontWeight.w400),
              ),
            ],
          ),
        );
      },
    );
  }
}

// —— 仓库设置分区 ——

class _RepoSection extends StatelessWidget {
  final AppController controller;

  const _RepoSection({required this.controller});

  @override
  Widget build(BuildContext context) {
    final colors = gvColorsOf(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        _Section(
          title: '添加仓库',
          description: '选择单个仓库，或扫描工作目录批量添加。',
          first: true,
          children: [
            Row(
              children: [
                _SettingsButton(
                  label: '添加仓库…',
                  icon: FluentIcons.add,
                  onPressed: () => controller.addRepo(),
                ),
                if (controller.addRepoBusy)
                  const Padding(
                    padding: EdgeInsets.only(left: GvSpacing.small),
                    child: SizedBox(
                        width: 16, height: 16, child: ProgressRing(strokeWidth: 2.5)),
                  ),
                const SizedBox(width: GvSpacing.small),
                _SettingsButton(
                  label: '扫描目录…',
                  icon: FluentIcons.search,
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
            // 扫描候选块（bg 底 + lineSoft 边 + radius 8，整行可点）。
            if (controller.scanCandidates.isNotEmpty) ...[
              const SizedBox(height: 6),
              for (final candidate in controller.scanCandidates)
                _CandidateRow(
                  controller: controller,
                  candidate: candidate,
                ),
              const SizedBox(height: 10),
              Row(
                children: [
                  _SettingsButton(
                    label: '添加所选',
                    small: true,
                    onPressed: () => controller.addSelectedCandidates(),
                  ),
                  const SizedBox(width: GvSpacing.small),
                  _SettingsButton(
                    label: '全部选择',
                    small: true,
                    onPressed: () => controller.selectAllCandidates(),
                  ),
                ],
              ),
            ],
          ],
        ),
        _Section(
          title: '仓库列表',
          description: '移除只从管理列表删除记录，不会删除磁盘上的仓库。',
          tag: '${controller.settings?.repos.length ?? 0} 个',
          children: [
            if (controller.settings == null ||
                controller.settings!.repos.isEmpty)
              Text('还没有仓库，使用上方入口添加。',
                  style: GvTextStyles.subtitle(colors.muted)
                      .copyWith(fontSize: 12.5))
            else
              for (final repo in controller.settings!.repos)
                _ManagedRepoRow(controller: controller, repo: repo),
          ],
        ),
        _Section(
          title: '分组管理',
          description: '分组只影响浏览与筛选，不改写仓库内的任何文件。',
          last: true,
          children: [
            _NewGroupRow(controller: controller),
            const SizedBox(height: 12),
            if (controller.settings != null &&
                controller.settings!.groups.isNotEmpty)
              for (final group in controller.settings!.groups)
                _GroupRow(controller: controller, group: group)
            else
              Text('还没有分组。',
                  style: GvTextStyles.subtitle(colors.muted)
                      .copyWith(fontSize: 12.5)),
          ],
        ),
      ],
    );
  }
}

/// 平铺分区（设计稿 sec：18/0/22 内边距 + lineSoft 下边框；末分区无边）。
class _Section extends StatelessWidget {
  final String title;
  final String description;
  final List<Widget> children;
  final String? tag;

  /// 分区级控件（如默认分组的单选下拉）：放在标题行右侧，
  /// 避免“分区标题 + 仅一行同名设置行”的重复层级。
  final Widget? trailing;
  final bool first;
  final bool last;

  const _Section({
    required this.title,
    required this.description,
    this.children = const <Widget>[],
    this.tag,
    this.trailing,
    this.first = false,
    this.last = false,
  });

  @override
  Widget build(BuildContext context) {
    final colors = gvColorsOf(context);
    return Container(
      width: double.infinity,
      padding: EdgeInsets.only(top: first ? 2 : 18, bottom: last ? 8 : 22),
      decoration: BoxDecoration(
        border: last
            ? null
            : Border(bottom: BorderSide(color: colors.lineSoft)),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(title,
                        style: GvTextStyles.body(colors.ink,
                            weight: GvTextStyles.semiBold)),
                    const SizedBox(height: 5),
                    Text(description,
                        style: GvTextStyles.subtitle(colors.muted)
                            .copyWith(fontSize: 12.5, height: 1.6)),
                  ],
                ),
              ),
              if (tag != null) _StateTag(tag!),
              if (trailing != null) ...[
                const SizedBox(width: 20),
                trailing!,
              ],
            ],
          ),
          if (children.isNotEmpty) ...[
            const SizedBox(height: 12),
            ...children,
          ],
        ],
      ),
    );
  }
}

/// 计数徽标（设计稿 state-tag：line 边胶囊、10px 小字）。
class _StateTag extends StatelessWidget {
  final String text;

  const _StateTag(this.text);

  @override
  Widget build(BuildContext context) {
    final colors = gvColorsOf(context);
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 7, vertical: 2),
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(999),
        border: Border.all(color: colors.line),
      ),
      child: Text(text,
          style: GvTextStyles.subtitle(colors.muted).copyWith(fontSize: 10)),
    );
  }
}

/// 扫描候选行（设计稿 candidate：bg 底 + lineSoft 边 + radius 8，整行可点）。
class _CandidateRow extends StatelessWidget {
  final AppController controller;
  final ScanCandidateVm candidate;

  const _CandidateRow({required this.controller, required this.candidate});

  @override
  Widget build(BuildContext context) {
    final colors = gvColorsOf(context);
    void toggle() =>
        controller.setCandidateChecked(candidate, !candidate.checked);
    return Padding(
      padding: const EdgeInsets.only(top: 6),
      child: HoverButton(
        onPressed: toggle,
        builder: (context, states) {
          final hovered = states.contains(WidgetState.hovered);
          return Container(
            padding:
                const EdgeInsets.symmetric(horizontal: 10, vertical: 8),
            decoration: BoxDecoration(
              color: hovered
                  ? Color.lerp(colors.bg, colors.hover, 0.35)
                  : colors.bg,
              borderRadius: BorderRadius.circular(GvSpacing.radius),
              border: Border.all(color: colors.lineSoft),
            ),
            child: Row(
              children: [
                Checkbox(
                  checked: candidate.checked,
                  onChanged: (value) =>
                      controller.setCandidateChecked(candidate, value ?? false),
                ),
                const SizedBox(width: 10),
                Expanded(
                  child: Text(
                    candidate.path,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: GvTextStyles.mono(colors.ink2, size: 12),
                  ),
                ),
              ],
            ),
          );
        },
      ),
    );
  }
}

// —— 已管理仓库行（名称 / 路径 / 归属 / 移除；整行悬停底色，移除悬停变红） ——

class _ManagedRepoRow extends StatefulWidget {
  final AppController controller;
  final RepoSettings repo;

  const _ManagedRepoRow({required this.controller, required this.repo});

  @override
  State<_ManagedRepoRow> createState() => _ManagedRepoRowState();
}

class _ManagedRepoRowState extends State<_ManagedRepoRow> {
  bool _hovered = false;
  late final TextEditingController _alias =
      TextEditingController(text: widget.repo.alias ?? '');
  late final FocusNode _aliasFocus = FocusNode()
    ..addListener(_onAliasFocusChanged);

  @override
  void dispose() {
    _aliasFocus.removeListener(_onAliasFocusChanged);
    _aliasFocus.dispose();
    _alias.dispose();
    super.dispose();
  }

  /// 失焦时提交变更（trim 后空 = 清除；同值由控制器忽略）。
  void _onAliasFocusChanged() {
    if (_aliasFocus.hasFocus) return;
    _submitAlias();
  }

  void _submitAlias() {
    widget.controller.setRepoAlias(widget.repo, _alias.text);
  }

  @override
  Widget build(BuildContext context) {
    final colors = gvColorsOf(context);
    final controller = widget.controller;
    final repo = widget.repo;
    final options = controller.groupOptions();
    final settings = controller.settings!;
    final selectedIndex =
        SettingsGroupPresentation.selectedIndex(settings.groups, repo.groupId);
    return MouseRegion(
      onEnter: (_) => setState(() => _hovered = true),
      onExit: (_) => setState(() => _hovered = false),
      child: Container(
        margin: const EdgeInsets.only(top: 2),
        padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 9),
        decoration: BoxDecoration(
          color: _hovered
              ? Color.lerp(colors.surface, colors.bg, 0.7)
              : Colors.transparent,
          borderRadius: BorderRadius.circular(GvSpacing.radius),
        ),
        child: Row(
          children: [
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(repo.name,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: GvTextStyles.caption(colors.ink)
                          .copyWith(fontWeight: GvTextStyles.medium)),
                  const SizedBox(height: 3),
                  Text(repo.path,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: GvTextStyles.mono(colors.muted, size: 12)),
                ],
              ),
            ),
            // 别名（用户反馈：帮助快速识别）。Enter 提交 / 失焦提交。
            Tooltip(
              message: '别名：在仓库列表中代替目录名显示，留空恢复目录名',
              child: SizedBox(
                width: 132,
                child: _SettingsTextBox(
                  controller: _alias,
                  focusNode: _aliasFocus,
                  placeholder: '别名',
                  maxLength: 64,
                  onSubmitted: (_) => _submitAlias(),
                ),
              ),
            ),
            const SizedBox(width: GvSpacing.small),
            // 归属下拉：值 = 选项索引（0 = 未分组），与纯逻辑层约定一致。
            SizedBox(
              width: 124,
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
            _SettingsButton(
              label: '移除',
              small: true,
              // 平时 muted、悬停变红（danger-quiet）。
              foreground: _hovered ? colors.red : colors.muted,
              borderColor:
                  _hovered ? Color.lerp(colors.line, colors.red, 0.34) : null,
              onPressed: () => _confirmRemove(context),
            ),
          ],
        ),
      ),
    );
  }

  /// 移除确认（C# ContentDialog：磁盘上的仓库文件不会被删除）。
  Future<void> _confirmRemove(BuildContext context) async {
    final repo = widget.repo;
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
      await widget.controller.removeRepo(repo);
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
          width: 200,
          child: _SettingsTextBox(
            controller: _name,
            placeholder: '新分组名称',
          ),
        ),
        const SizedBox(width: GvSpacing.small),
        _SettingsButton(
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
    final colors = gvColorsOf(context);
    final settings = controller.settings;
    final count = settings == null
        ? 0
        : settings.repos.where((repo) => repo.groupId == group.id).length;
    return Container(
      padding: const EdgeInsets.symmetric(vertical: 7),
      decoration: BoxDecoration(
        border: Border(
            bottom: BorderSide(color: colors.lineSoft, width: 0.5)),
      ),
      child: Row(
        children: [
          Expanded(
            child: Text(group.name, style: GvTextStyles.caption(colors.ink)),
          ),
          Text('$count 个仓库', style: GvTextStyles.subtitle(colors.muted)),
          const SizedBox(width: 10),
          _SettingsButton(
            label: '删除',
            tiny: true,
            quiet: true,
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
  final ThemeMode themeMode;
  final ValueChanged<ThemeMode>? onThemeModeChanged;

  const _GeneralSection({
    required this.controller,
    required this.themeMode,
    this.onThemeModeChanged,
  });

  @override
  State<_GeneralSection> createState() => _GeneralSectionState();
}

class _GeneralSectionState extends State<_GeneralSection> {
  // 刷新策略是“编辑后按保存提交”的本地草稿（C# RefreshEnabled / RefreshInterval）。
  bool _refreshEnabled = true;
  int _refreshInterval = 5;
  int _adoptedRevision = -1;
  final TextEditingController _intervalText = TextEditingController();

  /// 设置快照变化（加载 / 保存成功）时采纳引擎值，覆盖本地草稿。
  void _adoptFromEngine() {
    final settings = widget.controller.settings;
    if (settings == null) return;
    if (_adoptedRevision == settings.revision) return;
    _adoptedRevision = settings.revision;
    _refreshEnabled = settings.refresh.enabled;
    _refreshInterval = settings.refresh.intervalMinutes;
    _intervalText.text = '${settings.refresh.intervalMinutes}';
  }

  @override
  void dispose() {
    _intervalText.dispose();
    super.dispose();
  }

  void _setInterval(int value) {
    final clamped = value.clamp(1, 60).toInt();
    setState(() {
      _refreshInterval = clamped;
      _intervalText.text = '$clamped';
    });
  }

  @override
  Widget build(BuildContext context) {
    final colors = gvColorsOf(context);
    _adoptFromEngine();
    final controller = widget.controller;
    final options = controller.defaultGroupOptions();
    final settings = controller.settings;
    final defaultIndex = settings == null
        ? 0
        : options.indexWhere((option) => option.$1 == settings.defaultGroupId);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        _Section(
          title: '自动刷新',
          description: '自动刷新只读取本地状态，不会执行 Fetch。',
          first: true,
          children: [
            // 任务区第一行：启用开关（设计稿 setting-row）。
            _SettingRow(
              title: '启用自动刷新',
              description: '按设定间隔自动读取本地状态，不会执行 Fetch。',
              end: Row(
                children: [
                  Text(_refreshEnabled ? '已开启' : '手动',
                      style: GvTextStyles.subtitle(colors.muted)),
                  const SizedBox(width: 10),
                  ToggleSwitch(
                    checked: _refreshEnabled,
                    onChanged: (value) =>
                        setState(() => _refreshEnabled = value),
                  ),
                ],
              ),
            ),
            // 任务区第二行：间隔 + 保存 + 固定位置的“已保存”反馈。
            _SettingRow(
              title: '刷新间隔',
              description: '两次自动读取之间等待的时间。',
              end: Row(
                children: [
                  Text('每隔', style: GvTextStyles.caption(colors.muted).copyWith(fontSize: 12)),
                  const SizedBox(width: GvSpacing.small),
                  _Stepper(
                    value: _refreshInterval,
                    textController: _intervalText,
                    onChanged: _setInterval,
                  ),
                  const SizedBox(width: GvSpacing.small),
                  Text('分钟', style: GvTextStyles.caption(colors.muted).copyWith(fontSize: 12)),
                  const SizedBox(width: GvSpacing.small),
                  _SettingsButton(
                    label: '保存',
                    primary: true,
                    onPressed: () => controller.saveRefresh(
                        enabled: _refreshEnabled,
                        intervalMinutes: _refreshInterval),
                  ),
                  if (controller.refreshSavedVisible) ...[
                    const SizedBox(width: GvSpacing.small),
                    Text('已保存', style: GvTextStyles.subtitle(colors.green)),
                  ],
                ],
              ),
            ),
          ],
        ),
        _Section(
          title: '默认分组',
          description: '打开 GitaView 时先选中的分组。',
          trailing: Padding(
            padding: const EdgeInsets.only(top: 2),
            child: SizedBox(
              width: 190,
              child: ComboBox<int>(
                value: defaultIndex < 0 ? 0 : defaultIndex,
                items: [
                  for (var i = 0; i < options.length; i++)
                    ComboBoxItem<int>(value: i, child: Text(options[i].$2)),
                ],
                onChanged: (index) {
                  if (index == null || settings == null) return;
                  controller.setDefaultGroup(options[index].$1);
                },
              ),
            ),
          ),
        ),
        _Section(
          title: '开机启动',
          description: '由系统宿主注册；GitaView 只读取当前状态，不在此处修改。',
          tag: '只读',
          children: [
            _SettingRow(
              title: '当前状态',
              description: controller.startupStateText,
            ),
          ],
        ),
        _Section(
          title: '外观',
          description: '主题只影响界面，不改变仓库数据。跟随系统时按系统明暗自动切换。',
          children: [
            _SettingRow(
              title: '主题',
              end: _ThemeSegmented(
                value: widget.themeMode,
                onChanged: widget.onThemeModeChanged,
              ),
            ),
          ],
        ),
        // 不可修改的安全规则：辅助说明（设计稿 callout），不与操作争抢层级。
        Container(
          width: double.infinity,
          padding: const EdgeInsets.only(top: 18, bottom: 8),
          child: Container(
            padding:
                const EdgeInsets.symmetric(horizontal: 13, vertical: 11),
            decoration: BoxDecoration(
              color: colors.bg,
              borderRadius: BorderRadius.circular(GvSpacing.radius),
              border: Border.all(color: colors.lineSoft),
            ),
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Icon(FluentIcons.shield, size: 16, color: colors.muted),
                const SizedBox(width: 9),
                Expanded(
                  child: Text(
                    'Pull 会修改工作区，Push 会更新远端分支。每次执行前都需要确认。',
                    style: GvTextStyles.caption(colors.muted)
                        .copyWith(fontSize: 12, height: 1.6),
                  ),
                ),
              ],
            ),
          ),
        ),
      ],
    );
  }
}

/// 设置行（设计稿 setting-row：左右两段，行间 lineSoft 分隔）。
class _SettingRow extends StatelessWidget {
  final String title;
  final String? description;
  final Widget? end;

  const _SettingRow({required this.title, this.description, this.end});

  @override
  Widget build(BuildContext context) {
    final colors = gvColorsOf(context);
    return Container(
      padding: const EdgeInsets.symmetric(vertical: 10),
      child: Row(
        children: [
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(title,
                    style: GvTextStyles.caption(colors.ink)
                        .copyWith(fontWeight: GvTextStyles.medium)),
                if (description != null) ...[
                  const SizedBox(height: 4),
                  Text(description!,
                      style: GvTextStyles.subtitle(colors.muted)
                          .copyWith(fontSize: 12.5, height: 1.6)),
                ],
              ],
            ),
          ),
          if (end != null) ...[
            const SizedBox(width: 20),
            end!,
          ],
        ],
      ),
    );
  }
}

/// 步进器（设计稿 stepper：− / 数值 / ＋，bg 底 line 边 radius 7）。
/// 主题三档分段选择（设计稿主题卡的落地形态：跟随系统 / 浅色 / 深色；
/// 选中 = selection 底 + onSelection 字；onChanged 为 null 时整组只读）。
class _ThemeSegmented extends StatelessWidget {
  final ThemeMode value;
  final ValueChanged<ThemeMode>? onChanged;

  const _ThemeSegmented({required this.value, this.onChanged});

  static const List<(ThemeMode, String)> _options = [
    (ThemeMode.system, '跟随系统'),
    (ThemeMode.light, '浅色'),
    (ThemeMode.dark, '深色'),
  ];

  @override
  Widget build(BuildContext context) {
    final colors = gvColorsOf(context);
    return Container(
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(7),
        border: Border.all(color: colors.line),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          for (var i = 0; i < _options.length; i++)
            Padding(
              padding: EdgeInsets.only(left: i == 0 ? 0 : 1),
              child: _segment(context, _options[i].$1, _options[i].$2),
            ),
        ],
      ),
    );
  }

  Widget _segment(BuildContext context, ThemeMode mode, String label) {
    final colors = gvColorsOf(context);
    final selected = value == mode;
    return HoverButton(
      onPressed: onChanged == null ? null : () => onChanged!(mode),
      builder: (context, states) {
        final hovered = states.contains(WidgetState.hovered);
        final background = selected
            ? colors.selection
            : hovered
                ? colors.hover
                : colors.surface;
        final foreground = selected ? colors.onSelection : colors.ink2;
        return Container(
          height: 28,
          padding: const EdgeInsets.symmetric(horizontal: 12),
          alignment: AlignmentDirectional.center,
          color: background,
          child: Text(
            label,
            style: TextStyle(
              fontSize: 12,
              height: 1,
              fontWeight: selected ? GvTextStyles.medium : FontWeight.w400,
              color: foreground,
              fontFamily: GvFonts.ui,
              fontFamilyFallback: GvFonts.uiFallback,
            ),
          ),
        );
      },
    );
  }
}

class _Stepper extends StatelessWidget {
  final int value;
  final TextEditingController textController;
  final ValueChanged<int> onChanged;

  const _Stepper({
    required this.value,
    required this.textController,
    required this.onChanged,
  });

  @override
  Widget build(BuildContext context) {
    final colors = gvColorsOf(context);
    return Container(
      decoration: BoxDecoration(
        color: colors.bg,
        borderRadius: BorderRadius.circular(7),
        border: Border.all(color: colors.line),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          _stepButton(context, '−', () => onChanged(value - 1)),
          Container(width: 1, height: 32, color: colors.line),
          SizedBox(
            width: 44,
            height: 32,
            child: TextBox(
              controller: textController,
              textAlign: TextAlign.center,
              decoration:
                  WidgetStateProperty.all(BoxDecoration(color: colors.surface)),
              padding: EdgeInsets.zero,
              style: GvTextStyles.mono(colors.ink, size: 12.5),
              cursorColor: colors.ink,
              onChanged: (text) {
                final parsed = int.tryParse(text);
                if (parsed != null) onChanged(parsed);
              },
            ),
          ),
          Container(width: 1, height: 32, color: colors.line),
          _stepButton(context, '+', () => onChanged(value + 1)),
        ],
      ),
    );
  }

  Widget _stepButton(BuildContext context, String glyph, VoidCallback onTap) {
    return HoverButton(
      onPressed: onTap,
      builder: (context, states) {
        final colors = gvColorsOf(context);
        final hovered = states.contains(WidgetState.hovered);
        return Container(
          width: 28,
          height: 32,
          alignment: AlignmentDirectional.center,
          color: hovered ? colors.hover : Colors.transparent,
          child: Text(glyph,
              style: TextStyle(
                  fontSize: 15,
                  height: 1,
                  color: hovered ? colors.ink : colors.muted,
                  fontFamily: GvFonts.ui)),
        );
      },
    );
  }
}

// —— 共享小组件 ——

/// 图标按钮（返回等：34×34、悬停浅层）。
class _IconActionButton extends StatelessWidget {
  final IconData icon;
  final String semanticLabel;
  final VoidCallback onPressed;

  const _IconActionButton({
    required this.icon,
    required this.semanticLabel,
    required this.onPressed,
  });

  @override
  Widget build(BuildContext context) {
    final colors = gvColorsOf(context);
    return HoverButton(
      onPressed: onPressed,
      semanticLabel: semanticLabel,
      builder: (context, states) {
        final hovered = states.contains(WidgetState.hovered);
        return Container(
          width: 34,
          height: 34,
          alignment: AlignmentDirectional.center,
          decoration: BoxDecoration(
            color: hovered ? colors.hover.withValues(alpha: 0.55) : null,
            borderRadius: BorderRadius.circular(GvSpacing.radius),
          ),
          child:
              Icon(icon, size: 18, color: hovered ? colors.ink : colors.muted),
        );
      },
    );
  }
}

/// 设置页输入框（设计稿 input：高 34、bg 底、line 边、radius 7）。
class _SettingsTextBox extends StatelessWidget {
  final TextEditingController controller;
  final String placeholder;
  final FocusNode? focusNode;
  final int? maxLength;
  final ValueChanged<String>? onSubmitted;

  const _SettingsTextBox({
    required this.controller,
    required this.placeholder,
    this.focusNode,
    this.maxLength,
    this.onSubmitted,
  });

  @override
  Widget build(BuildContext context) {
    final colors = gvColorsOf(context);
    return SizedBox(
      height: 34,
      child: TextBox(
        controller: controller,
        focusNode: focusNode,
        placeholder: placeholder,
        maxLength: maxLength,
        onSubmitted: onSubmitted,
        padding: const EdgeInsets.symmetric(horizontal: 11),
        style: GvTextStyles.caption(colors.ink),
        placeholderStyle: GvTextStyles.caption(colors.muted),
        cursorColor: colors.ink,
        decoration: WidgetStateProperty.all(
          BoxDecoration(
            color: colors.bg,
            borderRadius: BorderRadius.circular(7),
            border: Border.all(color: colors.line),
          ),
        ),
      ),
    );
  }
}

/// 设置页按钮（设计稿 btn / btn.small / btn.tiny / btn.primary / btn.quiet）。
class _SettingsButton extends StatelessWidget {
  final String label;
  final IconData? icon;
  final VoidCallback? onPressed;
  final bool small;
  final bool tiny;
  final bool quiet;
  final bool primary;
  final Color? foreground;
  final Color? borderColor;

  const _SettingsButton({
    required this.label,
    required this.onPressed,
    this.icon,
    this.small = false,
    this.tiny = false,
    this.quiet = false,
    this.primary = false,
    this.foreground,
    this.borderColor,
  });

  @override
  Widget build(BuildContext context) {
    return HoverButton(
      onPressed: onPressed,
      builder: (context, states) {
        final colors = gvColorsOf(context);
        final disabled = states.contains(WidgetState.disabled);
        final hovered = states.contains(WidgetState.hovered) && !disabled;
        final double height;
        final double fontSize;
        final EdgeInsets padding;
        if (tiny) {
          height = 26;
          fontSize = 11.5;
          padding = const EdgeInsets.symmetric(horizontal: 8);
        } else if (small) {
          height = 28;
          fontSize = 12;
          padding = const EdgeInsets.symmetric(horizontal: 10);
        } else {
          height = 30;
          fontSize = 12.5;
          padding = const EdgeInsets.symmetric(horizontal: 12);
        }
        final Color effectiveForeground;
        final Color effectiveBorder;
        final Color background;
        if (primary) {
          effectiveForeground = colors.onSelection;
          effectiveBorder = colors.selection;
          background = hovered
              ? Color.lerp(colors.selection, colors.tint, 0.12)!
              : colors.selection;
        } else if (quiet) {
          effectiveForeground = foreground ?? colors.muted;
          effectiveBorder = Colors.transparent;
          background =
              hovered ? colors.hover.withValues(alpha: 0.6) : Colors.transparent;
        } else {
          effectiveForeground = foreground ?? colors.ink;
          effectiveBorder = borderColor ??
              (hovered
                  ? Color.lerp(colors.line, colors.ink, 0.35)!
                  : colors.line);
          background = hovered
              ? Color.lerp(colors.surface, colors.hover, 0.30)!
              : colors.surface;
        }
        return Opacity(
          opacity: disabled ? 0.36 : 1,
          child: Container(
            height: height,
            padding: padding,
            alignment: AlignmentDirectional.center,
            decoration: BoxDecoration(
              color: background,
              borderRadius: BorderRadius.circular(7),
              border: Border.all(color: effectiveBorder),
            ),
            child: Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                if (icon != null) ...[
                  Icon(icon, size: 14, color: effectiveForeground),
                  const SizedBox(width: 6),
                ],
                Text(
                  label,
                  style: TextStyle(
                    fontSize: fontSize,
                    color: effectiveForeground,
                    fontFamily: GvFonts.ui,
                    fontFamilyFallback: GvFonts.uiFallback,
                  ),
                ),
              ],
            ),
          ),
        );
      },
    );
  }
}
