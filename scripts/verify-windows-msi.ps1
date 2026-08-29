# P59: Release verify - silent-install the Windows MSI and check bcr.exe exists.
# Run inside a directory containing the .msi file.
#
# NOTE: keep this file ASCII-only (Windows PowerShell 5.1 reads BOM-less files as ANSI).
#
# Usage: powershell -File scripts/verify-windows-msi.ps1
$ErrorActionPreference = "Stop"

$msi = Get-ChildItem -Path . -Filter "*.msi" | Select-Object -First 1
if (-not $msi) {
  Write-Error "No .msi file found"
  exit 1
}

Write-Host "Installing $($msi.Name) ..."
$p = Start-Process msiexec -ArgumentList @("/i", $msi.FullName, "/qn", "/norestart") -Wait -PassThru
if ($p.ExitCode -ne 0) {
  Write-Error "msiexec install failed exit=$($p.ExitCode)"
  exit 1
}
Start-Sleep -Seconds 3

$installed = "C:\Program Files\bcr\bcr.exe"
if (-not (Test-Path $installed)) {
  Write-Error "Installed binary not found: $installed"
  exit 1
}
Write-Host "OK $installed"
