#!/usr/bin/env bash
# P30+ / P59: Linux 正式安装包 — tar.gz（便携）+ deb（Debian/Ubuntu）+ rpm（Fedora/RHEL）+ AppImage（跨发行版）。
#
# 用法:
#   scripts/package-linux.sh [release二进制路径] [输出目录]
#   默认: target/release/bcr  →  dist/
#
# 产物:
#   dist/bcr-<ver>-linux-x86_64.tar.gz    （strip 后单二进制，便携）
#   dist/bcr-<ver>-linux-x86_64.deb       （dpkg-deb，含 .desktop + 图标）
#   dist/bcr-<ver>-linux-x86_64.rpm       （rpmbuild，含 .desktop + 图标）
#   dist/bcr-<ver>-linux-x86_64.AppImage  （appimagetool，含 .desktop + 图标）
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
RPM="$OUT_DIR/bcr-${VER}-linux-${ARCH}.rpm"
APPIMAGE="$OUT_DIR/bcr-${VER}-linux-${ARCH}.AppImage"

# 图标（正式分发必需；缺失则报错，避免产出无图标产物）
[ -f "assets/icon.png" ] || { echo "错误: assets/icon.png 不存在" >&2; exit 1; }

# ---------------------------------------------------------------- 1) tar.gz
STRIP_DIR="$(mktemp -d)"
cp "$BIN" "$STRIP_DIR/bcr"
strip "$STRIP_DIR/bcr" 2>/dev/null || true
tar czf "$TARBALL" -C "$STRIP_DIR" bcr
rm -rf "$STRIP_DIR"
echo "✓ $TARBALL ($(du -h "$TARBALL" | cut -f1))"

# ---------------------------------------------------------------- 2) deb
DEB_ROOT="$(mktemp -d)"
mkdir -p "$DEB_ROOT/DEBIAN" "$DEB_ROOT/usr/bin" "$DEB_ROOT/usr/share/doc/bcr" \
         "$DEB_ROOT/usr/share/icons/hicolor/256x256/apps" "$DEB_ROOT/usr/share/applications"
cp "$BIN" "$DEB_ROOT/usr/bin/bcr"
cp "assets/icon.png" "$DEB_ROOT/usr/share/icons/hicolor/256x256/apps/bcr.png"
cat > "$DEB_ROOT/usr/share/applications/bcr.desktop" <<'EOF'
[Desktop Entry]
Type=Application
Name=bcr
Comment=Beyond Compare style file comparison tool
Exec=bcr gui
Icon=bcr
Terminal=false
Categories=Utility;Development;
EOF
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
echo "✓ $DEB    ($(du -h "$DEB" | cut -f1))"

# ---------------------------------------------------------------- 3) rpm
# 需要 rpmbuild（CI 已 apt install rpm；本机无则跳过并提示）
if command -v rpmbuild >/dev/null 2>&1; then
  RPM_ROOT="$(mktemp -d)"
  mkdir -p "$RPM_ROOT"/{BUILD,RPMS,SOURCES,SPECS,SRPMS}
  # Source 目录放二进制 + 图标 + desktop，spec 内 install 布局
  mkdir -p "$RPM_ROOT/srctree/usr/bin" "$RPM_ROOT/srctree/usr/share/icons/hicolor/256x256/apps" "$RPM_ROOT/srctree/usr/share/applications"
  cp "$BIN" "$RPM_ROOT/srctree/usr/bin/bcr"
  cp "assets/icon.png" "$RPM_ROOT/srctree/usr/share/icons/hicolor/256x256/apps/bcr.png"
  cat > "$RPM_ROOT/srctree/usr/share/applications/bcr.desktop" <<'EOF'
[Desktop Entry]
Type=Application
Name=bcr
Comment=Beyond Compare style file comparison tool
Exec=bcr gui
Icon=bcr
Terminal=false
Categories=Utility;Development;
EOF
  cat > "$RPM_ROOT/SPECS/bcr.spec" <<EOF
Name:           bcr
Version:        ${VER}
Release:        1%{?dist}
Summary:        Beyond Compare style file comparison tool (Rust)

