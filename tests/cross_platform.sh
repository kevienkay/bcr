#!/usr/bin/env bash
# bcr 跨平台专项测试：CRLF 换行、文件名大小写、路径分隔符、merge 换行风格
# 三端运行：Linux/macOS 原生 bash，Windows 用 Git Bash（shell: bash）
set -u

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="$ROOT/target/release/bcr"
if [ ! -x "$BIN" ] && [ -x "$ROOT/target/release/bcr.exe" ]; then
  BIN="$ROOT/target/release/bcr.exe"
fi
if [ ! -x "$BIN" ]; then
  echo "== 构建 release 二进制 =="
  (cd "$ROOT" && cargo build --release) || { echo "构建失败"; exit 2; }
fi

WORK="$(mktemp -d "${TMPDIR:-/tmp}/bcr-xp.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT
cd "$WORK"

PASS=0; FAIL=0; FAILED=()
pass() { PASS=$((PASS+1)); echo "  ✅ $1"; }
fail() { FAIL=$((FAIL+1)); FAILED+=("$1"); echo "  ❌ $1"; }
# $1=name $2=expected $3=actual
check() {
  if [ "$2" = "$3" ]; then pass "$1"; else fail "$1  [期望: $2 | 实际: $3]"; fi
}

echo "=============================================="
echo " XP: 跨平台行为（$(uname -s)）"

# XP1 CRLF：LF 与 CRLF 内容一致时应判相同（行尾归一化）
printf 'alpha\nbeta\n' > xp1_lf.txt
printf 'alpha\r\nbeta\r\n' > xp1_crlf.txt
"$BIN" diff xp1_lf.txt xp1_crlf.txt > /dev/null 2>&1
check "XP1 CRLF 文件 diff 无差异" 0 "$?"

# XP2 CRLF 混合：真实内容差异应仍被检出（exit=1）
printf 'alpha\nbeta\n' > xp2_a.txt
printf 'alpha\r\ngamma\r\n' > xp2_b.txt
"$BIN" diff xp2_a.txt xp2_b.txt > /dev/null 2>&1
check "XP2 CRLF 真实差异 exit=1" 1 "$?"

# XP3 文件名大小写：仅当文件系统区分大小写时断言（Linux 典型；Windows/macOS 默认不敏感）
rm -f xp3_A.txt xp3_a.txt
if touch xp3_A.txt 2>/dev/null && touch xp3_a.txt 2>/dev/null \
   && [ -f xp3_A.txt ] && [ -f xp3_a.txt ] \
   && [ "$(ls xp3_?.txt 2>/dev/null | wc -l | tr -d ' ')" = "2" ]; then
  mkdir -p xp3_l xp3_r
  printf 'X\n' > xp3_l/A.txt
  printf 'X\n' > xp3_l/a.txt
  printf 'X\n' > xp3_r/A.txt
  printf 'Y\n' > xp3_r/a.txt
  "$BIN" compare xp3_l xp3_r --compare-content > /dev/null 2>&1
  check "XP3 大小写敏感：a.txt 内容不同=1" 1 "$?"
  echo "       (文件系统大小写敏感)"
else
  echo "  ⏭ XP3 跳过（文件系统大小写不敏感）"
fi
rm -f xp3_A.txt xp3_a.txt

# XP4 路径分隔符：输出相对路径统一用 '/'（Windows 反斜杠仅作分隔符，不应外露）
mkdir -p xp4_l/sub xp4_r/sub
printf 'a\n' > xp4_l/sub/only_l.txt
"$BIN" compare xp4_l xp4_r > xp4_out.txt 2>&1
if grep -q '\\' xp4_out.txt; then
  fail "XP4 输出路径含反斜杠: $(tr '\n' ' ' < xp4_out.txt)"
else
  pass "XP4 路径分隔符统一 '/'"
fi

# XP5 merge 输出换行风格跟随 base（CRLF 源文件 → CRLF 输出）
printf 'a\r\nb\r\n' > xp5_base.txt
printf 'a\r\nB\r\n' > xp5_left.txt
printf 'a\r\nb\r\nc\r\n' > xp5_right.txt
"$BIN" merge xp5_base.txt xp5_left.txt xp5_right.txt -o xp5_out.txt > /dev/null 2>&1
if [ -f xp5_out.txt ] && LC_ALL=C grep -q "$(printf '\r')" xp5_out.txt; then
  pass "XP5 merge 输出保持 CRLF"
else
  fail "XP5 merge 输出应为 CRLF"
fi

echo "=============================================="
echo "跨平台: $PASS 通过 / $FAIL 失败"
if [ "$FAIL" -ne 0 ]; then
  printf '失败项:\n'
  printf '  %s\n' "${FAILED[@]}"
  exit 1
fi
