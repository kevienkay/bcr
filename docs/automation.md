# bcr 自动化指南

> P27 自动化方案 v3（已确认路线）：**不发明脚本语言**。
> bcr 输出稳定的版本化 JSON 契约，自动化逻辑用 Python 编写，
> 通过官方薄封装 `bcr.py`（subprocess + JSON）调用；
> 不想写代码的简单场景用纯数据任务清单 `bcr task`。

配套设计文档：`docs/P27-python-binding-design.md`（方案 v3）、`docs/P27-script-engine-design.md`（早期方案，已弃用）。

```
┌──────────────┐   subprocess    ┌──────────────────────┐   JSON 契约   ┌────────────┐
│  Python 脚本  │ ──────────────► │  bcr CLI(现有)        │ ────────────► │  用户逻辑   │
│  (用户编写)    │   bcr.py 封装   │  compare/sync/report  │   stdout 输出  │  if/for/   │
└──────────────┘                └──────────────────────┘               │  异常处理   │
                                                                        └────────────┘
```

## 1. 快速上手

### 1.1 安装 bcr 二进制

从 GitHub Releases 下载对应平台二进制，或 `cargo install --path .`。
确保 `bcr` 在 PATH 中；不在 PATH 时用环境变量指定：

```bash
export BCR_BIN=/path/to/bcr
```

### 1.2 使用 bcr.py

```bash
# 直接使用仓库内绑定
python3 bindings/bcr.py /data/in /data/out

# 或安装为 Python 包（纯标准库，零第三方依赖）
pip install ./bindings
```

```python
import bcr

r = bcr.compare("/data/in", "/data/out", content=True)
print(r.stats)               # Stats(same=120, left_only=3, right_only=2, differ=5, moved=1)
print(r.has_differences)     # True
for e in r.differences:      # 仅差异条目
    print(f"[{e.status}] {e.rel}")
```

### 1.3 使用任务清单（无代码场景）

`tasks/nightly.json`：

```json
{
  "name": "夜间对比",
  "steps": [
    { "cmd": "load", "left": "/data", "right": "/backup/%date%" },
    { "cmd": "compare", "left": "/data", "right": "/backup/%date%", "content": true },
    { "cmd": "report", "format": "txt", "output": "report-%date%.txt" },
    { "cmd": "echo", "text": "done" }
  ]
}
```

```bash
bcr task check tasks/nightly.json   # 只校验，不执行
bcr task run tasks/nightly.json     # 执行（遇错即停）
bcr task run tasks/nightly.json --dry-run   # 只打印步骤
```

## 2. JSON 契约

所有支持的命令加 `--json` 后，stdout **只输出一个 JSON 对象**（人类可读错误走 stderr），
schema 版本化，外部脚本不因 bcr 升级而崩。退出码保持 CLI 语义：`0` 无差异、`1` 有差异、`2` 错误。

### 2.1 通用信封（所有命令）

```json
{
  "schema": "compare.v1",
  "ok": true,
  "command": "compare",
  "args": { "left": "/a", "right": "/b" },
  "result": { "...": "命令专属字段" },
  "warnings": [],
  "error": null
}
```

- `schema`：`compare.v1` / `sync.v1` / `compare3.v1` / `csv.v1` / `merge.v1` / `mp3tag.v1` / `imgcmp.v1`
- `ok=false` 时 `error` 有值、`result` 为 null、退出码为 2
- 新版本加字段不破坏旧脚本（bcr.py 对缺失字段容错）

### 2.2 compare（目录/文件对比）→ `compare.v1`

```json
{
  "schema": "compare.v1",
  "ok": true,
  "result": {
    "stats": { "same": 120, "left_only": 3, "right_only": 2, "differ": 5, "moved": 1 },
    "has_differences": true,
    "entries": [
      {
        "rel": "src/main.rs",
        "status": "differ",
        "left":  { "size": 1024, "mtime": "2026-08-11T10:00:00Z", "mode": 420, "symlink": null },
        "right": { "size": 1100, "mtime": "2026-08-11T11:30:00Z", "mode": 420, "symlink": null },
        "moved_to": null,
        "attrs_differ": false
      }
    ]
  }
}
```

