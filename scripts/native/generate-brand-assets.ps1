$ErrorActionPreference = 'Stop'
& node (Join-Path $PSScriptRoot 'generate-brand-assets.mjs')
if ($LASTEXITCODE -ne 0) { throw 'Brand asset generation failed' }
