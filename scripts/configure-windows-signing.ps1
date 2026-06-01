$ErrorActionPreference = "Stop"

$required = @(
  "WINDOWS_CERTIFICATE",
  "WINDOWS_CERTIFICATE_PASSWORD",
  "WINDOWS_CERTIFICATE_THUMBPRINT",
  "WINDOWS_TIMESTAMP_URL"
)
foreach ($name in $required) {
  if ([string]::IsNullOrWhiteSpace([Environment]::GetEnvironmentVariable($name))) {
    throw "Missing required environment variable: $name"
  }
}

$pfxPath = Join-Path $env:RUNNER_TEMP "gitaview-code-signing.pfx"
try {
  [IO.File]::WriteAllBytes($pfxPath, [Convert]::FromBase64String($env:WINDOWS_CERTIFICATE))
  $password = ConvertTo-SecureString $env:WINDOWS_CERTIFICATE_PASSWORD -AsPlainText -Force
  $expectedThumbprint = $env:WINDOWS_CERTIFICATE_THUMBPRINT.Replace(" ", "").ToUpperInvariant()
  $imported = @(Import-PfxCertificate -FilePath $pfxPath -CertStoreLocation Cert:\CurrentUser\My -Password $password)
  $certificate = $imported | Where-Object { $_.Thumbprint.Replace(" ", "").ToUpperInvariant() -eq $expectedThumbprint } | Select-Object -First 1
  if (-not $certificate) {
    throw "Imported certificate did not contain expected thumbprint $expectedThumbprint"
  }

  $configPath = Resolve-Path "src-tauri/tauri.conf.json"
  $config = Get-Content -LiteralPath $configPath -Raw | ConvertFrom-Json
  $config.bundle.windows | Add-Member -NotePropertyName certificateThumbprint -NotePropertyValue $certificate.Thumbprint -Force
  $config.bundle.windows | Add-Member -NotePropertyName timestampUrl -NotePropertyValue $env:WINDOWS_TIMESTAMP_URL -Force
  $config | ConvertTo-Json -Depth 100 | Set-Content -LiteralPath $configPath -Encoding utf8NoBOM
  Write-Output "Configured Windows signing certificate $($certificate.Thumbprint)."
}
finally {
  Remove-Item -LiteralPath $pfxPath -Force -ErrorAction SilentlyContinue
}
