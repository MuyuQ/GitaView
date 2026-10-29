# GitaView 原生 Windows 测试脚本（N17）。
# 运行打包布局回归、共享 Rust workspace 测试和 C# 跨语言测试；失败返回非零。
#
# 用法：
#   powershell -NoProfile -File scripts\native\test-windows.ps1

$ErrorActionPreference = "Continue"
$RepoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
Set-Location $RepoRoot

$failed = $false

Write-Host "== package layout regression checks =="
powershell -NoProfile -File "$PSScriptRoot\test-package-layout.ps1"
if ($LASTEXITCODE -ne 0) { $failed = $true }

Write-Host "== cargo test --workspace =="
cargo test --workspace
if ($LASTEXITCODE -ne 0) { $failed = $true }

Write-Host "== dotnet test GitaView.Native.Tests =="
dotnet test "apps\windows\GitaView.Native.Tests\GitaView.Native.Tests.csproj"
if ($LASTEXITCODE -ne 0) { $failed = $true }

if ($failed) {
    Write-Host "TEST FAILED" -ForegroundColor Red
    exit 1
}
Write-Host "TEST OK"
exit 0
