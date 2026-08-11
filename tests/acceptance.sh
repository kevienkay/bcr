#!/usr/bin/env bash
# bcr M1-M4 验收测试套件
# 用法: bash tests/acceptance.sh
set -u

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="$ROOT/target/release/bcr"
if [ ! -x "$BIN" ] && [ -x "$ROOT/target/release/bcr.exe" ]; then
  BIN="$ROOT/target/release/bcr.exe"
fi
if [ ! -x "$BIN" ]; then
  echo "== 构建 release 二进制 =="
  (cd "$ROOT" && cargo build --release) || { echo "构建失败"; exit 2; }
  if [ -x "$ROOT/target/release/bcr.exe" ]; then BIN="$ROOT/target/release/bcr.exe"; fi
fi

WORK="$(mktemp -d /tmp/bcr-accept.XXXXXX)"
trap 'rm -rf "$WORK"' EXIT
cd "$WORK"

# 跨平台：GNU stat 用 -c %Y（Linux/Git Bash），BSD stat 用 -f %m（macOS）
mtime_of() {
  if stat -c %Y "$1" >/dev/null 2>&1; then
    stat -c %Y "$1"
  else
    stat -f %m "$1"
  fi
}
# Windows（Git Bash）下把 /tmp 路径转成原生路径，zip 后端才能打开
ZIP_WORK="$WORK"
if command -v cygpath >/dev/null 2>&1; then
  ZIP_WORK="$(cygpath -m "$WORK")"
fi

# 跨平台 ZIP 打包：优先 zip 命令；Windows runner 无 zip 时回退 Python zipfile
mkzip() {
  local src_dir="$1" out_zip="$2" exclude="${3:-}"
  if command -v zip >/dev/null 2>&1; then
    if [ -n "$exclude" ]; then
      (cd "$src_dir" && zip -qr "$out_zip" . -x "$exclude")
    else
      (cd "$src_dir" && zip -qr "$out_zip" .)
    fi
    return $?
  fi
  local py=python3; command -v python3 >/dev/null 2>&1 || py=python
  local abs_src abs_out
  abs_src="$(cd "$src_dir" && pwd)"
  # 与 zip 命令分支保持一致：输出路径相对 src_dir 解析
  # （调用形如 mkzip m6_dir ../m6_arch.zip，../ 是相对 src_dir 的）
  abs_out="$(cd "$src_dir" && cd "$(dirname "$out_zip")" && pwd)/$(basename "$out_zip")"
  # Git Bash 的 POSIX 路径 Python 打不开，转成原生 Windows 路径
  if command -v cygpath >/dev/null 2>&1; then
    abs_src="$(cygpath -m "$abs_src")"
    abs_out="$(cygpath -m "$abs_out")"
  fi
  "$py" - "$abs_src" "$abs_out" "$exclude" <<'PYEOF'
import os, sys, zipfile
src, out, exclude = sys.argv[1], sys.argv[2], (sys.argv[3] or None)
with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as z:
    for root, _, files in os.walk(src):
        for f in files:
            p = os.path.join(root, f)
            rel = os.path.relpath(p, src).replace(os.sep, "/")
            if exclude and rel == exclude:
                continue
            z.write(p, rel)
PYEOF
}

PASS=0; FAIL=0; FAILED=()
pass() { PASS=$((PASS+1)); echo "  ✅ $1"; }
fail() { FAIL=$((FAIL+1)); FAILED+=("$1"); echo "  ❌ $1"; }
# $1=name $2=expected $3=actual
check() {
  if [ "$2" = "$3" ]; then pass "$1"; else fail "$1  [期望: $2 | 实际: $3]"; fi
}
# $1=name $2=needle $3=haystack
check_contains() {
  if printf '%s' "$3" | grep -Fq -- "$2"; then pass "$1"; else fail "$1  [输出缺少: $2]"; fi
}

echo "=============================================="
echo " M1: 文本 diff"
echo "=============================================="
printf 'fn main() {\n    let x = 10;\n    println!("hello world");\n    let y = x * 2;\n    println!("done: {}", y);\n    // comment\n}\n' > m1_a.txt
printf 'fn main() {\n    let x = 20;\n    println!("hello rust!");\n    let y = x * 2;\n    let z = y + 1;\n    println!("done: {}", y);\n}\n' > m1_b.txt

"$BIN" diff m1_a.txt m1_b.txt > m1_out.txt; rc=$?
check "M1.1 有差异退出码=1" 1 "$rc"
check_contains "M1.2 unified 头部 ---" "--- m1_a.txt" "$(cat m1_out.txt)"
check_contains "M1.3 unified 头部 +++" "+++ m1_b.txt" "$(cat m1_out.txt)"
check_contains "M1.4 hunk 头" "@@ -1,7 +1,7 @@" "$(cat m1_out.txt)"
check_contains "M1.5 删除行标记" "-    let x = 10;" "$(cat m1_out.txt)"
check_contains "M1.6 插入行标记" "+    let x = 20;" "$(cat m1_out.txt)"

