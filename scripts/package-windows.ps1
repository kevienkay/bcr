# P30+ / P59: Windows packages - zip (portable, guaranteed) + MSI (WiX full install, guaranteed).
#
# MSI covers the formal distribution essentials:
#   - WixUI_InstallDir wizard (License page + install dir + finish)
#   - Start Menu shortcut (ProgramMenuFolder, auto-removed on uninstall)
#   - Control Panel uninstall entry (ARP: Manufacturer/HelpLink/icon)
#   - perMachine install + MajorUpgrade (auto-upgrade older versions)
#   - exe icon embedded at build time (build.rs winresource); shortcut reuses bcr.ico
#
# NOTE: keep this file ASCII-only. Windows PowerShell 5.1 reads BOM-less files as
# ANSI, so non-ASCII text breaks parsing on CI.
#
# Usage:
#   powershell -File scripts/package-windows.ps1 [-Bin target\release\bcr.exe] [-OutDir dist] [-WixDir <wix311-dir>]
#
# Outputs: dist\bcr-<ver>-windows-x86_64.zip
#          dist\bcr-<ver>-windows-x86_64.msi
param(
  [string]$Bin = "target\release\bcr.exe",
  [string]$OutDir = "dist",
  [string]$WixDir = ""   # optional: dir containing candle.exe/light.exe; else PATH
)

$ErrorActionPreference = "Stop"

# Version: read from Cargo.toml
$Cargo = Get-Content "Cargo.toml" -Raw
$Ver = [regex]::Match($Cargo, '^version\s*=\s*"([^"]+)"', [System.Text.RegularExpressions.RegexOptions]::Multiline).Groups[1].Value
if (-not $Ver) { $Ver = "0.1.0" }

if (-not (Test-Path $Bin)) {
  Write-Error "Binary not found: $Bin (run cargo build --release first)"
  exit 1
}

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$Base = "bcr-$Ver-windows-x86_64"

# Icon is required for the formal package; fail loudly if missing.
if (-not (Test-Path "assets\bcr.ico")) {
  Write-Error "assets\bcr.ico not found"
  exit 1
}

# ---------------------------------------------------------------- 1) zip (portable)
$Zip = Join-Path $OutDir "$Base.zip"
$Stage = Join-Path $env:TEMP "bcr-stage-$([guid]::NewGuid().ToString('N'))"
New-Item -ItemType Directory -Force -Path $Stage | Out-Null
Copy-Item $Bin (Join-Path $Stage "bcr.exe")
Copy-Item "assets\bcr.ico" (Join-Path $Stage "bcr.ico")
@"
bcr $Ver - Beyond Compare style file comparison tool
Portable edition: unzip and double-click bcr.exe, no install needed.
For the full installer (Start Menu shortcut / uninstall entry), use the .msi.
Usage: bcr --help
GUI:   bcr gui
"@ | Set-Content (Join-Path $Stage "README.txt") -Encoding UTF8
Compress-Archive -Path (Join-Path $Stage "*") -DestinationPath $Zip -Force
Remove-Item -Recurse -Force $Stage
Write-Host "OK $Zip"

# ---------------------------------------------------------------- 2) MSI (WiX)
# Locate toolchain: prefer $WixDir, else PATH
function Find-WixTool([string]$Name) {
  if ($WixDir -and (Test-Path (Join-Path $WixDir "$Name.exe"))) {
    return Join-Path $WixDir "$Name.exe"
  }
  $cmd = Get-Command $Name -ErrorAction SilentlyContinue
  if ($cmd) { return $cmd.Source }
  return $null
}

$candle = Find-WixTool "candle"
$light  = Find-WixTool "light"
if (-not $candle -or -not $light) {
  Write-Error "WiX toolchain (candle/light) not found. Pass -WixDir <wix311-dir> or add to PATH."
  exit 1
}

$WixWork = Join-Path $env:TEMP "bcr-wix-$([guid]::NewGuid().ToString('N'))"
New-Item -ItemType Directory -Force -Path $WixWork | Out-Null
Copy-Item $Bin (Join-Path $WixWork "bcr.exe")
Copy-Item "assets\bcr.ico" (Join-Path $WixWork "bcr.ico")

# License.rtf: build a minimal RTF from LICENSE (WiX License page requires .rtf)
$LicenseText = (Get-Content "LICENSE" -Raw).Trim()
$Escaped = $LicenseText -replace '\\', '\\' -replace '\{', '\{' -replace '\}', '\}'
$Rtf = "{\rtf1\ansi\deff0 {\fonttbl {\f0 \fnil Consolas;}}\f0\fs20 " +
       ($Escaped -replace "`r?`n", "\par ") + "\par }"
