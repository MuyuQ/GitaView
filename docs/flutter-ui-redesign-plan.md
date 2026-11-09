# Flutter 宿主界面重设计实施方案

状态：待执行（未开始改代码）
日期：2026-11-08
视觉基准：`design/preview/index.html`（高保真 HTML 设计稿，已定稿）
适用范围：`apps/flutter/lib/**` 与 `apps/flutter/test/ui_test.dart`

> 本文以定稿设计稿为**视觉唯一基准**。文中所有行号对应提交 `a5fda38` 之后的工作区状态，改代码前先确认行号未漂移。
> 设计稿 `design/preview/index.html` 是单文件、无外部依赖，双击即可在浏览器打开；它已随仓库跟踪，不再依赖易被清理的临时工作区目录。

---

## 1. 目标与边界

| 项 | 内容 |
| --- | --- |
| 目标 | 把设计稿的四个视图（主界面 / 设置 / 托盘浮窗 / 状态与空态）+ 浅深主题，1:1 落到 Flutter 宿主 |
| 可改 | `apps/flutter/lib/ui/{main_page,settings_page,flyout_window}.dart`、`lib/ui/theme/tokens.dart`、`lib/app_controller.dart` 的投影层（`GvStatusColor` / `_dotOf` / `_relationColorOf`）、`test/ui_test.dart` |
| 不可改 | `crates/**`、`contracts/**`、`apps/windows/**`、FFI 绑定与引擎生命周期、`pubspec.yaml` 依赖 |
| 交付判定 | `flutter analyze --no-pub` 0 issue；`flutter test --no-pub` 全绿；macOS 真机运行截图（浅 / 深 × 主界面 / 设置 / 浮窗 / 空态） |

产品硬约束不变（见 `AGENTS.md`）：只有 origin 远端、固定 Git 参数、自动扫描不 Fetch、Pull 必须确认、状态关系只有五种、读取失败是错误不是第六种关系、`no_remote` 永远最后。

---

## 2. 视觉基准取值

设计稿的 token 定义在 `index.html` 的 **L8–L82**（`:root` 浅色 L8–L45、`html[data-theme="dark"]` L47–L82），是唯一取值来源。以下为已核对的实际值。

### 2.1 颜色

| 语义角色 | 浅色 | 深色 | 现有字段 |
| --- | --- | --- | --- |
| 正文 ink | `#202733` | `#EDF0F5` | `ink` 不变 |
| 二级正文 | `#3F4B5C` | `#C6CFDB` | **新增** `ink2` |
| 次要文字 | `#54617A` | `#98A5B6` | `muted` 改值 |
| 分隔线 | `#D5DDE7` | `#3C4757` | `line` 不变 |
| 弱分隔线 | `#E6ECF2` | `#293240` | **新增** `lineSoft` |
| 内容底 | `#F7F9FC` | `#0F1318` | `bg` 改值（去 alpha） |
| 表面 | `#FFFFFF` | `#191E26` | `surface` 改值（去 alpha） |
| 悬停层 | `#E7EDF4` | `#252D38` | `hover` 改值（去 alpha） |
| 筛选条底 | `#F4F7FA` | `#151A21` | `filterSurface` 改值（去 alpha） |
| 表头底 | `#EEF3F8` | `#1D242E` | `tableHeader` 改值（去 alpha） |
| 行选中底 | `#EAF1F7` | `#232B36` | `rowSelected` 改值（去 alpha） |
| 同步 | `#0F7A56` | `#46D69D` | `green` 改值 |
| 本地领先 | `#0E7490` | `#67D4E0` | **新增** `teal` |
| 远端领先 | `#2F6BD0` | `#7FA8F0` | **新增** `blue` |
| 分叉 | `#B45309` | `#F2B45E` | `amberText` → 改名 `amber` |
| 错误 | `#C2332C` | `#FF7B72` | `red` 改值 |
| 未比较 / 无远端 | `#64748B` | `#A6B2C3` | `slate` 改值 |
| 品牌强调 | `#7CE2B6` | `#7CE2B6` | **新增** `tint` |
| 强调上的文字 | `#146C4C` | `#9DEFC9` | **新增** `tintInk` |
| 琥珀胶囊文字 | `#FFFFFF` | `#241703` | **新增** `amberOnPill` |
| 强选中底 | `#202733` | `#DCE5F2` | `selection` 不变 |
| 强选中文字 | `#FFFFFF` | `#15191F` | `onSelection` 不变 |
| 浮窗底 | `#F7F9FC` | `#15191F` | `flyoutTint` 改值（去 alpha） |