"$BIN" diff m1_a.txt m1_b.txt --color=always > m1_color.txt
check_contains "M1.7 行内高亮 ANSI(红底)" "$(printf '\e[1m\e[41m')" "$(cat m1_color.txt)"
check_contains "M1.8 行内高亮 ANSI(绿底)" "$(printf '\e[1m\e[42m')" "$(cat m1_color.txt)"

"$BIN" diff m1_a.txt m1_a.txt > /dev/null; check "M1.9 相同文件退出码=0" 0 "$?"
"$BIN" diff m1_a.txt /nonexistent > /dev/null 2>&1; check "M1.10 缺失文件退出码=2" 2 "$?"

printf 'function foo() {\n  const a = 10;\n  return a * 2;\n}\n' > m1_ws_a.txt
printf 'function foo() {\n\tconst a = 10;\n  return a * 2;\n}\n' > m1_ws_b.txt
"$BIN" diff --ignore-whitespace m1_ws_a.txt m1_ws_b.txt > /dev/null; check "M1.11 --ignore-whitespace=0" 0 "$?"
"$BIN" diff m1_ws_a.txt m1_ws_b.txt > /dev/null; check "M1.12 不忽略时=1" 1 "$?"

printf 'Hello World\nFoo Bar\n' > m1_case_a.txt
printf 'hello world\nfoo bar\n' > m1_case_b.txt
"$BIN" diff --ignore-case m1_case_a.txt m1_case_b.txt > /dev/null; check "M1.13 --ignore-case=0" 0 "$?"

printf 'trailing  \nnext\n' > m1_tr_a.txt
printf 'trailing\nnext\n' > m1_tr_b.txt
"$BIN" diff --ignore-trailing m1_tr_a.txt m1_tr_b.txt > /dev/null; check "M1.14 --ignore-trailing=0" 0 "$?"

printf 'a\nb\nc\n' > m1_stdin.txt
out=$(printf 'a\nb\nc\n' | "$BIN" diff - m1_stdin.txt -L stdin -L file 2>/dev/null); rc=$?
check "M1.15 stdin 相同=0" 0 "$rc"

seq 1 1000 > m1_big.txt
sed 's/^100$/X100/; s/^500$/X500/; s/^900$/X900/' m1_big.txt > m1_big2.txt
n=$(grep -c '^@@' <("$BIN" diff m1_big.txt m1_big2.txt))
check "M1.16 大文件 3 处修改=3 hunk" 3 "$n"

printf '中文第一行\n中文第二行\n中文第三行\n' > m1_cn_a.txt
printf '中文第一行\n中文第二行改\n中文第三行\n' > m1_cn_b.txt
out=$("$BIN" diff m1_cn_a.txt m1_cn_b.txt)
check_contains "M1.17 中文 diff 正常" "中文第二行改" "$out"

"$BIN" diff --algo myers m1_a.txt m1_b.txt > /dev/null; check "M1.18 --algo myers=1" 1 "$?"
"$BIN" diff --algo patience m1_a.txt m1_b.txt > /dev/null; check "M1.19 --algo patience=1" 1 "$?"

echo "=============================================="
echo " M2: 文件夹对比"
echo "=============================================="
mkdir -p m2_l/sub m2_r/sub
echo aaa > m2_l/only_l.txt
echo bbb > m2_r/only_r.txt
echo same > m2_l/same.txt; echo same > m2_r/same.txt
echo v1 > m2_l/diff.txt; echo v2 > m2_r/diff.txt
echo x > m2_l/sub/deep.txt; echo x > m2_r/sub/deep.txt
echo y > m2_l/sub/only_l_deep.txt
touch -t 202601010101 m2_l/diff.txt
touch -t 202601010102 m2_r/diff.txt
touch -t 202601010101 m2_l/same.txt m2_r/same.txt m2_l/sub/deep.txt m2_r/sub/deep.txt

"$BIN" compare m2_l m2_r > m2_out.txt; rc=$?
check "M2.1 有差异退出码=1" 1 "$rc"
check_contains "M2.2 仅左侧 [L]" "[L] only_l.txt" "$(cat m2_out.txt)"
check_contains "M2.3 仅右侧 [R]" "[R] only_r.txt" "$(cat m2_out.txt)"
check_contains "M2.4 内容不同 [C]" "[C] diff.txt" "$(cat m2_out.txt)"
check_contains "M2.5 子目录差异" "[L] sub/only_l_deep.txt" "$(cat m2_out.txt)"
if grep -q '\[S\]' m2_out.txt; then fail "M2.6 默认不显示相同文件"; else pass "M2.6 默认不显示相同文件"; fi

"$BIN" compare m2_l m2_l > /dev/null; check "M2.7 相同目录退出码=0" 0 "$?"
"$BIN" compare m2_l /nonexistent > /dev/null 2>&1; check "M2.8 目录不存在退出码=2" 2 "$?"