Set-Content -Path (Join-Path $WixWork "License.rtf") -Value $Rtf -Encoding ASCII

$Wxs = Join-Path $WixWork "bcr.wxs"
# Single-quoted here-string so PowerShell does NOT interpolate `$(var.Version)`
# (that is a WiX preprocessor variable); replace the placeholder with the real version.
$WxsTemplate = @'
<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="*" Name="bcr" Language="1033" Version="$(var.Version)"
           Manufacturer="bcr" UpgradeCode="{B4C2E7A1-3D6F-4A9B-8C1D-2E5F6A7B8C9D}">
    <Package InstallerVersion="200" Compressed="yes" InstallScope="perMachine"
             Manufacturer="bcr" Description="Beyond Compare style file comparison tool"
             Keywords="diff,compare,merge,sync" />

    <!-- Auto-upgrade older versions; allow same-version reinstall -->
    <MajorUpgrade DowngradeErrorMessage="A newer version of bcr is already installed."
                  AllowSameVersionUpgrades="yes" />

    <MediaTemplate EmbedCab="yes" />

    <!-- Icons: shortcut + Control Panel uninstall entry -->
    <Icon Id="app.ico" SourceFile="bcr.ico" />
    <Property Id="ARPPRODUCTICON" Value="app.ico" />
    <Property Id="ARPHELPLINK" Value="https://github.com/kevienkay/bcr" />
    <Property Id="ARPCOMMENTS" Value="Beyond Compare style file comparison tool (Rust)" />

    <!-- Install wizard: License + dir selection + finish -->
    <WixVariable Id="WixUILicenseRtf" Value="License.rtf" />
    <UIRef Id="WixUI_InstallDir" />
    <Property Id="WIXUI_INSTALLDIR" Value="INSTALLDIR" />

    <Directory Id="TARGETDIR" Name="SourceDir">
      <Directory Id="ProgramFiles64Folder">
        <Directory Id="INSTALLDIR" Name="bcr" />
      </Directory>
      <Directory Id="ProgramMenuFolder">
        <Directory Id="ProgramMenuDir" Name="bcr">
          <Component Id="StartMenuShortcut" Guid="{7A1F0C22-4B8D-4E6A-9C35-2D8B1A6F4E03}">
            <Shortcut Id="AppShortcut" Name="bcr" Description="bcr file comparison tool"
                      Target="[INSTALLDIR]bcr.exe" WorkingDirectory="INSTALLDIR"
                      Icon="app.ico" IconIndex="0" />
            <RemoveFolder Id="ProgramMenuDir" On="uninstall" />
            <RegistryValue Root="HKCU" Key="Software\bcr" Name="installed"
                           Type="integer" Value="1" KeyPath="yes" />
          </Component>
        </Directory>
      </Directory>
    </Directory>

    <Feature Id="Main" Title="bcr" Level="1">
      <Component Id="C1" Guid="{9D3E7B54-6C2F-4A1D-8E5B-3F7C9A0B2D41}" Directory="INSTALLDIR">
        <File Id="F1" Source="bcr.exe" KeyPath="yes" />
      </Component>
      <ComponentRef Id="StartMenuShortcut" />
    </Feature>
  </Product>
</Wix>
'@
$WxsContent = $WxsTemplate.Replace('$(var.Version)', $Ver)
Set-Content -Path $Wxs -Value $WxsContent -Encoding UTF8

Push-Location $WixWork
try {
  & $candle -dVersion="$Ver" bcr.wxs | Out-Null
  if ($LASTEXITCODE -ne 0) { throw "candle failed (exit $LASTEXITCODE)" }
  & $light -ext WixUIExtension -o "bcr.msi" "bcr.wixobj" | Out-Null
  if ($LASTEXITCODE -ne 0) { throw "light failed (exit $LASTEXITCODE)" }
  $Msi = Join-Path $OutDir "$Base.msi"
  Copy-Item "bcr.msi" $Msi -Force
  Write-Host "OK $Msi ($([math]::Round((Get-Item $Msi).Length/1MB,1)) MB)"
} catch {
  Write-Error "MSI build failed: $($_.Exception.Message)"
  exit 1
} finally {
  Pop-Location
  Remove-Item -Recurse -Force $WixWork
}
