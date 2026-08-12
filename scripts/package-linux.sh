#!/usr/bin/env bash
# P30: Linux 安装包打包 — tar.gz（strip 后）+ deb（dpkg-deb）。
#
# 用法:
#   scripts/package-linux.sh [release二进制路径] [输出目录]
#   默认: target/release/bcr  →  dist/
#
# 产物: dist/bcr-<ver>-linux-x86_64.tar.gz
#       dist/bcr-<ver>-linux-x86_64.deb
set -euo pipefail

BIN="${1:-target/release/bcr}"
OUT_DIR="${2:-dist}"
VER="${BCR_VERSION:-$(grep '^version' Cargo.toml | head -1 | sed -E 's/.*"([^"]+)".*/\1/')}"
ARCH="$(uname -m | sed 's/x86_64/x86_64/; s/aarch64/arm64/')"

if [ ! -x "$BIN" ]; then
  echo "错误: 找不到二进制 $BIN（先 cargo build --release）" >&2
  exit 1
fi

mkdir -p "$OUT_DIR"
TARBALL="$OUT_DIR/bcr-${VER}-linux-${ARCH}.tar.gz"
DEB="$OUT_DIR/bcr-${VER}-linux-${ARCH}.deb"

# 1) tar.gz：strip 后打包（归档内文件名为 bcr）
STRIP_DIR="$(mktemp -d)"
cp "$BIN" "$STRIP_DIR/bcr"
strip "$STRIP_DIR/bcr" 2>/dev/null || true
tar czf "$TARBALL" -C "$STRIP_DIR" bcr
rm -rf "$STRIP_DIR"

# 2) deb：dpkg-deb 组装
DEB_ROOT="$(mktemp -d)"
mkdir -p "$DEB_ROOT/DEBIAN" "$DEB_ROOT/usr/bin" "$DEB_ROOT/usr/share/doc/bcr"
cp "$BIN" "$DEB_ROOT/usr/bin/bcr"
cat > "$DEB_ROOT/DEBIAN/control" <<EOF
Package: bcr
Version: ${VER}
Section: utils
Priority: optional
Architecture: amd64
Maintainer: bcr <noreply@example.com>
Depends: libgtk-3-0, libxkbcommon0, libgl1
Description: Beyond Compare 风格的文件对比工具（Rust 实现）
 文本/文件夹/三路合并/图片/音频标签/CSV 对比，14 种存储后端，GUI + CLI。
EOF
echo "bcr ${VER} — Beyond Compare 风格对比工具" > "$DEB_ROOT/usr/share/doc/bcr/changelog"
dpkg-deb --build --root-owner-group "$DEB_ROOT" "$DEB" >/dev/null
rm -rf "$DEB_ROOT"

echo "✓ $TARBALL ($(du -h "$TARBALL" | cut -f1))"
echo "✓ $DEB    ($(du -h "$DEB" | cut -f1))"