echo samecontent > m2_l/mt.txt; echo samecontent > m2_r/mt.txt
touch -t 202601010101 m2_l/mt.txt
touch -t 202602020202 m2_r/mt.txt
"$BIN" compare m2_l m2_r > /dev/null; check "M2.9 快速模式 mtime 不同=1" 1 "$?"
"$BIN" compare m2_l m2_r --compare-content --show-same > m2_c.txt
check_contains "M2.10 内容模式判相同 [S]" "[S] mt.txt" "$(cat m2_c.txt)"
"$BIN" compare m2_l m2_r --compare-content > /dev/null; check "M2.11 内容模式仍有差异=1" 1 "$?"

"$BIN" compare m2_l m2_r --exclude 'sub/**' > m2_f.txt
if grep -q 'sub/' m2_f.txt; then fail "M2.12 --exclude 过滤子目录"; else pass "M2.12 --exclude 过滤子目录"; fi

sum=$("$BIN" compare m2_l m2_r --include 'same.txt' --summary | tail -1)
check_contains "M2.13 --include 白名单统计" "统计: 1 相同, 0 仅左侧, 0 仅右侧, 0 内容不同" "$sum"

if "$BIN" compare m2_l m2_r --color=always | grep -Fq "$(printf '\e[33m')"; then
  pass "M2.14 彩色输出"
else
  fail "M2.14 彩色输出"
fi

echo "=============================================="
echo " M3: 三路合并"
echo "=============================================="
printf 'line1\nline2\nline3\nline4\nline5\n' > m3_base.txt
printf 'L1\nline2\nline3\nline4\nline5\n' > m3_left.txt
printf 'line1\nline2\nline3\nline4\nR5\n' > m3_right.txt

"$BIN" merge m3_base.txt m3_left.txt m3_right.txt > m3_out.txt; rc=$?
check "M3.1 两侧不同区域=0" 0 "$rc"
check_contains "M3.2 合并左侧修改" "L1" "$(cat m3_out.txt)"
check_contains "M3.3 合并右侧修改" "R5" "$(cat m3_out.txt)"

printf 'line1\nline2\nX3\nline4\nline5\n' > m3_left.txt
printf 'line1\nline2\nY3\nline4\nline5\n' > m3_right.txt
"$BIN" merge m3_base.txt m3_left.txt m3_right.txt > m3_conf.txt; rc=$?
check "M3.4 同位置冲突退出码=1" 1 "$rc"
check_contains "M3.5 冲突开标记" "<<<<<<< LEFT" "$(cat m3_conf.txt)"
check_contains "M3.6 冲突中标记" "=======" "$(cat m3_conf.txt)"
check_contains "M3.7 冲突闭标记" ">>>>>>> RIGHT" "$(cat m3_conf.txt)"

printf 'line1\nZ2\nline3\nline4\nline5\n' > m3_left.txt
printf 'line1\nZ2\nline3\nline4\nline5\n' > m3_right.txt
"$BIN" merge m3_base.txt m3_left.txt m3_right.txt > /dev/null; check "M3.8 两侧相同修改=0" 0 "$?"

printf 'line1\nIA\nline2\nline3\nline4\nline5\n' > m3_left.txt
printf 'line1\nline2\nline3\nline4\nline5\n' > m3_right.txt
"$BIN" merge m3_base.txt m3_left.txt m3_right.txt > m3_ins.txt; check "M3.9 单侧插入=0" 0 "$?"
check_contains "M3.10 插入内容合并" "IA" "$(cat m3_ins.txt)"

printf 'line1\nline2\nX3\nline4\nline5\n' > m3_left.txt
printf 'line1\nline2\nY3\nline4\nline5\n' > m3_right.txt
"$BIN" merge m3_base.txt m3_left.txt m3_right.txt -o m3_merged.txt; rc=$?
check "M3.11 -o 写文件退出码=1" 1 "$rc"
check_contains "M3.12 -o 文件含冲突标记" "=======" "$(cat m3_merged.txt)"

if command -v git >/dev/null 2>&1; then
  printf 'L1\nline2\nline3\nline4\nline5\n' > m3_left.txt
  printf 'line1\nline2\nline3\nline4\nR5\n' > m3_right.txt
  "$BIN" merge m3_base.txt m3_left.txt m3_right.txt > m3_bcr.txt
  git merge-file -p m3_left.txt m3_base.txt m3_right.txt > m3_git.txt
  if diff -q m3_bcr.txt m3_git.txt > /dev/null; then
    pass "M3.13 无冲突输出与 git merge-file 逐字节一致"
  else
    fail "M3.13 无冲突输出与 git merge-file 逐字节一致"
  fi
else
  echo "  ⏭  M3.13 跳过（git 未安装）"
fi

seq 1 1000 > m3_big.txt
sed 's/^100$/L100/' m3_big.txt > m3_bl.txt
sed 's/^900$/R900/' m3_big.txt > m3_br.txt
"$BIN" merge m3_big.txt m3_bl.txt m3_br.txt > m3_bigout.txt; check "M3.14 大文件无冲突=0" 0 "$?"
n=$(wc -l < m3_bigout.txt | tr -d ' ')
check "M3.15 大文件输出行数=1000" 1000 "$n"

