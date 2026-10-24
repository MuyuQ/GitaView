# GitaView 原生 Windows 构建脚本（N17）。
# 全部步骤来自 docs/native-app-progress.md 检查记录中实测过的命令。
#
# 用法：
#   powershell -NoProfile -File scripts\native\build-windows.ps1 [-Configuration Debug] [-Architecture x64]
#
# 产物：apps\windows\GitaView.Native\pkg\build\<布局>（供 package-windows.ps1 打包）。
# 签名在 package 阶段完成；本脚本只编译与组装布局。

param(
    [ValidateSet("Debug", "Release")]
    [string]$Configuration = "Debug",
    [ValidateSet("x64")]
    [string]$Architecture = "x64"
)

$ErrorActionPreference = "Stop"
$RepoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
Set-Location $RepoRoot

Write-Host "== GitaView native build (Windows $Architecture $Configuration) =="

# 1) Rust FFI（cdylib + staticlib）
# Configuration → Cargo profile 映射（review R11）：Release 必须 --release，
# 复制 target/release 的 DLL；Debug 对应 target/debug。
$cargoArgs = @("build", "-p", "gitaview-ffi")
if ($Configuration -eq "Release") { $cargoArgs += "--release" }
Write-Host "-- cargo $($cargoArgs -join ' ')"
& cargo @cargoArgs
if ($LASTEXITCODE -ne 0) { Write-Error "cargo build failed"; exit 1 }

# 2) WinUI 应用（官方 MSIX 打包目标产出 build recipe + AppX 布局）
# SkipRustBuild=true：Rust 已由本脚本按正确 profile 编译，MSBuild 不再重复构建。
Write-Host "-- dotnet publish GitaView.Native ($Configuration)"
dotnet publish "apps\windows\GitaView.Native\GitaView.Native.csproj" `
    -c $Configuration -p:Platform=$Architecture `
    -p:AppxPackage=true -p:AppxPackageSigningEnabled=false -p:SkipRustBuild=true
if ($LASTEXITCODE -ne 0) { Write-Error "dotnet publish failed"; exit 1 }

# 3) 从官方 build recipe 组装 MSIX 布局（Include → PackagePath 机械复制）
$recipePath = "apps\windows\GitaView.Native\bin\$Architecture\$Configuration\net10.0-windows10.0.26100.0\win-x64\GitaView.Native.build.appxrecipe"
if (-not (Test-Path $recipePath)) {
    # Release 布局路径可能不同：按模式查找最新的 recipe。
    $recipePath = Get-ChildItem "apps\windows\GitaView.Native\bin" -Recurse -Filter "GitaView.Native.build.appxrecipe" |
        Where-Object { $_.FullName -like "*\$Configuration\*" } | Sort-Object LastWriteTime -Descending | Select-Object -First 1 -ExpandProperty FullName
}
if (-not (Test-Path $recipePath)) { Write-Error "build recipe not found"; exit 1 }

$layout = "apps\windows\GitaView.Native\pkg\build"
if (Test-Path $layout) { Remove-Item $layout -Recurse -Force }

[xml]$recipe = Get-Content $recipePath
$ns = New-Object System.Xml.XmlNamespaceManager($recipe.NameTable)
$ns.AddNamespace("m", "http://schemas.microsoft.com/developer/msbuild/2003")
$items = $recipe.SelectNodes("//m:AppxPackagedFile | //m:AppXManifest", $ns)
$copied = 0
foreach ($item in $items) {
    $source = $item.GetAttribute("Include")
    $packagePathNode = $item.SelectSingleNode("m:PackagePath", $ns)
    $relative = if ($packagePathNode) { $packagePathNode.InnerText } else { [System.IO.Path]::GetFileName($source) }
    $destination = Join-Path $layout ($relative -replace '\\', '/')
    $destinationDir = Split-Path -Parent $destination
    if (-not (Test-Path $destinationDir)) { New-Item -ItemType Directory -Path $destinationDir -Force | Out-Null }
    Copy-Item $source $destination -Force
    $copied++
}
Write-Host "-- layout assembled: $copied files -> $layout"
Write-Host "BUILD OK"
exit 0