- `status`：`same` | `left_only` | `right_only` | `differ` | `moved`
- `mtime` 统一 ISO-8601 UTC 字符串（`datetime.fromisoformat` 可直接解析）
- `mode` 为十进制权限位（Unix）或 null；`symlink` 为链接目标或 null
- `moved` 条目 `moved_to` 给出新路径；`attrs_differ` 表示属性差异（需 `--compare-attrs`）

### 2.3 sync（目录同步）→ `sync.v1`

```json
{
  "schema": "sync.v1",
  "ok": true,
  "result": {
    "dry_run": true,
    "mode": "mirror",
    "plan": [
      { "op": "copy",   "rel": "src/a.rs", "from": "left" },
      { "op": "delete", "rel": "tmp/old.log" },
      { "op": "rename", "rel": "old.txt",  "to": "new.txt" },
      { "op": "rmdir",  "rel": "empty/" },
      { "op": "skip",   "rel": "same.txt", "reason": "identical" },
      { "op": "conflict", "rel": "x.txt",  "reason": "both changed" }
    ],
    "stats": { "copy": 1, "delete": 1, "rename": 0, "rmdir": 0, "skip": 1, "conflict": 0, "errors": 0 }
  }
}
```

- `--dry-run`：`plan` 完整列出、`stats` 为零、不执行
- 非 dry-run：`plan` 为实际执行列表，`stats` 为结果（`errors` 计入失败项）

### 2.4 compare3（三路目录对比）→ `compare3.v1`

```json
{
  "schema": "compare3.v1",
  "ok": true,
  "result": {
    "stats": { "same": 1, "base_only": 0, "left_only": 0, "right_only": 1,
               "left_deleted": 0, "right_deleted": 1, "left_modified": 0,
               "right_modified": 1, "both_modified": 0, "conflict": 0 },
    "has_differences": true,
    "entries": [ { "rel": "f2.txt", "status": "RM" } ]
  }
}
```

- `status` 为三路标记（如 `RM`=右侧修改、`RD`=右侧删除、`L`/`R`/`B` 单侧存在、`C` 冲突等）

### 2.5 csv（表格对比）→ `csv.v1`

```json
{
  "schema": "csv.v1",
  "ok": true,
  "result": {
    "stats": { "same": 1, "left_only": 0, "right_only": 0, "modified": 1 },
    "has_differences": true
  }
}
```

### 2.6 merge（三路合并）→ `merge.v1`

```json
{
  "schema": "merge.v1",
  "ok": true,
  "result": { "conflicts": 2, "has_conflicts": true, "output": "merged.txt" }
}
```

- JSON 模式不输出合并内容本身，只给冲突统计与输出路径（`-o` 缺省为 null）

### 2.7 mp3tag（MP3 标签对比）→ `mp3tag.v1`

```json
{
  "schema": "mp3tag.v1",
  "ok": true,
  "result": {
    "fields": [
      { "name": "title", "left": "A", "right": "B", "diff": true }
    ],
    "has_differences": true
  }
}
```

### 2.8 imgcmp（图片对比）→ `imgcmp.v1`

```json
{
  "schema": "imgcmp.v1",
  "ok": true,
  "result": {
    "left_size": [800, 600], "right_size": [800, 600],
    "size_differs": false,
    "diff_pixels": 123, "total_pixels": 480000, "diff_ratio": 0.0003,
    "bounds": [10, 20, 300, 200],
    "has_differences": true
  }
}
```

### 2.9 错误示例

```json
{
  "schema": "compare.v1",
  "ok": false,
  "command": "compare",
  "args": { "left": "/missing", "right": "/b" },
  "result": null,
  "warnings": [],
  "error": "读取 /missing 失败: No such file or directory (os error 2)"
}
```

## 3. bcr.py Python 绑定

单文件 `bindings/bcr.py`，仅标准库（`subprocess`/`json`/`dataclasses`），Python ≥ 3.9，零第三方依赖。
`BCR_BIN` 环境变量覆盖可执行文件路径（默认 `bcr`）。

### 3.1 目录/文件对比