printf 'line1\nline2\nline3\nline4\nline5\n' > m3_base.txt
printf 'L2\nline2\nline3\nline4\nline5\n' > m3_left.txt
printf 'line1\nR2\nline3\nline4\nline5\n' > m3_right.txt
"$BIN" merge m3_base.txt m3_left.txt m3_right.txt > /dev/null; check "M3.16 相邻行独立修改=0(经典diff3语义)" 0 "$?"

# stdin 作为 left：与 right 相同修改 → 无冲突，验证 stdin 被正确读取并参与合并
printf 'line1\nline2\nX3\nline4\nline5\n' > m3_right.txt
out=$(printf 'line1\nline2\nX3\nline4\nline5\n' | "$BIN" merge m3_base.txt - m3_right.txt 2>/dev/null); rc=$?
check "M3.17 stdin 输入合并=0" 0 "$rc"
check_contains "M3.18 stdin 内容参与合并" "X3" "$out"

echo "=============================================="
echo " M4: 同步引擎"
echo "=============================================="
mkdir -p m4_src m4_dst
echo new > m4_src/new.txt
echo v2 > m4_src/upd.txt
echo same > m4_src/common.txt; echo same > m4_dst/common.txt
echo old > m4_dst/old.txt
echo v1 > m4_dst/upd.txt
touch -t 202601010101 m4_src/common.txt m4_dst/common.txt
touch -t 202601020101 m4_src/upd.txt
touch -t 202601010101 m4_dst/upd.txt

"$BIN" sync m4_src m4_dst --dry-run > m4_dry.txt; check "M4.1 dry-run 有计划=1" 1 "$?"
check_contains "M4.2 dry-run 输出 COPY" "[COPY]   new.txt" "$(cat m4_dry.txt)"
if [ -f m4_dst/new.txt ]; then fail "M4.3 dry-run 未执行写入"; else pass "M4.3 dry-run 未执行写入"; fi

"$BIN" sync m4_src m4_dst > /dev/null; check "M4.4 update 执行=0" 0 "$?"
if [ -f m4_dst/new.txt ]; then pass "M4.5 新增文件已复制"; else fail "M4.5 新增文件已复制"; fi
if grep -q v2 m4_dst/upd.txt; then pass "M4.6 更新文件已覆盖"; else fail "M4.6 更新文件已覆盖"; fi
if [ -f m4_dst/old.txt ]; then pass "M4.7 update 不删除目标独有"; else fail "M4.7 update 不删除目标独有"; fi

"$BIN" sync m4_src m4_dst --dry-run > /dev/null; check "M4.8 update 幂等=0" 0 "$?"

m1=$(mtime_of m4_src/upd.txt); m2=$(mtime_of m4_dst/upd.txt)
if [ "$m1" = "$m2" ]; then pass "M4.9 复制保留源 mtime"; else fail "M4.9 复制保留源 mtime  [$m1 != $m2]"; fi

mkdir -p m4_s2 m4_d2
echo src > m4_s2/s.txt
echo dst > m4_d2/d.txt
echo both-src > m4_s2/b.txt; echo both-dst > m4_d2/b.txt
if "$BIN" sync m4_s2 m4_d2 --mode mirror --dry-run | grep -q '\[DELETE\]'; then
  pass "M4.10 mirror 计划含删除"; else fail "M4.10 mirror 计划含删除"; fi
"$BIN" sync m4_s2 m4_d2 --mode mirror > /dev/null; check "M4.11 mirror 执行=0" 0 "$?"
if [ ! -f m4_d2/d.txt ]; then pass "M4.12 mirror 删除目标独有"; else fail "M4.12 mirror 删除目标独有"; fi
if grep -q both-src m4_d2/b.txt; then pass "M4.13 mirror 无条件覆盖"; else fail "M4.13 mirror 无条件覆盖"; fi
"$BIN" sync m4_s2 m4_d2 --mode mirror --dry-run > /dev/null; check "M4.14 mirror 幂等=0" 0 "$?"

mkdir -p m4_s3 m4_d3
echo L > m4_s3/l.txt
echo R > m4_d3/r.txt
echo LS > m4_s3/both.txt; echo RS > m4_d3/both.txt
touch -t 202601020101 m4_s3/both.txt
touch -t 202601010101 m4_d3/both.txt
"$BIN" sync m4_s3 m4_d3 --mode two-way > /dev/null; check "M4.15 two-way 执行=0" 0 "$?"
if [ -f m4_d3/l.txt ]; then pass "M4.16 two-way 左侧独有→右侧"; else fail "M4.16 two-way 左侧独有→右侧"; fi
if [ -f m4_s3/r.txt ]; then pass "M4.17 two-way 右侧独有→左侧"; else fail "M4.17 two-way 右侧独有→左侧"; fi
if grep -q LS m4_d3/both.txt; then pass "M4.18 two-way 新者胜"; else fail "M4.18 two-way 新者胜"; fi

