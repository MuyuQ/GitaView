$ErrorActionPreference = "Stop"

$artifacts = @(
  Get-ChildItem -Path "src-tauri/target/release/bundle" -Recurse -File -ErrorAction Stop |
    Where-Object { $_.Extension -in @(".exe", ".msi") }
)
if ($artifacts.Count -eq 0) {
  throw "No Windows installer artifacts were found."
}

foreach ($artifact in $artifacts) {
  $signature = Get-AuthenticodeSignature -LiteralPath $artifact.FullName
  if ($signature.Status -ne "Valid") {
    throw "Invalid Authenticode signature for $($artifact.FullName): $($signature.Status)"
  }
  Write-Output "Verified Authenticode signature: $($artifact.Name)"
}