**两件必须同时处理的事：**

1. **去掉 alpha**。现有 `bg`/`surface`/`hover`/`filterSurface`/`tableHeader`/`rowSelected`/`flyoutTint` 带透明度（Windows 用 `flutter_acrylic` 透出桌面）。设计稿是实心表面，且半透明会让表头对比度随壁纸漂移。改不透明是**刻意决定**，要在 token 注释里写清，不允许实现者"顺手"改回半透明。副作用：Windows 窗口内毛玻璃不可见。
2. **改写文件头注释**。`tokens.dart` L2–L13 现声明"颜色值逐条对应 `GitaViewTheme.xaml`"。改完不再一一对应，必须改为：`teal`/`blue`/`tint`/`tintInk`/`ink2`/`lineSoft` 为 Flutter 侧新增；`amber` 语义由"可同步"收窄为"分叉"；`red` 收窄为错误；表面色去 alpha 的原因。

### 2.2 状态语义（跨文件一致性核心）

| 状态 | `GvStatusColor` | 颜色角色 | 胶囊文字（沿用 `RepoRowVm.statusLabel`，勿新造） |
| --- | --- | --- | --- |
| 读取失败 | `red` | `red` | 失败 |
| 待读 pending | `slate` | `slate` | 待读 |
| 分叉 diverged | `amber` | `amber` | 分叉 |
| 远端领先 remoteAhead | `blue`（新增） | `blue` | 落后 |
| 本地领先 localAhead | `teal`（新增） | `teal` | 领先 |
| 同步 synced | `green` | `green` | 同步 |
| 未比较 / 无远端 | `slate` | `slate` | 无远端 / 未比较（`ErrorText.noComparisonLabel`） |

改动点：

| 文件 | 行 | 动作 |
| --- | --- | --- |
| `app_controller.dart` | 33 | `enum GvStatusColor` 增 `teal`、`blue` |
| `app_controller.dart` | 433–443 `_dotOf` | `localAhead→teal`、`remoteAhead→blue`、`diverged→amber`、`error→red`、`pending→slate` |
| `app_controller.dart` | 445–455 `_relationColorOf` | 与上表同映射（`diverged→amber`，不再 `red`） |
| `main_page.dart` | 628–636 `_colorOf` | exhaustive switch，补 `teal`/`blue`/`amber` 三个 case（编译器会强制） |
| `flyout_window.dart` | 222–231 `_dotColor` | 把 leading/lagging 共用 `amberText` 拆成 `teal`/`blue`，`diverged` 用 `amber` |
| `flyout_window.dart` | 233–242 `_statusColor` | 同上 |

`_statusLabelOf`（L389）、`_relationLabelOf`（L402）、`_changesLabelOf`（L415）、`_hintOf`（L425）**不需要改**，设计稿胶囊用的就是这些既有文案。

### 2.3 字号

`GvFontSizes` 现为 `brand 18 / title 17 / body 14 / caption 13 / micro 12`。

| 用途 | 设计稿 | Flutter 取值 | 说明 |
| --- | --- | --- | --- |
| 品牌名 | 17 | `brand` 由 18 → **17** | 副标题另设 11.5、`muted` |
| 页面标题（设置） | 17 w600 | `title` | — |
| 仓库名 | 14 w500 | `body` + `medium` | — |
| 路径 / 分组 / 分支 | 12 / 12.5 / 12.5 | **统一 13** | `docs/native-app-ui-design.md` §8.2 要求主数据 ≥13，此处刻意比设计稿高 0.5 |
| 表头 | 12 + 0.05em | 12 + `letterSpacing: 0.6` | 新增 12 号角色或局部常量 |
| 状态胶囊 | 12 w500 | 12 w500 | — |
| 差异数字 | 13.5 w600 mono | `GvTextStyles.mono(size: 13.5)` + `semiBold` | — |
| 辅助说明 / 状态栏 | 11.5–12 | **12** | §8.2 下限 |
| 设置分区标题 / 说明 | 14 w600 / 12.5 | 14 w600 / 13 | — |
| 浮窗名称 / 时间 / 统计数字 | 15.5 / 11.5 / 17 mono w600 | 同值 | — |
| 浮窗路径 / 标签 | 11.5 | 11.5 | — |

