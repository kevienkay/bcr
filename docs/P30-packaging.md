# P30 安装包打包（三平台正式分发）

## 背景

P29 完成后 bcr 功能已对标 BC 主能力，但 CI 只产出**裸二进制**（`bcr-<OS>` artifact，
实测 gzip 压缩 12–16 MB）。用户要求补**正式安装包**：macOS dmg / Windows msi+zip /
Linux deb+tar.gz。

## 目标

| 平台 | 产物 | 预期体积 | 方式 |
|---|---|---|---|
| macOS (arm64) | `bcr-<ver>-macos-arm64.dmg` | ~13 MB | .app 骨架 + hdiutil |
| Windows (x86-64) | `bcr-<ver>-windows-x86_64.zip` | ~13 MB | Compress-Archive（便携版） |
| Windows (x86-64) | `bcr-<ver>-windows-x86_64.msi` | ~13 MB | WiX 完整安装（正式） |
| Linux (x86-64) | `bcr-<ver>-linux-x86_64.tar.gz` | ~16 MB | tar.gz（便携版） |
| Linux (x86-64) | `bcr-<ver>-linux-x86_64.deb` | ~17 MB | dpkg-deb（Debian/Ubuntu） |
| Linux (x86-64) | `bcr-<ver>-linux-x86_64.rpm` | ~17 MB | rpmbuild（Fedora/RHEL） |
| Linux (x86-64) | `bcr-<ver>-linux-x86_64.AppImage` | ~17 MB | appimagetool（跨发行版） |
| 全部 | `SHA256SUMS` | — | sha256sum |

版本号：读取 `CARGO_PKG_VERSION`（Cargo.toml `0.1.0`）。

## 设计

### 1. 打包脚本（`scripts/` 新增）

- **`scripts/package-macos.sh`**：构建 .app 结构
  ```
  bcr.app/Contents/
    Info.plist            # CFBundleIdentifier=com.bcr.app, CFBundleVersion=<ver>
    MacOS/bcr             # release 二进制
    Resources/            # bcr.icns 图标
  ```
  然后 `hdiutil create -volname bcr -srcfolder bcr.app` 生成 dmg。
  GUI 应用带 .app 壳双击可启动（egui 无 Bundle 也能跑，但 dmg 需 .app 结构）。
- **`scripts/package-linux.sh`**（tar.gz + deb + rpm + AppImage）：
  - tar.gz：strip 后打包
  - deb：`dpkg-deb --build` 组装 `DEBIAN/control`（Package: bcr, Version, Architecture: amd64, Depends: libgtk-3-0 libxkbcommon0 libgl1）+ `/usr/bin/bcr` + .desktop + 图标
  - rpm：rpmbuild 从 `bcr.spec`（自动生成）构建，含 /usr/bin/bcr + .desktop + 图标；依赖宿主 rpmbuild（CI 已 `apt install rpm`）
  - AppImage：AppDir 组装（usr/bin/bcr + AppRun + .desktop + 图标），appimagetool 打包；依赖 `APPIMAGETOOL` 环境变量指向 appimagetool 二进制
- **`scripts/package-windows.ps1`**（zip + msi）：
  - zip：`Compress-Archive`（bcr.exe + bcr.ico + README.txt，便携版）
  - msi：WiX 完整安装包——WixUI_InstallDir 向导（License 页 + 目录选择 + 完成页）、开始菜单快捷方式、控制面板卸载入口（ARP：Manufacturer/HelpLink/图标）、perMachine + MajorUpgrade 自动升级；需要 `-WixDir` 指向 wix311 解压目录（candle/light）
  - exe 图标：`build.rs` 用 winresource 把 `assets/bcr.ico` 嵌入资源节（Windows 构建时自动）

### 2. Release 工作流（`.github/workflows/release.yml`）

- 触发：`push` tags `v*`（如 `v0.1.0`）
- matrix 三平台：构建 `--release --locked` → 跑打包脚本 → 上传 artifact → 汇总
  `SHA256SUMS` → 创建 GitHub Release（`softprops/action-gh-release@v2`）挂产物
- 产物命名含版本 + 平台 + 架构，避免冲突
- Windows 工具链：WiX 从官方 GitHub 直链下载 wix311-binaries.zip（`C:\wix311`，弃 chocolatey）；cmake 用预装、nasm 官方直链
- Linux 工具链：`apt install rpm`（rpmbuild）+ 下载 appimagetool 到 `$HOME/tools` 并设 `APPIMAGETOOL`
- verify 矩阵：Windows 静默安装 MSI 后跑 `--version` + GUI 冒烟（`C:\Program Files\bcr\bcr.exe`）；Linux 装 deb 后附加 rpm 内容（rpm2cpio|cpio -t）与 AppImage 运行（`--appimage-extract-and-run --version`，免 FUSE）验证

### 3. 本地可验证

- macOS 本机直接跑 `scripts/package-macos.sh` 出 dmg（hdiutil 系统自带）
- Linux/Windows 由 CI 验证（本机无对应工具链，靠三平台 CI + artifact 下载校验）

## 批次计划

| 批次 | 内容 | 验证 |
|---|---|---|
| 1 | 方案文档 + `scripts/package-macos.sh` + 本地 dmg 实测 | 本机 hdiutil 出 dmg，`hdiutil attach` 校验 |
| 2 | `package-linux.sh` + `package-windows.ps1` | 脚本语法 + 逻辑审查 |
| 3 | `release.yml` 工作流 + SHA256SUMS + GitHub Release | 打测试 tag 触发 CI，下载产物校验大小/哈希 |

## 风险与取舍

- **Windows msi**：WiX 从官方 GitHub 直链下载（wix311-binaries.zip），candle/light 稳定可用；msi 为 Windows 正式安装包（向导/快捷方式/卸载入口），zip 保留为便携版
- **macOS 架构**：当前 runner 产 arm64；Intel 版需加 `macos-13`（x86_64）job，本期不做
  （本机与主流新机器均为 arm64）
- **签名/公证**：不做（个人项目，dmg 首次打开需右键"打开"）。macOS 正式分发（Developer ID 签名 + notarize + staple）需付费开发者账号，待用户购买后接入；Windows Authenticode 签名同理待证书
- **Linux rpm**：BuildArch x86_64，spec 自动生成；rpm 校验/签名（GPG）待正式发布时接入
