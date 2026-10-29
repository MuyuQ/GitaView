# Isolated package-layout regression tests; no SDK, build, signing or installation.
$ErrorActionPreference = "Stop"
. "$PSScriptRoot\package-layout.ps1"
$tempBase = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd([IO.Path]::DirectorySeparatorChar)
$testRoot = Join-Path $tempBase ("gitaview-package-layout-" + [Guid]::NewGuid().ToString("N"))
$checks = 0

function Expect-Failure([scriptblock]$Action, [string]$Message) {
    $rejected = $false
    try { & $Action | Out-Null }
    catch {
        $rejected = $true
        if ($_.Exception.Message -notlike $Message) { throw }
    }
    if (-not $rejected) { throw "Expected validation failure: $Message" }
    $script:checks++
}

function Write-Layout([string]$Root, [string]$Configuration, [byte[]]$Bytes) {
    $path = Get-NativeLayoutPath -RepositoryRoot $Root -Configuration $Configuration -Architecture x64
    New-Item -ItemType Directory -Path $path -Force | Out-Null
    [IO.File]::WriteAllText((Join-Path $path "AppxManifest.xml"),
        '<Package><Identity Name="GitaView.Native" Version="0.0.1.0" ProcessorArchitecture="x64" /></Package>')
    [IO.File]::WriteAllBytes((Join-Path $path "gitaview_ffi.dll"), $Bytes)
    foreach ($runtimeFile in @("coreclr.dll", "hostfxr.dll", "System.Private.CoreLib.dll")) {
        [IO.File]::WriteAllText((Join-Path $path $runtimeFile), "self-contained runtime fixture")
    }
    return $path
}

try {
    $debugBytes = [Text.Encoding]::UTF8.GetBytes("debug Rust fixture")
    $releaseBytes = [Text.Encoding]::UTF8.GetBytes("release Rust fixture")
    foreach ($profile in @("debug", "release")) {
        New-Item -ItemType Directory -Path (Join-Path $testRoot "target\$profile") -Force | Out-Null
    }
    $debugDll = Join-Path $testRoot "target\debug\gitaview_ffi.dll"
    $releaseDll = Join-Path $testRoot "target\release\gitaview_ffi.dll"
    [IO.File]::WriteAllBytes($debugDll, $debugBytes)
    [IO.File]::WriteAllBytes($releaseDll, $releaseBytes)

    # Build order cannot change which layout Release packaging selects.
    $releaseLayout = Write-Layout $testRoot Release $releaseBytes
    $debugLayout = Write-Layout $testRoot Debug $debugBytes
    if ($releaseLayout -eq $debugLayout) { throw "Debug overwrote the Release layout" }
    $checks++
    $release = Assert-NativePackageLayout -RepositoryRoot $testRoot -Configuration Release -Architecture x64
    if ($release.LayoutPath -ne $releaseLayout -or $release.FfiHash -ne (Get-FileHash -LiteralPath $releaseDll).Hash) {
        throw "Release selected the wrong payload"
    }
    $checks++
    $debug = Assert-NativePackageLayout -RepositoryRoot $testRoot -Configuration Debug -Architecture x64
    if ($debug.LayoutPath -ne $debugLayout -or $debug.FfiHash -ne (Get-FileHash -LiteralPath $debugDll).Hash) {
        throw "Debug selected the wrong payload"
    }
    $checks++

    foreach ($runtimeFile in @("coreclr.dll", "hostfxr.dll", "System.Private.CoreLib.dll")) {
        $runtimePath = Join-Path $releaseLayout $runtimeFile
        Remove-Item -LiteralPath $runtimePath
        Expect-Failure { Assert-NativePackageLayout $testRoot Release x64 } "*self-contained runtime missing*"
        [IO.File]::WriteAllText($runtimePath, "self-contained runtime fixture")
    }

    $layoutDll = Join-Path $releaseLayout "gitaview_ffi.dll"
    [IO.File]::WriteAllBytes($layoutDll, $debugBytes)
    Expect-Failure { Assert-NativePackageLayout $testRoot Release x64 } "*layout DLL does not match*"
    [IO.File]::WriteAllBytes($layoutDll, $releaseBytes)

    [IO.File]::WriteAllBytes($releaseDll, [Text.Encoding]::UTF8.GetBytes("new release not copied into layout"))
    Expect-Failure { Assert-NativePackageLayout $testRoot Release x64 } "*layout DLL does not match*"
    [IO.File]::WriteAllBytes($releaseDll, $releaseBytes)

    $manifestPath = Join-Path $releaseLayout "AppxManifest.xml"
    [IO.File]::WriteAllText($manifestPath,
        '<Package><Identity Name="GitaView.Native" Version="0.0.1.0" ProcessorArchitecture="arm64" /></Package>')
    Expect-Failure { Assert-NativePackageLayout $testRoot Release x64 } "*layout architecture*"
    [IO.File]::WriteAllText($manifestPath,
        '<Package><Identity Name="GitaView.Native" Version="0.0.1.0" ProcessorArchitecture="x64" /></Package>')

    Remove-Item -LiteralPath $layoutDll
    Expect-Failure { Assert-NativePackageLayout $testRoot Release x64 } "*layout DLL missing*"
    [IO.File]::WriteAllBytes($layoutDll, $releaseBytes)
    Remove-Item -LiteralPath $releaseDll
    Expect-Failure { Assert-NativePackageLayout $testRoot Release x64 } "*profile DLL missing*"
    [IO.File]::WriteAllBytes($releaseDll, $releaseBytes)

    # A Debug-only build must not be silently used for Release -SkipBuild.
    $debugOnlyRoot = Join-Path $testRoot "debug-only"
    Write-Layout $debugOnlyRoot Debug $debugBytes | Out-Null
    Expect-Failure { Assert-NativePackageLayout $debugOnlyRoot Release x64 } "*layout missing*"
    Write-Host "PACKAGE LAYOUT TESTS OK: $checks"
}
finally {
    if (Test-Path -LiteralPath $testRoot) {
        $resolved = (Resolve-Path -LiteralPath $testRoot).ProviderPath
        if ([IO.Directory]::GetParent($resolved).FullName -ne $tempBase) { throw "Unexpected test cleanup path" }
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
}
