# GitaView Flutter 宿主 Windows 打包脚本。
# flutter build windows → 布局加注 MSIX 清单与瓦片资产 → makeappx → 可选签名。
#
# 用法：
#   powershell -NoProfile -File scripts\native\package-flutter-windows.ps1 `
#       [-SkipBuild] [-Sign]
#
# 签名：-Sign 时从环境变量 GITAVIEW_SIGN_THUMBPRINT 读取证书指纹
#（测试签名证书须已导入 LocalMachine\TrustedPeople 或 CurrentUser\My）。
# 不传指纹且未设置环境变量时，-Sign 报错退出；无 -Sign 产出未签名包
#（不能直接安装，仅作 CI 可审查产物）。本脚本不提供 -Install；
# 安装仍使用 scripts\native\package-windows.ps1 时代约定：仅在用户明确要求时执行。
#
# 包身份 = GitaView.Native 0.2.0.0（apps\flutter\windows\msix\Package.appxmanifest），
# 与 0.1.5.x WinUI 宿主原位升级替换（LocalState 共用，settings.json 不迁移）。

param(
    [switch]$SkipBuild,
    [switch]$Sign
)

$ErrorActionPreference = "Stop"
$RepoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
Set-Location $RepoRoot

$flutterProject = "$RepoRoot\apps\flutter"
$buildLayout = "$flutterProject\build\windows\x64\runner\Release"
$msixSource = "$flutterProject\windows\msix"

if (-not (Test-Path "$flutterProject\pubspec.yaml")) { Write-Error "flutter project missing"; exit 1 }

if (-not $SkipBuild) {
    $flutter = Get-Command flutter -ErrorAction SilentlyContinue
    if (-not $flutter) { Write-Error "flutter not on PATH; install Flutter SDK or activate env"; exit 1 }
    Write-Host "== flutter build windows --release =="
    & flutter build windows --release
    if ($LASTEXITCODE -ne 0) { Write-Error "flutter build failed"; exit 1 }
}

# 布局完整性：可执行文件、Flutter 引擎 DLL、Rust FFI DLL 与资产。
$required = @(
    "gitaview.exe",
    "flutter_windows.dll",
    "gitaview_ffi.dll",
    "data\flutter_assets"
)
foreach ($item in $required) {
    if (-not (Test-Path "$buildLayout\$item")) {
        Write-Error "layout missing: $item (build first without -SkipBuild)"
        exit 1
    }
}
$ffiHash = (Get-FileHash "$buildLayout\gitaview_ffi.dll" -Algorithm SHA256).Hash
Write-Host "layout: $buildLayout"
Write-Host "layout gitaview_ffi.dll sha256: $ffiHash"

# 注入 MSIX 清单与瓦片资产（AppxManifest.xml 为 makeappx 要求的名称）。
# flutter build 输出没有顶层 assets\ 目录：先创建再拷贝，避免 Copy-Item
# 对不存在目标的行为歧义。
New-Item -ItemType Directory -Force "$buildLayout\assets" | Out-Null
Copy-Item "$msixSource\Package.appxmanifest" "$buildLayout\AppxManifest.xml" -Force
Copy-Item "$msixSource\assets\*" "$buildLayout\assets\" -Force
# 手工打包链不生成 resources.pri：MRT 不解析 scale 限定符，清单引用的
# 基名文件必须实存（从 scale-200 变体生成；多尺度分辨率留设备验收评估）。
$pairs = @(
    @('Square150x150Logo.scale-200.png', 'Square150x150Logo.png'),
    @('Square44x44Logo.scale-200.png', 'Square44x44Logo.png'),
    @('Wide310x150Logo.scale-200.png', 'Wide310x150Logo.png'),
    @('SplashScreen.scale-200.png', 'SplashScreen.png')
)
foreach ($pair in $pairs) {
    Copy-Item "$buildLayout\assets\$($pair[0])" "$buildLayout\assets\$($pair[1])" -Force
}

# 版本来自清单（发布迭代必须递增；同版本原位安装复用旧部署文件）。
[xml]$manifest = Get-Content "$msixSource\Package.appxmanifest"
$version = $manifest.Package.Identity.Version
$identityName = $manifest.Package.Identity.Name
$outMsix = "$flutterProject\GitaView.Native_${version}_x64.msix"

$makeappx = Get-ChildItem "C:\Program Files (x86)\Windows Kits\10\bin" -Recurse -Filter "makeappx.exe" |
    Where-Object { $_.FullName -match "x64" } |
    Sort-Object FullName -Descending | Select-Object -First 1 -ExpandProperty FullName
$signtool = Get-ChildItem "C:\Program Files (x86)\Windows Kits\10\bin" -Recurse -Filter "signtool.exe" |
    Where-Object { $_.FullName -match "x64" } |
    Sort-Object FullName -Descending | Select-Object -First 1 -ExpandProperty FullName
if (-not $makeappx -or -not $signtool) { Write-Error "Windows SDK tools (makeappx/signtool) not found"; exit 1 }

Write-Host "== makeappx pack =="
& $makeappx pack /h SHA256 /o /d $buildLayout /p $outMsix
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

Write-Host "PACKAGE OK"
exit 0
