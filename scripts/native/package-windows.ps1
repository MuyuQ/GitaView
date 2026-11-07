# GitaView 原生 Windows 打包脚本（N17）。
# 从 build-windows.ps1 的布局产出 MSIX；可选测试签名与安装。
#
# 用法：
#   powershell -NoProfile -File scripts\native\package-windows.ps1 `
#       [-Configuration Debug] [-Architecture x64] [-SkipBuild] [-Sign] [-Install]
#
# 签名：-Sign 时从环境变量 GITAVIEW_SIGN_THUMBPRINT 读取证书指纹
#（测试签名证书须已导入 LocalMachine\TrustedPeople 或 CurrentUser\My）。
# 不传指纹且未设置环境变量时，-Sign 报错退出；无 -Sign 产出未签名包
#（不能直接安装，仅作 CI 可审查产物）。
#
# -Install：签名安装到本机（需要签名成功）。安装会清除应用 LocalState。

param(
    [ValidateSet("Debug", "Release")]
    [string]$Configuration = "Debug",
    [ValidateSet("x64")]
    [string]$Architecture = "x64",
    [switch]$SkipBuild,
    [switch]$Sign,
    [switch]$Install
)

$ErrorActionPreference = "Stop"
$RepoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
. "$PSScriptRoot\package-layout.ps1"
Set-Location $RepoRoot

if (-not $SkipBuild) {
    & "$PSScriptRoot\build-windows.ps1" -Configuration $Configuration -Architecture $Architecture
    if ($LASTEXITCODE -ne 0) { Write-Error "build failed"; exit 1 }
}

$validatedLayout = Assert-NativePackageLayout $RepoRoot $Configuration $Architecture
$layout = $validatedLayout.LayoutPath

# 版本号来自清单（发布迭代必须递增，同版本 Remove+Add 会复用旧部署文件）。
$version = $validatedLayout.Version
$identityName = $validatedLayout.IdentityName
$outMsix = "$RepoRoot\apps\windows\GitaView.Native\GitaView.Native_${version}_${Architecture}.msix"

$makeappx = Get-ChildItem "C:\Program Files (x86)\Windows Kits\10\bin" -Recurse -Filter "makeappx.exe" |
    Where-Object { $_.FullName -match "x64" } |
    Sort-Object FullName -Descending | Select-Object -First 1 -ExpandProperty FullName
$signtool = Get-ChildItem "C:\Program Files (x86)\Windows Kits\10\bin" -Recurse -Filter "signtool.exe" |
    Where-Object { $_.FullName -match "x64" } |
    Sort-Object FullName -Descending | Select-Object -First 1 -ExpandProperty FullName
if (-not $makeappx -or -not $signtool) { Write-Error "Windows SDK tools (makeappx/signtool) not found"; exit 1 }

# Log the payload actually passed to makeappx, after validating its profile.
Write-Host "layout: $layout"
Write-Host "rust profile: $($validatedLayout.RustProfile); layout gitaview_ffi.dll sha256: $($validatedLayout.FfiHash)"

Write-Host "== makeappx pack =="
& $makeappx pack /h SHA256 /o /d $layout /p $outMsix
if ($LASTEXITCODE -ne 0) { Write-Error "makeappx failed"; exit 1 }

if ($Sign) {
    $thumbprint = $env:GITAVIEW_SIGN_THUMBPRINT
    if (-not $thumbprint) {
        Write-Error "GITAVIEW_SIGN_THUMBPRINT not set; cannot sign. Set it to your test certificate thumbprint."
        exit 1
    }
    Write-Host "== signtool sign =="
    & $signtool sign /fd SHA256 /sha1 $thumbprint $outMsix
    if ($LASTEXITCODE -ne 0) { Write-Error "signtool failed"; exit 1 }
    Write-Host "SIGNED: $outMsix"
}
else {
    Write-Host "UNSIGNED DEV ARTIFACT (not installable): $outMsix"
}

if ($Install) {
    if (-not $Sign) { Write-Error "-Install requires -Sign (unsigned packages cannot deploy)"; exit 1 }
    $family = (Get-AppxPackage $identityName -ErrorAction SilentlyContinue).PackageFamilyName
    if ($family) { Remove-AppxPackage -Package (Get-AppxPackage $identityName).PackageFullName }
    Add-AppxPackage $outMsix
    if ($LASTEXITCODE -ne 0) { Write-Error "Add-AppxPackage failed"; exit 1 }
    Write-Host "INSTALLED: $identityName $version"
}
Write-Host "PACKAGE OK"
exit 0
