#!/usr/bin/env bash
# GUI 冒烟测试：验证 bcr 双击/启动行为（防"闪退"回归）。
#
# 覆盖：
#  1. 非终端无参数（Windows 双击等价场景）→ 应自动进入 GUI 并保持运行
#  2. 显式 `bcr gui` → 进程保持运行（不立即退出）
#  3. 终端内无参数 → 仍打印帮助退出（CLI 行为不回归）
#
# 用法: bash tests/gui_smoke.sh <bcr 二进制路径>
set -euo pipefail

BIN="${1:?用法: bash tests/gui_smoke.sh <bcr 二进制>}"
[ -x "$BIN" ] || { echo "错误: 找不到二进制 $BIN"; exit 1; }

# 等待时长（秒）：GUI 启动窗口期
WAIT=3
OS="$(uname -s)"

# 后台启动 + 等待 + 存活检查（返回 0=存活）
launch_and_check() {
  local label="$1"; shift
  "$@" >/dev/null 2>&1 &
  local pid=$!
  sleep "$WAIT"
  if kill -0 "$pid" 2>/dev/null; then
    echo "✓ $label —— 进程存活（GUI 正常）"
    kill "$pid" 2>/dev/null || true
    wait "$pid" 2>/dev/null || true
    return 0
  else
    wait "$pid"
    local code=$?
    echo "✗ $label —— 进程提前退出 code=$code"
    return 1
  fi
}

fail=0

# 1) 非终端无参数 → 应进 GUI（echo | 模拟 stdin 非终端）
if ! launch_and_check "非终端无参数(双击等价)" sh -c "echo '' | '$BIN'"; then
  fail=1
fi

# 2) 显式 gui → 应保持运行
if ! launch_and_check "bcr gui 显式启动" "$BIN" gui; then
  fail=1
fi

# 3) 终端内无参数 → 打印帮助退出（exit 0）
#    仅在有真实 tty 的环境可测（CI runner / 非交互 shell 无可用 /dev/tty）
if (exec 3<>/dev/tty) 2>/dev/null; then
  exec 3<&- 3>&-
  out="$("$BIN" </dev/tty 2>&1 || true)"
  if printf '%s' "$out" | grep -qi "usage\|用法"; then
    echo "✓ 终端无参数 —— 打印帮助（CLI 行为保留）"
  else
    echo "✗ 终端无参数 —— 未打印帮助"
    fail=1
  fi
else
  echo "— 跳过终端无参数场景（无可用 /dev/tty，CI 环境预期）"
fi

if [ "$fail" -ne 0 ]; then
  echo "GUI 冒烟测试失败"
  exit 1
fi
echo "GUI 冒烟测试全部通过"