echo L2 > m4_s3/both.txt; echo R2 > m4_d3/both.txt
touch -t 202601010101 m4_s3/both.txt m4_d3/both.txt
"$BIN" sync m4_s3 m4_d3 --mode two-way --compare-content > m4_tw.txt; check "M4.19 冲突退出码=1" 1 "$?"
check_contains "M4.20 CONFLICT 标记" "[CONFLICT]" "$(cat m4_tw.txt)"

mkdir -p m4_s4 m4_d4
echo only-dst > m4_d4/rev.txt
if "$BIN" sync m4_s4 m4_d4 --reverse --dry-run | grep -q '\[COPY\]'; then
  pass "M4.21 --reverse 方向反转"; else fail "M4.21 --reverse 方向反转"; fi

mkdir -p m4_s5/a/b m4_d5
mkdir -p m4_s5/a/b && echo deep > m4_s5/a/b/c.txt
"$BIN" sync m4_s5 m4_d5 > /dev/null 2>&1; check "M4.22 嵌套目录自动创建=0" 0 "$?"
if [ -f m4_d5/a/b/c.txt ]; then pass "M4.23 深层目标已创建"; else fail "M4.23 深层目标已创建"; fi

echo "=============================================="
echo " M6: 虚拟文件系统（ZIP）"
echo "=============================================="
mkdir -p m6_dir/sub
printf 'same-content' > m6_dir/same.txt
printf 'version-1' > m6_dir/diff.txt
printf 'deep-content' > m6_dir/sub/deep.txt
mkzip m6_dir ../m6_arch.zip

# 对照目录：same.txt 相同、diff.txt 不同、sub/deep.txt 相同
mkdir -p m6_other/sub
printf 'same-content' > m6_other/same.txt
printf 'version-2' > m6_other/diff.txt
printf 'deep-content' > m6_other/sub/deep.txt

"$BIN" compare m6_other "zip://$ZIP_WORK/m6_arch.zip" --compare-content --show-same > m6_out.txt; rc=$?
check "M6.1 本地 vs zip 有差异=1" 1 "$rc"
check_contains "M6.2 内容不同 [C]" "[C] diff.txt" "$(cat m6_out.txt)"
check_contains "M6.3 内容相同 [S]" "[S] same.txt" "$(cat m6_out.txt)"
check_contains "M6.4 子目录条目" "[S] sub/deep.txt" "$(cat m6_out.txt)"

"$BIN" compare "zip://$ZIP_WORK/m6_arch.zip" "zip://$ZIP_WORK/m6_arch.zip" --compare-content > /dev/null; check "M6.5 zip vs zip 无差异=0" 0 "$?"

"$BIN" compare m6_other "zip://$ZIP_WORK/m6_arch.zip" --include 'same.txt' --compare-content > /dev/null; check "M6.6 include 过滤作用于 zip" 0 "$?"

# 子集 zip（缺 diff.txt）：该文件应显示为仅左侧
mkzip m6_dir ../m6_subset.zip diff.txt
"$BIN" compare m6_other "zip://$ZIP_WORK/m6_subset.zip" --compare-content > m6_sub.txt; check "M6.7 缺失条目=1" 1 "$?"
check_contains "M6.8 缺失条目标记 [L]" "[L] diff.txt" "$(cat m6_sub.txt)"

printf 'not-a-zip' > m6_bad.zip
"$BIN" compare m6_dir "zip://$ZIP_WORK/m6_bad.zip" > /dev/null 2>&1; check "M6.9 非法 zip 退出码=2" 2 "$?"

echo "=============================================="
echo " HEX: 十六进制对比（二进制文件）"
echo "=============================================="
# HEX1 相同文件退出码=0
printf 'hello world\n' > hex_a.bin
cp hex_a.bin hex_b.bin
"$BIN" hex hex_a.bin hex_b.bin > hex_out.txt; check "HEX1 相同文件退出码=0" 0 "$?"
# HEX2 内容不同退出码=1
printf 'hello world\n' > hex_c.bin
printf 'hello Borld\n' > hex_d.bin
"$BIN" hex hex_c.bin hex_d.bin > hex_diff.txt; check "HEX2 内容不同退出码=1" 1 "$?"
# HEX3 差异行带 ! 标记
check_contains "HEX3 差异行 ! 标记" "!" "$(head -1 hex_diff.txt)"
# HEX4 输出含偏移
check_contains "HEX4 输出含偏移" "00000000" "$(head -1 hex_diff.txt)"
# HEX5 含 NUL 二进制文件可对比（不报错）
printf 'A\0B\0C' > hex_nul1.bin
printf 'A\0X\0C' > hex_nul2.bin
"$BIN" hex hex_nul1.bin hex_nul2.bin > hex_nul.txt; check "HEX5 含NUL二进制对比退出码=1" 1 "$?"
# HEX6 --show-same 显示全部行（两行：L 与 R）
"$BIN" hex hex_a.bin hex_b.bin --show-same > hex_all.txt
check_contains "HEX6 --show-same 显示行" "L " "$(cat hex_all.txt)"
# HEX7 不同长度文件
printf '0123456789abcdef' > hex_len1.bin
printf '0123456789abcdefg' > hex_len2.bin
"$BIN" hex hex_len1.bin hex_len2.bin > hex_len.txt; check "HEX7 不同长度退出码=1" 1 "$?"

