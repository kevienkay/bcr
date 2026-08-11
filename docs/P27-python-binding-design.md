# P27 自动化方案 v3 详细设计 — Python 绑定(bcr.py)

> 已确认路线:不发明脚本语言;bcr 输出稳定 JSON 契约,
> 自动化用 Python 编写,通过官方薄封装 `bcr.py`(subprocess + JSON)调用。

## 1. 架构总览

```
┌──────────────┐   subprocess    ┌──────────────────────┐   JSON 契约   ┌────────────┐
│  Python 脚本  │ ──────────────► │  bcr CLI(现有)        │ ────────────► │  用户逻辑   │
│  (用户编写)    │   bcr.py 封装   │  compare/sync/report  │   stdout 输出  │  if/for/   │
└──────────────┘                └──────────────────────┘               │  异常处理   │
                                                                        └────────────┘
```

- **bcr.py 是唯一的封装层**:拼 CLI 参数 → 跑子进程 → 解析 JSON → 返回类型化对象
- **零新语法**:用户写标准 Python,能力无上限
- **契约稳定**:JSON schema 版本化,外部脚本不因 bcr 升级而崩

## 2. JSON 契约(schema 定义)

### 2.1 通用信封(所有命令)

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

- `schema`:版本标识(`compare.v1` / `sync.v1` / `report.v1` / `mp3tag.v1` / `imgcmp.v1` / `csv.v1` / `compare3.v1` / `merge.v1`)
- `ok`:false 时 `error` 有值、退出码为 2
- 错误时 `result` 为 null;stdout 只输出此 JSON,人类可读错误走 stderr

### 2.2 compare --json(核心)

```json
{
  "schema": "compare.v1",
  "ok": true,
  "stats": { "same": 120, "left_only": 3, "right_only": 2, "differ": 5, "moved": 1 },
  "has_differences": true,
  "entries": [
    {
      "rel": "src/main.rs",
      "status": "differ",                    // same|left_only|right_only|differ|moved
      "left":  { "size": 1024, "mtime": "2026-08-11T10:00:00Z",
                 "mode": 420, "symlink": null },
      "right": { "size": 1100, "mtime": "2026-08-11T11:30:00Z",
                 "mode": 420, "symlink": null },
      "moved_to": null,
      "attrs_differ": false
    }
  ],
  "warnings": []
}
```

- `mtime` 统一 ISO-8601 UTC(字符串,Python `datetime.fromisoformat` 直接解析)
- `mode` 为十进制权限位(Unix)或 null
- `entries` 按路径排序,与 CLI 文本输出一致

### 2.3 sync --json(计划 + 执行)

```json
{
  "schema": "sync.v1",
  "ok": true,
  "dry_run": false,
  "mode": "mirror",
  "plan": [
    { "op": "copy",   "rel": "src/a.rs", "from": "left",  "size": 2048 },
    { "op": "delete", "rel": "tmp/old.log" },
    { "op": "rename", "rel": "old.txt",  "to": "new.txt" },
    { "op": "rmdir",  "rel": "empty/" },
    { "op": "skip",   "rel": "same.txt", "reason": "identical" },
    { "op": "conflict", "rel": "x.txt",  "reason": "both changed" }
  ],
  "stats": { "copy": 1, "delete": 1, "rename": 0, "rmdir": 0,
             "skip": 1, "conflict": 0, "errors": 0 }
}
```

- `--dry-run` 时 `plan` 完整、`stats` 为空(或预计值)、不执行
- 非 dry-run 时 `plan` 为实际执行列表,`stats` 为结果
- `errors` 计入失败的执行项

### 2.4 report --json

```json
{
  "schema": "report.v1",
  "ok": true,
  "format": "json",
  "title": null,
  "rows": [
    { "status": "differ", "path": "src/a.rs",
      "size_left": 1024, "size_right": 1100,
      "mtime_left": "...", "mtime_right": "...", "moved_to": null }
  ],
  "stats": { "same": 120, "left_only": 3, "right_only": 2, "differ": 5, "moved": 1 }
}
```

### 2.5 其他命令

- `compare3 --json`:与 compare 同构,status 含三路标记
- `csv --json`:列级差异,`{ "key": "...", "columns": [{ "name": "价格", "left": "10", "right": "12", "changed": true }] }`
- `mp3tag --json`:`{ "fields": [{ "name": "title", "left": "A", "right": "B", "diff": true }], "has_differences": true }`
- `imgcmp --json`:`{ "size_differs": false, "diff_pixels": 123, "diff_ratio": 0.05, "bounds": [x,y,w,h] }`
- `merge --json`:`{ "conflicts": 2, "merged": true, "output": "..." }`