### 2.4 几何（逐块对照 `index.html` 行号）

| 区块 | 值 | 设计稿行 |
| --- | --- | --- |
| 主工具栏高 | 66（新增 `toolbarHeightMain`；`toolbarHeight = 56` 保留给设置页） | L190 |
| 搜索框 | 高 36、max-width 430、radius 9 | L201 |
| 筛选 chip | 高 29、radius 7、padding 0/11 | L257 |
| 内容区最大宽 | 1000 居中；表格最小宽 680（不足横向滚动） | L275、L276 |
| 表格列 | `minmax(220, 2.5fr) 90 minmax(120, 1.3fr) 112 92 26` | L279 |
| 表头 | 高 37、padding 0/22、下边框 `line` | L282–L288 |
| 数据行 | min-height 60、padding 0/22、悬停 `hover` 38%、展开左侧 3px `tint` 条（上下内缩 9） | L291–L302 |
| 状态胶囊 | 高 24、padding 0/10/0/8、radius 999、边 `currentColor 22%`、底 9%、点 7px | L327–L336 |
| 展开面板 | padding 2/22/16/32、底 `rowSelected`、reveal 200ms | L345–L357 |
| 操作按钮 | 高 30、padding 0/12、radius 7、字号 12.5 | L359–L377 |
| 确认框 | `tint` 55% 边 + `tintSoft` 底 | L378–L385 |
| 状态栏 | 高 42、字号 12、上边框 `lineSoft` | L426 |
| 骨架行 | 高 60，5 行 | L440 |
| 设置侧栏 | 宽 196、nav 项高 34、分组标签 10.5 + 0.09em | L462–L469 |
| 浮窗 | 宽 352；统计 2×2；`fly-row.urgent` 红 7% 底；`fly-note` 虚线上分隔 | L656、L668、L695、L696 |

---

## 3. 主状态页（`main_page.dart`）

### 3.1 结构

```
Column
 ├ _Toolbar          高 66：品牌(30 mark + 名称/副标题) + 搜索(⌘K) + 刷新 + 设置
 ├ _FilterBar 分组   一层（label 宽 32）
 ├ _FilterBar 状态   一层
 ├ Expanded(_RepoTable)
 └ _StatusBar        高 42（替换现有 _Footer，L783）
```

保留 `startupRecovered` 的 InfoBar（配置恢复必须可见）。

### 3.2 工具栏

- 品牌：内嵌 `assets/brand/gitaview.svg` 的 30px 版本（`#202733` 圆角底 + `#F7F9FC` 分支线 + `#7CE2B6` 节点），名称 17、副标题「本地仓库状态台」11.5。
- 搜索：现有 `TextBox` + `maxWidth: 240`（L160）高度与焦点环不匹配设计。改成自绘容器 + `TextField`：高 36、`bg` 底、`line` 边、radius 9、聚焦时 3px `tint` 22% 环、右侧快捷键提示。
- **快捷键必须平台化**：macOS `⌘K`、Windows `Ctrl+K`，用 `Platform.isMacOS` 判断，不要硬编码。
- 搜索占位文案：「搜索仓库、路径」。**不要写"或分支"**——除非先确认核心 `viewQuery` 真的匹配分支字段（属契约行为，UI 不得先行承诺）。
- 刷新：quiet 按钮，高 34，忙态文案「刷新中」+ 图标旋转。
- **删除**刷新按钮上那颗无说明的绿色小圆点（设计稿已删）。

### 3.3 表格（结构改造要点）

现状问题（已核实）：`main_page.dart` L421–L449 里，`_TableHeader`（L429）与所有行（L430 `for` 循环全量构建）同处一个 `Column`，外层包 `SingleChildScrollView`（L444）。**表头会随内容滚走，且无行虚拟化。**

