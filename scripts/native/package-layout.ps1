# Shared build/package validation. Layouts are isolated by architecture/configuration.
function Get-NativeLayoutPath {
    param(
        [Parameter(Mandatory = $true)][string]$RepositoryRoot,
        [Parameter(Mandatory = $true)][ValidateSet("Debug", "Release")][string]$Configuration,
        [Parameter(Mandatory = $true)][ValidateSet("x64")][string]$Architecture
    )
    return [IO.Path]::GetFullPath((Join-Path $RepositoryRoot "apps\windows\GitaView.Native\pkg\build\$Architecture\$Configuration"))
}

function Assert-NativePackageLayout {
    param(
        [Parameter(Mandatory = $true)][string]$RepositoryRoot,
        [Parameter(Mandatory = $true)][ValidateSet("Debug", "Release")][string]$Configuration,
        [Parameter(Mandatory = $true)][ValidateSet("x64")][string]$Architecture
    )
    $layout = Get-NativeLayoutPath $RepositoryRoot $Configuration $Architecture
    $manifest = Join-Path $layout "AppxManifest.xml"
    if (-not (Test-Path -LiteralPath $manifest -PathType Leaf)) {
        throw "layout missing for $Architecture/$Configuration (run build-windows.ps1 -Configuration $Configuration)"
    }
    [xml]$manifestXml = Get-Content -LiteralPath $manifest
    if ($manifestXml.Package.Identity.ProcessorArchitecture -ne $Architecture) {
        throw "layout architecture does not match requested $Architecture"
    }
    $profile = if ($Configuration -eq "Release") { "release" } else { "debug" }
    $profileDll = Join-Path $RepositoryRoot "target\$profile\gitaview_ffi.dll"
    $layoutDll = Join-Path $layout "gitaview_ffi.dll"
    if (-not (Test-Path -LiteralPath $profileDll -PathType Leaf)) { throw "profile DLL missing: $profileDll" }
    if (-not (Test-Path -LiteralPath $layoutDll -PathType Leaf)) { throw "layout DLL missing: $layoutDll" }
    $actualHash = (Get-FileHash -LiteralPath $layoutDll -Algorithm SHA256).Hash
    if ($actualHash -ne (Get-FileHash -LiteralPath $profileDll -Algorithm SHA256).Hash) {
        throw "layout DLL does not match Rust profile '$profile'; rebuild $Configuration before packaging"
    }
    foreach ($runtimeFile in @("coreclr.dll", "hostfxr.dll", "System.Private.CoreLib.dll")) {
        if (-not (Test-Path -LiteralPath (Join-Path $layout $runtimeFile) -PathType Leaf)) {
            throw "self-contained runtime missing: $runtimeFile; rebuild before packaging"
        }
    }
    return [pscustomobject]@{
        LayoutPath = $layout
        ManifestPath = $manifest
        IdentityName = $manifestXml.Package.Identity.Name
        Version = $manifestXml.Package.Identity.Version
        RustProfile = $profile
        FfiHash = $actualHash
    }
}