## 3. bcr.py 封装设计

### 3.1 定位

- 单文件 `bindings/bcr.py`,标准库实现(仅 `subprocess`/`json`/`dataclasses`),**零第三方依赖**
- 分发:随仓库提供 + 文档复制即可用;可选 `pip install bcr` 打包(maturin 不需要——纯 Python 包即可)
- Python 版本:≥ 3.9(dataclasses)

### 3.2 API 设计

```python
import bcr

# ── 目录/文件对比 ──
r = bcr.compare(left="/data/in", right="/data/out", content=True,
                includes=["*.rs"], excludes=["*.log", "target/"],
                detect_moves=True)
r.stats            # CompareStats(same=120, left_only=3, right_only=2, differ=5, moved=1)
r.has_differences  # True
r.entries          # [Entry(rel="src/main.rs", status="differ", left=Meta(...), ...)]
r.differences      # 便捷过滤: [e for e in r.entries if e.status != "same"]

# ── 三路对比 ──
r3 = bcr.compare3(base="/base", left="/left", right="/right")

# ── 同步 ──
plan = bcr.sync(left="/a", right="/b", mode="mirror", dry_run=True)  # 只预览
result = bcr.sync(left="/a", right="/b", mode="mirror")              # 执行
result.stats       # SyncStats(copy=1, delete=1, ...)

# ── 报告 ──
bcr.report(format="html", output="diff.html", fields=["status", "path"])
data = bcr.report(format="json")   # 直接返回结构化行

# ── 特殊格式 ──
m = bcr.mp3tag("/a.mp3", "/b.mp3")        # Mp3Compare(fields=[...])
i = bcr.imgcmp("/a.png", "/b.png")        # ImgCompare(diff_pixels=..., ratio=...)
c = bcr.csv("/a.csv", "/b.csv")           # CsvCompare(rows=...)

# ── 会话/规则 ──
bcr.session_save(name="nightly", left="/a", right="/b")
r = bcr.compare(session="nightly")

# ── 底层:任意命令透传 ──
out = bcr.run(["compare", "/a", "/b", "--json"])   # 返回解析后的 dict
```

### 3.3 返回类型(dataclass)

```python
@dataclass
class Stats: same: int; left_only: int; right_only: int; differ: int; moved: int
@dataclass
class Meta: size: int; mtime: datetime; mode: int | None; symlink: str | None
@dataclass
class Entry: rel: str; status: str; left: Meta | None; right: Meta | None
              moved_to: str | None; attrs_differ: bool
@dataclass
class CompareResult: stats: Stats; has_differences: bool; entries: list[Entry]
                     warnings: list[str]; raw: dict
# SyncResult / ReportResult / Mp3Compare / ImgCompare / CsvCompare 同理
```

- 字段缺失容错:JSON 缺字段时给默认值,新版本加字段不破坏旧脚本
- 时间字段自动转 `datetime`;`raw` 保留原始 dict 供高级用户
- 异常:`bcr.Error`(退出码 2,含 stderr 文本)

### 3.4 关键实现细节

```python
def _run(cmd: list[str]) -> dict:
    p = subprocess.run([BCR_BIN, *cmd, "--json"],
                       capture_output=True, text=True, encoding="utf-8")
    if p.returncode == 2 or not p.stdout:
        raise Error(p.stderr.strip() or f"bcr failed: {p.returncode}")
    data = json.loads(p.stdout)
    return data
```

- `BCR_BIN` 环境变量可覆盖(默认 `bcr`,支持 `BCR_BIN=/path/to/bcr`)
- Windows 下同 `subprocess` 工作(CLI 已三平台一致)
- 大输出:`entries` 可能很大 → 提供 `bcr.compare(..., stream=True)` 流式读取 JSON lines?Phase 4 评估(第一版直接全量返回,compare 结果万级条目 JSON 几十 MB 可接受)

## 4. 使用场景示例

### 4.1 CI 差异门禁

```python
import bcr, sys

r = bcr.compare("/build/out", "/expected", content=True)
if r.has_differences:
    bcr.report(format="txt", output="diff-report.txt")
    print(f"构建产物与预期不一致: {r.stats.differ} 个文件不同")
    sys.exit(1)   # 阻断 CI
```