```python
import bcr

r = bcr.compare(
    "/data/in", "/data/out",
    content=True,                      # 内容哈希兜底（默认快速模式 mtime+size）
    includes=["*.rs"],                 # glob 包含
    excludes=["*.log", "target/"],     # glob 排除
    detect_moves=True,
    compare_attrs=True,                # 权限位/符号链接
    compare_version=True,              # 版本号比较（P25）
    profile="nightly",                 # 比较规则 Profile
)
r.stats            # Stats(same=..., left_only=..., right_only=..., differ=..., moved=...)
r.has_differences  # bool
r.entries          # [Entry(rel, status, left: Meta|None, right: Meta|None, moved_to, attrs_differ)]
r.differences      # 便捷过滤：仅非 same 条目
```

### 3.2 三路对比

```python
r3 = bcr.compare3("/base", "/left", "/right", content=True)
r3.stats           # TriStats(base_only=..., left_only=..., conflict=..., ...)
r3.entries         # [TriEntry(rel, status)]
```

### 3.3 同步

```python
plan = bcr.sync("/a", "/b", mode="mirror", dry_run=True)   # 只预览，不执行
for op in plan.plan:
    print(op.op, op.rel)          # copy/delete/rename/rmdir/skip/conflict

result = bcr.sync("/a", "/b", mode="mirror", content=True)  # 执行
result.stats       # SyncStats(copy=..., delete=..., rename=..., rmdir=..., skip=..., conflict=..., errors=...)
```

### 3.4 报告与特殊格式

```python
r = bcr.compare("/a", "/b")

# 报告：txt/csv/html 写文件
bcr.run(["report", "/a", "/b", "--txt", "r.txt", "--fields", "status,path"])

# MP3 标签
m = bcr.mp3tag("/a.mp3", "/b.mp3")
for f in m.fields:
    if f.diff:
        print(f"{f.name}: {f.left!r} -> {f.right!r}")

# 图片差异
i = bcr.imgcmp("/a.png", "/b.png")
print(i.diff_pixels, i.diff_ratio, i.bounds)

# CSV
c = bcr.csv("/a.csv", "/b.csv", key="id")

# 三路合并（-o 输出文件）
m = bcr.merge("/base.txt", "/left.txt", "/right.txt", output="merged.txt")
print(m.conflicts)
```

### 3.5 底层透传

```python
data = bcr.run(["compare", "/a", "/b", "--json"])   # 返回解析后的完整 dict（含 result/error）
```

### 3.6 返回类型

```python
Meta(size, mtime: datetime|None, mode: int|None, symlink: str|None)
Entry(rel, status, left, right, moved_to, attrs_differ)
Stats(same, left_only, right_only, differ, moved)
CompareResult(stats, has_differences, entries, warnings, raw)
SyncPlanItem(op, rel, to, from_, reason, size)
SyncResult(dry_run, mode, plan, stats, raw)
TriStats(base_only, left_only, right_only, left_deleted, right_deleted,
         left_modified, right_modified, both_modified, conflict, same)
MergeResult(conflicts, has_conflicts, output, raw)
Mp3Field(name, left, right, diff) / Mp3Result(fields, has_differences, raw)
ImgResult(left_size, right_size, size_differs, diff_pixels, total_pixels,
          diff_ratio, bounds, has_differences, raw)
```

- 时间字段自动转 `datetime`；`raw` 保留原始 dict 供高级用户
- 字段缺失容错：JSON 缺字段给默认值，新版本加字段不破坏旧脚本
- 失败抛 `bcr.Error`（含 stderr 文本）

## 4. 任务清单（bcr task）

纯数据清单（JSON 或 TOML），适合**不想写 Python 的简单场景**。无表达式/循环/分支，
动态变量仅字符串替换。复杂逻辑请用 bcr.py。

### 4.1 命令集

