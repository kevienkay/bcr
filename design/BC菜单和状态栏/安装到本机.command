#!/bin/bash
# ─────────────────────────────────────────────────────────────
# 把「BC菜单和状态栏」设计稿安装到本机两处位置
#   1) ~/Downloads/BC菜单和状态栏        （你最初指定的交付路径）
#   2) ~/.openclaw/workspace-huaqi/bcr/design/  （让编码 Agent 在仓库里就能读到）
#
# 用法：双击本文件，或在终端执行  bash "本文件的完整路径"
# 说明：脚本只做复制，不会改动项目里的源文件；目标已存在时会覆盖同名文件。
# ─────────────────────────────────────────────────────────────
set -uo pipefail

SRC="$(cd "$(dirname "$0")" && pwd)"          # 本脚本所在目录 = 设计稿根目录
DEST_DL="$HOME/Downloads/BC菜单和状态栏"
DEST_REPO="$HOME/.openclaw/workspace-huaqi/bcr/design"

if [ ! -f "$SRC/index.html" ]; then
  echo "✗ 找不到 $SRC/index.html —— 请把本脚本放在「BC菜单和状态栏」文件夹里再运行。"
  exit 1
fi

echo "源目录：$SRC"
echo

# ① 复制到 ~/Downloads
mkdir -p "$DEST_DL"
cp -R "$SRC/." "$DEST_DL/"
find "$DEST_DL" -name "*.artifact.json" -delete 2>/dev/null
echo "✓ 已复制到 $DEST_DL"
ls -1 "$DEST_DL"

# ② 复制到 bcr 仓库（若仓库存在）
if [ -d "$HOME/.openclaw/workspace-huaqi/bcr" ]; then
  mkdir -p "$DEST_REPO"
  cp -R "$SRC" "$DEST_REPO/"
  find "$DEST_REPO/BC菜单和状态栏" -name "*.artifact.json" -delete 2>/dev/null
  echo
  echo "✓ 已复制到 $DEST_REPO/BC菜单和状态栏"
  echo "  编码 Agent 从仓库根目录读：design/BC菜单和状态栏/交接/实现交接.md"
else
  echo
  echo "· 未找到 bcr 仓库（$HOME/.openclaw/workspace-huaqi/bcr），跳过第 ② 步。"
fi

# ③ 打开 Finder 方便查看
if command -v open >/dev/null 2>&1; then
  open "$DEST_DL"
fi

echo
echo "完成。"