目标结构：

```dart
LayoutBuilder(builder: (ctx, c) {
  final metrics = _TableMetrics.calculate(math.max(c.maxWidth, minContentWidth));
  Widget body = SizedBox(
    width: metrics.contentWidth,
    child: Column(children: [
      _TableHeader(metrics: metrics),            // sticky
      Expanded(child: ListView.builder(           // 行虚拟化
        itemCount: controller.rows.length,
        itemBuilder: (ctx, i) =>
            _RepoRow(controller: controller, row: controller.rows[i], metrics: metrics),
      )),
    ]),
  );
  if (c.maxWidth < minContentWidth) {
    body = SingleChildScrollView(scrollDirection: Axis.horizontal, child: body);
  }
  return body;
});
```

`_TableMetrics`（L454–472）列定义改为 6 列，`minContentWidth` 由 624 → **680**：

| # | 列 | 宽度 | 内容 |
| --- | --- | --- | --- |
| 0 | 仓库 | `minmax(220, 2.5fr)` | 名称 14/w500 单行省略；路径 13 `muted` 单行省略；`hint` 非空时追加 `· hint`（`warn` 用 `red`、`quiet` 用 `slate`） |
| 1 | 分组 | 90 | 13 `muted` |
| 2 | 分支 | `minmax(120, 1.3fr)` | 13 mono `muted` |
| 3 | 状态 | 112 | 状态胶囊：7px 点 + `statusLabel` 12/w500 |
| 4 | 差异 | 92 | 13.5 mono w600，色 = teal/blue/amber/muted |
| 5 | 展开 | 26 | 24px chevron，展开态旋转 90° |

**这是与 `docs/native-app-ui-design.md` §3 的显式偏差**：文档列序为「状态·仓库·分类·分支·关系·提交差异·提示」，设计稿把独立色点列与「关系」列合并成一枚胶囊、把「提示」并入仓库单元格第二行。信息未丢失，列数 7 → 6。**必须同步改文档（见 §6），不能只改代码。**

### 3.4 行与展开面板

- 行：去掉现有 `horizontal: 8` 的卡片内缩（L538 附近），改整行满幅 + `lineSoft` 分隔线；展开行左侧 3px `tint` 竖条（`Stack` + `Positioned`，上下内缩 9），不移动内容。
- 展开面板（`_RowActionsPanel`）分四层：
  1. **元信息行**：`对照 origin/<branch> · ↑3`（右对齐、mono、`ink2`）。无远端 →「未配置远端」`slate`；读取失败 →「读取失败」`red`。全部由 `branch` / `relationLabel` / `changesLabel` 推导，**不新增 controller 字段**。
  2. **操作行**：`Wrap` 间距 6，按钮高 30。Pull/Push 文字色改 `tintInk`（动作色与状态色分离，现为 `amberText`，见 L678/L684）。
  3. **确认框**：独立块（`tintSoft` 底 + `tint` 55% 边）。`strong` = 确认文案，`span` = 后果说明，右侧「取消确认 / 确认 Pull」两个按钮。**不要把取消做成裸文字按钮**（现实现 L694–L702）。
  4. **结果行**：独立一行，成功 `green`、失败 `red`，忙态前缀 14px 环。

### 3.5 状态栏（替换 `_Footer`）

| 位置 | 文案规则 |
| --- | --- |
| 左（刷新失败） | `刷新失败 · 当前显示最近一次成功结果`（`amber`） |
| 左（有筛选） | `已筛选 N / M 个仓库` |
| 左（默认） | `<快捷键> 搜索 · 单击仓库行展开操作 · Esc 取消确认`（平台化） |
| 右 | `上次刷新 HH:mm:ss` |

⚠️ **数据缺口**：设计稿右侧还有「每 N 分钟自动刷新 / 手动刷新」，但主页面不持有 settings（只有进入设置页 `loadSettings()` 才拉取）。v1 **只显示上次刷新时间**；不要为一行文案在主页面新起一次设置查询（会牵动设置页 revision 状态机）。

### 3.6 四种状态