### 4.2 夜间备份 + 清理

```python
import bcr
from datetime import date

bcr.session_save("backup", "/data", f"/backup/{date.today()}")
r = bcr.sync(left="/data", right=f"/backup/{date.today()}",
             mode="mirror", compare_content=True)
print(f"复制 {r.stats.copy},删除 {r.stats.delete}")
# 保留最近 7 天备份
```

### 4.3 多目录批量对比 + 聚合

```python
import bcr, glob

bad = []
for src in glob.glob("/projects/*"):
    r = bcr.compare(src, f"/expected/{src.rsplit('/',1)[-1]}", content=True)
    if r.has_differences:
        bad.append((src, r.stats.differ))
print("异常项目:", bad)
```

### 4.4 与调度器/通知集成

```python
import bcr, smtplib

r = bcr.compare("/prod/config", "/staging/config")
if r.has_differences:
    # 发送邮件/钉钉/写入数据库——Python 生态任意能力
    notify(r.differences)
```

## 5. 保留的 BC 优点对照

| BC 优点 | Python 方案对应 |
|---|---|
| 无人值守 | `--silent` + 退出码;Python 脚本可被 cron/CI 调度 |
| 报告生成 | `bcr.report(format=...)` |
| 会话复用 | `bcr.session_save / compare(session=...)` |
| 动态变量 | Python 侧 `datetime`/`os.environ`(更强大) |
| 跨平台 | CLI 三平台一致 + Python 跨平台 |

## 6. 测试策略

1. **契约测试(核心)**:
   - 每个 `--json` 输出的 schema 字段/类型/退出码(单元测试 + 验收套件)
   - 错误路径:`ok=false`、`error` 字段、退出码 2
2. **bcr.py 单元测试**(pytest,`tests/py/`):
   - 参数拼装正确性(与 CLI 帮助逐项对照)
   - 返回对象字段解析、缺字段容错、时间转换
   - 错误抛出、`BCR_BIN` 覆盖
3. **端到端**:tempfile 构造目录 → bcr.py 对比/同步 → 断言结果对象
4. **跨平台 CI**:三平台跑同一份 pytest(CLI 已跨平台,Python 脚本天然跨平台)

## 7. 实施计划

### Phase 1 — compare/sync JSON 契约 + bcr.py 骨架
- `jsonout.rs` 统一输出框架(schema 版本化 + 错误信封)
- `compare --json`、`sync --json`(含 dry-run)
- `bindings/bcr.py`:run/compare/sync 三个核心函数 + dataclass
- 契约测试 + bcr.py pytest 骨架

### Phase 2 — 全命令 JSON 契约
- compare3/csv/merge/imgcmp/mp3tag `--json`
- `report --json`
- bcr.py 补全全部函数
- 验收套件 JSON 用例

### Phase 3 — 任务清单(可选增强,纯数据)
- `bcr task run/check`(JSON/TOML 步骤清单,无表达式)
- 定位:给不想写 Python 的简单场景;复杂场景仍走 Python

### Phase 4 — 分发与文档
- `pip install bcr` 打包(PyPI 发布可选)
- `docs/automation.md`:JSON 契约参考 + Python API 文档 + 场景示例
- README 章节 + CHANGELOG

## 8. 风险与对策

| 风险 | 对策 |
|---|---|
| JSON schema 漂移 | schema 版本化 + 契约测试锁定字段 |
| 大目录 JSON 内存 | 第一版全量返回(万级条目可接受);Phase 4 评估流式 |
| Python 版本碎片 | 目标 ≥3.9,只用标准库 |
| 用户想要"无代码" | Phase 3 task 清单兜底;文档引导 |
| bcr 二进制不在 PATH | `BCR_BIN` 环境变量 + 文档说明 |

## 9. 关键决策

1. **Python 是唯一官方绑定语言**(用户确认):薄封装,不引第三方依赖
2. **JSON 是唯一契约**:版本化 schema,外部脚本稳定
3. **bcr.py 只做胶水**:拼参数/跑进程/解析 JSON,逻辑全在用户侧
4. **不引入 PyO3/maturin 原生扩展**:避免编译复杂度;subprocess 性能对自动化场景足够(单次调用毫秒级)
5. **task 清单是可选项**:非核心,避免功能蔓延
