# P59: Release verify — Windows MSI 静默安装验证。
# 在含 .msi 的目录运行：安装到默认位置，确认 bcr.exe 存在。
#
# 用法: powershell -File scripts/verify-windows-msi.ps1
$ErrorActionPreference = "Stop"

$msi = Get-ChildItem -Path . -Filter "*.msi" | Select-Object -First 1
if (-not $msi) {
  Write-Error "未找到 .msi 文件"
  exit 1
}

Write-Host "安装 $($msi.Name) ..."
$p = Start-Process msiexec -ArgumentList @("/i", $msi.FullName, "/qn", "/norestart") -Wait -PassThru
if ($p.ExitCode -ne 0) {
  Write-Error "msiexec 安装失败 exit=$($p.ExitCode)"
  exit 1
}
Start-Sleep -Seconds 3

$installed = "C:\Program Files\bcr\bcr.exe"
if (-not (Test-Path $installed)) {
  Write-Error "安装后未找到 $installed"
  exit 1
}
Write-Host "OK $installed"
