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
# -Install：签名安装到本机（需要签名成功），并重启 Widget 宿主强制重扫
#（重装包后 Board 不刷新的实测结论）。安装会清除应用 LocalState。

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
Set-Location $RepoRoot

if (-not $SkipBuild) {
    & "$PSScriptRoot\build-windows.ps1" -Configuration $Configuration -Architecture $Architecture
    if ($LASTEXITCODE -ne 0) { Write-Error "build failed"; exit 1 }
}

$layout = "$RepoRoot\apps\windows\GitaView.Native\pkg\build"
$manifest = Join-Path $layout "AppxManifest.xml"
if (-not (Test-Path $manifest)) { Write-Error "layout missing AppxManifest.xml (run build-windows.ps1)"; exit 1 }

# 版本号来自清单（发布迭代必须递增，同版本 Remove+Add 会复用旧部署文件）。
[xml]$manifestXml = Get-Content $manifest
$version = $manifestXml.Package.Identity.Version
$identityName = $manifestXml.Package.Identity.Name
$outMsix = "$RepoRoot\apps\windows\GitaView.Native\GitaView.Native_${version}_${Architecture}.msix"

$makeappx = Get-ChildItem "C:\Program Files (x86)\Windows Kits\10\bin" -Recurse -Filter "makeappx.exe" |
    Where-Object { $_.FullName -match "x64" } |
    Sort-Object FullName -Descending | Select-Object -First 1 -ExpandProperty FullName
$signtool = Get-ChildItem "C:\Program Files (x86)\Windows Kits\10\bin" -Recurse -Filter "signtool.exe" |
    Where-Object { $_.FullName -match "x64" } |
    Sort-Object FullName -Descending | Select-Object -First 1 -ExpandProperty FullName
if (-not $makeappx -or -not $signtool) { Write-Error "Windows SDK tools (makeappx/signtool) not found"; exit 1 }

# 记录实际 Rust profile 与 DLL 来源（review R11）：Release 包必须带 release DLL。
$rustDir = if ($Configuration -eq "Release") { "release" } else { "debug" }
$ffiDll = "$RepoRoot\target\$rustDir\gitaview_ffi.dll"
if (-not (Test-Path $ffiDll)) {
    Write-Error "gitaview_ffi.dll not found for profile '$rustDir' (run build-windows.ps1 -Configuration $Configuration)"; exit 1
}
Write-Host "rust profile: $rustDir; gitaview_ffi.dll sha256: $((Get-FileHash $ffiDll -Algorithm SHA256).Hash)"

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
    # 重装后 Widget 宿主缓存不刷新（实测）：强制重扫。
    Get-Process WidgetBoard, WidgetService -ErrorAction SilentlyContinue | Stop-Process -Force
    # 重装不清运行中的旧 Provider 进程（包删除后进程存活，占用内存且持旧代码）
    Get-Process GitaView.Native.WidgetProvider -ErrorAction SilentlyContinue | Stop-Process -Force
    # 预热 Provider：COM 激活一次，让自包含单文件的首次解压发生在安装时
    #（实测全新缓存首次 COM 激活 5.3s，超过 Board 添加组件的超时 → "无法添加小组件"；
    #  解压缓存永久生效，之后激活 0.1s）。预热后无固定组件，Provider 自行退出。
    try {
        $t = [Type]::GetTypeFromCLSID([Guid]'4caa94b6-2e9c-455a-a482-3af61a355f64')
        $null = [Activator]::CreateInstance($t)
        Start-Sleep -Seconds 2
        Write-Host "provider warmed (extraction cached)"
    } catch {
        Write-Host "provider warm-up skipped: $($_.Exception.Message)"
    }
    Write-Host "INSTALLED: $identityName $version (widget hosts restarted)"
}
Write-Host "PACKAGE OK"
exit 0