echo "=============================================="
echo " MOVE: 重命名/移动检测"
echo "=============================================="
mkdir -p mv_l mv_r
printf 'same-content-abc' > mv_l/old_name.txt
printf 'same-content-abc' > mv_r/new_name.txt
# MOVE1 相同内容不同路径 → [M] 标记
"$BIN" compare mv_l mv_r > mv_out.txt; rc=$?
grep -q '\[M\] old_name.txt -> new_name.txt' mv_out.txt && pass "MOVE1 重命名检测 [M] 标记" || fail "MOVE1 重命名检测 [M] 标记"
# MOVE2 移动视为差异（退出码 1）
check "MOVE2 移动退出码=1" 1 "$rc"
# MOVE3 内容不同不误判（仍是 L/R）
mkdir -p mv2_l mv2_r
printf 'content-aaaa' > mv2_l/a.txt
printf 'content-bbbb' > mv2_r/b.txt
"$BIN" compare mv2_l mv2_r > mv2_out.txt
grep -q '\[L\] a.txt' mv2_out.txt && grep -q '\[R\] b.txt' mv2_out.txt && pass "MOVE3 内容不同不误判" || fail "MOVE3 内容不同不误判"
# MOVE4 --detect-moves false 关闭检测
"$BIN" compare mv_l mv_r --detect-moves false > mv_off.txt
if grep -q '\[M\]' mv_off.txt; then
  fail "MOVE4 关闭检测后无 [M]"
else
  pass "MOVE4 关闭检测后无 [M]"
fi
# MOVE5 summary 统计移动对数
"$BIN" compare mv_l mv_r --summary > mv_sum.txt
grep -q '移动' mv_sum.txt && pass "MOVE5 summary 含移动统计" || fail "MOVE5 summary 含移动统计"

echo "=============================================="
echo " HTML: 对比报告导出"
echo "=============================================="
mkdir -p html_d1 html_d2
printf 'x' > html_d1/same.txt; printf 'x' > html_d2/same.txt
printf 'v1' > html_d1/diff.txt; printf 'v22' > html_d2/diff.txt
printf 'a' > html_d1/only_l.txt
# HTML1 导出文件生成
"$BIN" compare html_d1 html_d2 --html html_report.html > /dev/null 2>&1; check "HTML1 导出退出码=1(有差异)" 1 "$?"
[ -f html_report.html ] && pass "HTML2 报告文件已生成" || fail "HTML2 报告文件已生成"
# HTML3 报告含差异条目（HTML 中 [L] 与路径间有标签，分开断言）
if grep -q '\[L\]' html_report.html && grep -q 'only_l.txt' html_report.html; then
  pass "HTML3 报告含 [L] 条目"
else
  fail "HTML3 报告含 [L] 条目"
fi
if grep -q '\[C\]' html_report.html && grep -q 'diff.txt' html_report.html; then
  pass "HTML4 报告含 [C] 条目"
else
  fail "HTML4 报告含 [C] 条目"
fi
# HTML5 报告含统计
check_contains "HTML5 报告含统计" "仅左侧" "$(cat html_report.html)"
# HTML6 报告含对比路径
check_contains "HTML6 报告含路径" "html_d1 ↔ html_d2" "$(cat html_report.html)"

# 移动检测在 HTML 报告中体现
mkdir -p html_mv_l html_mv_r
printf 'moved-content-xyz' > html_mv_l/old.txt
printf 'moved-content-xyz' > html_mv_r/new.txt
"$BIN" compare html_mv_l html_mv_r --html html_mv.html > /dev/null 2>&1
check_contains "HTML7 报告含移动标记" "old.txt → new.txt" "$(cat html_mv.html)"

echo "=============================================="
echo " SESSION: 会话保存/恢复"
echo "=============================================="
# 用临时 HOME 隔离会话文件
export HOME="$WORK/session-home"
mkdir -p "$HOME"
mkdir -p ses_d1 ses_d2
printf 'a' > ses_d1/only.txt
printf 'b' > ses_d2/only2.txt
# SESSION1 保存
"$BIN" session save ses_cmp ses_d1 ses_d2 --compare-content > /dev/null 2>&1; check "SESSION1 保存退出码=0" 0 "$?"
# SESSION2 列出包含会话名
"$BIN" session list > ses_list.txt
check_contains "SESSION2 列表含会话名" "ses_cmp" "$(cat ses_list.txt)"
# SESSION3 运行会话（有差异 rc=1）
"$BIN" session run ses_cmp > /dev/null 2>&1; check "SESSION3 运行会话退出码=1" 1 "$?"
# SESSION4 重复保存报错 rc=2
"$BIN" session save ses_cmp ses_d1 ses_d2 > /dev/null 2>&1; check "SESSION4 重复保存退出码=2" 2 "$?"
# SESSION5 运行不存在会话 rc=2
"$BIN" session run not_exist > /dev/null 2>&1; check "SESSION5 不存在会话退出码=2" 2 "$?"
# SESSION6 删除
"$BIN" session delete ses_cmp > /dev/null 2>&1; check "SESSION6 删除退出码=0" 0 "$?"
"$BIN" session run ses_cmp > /dev/null 2>&1; check "SESSION7 删除后运行退出码=2" 2 "$?"