| 状态 | 落地 |
| --- | --- |
| 加载 | 5 行骨架屏（高 60，shimmer 1.5s），替换现有单个 `ProgressRing`；`MediaQuery.disableAnimations` 时退化为静态灰条 |
| 空库 | 品牌图形 16% 透明 → 标题 17/w600「从第一个仓库开始」→ 说明 13「添加仓库目录，或扫描一个工作目录。」→ 两个按钮（添加仓库主色 / 扫描目录次色） |
| 无匹配 | 居中块：「没有匹配的仓库」+ 说明 + 「清除筛选」按钮（现实现只有一行文字，L394–L404） |
| 刷新失败 | 保留历史行 + 状态栏琥珀文案，**不新增大横幅** |

---

## 4. 设置页（`settings_page.dart`）

| 区块 | 改动 |
| --- | --- |
| 标题栏 | 返回 + 「设置」17/w600 + 右侧辅助说明（12 `muted`）；不重复品牌 |
| 侧栏 | 宽 132（L91）→ **196**；分组标签 10.5 + 0.09em；nav 项高 34，选中 `rowSelected`；底部分隔 + 脚注 |
| 内容 | padding 22/34/36，`max-width: 760` 居中 |
| `_SectionCard`（L653） | 由"卡片"改"平铺行"：`padding 18/0/22` + 下边框 `lineSoft`，去掉额外底色与圆角 |
| 仓库设置 | 添加仓库两入口 + 扫描候选块（候选改 `bg` 底 + `lineSoft` 边 + radius 8，整行可点）；已管理仓库行（整行悬停底色 + radius 8，名称 13/w500 + 路径 12，分组下拉收窄到 124，移除平时 `muted`、悬停 `red`）；分组管理（行 + 计数） |
| 常规设置 | 自动刷新拆两行：`启用自动刷新`（开关）+ `刷新间隔`（步进器 + 保存 + 固定位置「已保存」）；`优先分组` 单行；`开机启动` 直接展示 `controller.startupStateText`（**不要加设计稿里的「暂不可用」标签**——状态未接线时那是编造）；`操作确认` 保持辅助说明层级 |
| 外观分区 | **v1 不实现**。设置 schema（`contracts/native/v1`）无外观字段，`docs/native-app-ui-design.md` §7.1 已记录 `AppearanceSettings` 未迁移。设计稿里的主题卡与「减少动效」开关属演示，落地需 Rust 核心 + 契约变更，是另一个任务。 |

---

## 5. 托盘浮窗（`flyout_window.dart`）

| 区块 | 改动 |
| --- | --- |
| 尺寸 | 宽 352 不变（L44）；底色 `flyoutTint`；圆角与投影由宿主窗口负责 |
| 头部 | padding 15/16/14；mark 22 + 名称 15.5/w600 + 右侧时间 11.5 mono；下缘 `lineSoft` |
| 统计 | 长句摘要 → **2×2 网格**：待推送 `localAhead`(teal) / 待拉取 `remoteAhead`(blue) / 需关注 `diverged+error`(red) / 未比较 `noRemote`(slate)。数字 17 mono w600；值为 0 时 `muted` + w500；标签 11.5 `muted` |
| 正文行 | 8px 点 + 名称 13 + 路径 11.5 + 状态胶囊 11.5；默认透明、悬停 `hover` 55%、`urgent`（分叉/失败）`red` 7% 底 |
| 补充行 | `noRemote > 0` 时补 `fly-note`：「另有 N 个仓库未配置远端，未计入待处理。」（虚线上分隔） |
| 空态 | 保留 `TrayFlyoutPresentation.emptyMessage(counts, refreshState)` 作正文文案（纯逻辑层不动），统计格仍显示 0 |
| 底部 | 右对齐三个 quiet 按钮：打开 GitaView / 刷新 / 退出 |

---

## 6. 测试与文档

### 6.1 `apps/flutter/test/ui_test.dart`

