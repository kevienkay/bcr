# P30+ / P59: Windows 正式安装包 — zip（便携，保证）+ MSI（WiX 完整安装，保证）。
#
# MSI 覆盖正式分发要素：
#   - WixUI_InstallDir 安装向导（License 页 + 安装目录选择 + 完成页）
#   - 开始菜单快捷方式（ProgramMenuFolder，卸载时自动移除）
#   - 卸载入口（ARP 控制面板，含 Manufacturer/HelpLink/图标）
#   - perMachine 安装 + MajorUpgrade（旧版本自动升级）
#   - exe 图标嵌入（build.rs winresource 已在构建时完成，快捷方式复用 bcr.ico）
#
# 用法:
#   powershell -File scripts/package-windows.ps1 [-Bin target\release\bcr.exe] [-OutDir dist] [-WixDir <wix311解压目录>]
#
# 产物: dist\bcr-<ver>-windows-x86_64.zip
#       dist\bcr-<ver>-windows-x86_64.msi
param(
  [string]$Bin = "target\release\bcr.exe",
  [string]$OutDir = "dist",
  [string]$WixDir = ""   # 缺省时从 PATH 找 candle/light
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

# 图标（正式分发必需；缺失则报错，避免产出无图标产物）
if (-not (Test-Path "assets\bcr.ico")) {
  Write-Error "assets\bcr.ico not found"
  exit 1
}

# ---------------------------------------------------------------- 1) zip 便携版
$Zip = Join-Path $OutDir "$Base.zip"
$Stage = Join-Path $env:TEMP "bcr-stage-$([guid]::NewGuid().ToString('N'))"
New-Item -ItemType Directory -Force -Path $Stage | Out-Null
Copy-Item $Bin (Join-Path $Stage "bcr.exe")
Copy-Item "assets\bcr.ico" (Join-Path $Stage "bcr.ico")
@"
bcr $Ver — Beyond Compare 风格的文件对比工具
便携版：解压后双击 bcr.exe 即可运行，无需安装。
正式安装（开始菜单快捷方式/卸载入口）：请使用同目录的 .msi 安装包。
用法: bcr --help
GUI:  bcr gui
"@ | Set-Content (Join-Path $Stage "README.txt") -Encoding UTF8
Compress-Archive -Path (Join-Path $Stage "*") -DestinationPath $Zip -Force
Remove-Item -Recurse -Force $Stage
Write-Host "OK $Zip"

# ---------------------------------------------------------------- 2) MSI（WiX）
# 定位工具链：优先 $WixDir，其次 PATH
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
  Write-Error "WiX toolchain (candle/light) not found. 传入 -WixDir <wix311解压目录> 或加入 PATH。"
  exit 1
}

$WixWork = Join-Path $env:TEMP "bcr-wix-$([guid]::NewGuid().ToString('N'))"
New-Item -ItemType Directory -Force -Path $WixWork | Out-Null
Copy-Item $Bin (Join-Path $WixWork "bcr.exe")
Copy-Item "assets\bcr.ico" (Join-Path $WixWork "bcr.ico")

# License.rtf：从 LICENSE 生成简单 RTF（WiX License 页要求 .rtf）
$LicenseText = (Get-Content "LICENSE" -Raw).Trim()
$Escaped = $LicenseText -replace '\\', '\\' -replace '\{', '\{' -replace '\}', '\}'
$Rtf = "{\rtf1\ansi\deff0 {\fonttbl {\f0 \fnil Consolas;}}\f0\fs20 " +
       ($Escaped -replace "`r?`n", "\par ") + "\par }"
Set-Content -Path (Join-Path $WixWork "License.rtf") -Value $Rtf -Encoding ASCII

$Wxs = Join-Path $WixWork "bcr.wxs"
# 注意：用单引号 here-string 避免 PowerShell 插值 `$(var.Version)`（WiX 变量），
# 随后用占位符替换真实版本号
$WxsTemplate = @'
<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="*" Name="bcr" Language="1033" Version="$(var.Version)"
           Manufacturer="bcr" UpgradeCode="{B4C2E7A1-3D6F-4A9B-8C1D-2E5F6A7B8C9D}">
    <Package InstallerVersion="200" Compressed="yes" InstallScope="perMachine"
             Manufacturer="bcr" Description="Beyond Compare 风格的文件对比工具"
             Keywords="diff,compare,merge,sync" />

    <!-- 旧版本自动升级；同版本重装提示 -->
    <MajorUpgrade DowngradeErrorMessage="已安装更新版本的 bcr，请先卸载。"
                  AllowSameVersionUpgrades="yes" />

    <MediaTemplate EmbedCab="yes" />

    <!-- 图标：快捷方式 + 控制面板卸载项 -->
    <Icon Id="app.ico" SourceFile="bcr.ico" />
    <Property Id="ARPPRODUCTICON" Value="app.ico" />
    <Property Id="ARPHELPLINK" Value="https://github.com/kevienkay/bcr" />
    <Property Id="ARPCOMMENTS" Value="Beyond Compare 风格的文件对比工具 (Rust)" />

    <!-- 安装向导：License + 目录选择 + 完成 -->
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
            <Shortcut Id="AppShortcut" Name="bcr" Description="bcr 文件对比工具"
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
  Write-Error "MSI 构建失败: $($_.Exception.Message)"
  exit 1
} finally {
  Pop-Location
  Remove-Item -Recurse -Force $WixWork
}