echo "=============================================="
echo " SYNC2: 移动同步 + 空目录清理"
echo "=============================================="
mkdir -p sy2_src sy2_dst
# SYNC2.1 mirror 识别重命名（内容相同）
printf 'same-content-xyz' > sy2_src/new.txt
printf 'same-content-xyz' > sy2_dst/old.txt
"$BIN" sync sy2_src sy2_dst --mode mirror --dry-run > sy2_plan.txt
check_contains "SYNC2.1 mirror 识别重命名" "[MOVE] old.txt -> new.txt" "$(cat sy2_plan.txt)"
# 实际执行后新路径存在、旧路径消失
"$BIN" sync sy2_src sy2_dst --mode mirror > /dev/null 2>&1
if [ -f sy2_dst/new.txt ] && [ ! -f sy2_dst/old.txt ]; then
  pass "SYNC2.2 重命名执行正确"
else
  fail "SYNC2.2 重命名执行正确"
fi
# SYNC2.3 mirror 清理独有空目录
mkdir -p sy2_dst/orphan/deep
printf 'x' > sy2_src/keep.txt
printf 'x' > sy2_dst/keep.txt
"$BIN" sync sy2_src sy2_dst --mode mirror --summary > sy2_sum.txt
if [ ! -d sy2_dst/orphan ]; then
  pass "SYNC2.3 空目录已清理"
else
  fail "SYNC2.3 空目录已清理"
fi
check_contains "SYNC2.4 summary 含目录清理统计" "清理空目录" "$(cat sy2_sum.txt)"
# SYNC2.5 内容不同不误判为移动
mkdir -p sy2b_src sy2b_dst
printf 'content-aaaa' > sy2b_src/a.txt
printf 'content-bbbb' > sy2b_dst/b.txt
"$BIN" sync sy2b_src sy2b_dst --mode mirror --dry-run > sy2b_plan.txt
if grep -q '\[MOVE\]' sy2b_plan.txt; then
  fail "SYNC2.5 内容不同不误判移动"
else
  pass "SYNC2.5 内容不同不误判移动"
fi

echo "=============================================="
echo " C3: 三路文件夹对比"
echo "=============================================="
mkdir -p c3_b c3_l c3_r
# 场景：a.txt 左侧修改；s.txt 三处相同；gone.txt 仅BASE；only_l 仅LEFT；only_r 仅RIGHT；
# 冲突文件：三处内容都不同
printf 'v1' > c3_b/a.txt; printf 'v2' > c3_l/a.txt; printf 'v1' > c3_r/a.txt
printf 'same' > c3_b/s.txt; printf 'same' > c3_l/s.txt; printf 'same' > c3_r/s.txt
printf 'x' > c3_b/gone.txt
printf 'new' > c3_l/only_l.txt
printf 'other' > c3_r/only_r.txt
printf 'b1' > c3_b/conf.txt; printf 'l1' > c3_l/conf.txt; printf 'r1' > c3_r/conf.txt
# C3.1 左侧修改标记
"$BIN" compare3 c3_b c3_l c3_r --show-same > c3_out.txt; check "C3.1 退出码=1(有差异)" 1 "$?"
check_contains "C3.2 左侧修改标记" "[LM] a.txt" "$(cat c3_out.txt)"
check_contains "C3.3 三处相同标记" "[S] s.txt" "$(cat c3_out.txt)"
check_contains "C3.4 仅BASE标记" "[B] gone.txt" "$(cat c3_out.txt)"
check_contains "C3.5 仅LEFT标记" "[L] only_l.txt" "$(cat c3_out.txt)"
check_contains "C3.6 仅RIGHT标记" "[R] only_r.txt" "$(cat c3_out.txt)"
check_contains "C3.7 冲突标记" "[C] conf.txt" "$(cat c3_out.txt)"
# C3.8 三路完全相同 → 退出码 0
"$BIN" compare3 c3_b c3_b c3_b > /dev/null 2>&1; check "C3.8 三路相同退出码=0" 0 "$?"
# C3.9 summary 统计
"$BIN" compare3 c3_b c3_l c3_r --summary > c3_sum.txt
check_contains "C3.9 summary 统计" "冲突" "$(cat c3_sum.txt)"
# C3.10 缺失目录退出码=2
"$BIN" compare3 /nonexistent-c3-a /nonexistent-c3-b c3_r > /dev/null 2>&1; check "C3.10 缺失目录退出码=2" 2 "$?"