License:        MIT
URL:            https://github.com/kevienkay/bcr
BuildArch:      x86_64

%description
Beyond Compare 风格的文件对比工具：文本/文件夹/三路合并/图片/音频标签/CSV
对比，14 种存储后端，GUI + CLI。

%install
rm -rf %{buildroot}
mkdir -p %{buildroot}/usr/bin
mkdir -p %{buildroot}/usr/share/icons/hicolor/256x256/apps
mkdir -p %{buildroot}/usr/share/applications
install -m755 %{_sourcedir}/usr/bin/bcr %{buildroot}/usr/bin/bcr
install -m644 %{_sourcedir}/usr/share/icons/hicolor/256x256/apps/bcr.png %{buildroot}/usr/share/icons/hicolor/256x256/apps/bcr.png
install -m644 %{_sourcedir}/usr/share/applications/bcr.desktop %{buildroot}/usr/share/applications/bcr.desktop

%files
/usr/bin/bcr
/usr/share/icons/hicolor/256x256/apps/bcr.png
/usr/share/applications/bcr.desktop

%changelog
* $(date '+%a %b %d %Y') bcr
- ${VER} release
EOF
  rpmbuild --define "_topdir $RPM_ROOT" --define "_sourcedir $RPM_ROOT/srctree" \
           -bb "$RPM_ROOT/SPECS/bcr.spec" >/dev/null 2>&1
  cp "$RPM_ROOT"/RPMS/x86_64/*.rpm "$RPM" 2>/dev/null || {
    echo "警告: rpm 构建失败（无 x86_64 产物），跳过 rpm" >&2
    rm -f "$RPM"
  }
  rm -rf "$RPM_ROOT"
  [ -f "$RPM" ] && echo "✓ $RPM  ($(du -h "$RPM" | cut -f1))" || true
else
  echo "警告: rpmbuild 未安装，跳过 rpm（CI 已装）" >&2
fi

# ---------------------------------------------------------------- 4) AppImage
# 需要 appimagetool；CI 已下载到 $APPIMAGETOOL，本机无则跳过并提示
if [ -n "${APPIMAGETOOL:-}" ] && [ -x "$APPIMAGETOOL" ]; then
  APP_DIR="$(mktemp -d)/bcr.AppDir"
  mkdir -p "$APP_DIR/usr/bin" "$APP_DIR/usr/share/icons/hicolor/256x256/apps" "$APP_DIR/usr/share/applications"
  cp "$BIN" "$APP_DIR/usr/bin/bcr"
  cp "assets/icon.png" "$APP_DIR/usr/share/icons/hicolor/256x256/apps/bcr.png"
  cat > "$APP_DIR/usr/share/applications/bcr.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=bcr
Comment=Beyond Compare style file comparison tool
Exec=bcr
Icon=bcr
Terminal=false
Categories=Utility;Development;
EOF
  # AppRun：AppImage 入口（直接 exec 二进制）
  cat > "$APP_DIR/AppRun" <<'EOF'
#!/usr/bin/env bash
HERE="$(dirname "$(readlink -f "${0}")")"
exec "$HERE/usr/bin/bcr" "$@"
EOF
  chmod +x "$APP_DIR/AppRun"
  # 图标与 desktop 也放根目录（appimagetool 要求）
  cp "assets/icon.png" "$APP_DIR/bcr.png"
  cp "$APP_DIR/usr/share/applications/bcr.desktop" "$APP_DIR/bcr.desktop"
  ARCH=x86_64 "$APPIMAGETOOL" --appimage-extract-and-run "$APP_DIR" "$APPIMAGE" >/dev/null 2>&1 || {
    echo "警告: AppImage 构建失败，跳过 AppImage" >&2
    rm -f "$APPIMAGE"
  }
  rm -rf "$(dirname "$APP_DIR")"
  [ -f "$APPIMAGE" ] && echo "✓ $APPIMAGE  ($(du -h "$APPIMAGE" | cut -f1))" || true
else
  echo "警告: APPIMAGETOOL 未设置，跳过 AppImage（CI 会下载）" >&2
fi
