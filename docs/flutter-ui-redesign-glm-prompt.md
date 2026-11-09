# 交给 GLM 的任务提示词

用法：把下面代码块里的全部内容复制给 GLM（同一个仓库工作区）。提示词本身要求它先读 `docs/flutter-ui-redesign-plan.md`，那份文档才是完整规格。

---

```text
你在 GitaView 仓库（原生桌面 Git 状态工具，Rust 核心 + Flutter 宿主）中执行一次 Flutter 界面重设计落地任务。

## 第一步：先读文档，不要直接改代码

按顺序读完这四份文件，再动手：

1. docs/flutter-ui-redesign-plan.md  ← 主规格，你的执行依据
2. design/preview/index.html  ← 视觉唯一基准（单文件，双击可开；可只读 CSS 变量区 L8-L82 和各视图样式段）
3. apps/flutter/lib/ui/theme/tokens.dart
4. docs/native-app-ui-design.md（§3 列序、§4 颜色、§8.2 字号下限、§8.3 尺寸矩阵）

规格文档、设计稿、代码三者冲突时，以 docs/flutter-ui-redesign-plan.md 为准；文档没写到的地方先按设计稿；设计稿也没写的，在最后汇报里列为"待确认"，不要自己发明规则。

## 任务

把设计稿确定的主界面、设置页、托盘浮窗、四种状态态（加载/空库/无匹配/刷新失败）和浅深两套主题，1:1 落到 Flutter 宿主。这是视觉与结构改造，不是功能重写：不改 Rust 核心、不改 FFI、不改契约、不改 Windows 宿主、不加依赖。

## 可改范围

- apps/flutter/lib/ui/main_page.dart
- apps/flutter/lib/ui/settings_page.dart
- apps/flutter/lib/ui/flyout_window.dart
- apps/flutter/lib/ui/theme/tokens.dart
- apps/flutter/lib/app_controller.dart 中仅限投影层：GvStatusColor 枚举、_dotOf、_relationColorOf（L33、L433-455）
- apps/flutter/test/ui_test.dart
- docs/native-app-ui-design.md（仅限规格文档里列出的三处同步）
- docs/native-platform-acceptance.md（新增一条待办）

其他文件一律不动。

## 必须遵守的铁律

1. 状态色映射（这是本次核心语义变更，任何一处不一致都算失败）：
   同步 = green；本地领先 localAhead = teal（新增）；远端领先 remoteAhead = blue（新增）；
   分叉 diverged = amber（由原 red 改为 amber）；读取失败 error = red；未比较/无远端 = slate。
   同步点：GvStatusColor 枚举、_dotOf、_relationColorOf、main_page 的 _colorOf 穷举 switch、
   flyout_window 的 _dotColor/_statusColor、设置页与状态页图例。
   胶囊文案继续用现成的 RepoRowVm.statusLabel，不要新造文案。

2. 表面色改不透明是刻意决定。bg / surface / hover / filterSurface / tableHeader / rowSelected / flyoutTint
   去掉 alpha 后，不要"顺手"改回半透明。副作用（Windows 丙烯毛玻璃不可见）已记录在案，照文档执行。
   同时必须改写 tokens.dart 文件头注释（现在写着"逐条对应 GitaViewTheme.xaml"，改完已不成立）。

3. 表格列序 7 列改 6 列：仓库(名称+路径双行) / 分组 / 分支 / 状态胶囊 / 差异 / 展开箭头。
   独立色点列与"关系"列合并成胶囊，"提示"并入仓库单元格第二行。信息不许丢失。
   这是与 docs/native-app-ui-design.md §3 的显式偏差，必须同步改文档，不能只改代码。

4. 表格必须改成 sticky 表头 + ListView.builder 行虚拟化。现状是全量 Column 包在一个
   SingleChildScrollView 里，表头会跟着滚走。最小内容宽由 624 改为 680。

5. 外观设置分区本次不实现。契约里没有外观字段，留一个假开关比不做更糟。
   设计稿里的主题卡和"减少动效"属于演示，不要落地。

6. 快捷键必须平台化：macOS 用 ⌘K，Windows 用 Ctrl+K，用 Platform.isMacOS 判断，不要硬编码。

7. 不要编造。搜索占位文案不许写"或分支"，除非你核实过核心 viewQuery 真的匹配分支。
   开机启动区不许加"暂不可用"标签（设计稿里那个是演示状态）。
   状态栏右侧只显示"上次刷新 HH:mm:ss"，不加"每 N 分钟自动刷新"——主页面拿不到 settings 数据。

8. 不改产品规则：只有 origin 远端、固定 Git 参数、自动扫描不 Fetch、Pull 必须确认、
   状态关系只有五种、读取失败是错误不是第六种关系、no_remote 永远排最后。

## 执行顺序

照文档第 8 节推进，不要跳步：
tokens → 状态色枚举与投影 → _colorOf switch → 工具栏与筛选条 → 表格 → 展开面板分层 →
状态栏与四态 → 设置页 → 浮窗 → 测试 → 验证 → 补文档。

## 验证（必须真跑，贴真实输出）

cd apps/flutter
flutter analyze --no-pub          # 必须 0 issue
flutter test --no-pub             # 必须全绿
cargo build -p gitaview-ffi --release
flutter run -d macos --debug --no-enable-impeller    # 本机 Intel GPU，Impeller 会挂

截图必须来自真机运行，不能拿设计稿截图充当实现结果。覆盖：
主界面浅色 / 深色、展开行 + 确认中、设置-仓库 / 设置-常规、浮窗（有数据 + 空态）、
空库、无匹配、刷新失败。
尺寸按 docs/native-app-ui-design.md §8.3：内容区 760×480、1040×680、640×480。

测试改动按文档 6.1：必须新增一条"关系 → 状态色映射"的断言，逐关系校验 RepoRowVm.dot。
这是本次语义变更的唯一保护网。

## 交付汇报格式

1. 改了哪些文件、每个文件改了什么（一句话一条）
2. analyze / test 的真实输出结论（失败就贴错误，不要粉饰）
3. 截图路径清单 + 每张的内容尺寸、缩放、主题、是原生运行还是预览
4. 与文档不一致、或你判断需要偏离的地方，以及理由
5. 未完成项与待确认项

不要开头写"好的，我来完成这个任务"。不要中途汇报进度。做完一次性交付。
遇到确实无法推进的阻塞（依赖缺失、平台限制、规格自相矛盾），停下来直接说明，不要猜着改。
```

---

## 备注（给使用者，不用复制）

- 提示词刻意把"必须新增状态色映射测试"和"截图必须真机"写成硬要求，这是历次界面任务最容易糊过去的两处。
- 若 GLM 一次改不完，让它按文档第 8 节的 1–10 步分批交付，每批都要 analyze + test 通过，不要留半成品。
- 设计稿已移入仓库：`design/preview/index.html`（单文件、无外部依赖，双击即可打开）。GLM 读得到，也随仓库一起交付。
