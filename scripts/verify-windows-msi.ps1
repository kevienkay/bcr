# P59: Release verify - silent-install the Windows MSI and check bcr.exe exists.
# Run inside a directory containing the .msi file.
#
# NOTE: keep this file ASCII-only (Windows PowerShell 5.1 reads BOM-less files as ANSI).
#
# msiexec stub caveat: msiexec.exe is a stub that spawns the real install engine
# and can return before the engine finishes, so we poll for the installed binary
# (with an msi log for diagnostics) instead of trusting the exit code alone.
#
# Usage: powershell -File scripts/verify-windows-msi.ps1
$ErrorActionPreference = "Stop"

$msi = Get-ChildItem -Path . -Filter "*.msi" | Select-Object -First 1
if (-not $msi) {
  Write-Error "No .msi file found"
  exit 1
}

$log = Join-Path $env:TEMP "bcr-msi-install.log"
Write-Host "Installing $($msi.Name) ..."
$p = Start-Process msiexec -ArgumentList @("/i", $msi.FullName, "/qn", "/norestart", "/l*v", $log) -Wait -PassThru
if ($p.ExitCode -ne 0) {
  Write-Error "msiexec install failed exit=$($p.ExitCode). Log: $log"
  exit 1
}

# Poll for the binary: the msiexec engine may still be finishing when the stub returns.
$installed = "C:\Program Files\bcr\bcr.exe"
$deadline = (Get-Date).AddSeconds(60)
while (-not (Test-Path $installed) -and (Get-Date) -lt $deadline) {
  Start-Sleep -Seconds 2
}

if (-not (Test-Path $installed)) {
  # Fallback: search common install roots before declaring failure.
  $found = Get-ChildItem "C:\Program Files", "C:\Program Files (x86)" -Filter "bcr.exe" -Recurse -ErrorAction SilentlyContinue | Select-Object -First 1
  if ($found) {
    Write-Host "Found at alternate path: $($found.FullName)"
    $installed = $found.FullName
  } else {
    Write-Error "Installed binary not found after install. Log: $log"
    exit 1
  }
}
Write-Host "OK $installed"
