# Widget 目录诊断工具

用于排查 GitaView 未出现在 Windows Widgets Board 选择器中的问题。工具查询当前用户可见的 `com.microsoft.windows.widgets` 扩展目录，输出包族名称、扩展 ID 和显示名称。

依赖 Windows 和 .NET SDK 10；目标框架为 `net10.0-windows10.0.26100.0`，构建架构为 x64。在仓库根目录运行：

```powershell
dotnet build tools/windows/WidgetCatalogProbe/WidgetCatalogProbe.csproj -c Release
dotnet run --project tools/windows/WidgetCatalogProbe/WidgetCatalogProbe.csproj -c Release --no-build
```

结果按应用清单中的包名 `GitaView.Native` 和扩展 ID `GitaViewWidget` 判断。包族名称中的 Publisher ID 随签名身份变化，匹配时保留该差异。

| 退出码 | 含义 |
| --- | --- |
| 0 | 目录中存在目标包及扩展 ID |
| 1 | 目录查询异常，错误输出中有具体原因 |
| 2 | 查询成功，但目录为空或没有目标扩展 |

目录中存在扩展只证明当前用户可发现注册，不代表 Provider 能激活、卡片能更新或 Board 已显示小组件。目录为空或缺少 GitaView 时，先检查应用包安装、清单身份和扩展注册，再结合 Provider / Board 日志排查；不能仅据查询结果断定服务缓存过期。

该工具只读取平台目录，不安装应用、不启动扫描引擎、不更改设置或重启系统组件。它独立于应用解决方案和 MSIX，CI 单独编译，并通过原生 C# 测试套件验证目录结果分类。