| 行 | 现状 | 改为 |
| --- | --- | --- |
| 449 | `find.text('分组')` findsNWidgets(2) | 不变 |
| 450 | `find.text('状态')` findsOneWidget | `findsNWidgets(2)`（筛选标签 + 表头） |
| 461–466 | 断言表头含 `关系`、`提示` | 删这两条，新增 `状态`；保留 `仓库 / 分组 / 分支 / 差异` |
| 470 | `↑3 ↓2` | 不变 |
| 473 | `Git 命令执行失败` | 保留（提示并入路径行，`find.text` 仍命中） |
| 477、599–600 | 断言底部计数摘要 | 改为新状态栏文案（筛选口径 / 刷新失败口径） |
| **新增** | — | `test('关系 → 状态色映射')`：用 `FakeGateway` 逐关系断言 `RepoRowVm.dot`（synced→green、localAhead→teal、remoteAhead→blue、diverged→amber、error→red、noRemote→slate）。这是本次语义变更的唯一保护网 |
| **新增** | — | 浅 / 深主题各渲染一次主页面，断言 `tester.takeException()` 为空（无 overflow） |

不改：`contracts_test`、`engine_service_test`、`ffi_test`、`presentation_test`（协议与纯逻辑未变）。

### 6.2 必须同步的文档

1. `apps/flutter/lib/ui/theme/tokens.dart` 文件头注释（见 §2.1 第 2 点）。
2. `docs/native-app-ui-design.md`：
   - §4 颜色表：新增 teal / blue / tint，`amber` 语义改为分叉，`red` 收窄为错误；
   - §3（L44）与 §7.1（L114）：列序 7 列 → 6 列（状态点与关系合并为胶囊、提示并入仓库单元格）；
   - §8.2（L147）：补充"表格主数据取 13、辅助取 12"。
3. `docs/native-platform-acceptance.md`：新增待办——Flutter 状态色与列序已改，WinUI 宿主（0.1.5.x）未同步，双端一致性未验收。

---

## 7. 验证（交付前必须全跑）

```bash
cd apps/flutter
flutter analyze --no-pub
flutter test --no-pub

cargo build -p gitaview-ffi --release
flutter run -d macos --debug --no-enable-impeller   # 本机 Intel GPU，Impeller 会挂
```

截图清单（必须真机运行，设计稿预览不算）：主界面浅 / 深、展开行 + 确认中、设置-仓库 / 设置-常规、浮窗（有数据 / 空态）、空库、无匹配、刷新失败。

尺寸按 `docs/native-app-ui-design.md` §8.3（L154）：内容区 **760×480、1040×680、640×480**，另加 100% / 150% / 200% 缩放与键盘焦点检查。

---

## 8. 执行顺序

1. `tokens.dart` 重写颜色 / 字号 / 新增角色（含注释改写）。
2. `GvStatusColor` 扩 `teal` / `blue`，`_dotOf` / `_relationColorOf` 改映射。
3. 修 `main_page.dart` `_colorOf` 穷举 switch（编译驱动补到通过）。
4. 工具栏 + 两层筛选条重做。
5. 表格重做（sticky 表头 + `ListView.builder` + 6 列 + 状态胶囊 + 680 最小宽）。
6. 展开面板分层（元信息 / 操作 / 确认 / 结果）。
7. `_StatusBar` 替换 `_Footer`；四态（骨架屏 / 空库 / 无匹配 / 失败）。
8. 设置页（侧栏 196 + 平铺 section + 仓库行 + 自动刷新任务区）。
9. 浮窗（2×2 统计 + 行 + note）。
10. 改测试 → 跑验证 → 补文档 → 出截图。

---

## 9. 风险与已知缺口

| 风险 | 说明 | 处理 |
| --- | --- | --- |
| ⚠️ 双端分裂 | 状态色语义与列序一变，WinUI 宿主与共享文档立刻不一致 | 文档三处同步 + 验收清单挂待办；本次**不碰 Windows 代码** |
| ⚠️ 外观分区 | 设计稿有、契约无 | v1 明确不实现，不留假开关 |
| ⚠️ 状态栏刷新节奏 | 主页面无 settings 数据 | v1 只显示上次刷新 |
| ⚠️ 毛玻璃失效 | 表面改不透明后 Windows 丙烯不可见 | 接受并记录，不自行回退透明 |
| ⚠️ `muted` 变浅 | `#445164 → #54617A`，浅色下对比约 8.4 → 5.6 | 仍 ≥4.5（AA）；若反馈发虚只调这一档 |
| ⚠️ 琥珀胶囊临界 | 浅色 `#B45309` 在表头底上约 4.50:1，胶囊 9% 底会再降 | 落地实测；不足时文字用 `#9A4608` 级别或加 0.5px 字重 |
| ⚠️ 设计稿与实现漂移 | `design/preview/index.html` 是可被随手改动的静态文件，没有测试保护 | 改 Flutter 前先确认设计稿未变；若改了设计稿，同步更新本文的取值表 |