echo "=============================================="
echo " CSV: 表格对比"
echo "=============================================="
printf 'id,name,age\n1,alice,30\n2,bob,25\n3,carol,40\n' > csv_a.csv
printf 'id,name,age\n1,alice,31\n2,bob,25\n4,dave,50\n' > csv_b.csv
# CSV1 无 key 行号对齐：修改退出码=1
"$BIN" csv csv_a.csv csv_b.csv > csv_out.txt; check "CSV1 行号对齐退出码=1" 1 "$?"
check_contains "CSV2 修改行标记" "[M]" "$(cat csv_out.txt)"
# CSV3 按主键对齐
"$BIN" csv csv_a.csv csv_b.csv --key id > csv_key.txt; check "CSV3 主键对齐退出码=1" 1 "$?"
check_contains "CSV4 孤儿行标记" "[L] 行3  id=3" "$(cat csv_key.txt)"
check_contains "CSV5 右侧孤儿行" "[R] 行3  id=4" "$(cat csv_key.txt)"
# CSV6 相同文件退出码=0
"$BIN" csv csv_a.csv csv_a.csv > /dev/null 2>&1; check "CSV6 相同退出码=0" 0 "$?"
# CSV7 引号字段解析（含逗号）
printf 'id,note\n1,"a,b"\n' > csv_q1.csv
printf 'id,note\n1,"a,c"\n' > csv_q2.csv
"$BIN" csv csv_q1.csv csv_q2.csv > csv_q.txt; check "CSV7 引号字段退出码=1" 1 "$?"
check_contains "CSV8 引号字段差异" "a,b -> a,c" "$(cat csv_q.txt)"
# CSV9 summary 统计
"$BIN" csv csv_a.csv csv_b.csv --key id --summary > csv_sum.txt
check_contains "CSV9 summary 统计" "CSV 统计" "$(cat csv_sum.txt)"
# CSV10 制表符分隔
printf 'a\tb\n1\t2\n' > csv_t1.csv
printf 'a\tb\n1\t3\n' > csv_t2.csv
"$BIN" csv csv_t1.csv csv_t2.csv --delimiter '\t' > /dev/null 2>&1; check "CSV10 制表符分隔退出码=1" 1 "$?"



echo "=============================================="
echo " I18N: 多语言"
echo "=============================================="
# --lang en：错误消息与统计行应为英文
"$BIN" --lang en diff a_missing.txt b_missing.txt 2>&1 | head -1 > i18n_err.txt
check_contains "I18N.1 --lang en 错误消息" "cannot read" "$(cat i18n_err.txt)"

"$BIN" --lang de compare /nonexistent-a /nonexistent-b 2>&1 | head -1 > i18n_de.txt
check_contains "I18N.2 --lang de 错误消息" "kein Verzeichnis" "$(cat i18n_de.txt)"

"$BIN" --lang ja compare /nonexistent-a /nonexistent-b 2>&1 | head -1 > i18n_ja.txt
check_contains "I18N.3 --lang ja 错误消息" "ディレクトリではありません" "$(cat i18n_ja.txt)"

# 环境变量 BCR_LANG 生效
BCR_LANG=fr "$BIN" compare /nonexistent-a /nonexistent-b 2>&1 | head -1 > i18n_fr.txt
check_contains "I18N.4 BCR_LANG=fr 错误消息" "pas un répertoire" "$(cat i18n_fr.txt)"

# 默认中文（无 --lang 时）
"$BIN" compare /nonexistent-a /nonexistent-b 2>&1 | head -1 > i18n_zh.txt
check_contains "I18N.5 默认中文" "不是目录" "$(cat i18n_zh.txt)"

# 非法语言代码回退中文
"$BIN" --lang xx compare /nonexistent-a /nonexistent-b 2>&1 | head -1 > i18n_fb.txt
check_contains "I18N.6 非法语言回退中文" "不是目录" "$(cat i18n_fb.txt)"

# --lang es 统计行（compare --summary）
mkdir -p i18n_d1 i18n_d2
printf 'a' > i18n_d1/only.txt
printf 'b' > i18n_d2/only2.txt
"$BIN" --lang es compare i18n_d1 i18n_d2 --summary 2>&1 | tail -1 > i18n_sum.txt
check_contains "I18N.7 --lang es 统计行" "resumen" "$(cat i18n_sum.txt)"

# --lang ru sync dry-run 输出
"$BIN" --lang ru sync i18n_d1 i18n_d2 --dry-run 2>&1 | head -1 > i18n_sync.txt
check_contains "I18N.8 --lang ru sync 输出" "[COPY]" "$(cat i18n_sync.txt)"

echo
echo "=============================================="
echo " 验收结果: $PASS 通过 / $FAIL 失败"
echo "=============================================="
if [ "$FAIL" -gt 0 ]; then
  echo "失败用例:"
  for t in "${FAILED[@]}"; do echo "  - $t"; done
  exit 1
fi
exit 0
