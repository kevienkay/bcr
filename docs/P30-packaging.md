# P30 安装包打包（三平台正式分发）

## 背景

P29 完成后 bcr 功能已对标 BC 主能力，但 CI 只产出**裸二进制**（`bcr-<OS>` artifact，
实测 gzip 压缩 12–16 MB）。用户要求补**正式安装包**：macOS dmg / Windows msi+zip /
Linux deb+tar.gz。

## 目标

| 平台 | 产物 | 预期体积 | 方式 |
|---|---|---|---|
| macOS (arm64) | `bcr-<ver>-macos-arm64.dmg` | ~13 MB | .app 骨架 + hdiutil |
| Windows (x86-64) | `bcr-<ver>-windows-x86_64.zip` | ~13 MB | Compress-Archive |
| Windows (x86-64) | `bcr-<ver>-windows-x86_64.msi` | ~13 MB | WiX（若工具链可用） |
| Linux (x86-64) | `bcr-<ver>-linux-x86_64.tar.gz` | ~16 MB | tar.gz |
| Linux (x86-64) | `bcr-<ver>-linux-x86_64.deb` | ~17 MB | dpkg-deb |
| 全部 | `SHA256SUMS` | — | sha256sum |

版本号：读取 `CARGO_PKG_VERSION`（Cargo.toml `0.1.0`）。

## 设计

### 1. 打包脚本（`scripts/` 新增）

- **`scripts/package-macos.sh`**：构建 .app 结构
  ```
  bcr.app/Contents/
    Info.plist            # CFBundleIdentifier=com.bcr.app, CFBundleVersion=<ver>
    MacOS/bcr             # release 二进制
    Resources/            # 可选：图标占位
  ```
  然后 `hdiutil create -volname bcr -srcfolder bcr.app` 生成 dmg。
  GUI 应用带 .app 壳双击可启动（egui 无 Bundle 也能跑，但 dmg 需 .app 结构）。
- **`scripts/package-linux.sh`**：
  - tar.gz：strip 后打包
  - deb：`dpkg-deb --build` 组装 `DEBIAN/control`（Package: bcr, Version, Architecture: amd64, Depends: libgtk-3-0 libxkbcommon0）+ `/usr/bin/bcr`
- **`scripts/package-windows.ps1`**：
  - zip：`Compress-Archive`（bcr.exe + 说明）
  - msi：优先 WiX（`candle`/`light`），工具链不可用时仅 zip（CI 上报 note 不失败）

### 2. Release 工作流（`.github/workflows/release.yml`）

- 触发：`push` tags `v*`（如 `v0.1.0`）
- matrix 三平台：构建 `--release --locked` → 跑打包脚本 → 上传 artifact → 汇总
  `SHA256SUMS` → 创建 GitHub Release（`softprops/action-gh-release@v2`）挂产物
- 产物命名含版本 + 平台 + 架构，避免冲突

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

- **Windows msi**：WiX 在 windows-latest runner 需单独安装（`choco install wixtoolset` 较慢，
  失败不阻塞 zip）；msi 为尽力而为，zip 是 Windows 保证产物
- **macOS 架构**：当前 runner 产 arm64；Intel 版需加 `macos-13`（x86_64）job，本期不做
  （本机与主流新机器均为 arm64）
- **图标**：暂用默认，后续可加 `.icns`/`.ico` 资源
- **签名/公证**：不做（个人项目，dmg 首次打开需右键"打开"）