---

## 10. 落地后补充修订（实施完成后追加）

| 项 | 修订 | 原因 |
| --- | --- | --- |
| 骨架屏 | 加载态先渲染真实 `_TableHeader`，列宽取同一 `_TableMetrics`，行区用 `ListView.builder`（`NeverScrollableScrollPhysics`） | 原实现无表头且骨架条用了另一套 flex 宽度：加载完成时表头“跳入”、骨架条与真实列错位；极矮视口下固定 `Column` 还会溢出 |
| 工具栏 | 搜索框外围加 `GvSpacing.content`（16）左右保护间距 | 窄窗下搜索框会填满中间剩余宽度，左边框贴住品牌副标题（640 宽真实渲染实测 ≈0–1.5px）。widget 测试的字体度量与真机不同、不复现该现象，以 640 宽真机截图为判据 |
| 设置页徽标 | 「更改即时保存」→「设置随改随存 · 刷新间隔需保存」 | 原文案与事实冲突：仓库 / 分组 / 开关提交即生效，只有刷新间隔按「保存」提交 |
| 表格 1000 宽上限 | 复核结论：无需改动 | `_ContentArea._frame`（`maxWidth: 1000`）已覆盖骨架屏 / 空库 / 无匹配 / 表格四种状态；1600 宽实测表格宽 1000 居中、左右各 300，不存在“表格满幅 / 空态居中”的横跳 |
| 默认分组 | 去掉“分区标题 + 同名设置行”的双层标题，改由 `_Section.trailing` 在分区标题行右侧直接放下拉 | 原结构在 760 内容宽下左侧只剩一行 12px 的“优先分组”，与上方 14px 分区标题重复，且左侧留白过多 |
| 开机启动 | 标题行加“只读”标记 + 说明文案，状态值下沉为“当前状态”设置行 | 原分区只有一个标题和一句错误文案，看起来像坏掉的开关而不是“只读状态”；核心未提供切换入口，因此不加任何开关或假动作 |
| 截图证据 | 全部 13 张按当前代码重抓（2026-11-08 07:29–07:33） | 原图早于骨架屏 / 文案 / 工具栏三处修订，证据与代码不一致；截图由真机实例经 Dart VM Service `_flutter.screenshot` 抓取（2× Retina），DEBUG 横幅已用 `ext.flutter.debugAllowBanner` 关闭 |

---

## 参考文件

| 文件 | 用途 |
| --- | --- |
| `design/preview/index.html` | 视觉唯一基准（token L8–L82；表格 L277–L337；设置 L462–L520；浮窗 L656–L700） |
| `apps/flutter/lib/ui/theme/tokens.dart` | 颜色 / 字号 / 间距 token |
| `apps/flutter/lib/app_controller.dart` | `RepoRowVm` 与 `*Of` 投影（L33、L389–455） |
| `apps/flutter/lib/ui/main_page.dart` | 主页面（工具栏 L148、表格 L421–506、`_Footer` L783） |
| `apps/flutter/lib/ui/settings_page.dart` | 设置页（侧栏 L91、`_SectionCard` L653） |
| `apps/flutter/lib/ui/flyout_window.dart` | 浮窗（L44 宽度、L222–242 颜色） |
| `apps/flutter/lib/presentation/*.dart` | 筛选 chip 顺序、浮窗空态文案（纯逻辑，勿动） |
| `docs/native-app-ui-design.md` | 双端共同设计约束（§3 列序、§4 颜色、§8.2 字号下限、§8.3 尺寸矩阵） |
| `assets/brand/gitaview.svg` | 品牌图形源 |
