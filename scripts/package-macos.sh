#!/usr/bin/env bash
# P30: macOS 安装包打包 — 生成 bcr.app 结构并用 hdiutil 制作 dmg。
#
# 用法:
#   scripts/package-macos.sh [release二进制路径] [输出目录]
#   默认: target/release/bcr  →  dist/
#
# 产物: dist/bcr-<ver>-macos-arm64.dmg
set -euo pipefail

BIN="${1:-target/release/bcr}"
OUT_DIR="${2:-dist}"
VER="${BCR_VERSION:-$(grep '^version' Cargo.toml | head -1 | sed -E 's/.*"([^"]+)".*/\1/')}"
ARCH="$(uname -m | sed 's/arm64/arm64/; s/x86_64/x86_64/')"

if [ ! -x "$BIN" ]; then
  echo "错误: 找不到二进制 $BIN（先 cargo build --release）" >&2
  exit 1
fi

APP="$OUT_DIR/bcr.app"
DMG="$OUT_DIR/bcr-${VER}-macos-${ARCH}.dmg"
STAGING="$OUT_DIR/staging"

rm -rf "$APP" "$STAGING"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources" "$STAGING"

# Info.plist（egui 应用无需特殊能力，最小 Bundle 即可双击启动）
cat > "$APP/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key>
  <string>bcr</string>
  <key>CFBundleDisplayName</key>
  <string>bcr</string>
  <key>CFBundleIdentifier</key>
  <string>com.bcr.app</string>
  <key>CFBundleVersion</key>
  <string>${VER}</string>
  <key>CFBundleShortVersionString</key>
  <string>${VER}</string>
  <key>CFBundleExecutable</key>
  <string>bcr</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleIconFile</key>
  <string>bcr</string>
  <key>LSMinimumSystemVersion</key>
  <string>11.0</string>
  <key>NSHighResolutionCapable</key>
  <true/>
</dict>
</plist>
EOF

cp "$BIN" "$APP/Contents/MacOS/bcr"
chmod +x "$APP/Contents/MacOS/bcr"

# 应用图标（存在时拷贝 icns 到 Resources）
if [ -f "assets/bcr.icns" ]; then
  cp "assets/bcr.icns" "$APP/Contents/Resources/bcr.icns"
fi

# 链接动态库（本机 rustls/aws-lc 通常静态链接，无需 dylib 拷贝；若依赖外部 dylib 会失败时提示）
if otool -L "$APP/Contents/MacOS/bcr" 2>/dev/null | grep -qE '^\s+/[^/]*(lib|\.dylib)' &&
   ! otool -L "$APP/Contents/MacOS/bcr" 2>/dev/null | grep -qE '/System/|/usr/lib/'; then
  echo "警告: 存在非系统动态库依赖，dmg 可能无法在其他机器运行" >&2
fi

# dmg: 先放一个 Applications 快捷方式（可选，保持简单仅含 .app）
cp -R "$APP" "$STAGING/"
hdiutil create -volname "bcr ${VER}" -srcfolder "$STAGING" -ov -format UDZO "$DMG" >/dev/null
rm -rf "$STAGING" "$APP"   # 只保留 dmg，避免 .app 目录混入产物

echo "✓ $DMG"
echo "  $(du -h "$DMG" | cut -f1)"