| 命令 | 必需参数 | 可选参数 |
|---|---|---|
| `load` | `left` 或 `session` | `right` |
| `compare` | `left`, `right` | `content`, `includes`, `excludes`, `show_same`, `no_moves`, `attrs`, `version`, `profile` |
| `compare3` | `base`, `left`, `right` | `content`, `includes`, `excludes`, `show_same` |
| `csv` | `left`, `right` | `key`, `delimiter`, `no_header`, `show_same` |
| `merge` | `base`, `left`, `right` | `output`, `algo`, `labels` |
| `sync` | `left`, `right` | `mode`, `reverse`, `content`, `includes`, `excludes` |
| `report` | `format`(txt/csv/html), `output` | `fields`, `title`, `no_stats`, `sort`, `group`（作用于最近一次 compare） |
| `echo` | — | `text` |
| `exit` | — | `code` |

### 4.2 动态变量

| 变量 | 含义 |
|---|---|
| `%date%` | 日期 `yyyy-mm-dd` |
| `%time%` | 时间 `HH:MM:SS` |
| `%fn_time%` | 文件名安全时间 `HH-MM-SS` |
| `%env:VAR%` | 环境变量 |
| `%1`–`%9` | 命令行位置参数（`bcr task run file.json -- a b c`） |

### 4.3 执行语义

- 顺序执行；compare/sync 返回 1（有差异）**不中止**脚本，只有错误（2）才中止
- `continue_on_error: true`（顶层）时错误也不中止，最终退出码取最严重者：错误(2) > 有差异(1) > 成功(0)
- `silent: true`（顶层或 `--silent`）抑制步骤输出
- `report` 使用最近一次 `compare` 的结果，因此必须先有 compare 步骤

### 4.4 TOML 示例

```toml
name = "nightly"
silent = true

[[steps]]
cmd = "compare"
left = "/data"
right = "/backup/%date%"
content = true
excludes = ["*.tmp"]

[[steps]]
cmd = "report"
format = "txt"
output = "report-%date%.txt"
```

### 4.5 校验

```bash
bcr task check tasks/nightly.toml   # schema/命令/必需参数校验，不执行
```

## 5. 场景示例

### 5.1 CI 差异门禁

```python
import bcr, sys

r = bcr.compare("/build/out", "/expected", content=True)
if r.has_differences:
    bcr.run(["report", "/build/out", "/expected", "--txt", "diff-report.txt"])
    print(f"构建产物与预期不一致: {r.stats.differ} 个文件不同")
    sys.exit(1)
```

### 5.2 夜间备份 + 清理（保留最近 7 天）

```python
import bcr
from datetime import date, timedelta

bcr.run(["session", "save", "backup", "/data", f"/backup/{date.today()}"])
r = bcr.sync(left="/data", right=f"/backup/{date.today()}",
             mode="mirror", content=True)
print(f"复制 {r.stats.copy}, 删除 {r.stats.delete}")

for i in range(8, 30):
    old = f"/backup/{date.today() - timedelta(days=i)}"
    bcr.run(["sync", old, "/backup/empty", "--mode", "mirror"])  # 或直接清理
```

### 5.3 多目录批量对比 + 聚合

```python
import bcr, glob

bad = []
for src in glob.glob("/projects/*"):
    name = src.rsplit("/", 1)[-1]
    r = bcr.compare(src, f"/expected/{name}", content=True)
    if r.has_differences:
        bad.append((src, r.stats.differ))
print("异常项目:", bad)
```

### 5.4 与调度器/通知集成

```python
import bcr

r = bcr.compare("/prod/config", "/staging/config")
if r.has_differences:
    # 发送邮件/钉钉/写数据库——Python 生态任意能力
    notify(r.differences)
```

## 6. 分发

- `bindings/bcr.py` 单文件随仓库提供，复制即用；`BCR_BIN` 指向 bcr 二进制
- 可选 `pip install ./bindings`（pyproject.toml 打包，纯 Python，无依赖）
- Windows 下 `subprocess` 同样工作（CLI 三平台一致）

## 7. 保留的 BC 优点对照

| BC 优点 | Python 方案对应 |
|---|---|
| 无人值守 | `--silent` + 退出码；Python 脚本可被 cron/CI 调度 |
| 报告生成 | `report` 命令 + bcr.py |
| 会话复用 | `session save/run` + bcr.py |
| 动态变量 | Python 侧 `datetime`/`os.environ`（更强大） |
| 跨平台 | CLI 三平台一致 + Python 跨平台 |
