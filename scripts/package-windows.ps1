# P30: Windows packaging - zip (guaranteed) + msi (WiX, best effort).
#
# 用法:
#   powershell -File scripts/package-windows.ps1 [-Bin target\release\bcr.exe] [-OutDir dist]
#
# 产物: dist\bcr-<ver>-windows-x86_64.zip  （保证）
#       dist\bcr-<ver>-windows-x86_64.msi  （WiX 可用时）
param(
  [string]$Bin = "target\release\bcr.exe",
  [string]$OutDir = "dist"
)

$ErrorActionPreference = "Stop"

# 版本号：从 Cargo.toml 读取
$Cargo = Get-Content "Cargo.toml" -Raw
$Ver = [regex]::Match($Cargo, '^version\s*=\s*"([^"]+)"', [System.Text.RegularExpressions.RegexOptions]::Multiline).Groups[1].Value
if (-not $Ver) { $Ver = "0.1.0" }

if (-not (Test-Path $Bin)) {
  Write-Error "Binary not found: $Bin (run cargo build --release first)"
  exit 1
}

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$Base = "bcr-$Ver-windows-x86_64"
$Zip = Join-Path $OutDir "$Base.zip"

# 1) zip：bcr.exe + 简短说明
$Stage = Join-Path $env:TEMP "bcr-stage-$([guid]::NewGuid().ToString('N'))"
New-Item -ItemType Directory -Force -Path $Stage | Out-Null
Copy-Item $Bin (Join-Path $Stage "bcr.exe")
# 图标（存在时一并打包）
if (Test-Path "assets\bcr.ico") {
  Copy-Item "assets\bcr.ico" (Join-Path $Stage "bcr.ico")
}
@"
bcr $Ver — Beyond Compare 风格的文件对比工具
用法: bcr --help
GUI:  bcr gui
"@ | Set-Content (Join-Path $Stage "README.txt") -Encoding UTF8
Compress-Archive -Path (Join-Path $Stage "*") -DestinationPath $Zip -Force
Remove-Item -Recurse -Force $Stage
Write-Host "OK $Zip"

# 2) msi：WiX 工具链可用时构建（尽力而为，失败不阻塞 zip）
$Wix = Get-Command candle, light -ErrorAction SilentlyContinue
if (-not $Wix) {
  Write-Host "NOTE: WiX (candle/light) not installed, skipping msi (zip only)"
  exit 0
}

$WixDir = Join-Path $env:TEMP "bcr-wix-$([guid]::NewGuid().ToString('N'))"
New-Item -ItemType Directory -Force -Path $WixDir | Out-Null
Copy-Item $Bin (Join-Path $WixDir "bcr.exe")

$Wxs = Join-Path $WixDir "bcr.wxs"
@"
<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="*" Name="bcr" Language="1033" Version="$Ver"
           Manufacturer="bcr" UpgradeCode="{B4C2E7A1-3D6F-4A9B-8C1D-2E5F6A7B8C9D}">
    <Package InstallerVersion="200" Compressed="yes" InstallScope="perMachine" />
    <MajorUpgrade DowngradeErrorMessage="A newer version is already installed." />
    <MediaTemplate EmbedCab="yes" />
    <Directory Id="TARGETDIR" Name="SourceDir">
      <Directory Id="ProgramFiles64Folder">
        <Directory Id="INSTALLDIR" Name="bcr" />
      </Directory>
    </Directory>
    <Feature Id="Main" Title="bcr" Level="1">
      <Component Id="C1" Guid="*" Directory="INSTALLDIR">
        <File Id="F1" Source="bcr.exe" KeyPath="yes" />
      </Component>
    </Feature>
  </Product>
</Wix>
"@ | Set-Content $Wxs -Encoding UTF8

Push-Location $WixDir
try {
  candle $Wxs | Out-Null
  light "bcr.wixobj" | Out-Null
  $Msi = Join-Path $OutDir "$Base.msi"
  Copy-Item "bcr.msi" $Msi -Force
  Write-Host "OK $Msi"
} catch {
  Write-Host "NOTE: WiX build failed ($($_.Exception.Message)), zip only"
} finally {
  Pop-Location
  Remove-Item -Recurse -Force $WixDir
}
