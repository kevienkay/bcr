# P27 自动化方案 v3 — 可编程 CLI 契约(不发明脚本语言)

> 放弃"脚本语言"路线。BC 缺点的根源是专用 DSL 形态本身;
> v3 把 bcr 做成**稳定 JSON 契约的 CLI 工具**(像 git/jq 一样),
> 自动化由用户熟悉的通用语言(Python/Shell/JS)编排,零学习成本、无功能天花板。

## 1. 为什么 v1/v2 都失败

| 方案 | 形态 | 未克服的 BC 缺点 |
|---|---|---|
| v1 仿 .bcscript | 专用 DSL | 语法老派、无编程能力、平台开关差异 |
| v2 自研语法 | 专用 DSL(换皮) | **仍是新语言**:要学、有天花板、调试难、无生态 |

**根因**:只要"脚本语言"存在,用户就要学一门新语法、受限于命令集、无法复用通用语言生态。
**解法**:不发明语言。bcr 输出稳定 JSON,自动化交给通用语言。

## 2. 核心设计:三件套,零新语言

```
┌─────────────────────────────────────────────────────┐
│  ① 结构化输出契约  bcr compare A B --json            │
│     → 稳定 JSON schema(结果/统计/计划,可被任意语言消费) │
│  ② 声明式任务清单    bcr task run <file.json>         │
│     → 纯数据步骤列表,无语言特性(无变量/表达式/循环)      │
│  ③ 通用语言绑定     Python/Shell/JS 示例库            │
│     → 官方提供封装,用户用自己会的语言写自动化           │
└─────────────────────────────────────────────────────┘
```

**关键**:②只是"常用操作的快捷清单"(纯 JSON 数据);真正复杂逻辑走③——用户自己的语言,能力无限。

## 3. ① 结构化输出契约(--json)

### 3.1 原则

- 每个子命令支持 `--json`:输出**唯一 JSON 文档**到 stdout(不再有人读文本)
- 错误:stderr 保持人类可读;JSON 输出包含 `"ok": false` + 错误信息
- 退出码不变(0/1/2),JSON 与退出码一致
- schema 稳定、版本化(`"schema": "compare.v1"`),保证外部脚本不因小改动崩溃

### 3.2 compare --json 输出示例

```json
{
  "schema": "compare.v1",
  "left": "/data/in",
  "right": "/data/out",
  "ok": true,
  "stats": {
    "same": 120, "left_only": 3, "right_only": 2, "differ": 5, "moved": 1
  },
  "has_differences": true,
  "entries": [
    { "rel": "src/main.rs", "status": "differ",
      "left": { "size": 1024, "mtime": "2026-08-11T10:00:00Z" },
      "right": { "size": 1100, "mtime": "2026-08-11T11:30:00Z" },
      "moved_to": null },
    { "rel": "old.txt", "status": "moved", "moved_to": "new.txt",
      "left": { "size": 10, "mtime": "..." }, "right": null }
  ],
  "warnings": []
}
```

### 3.3 sync --json(计划 + 执行)

```
bcr sync /a /b --mode mirror --dry-run --json   # 仅输出计划(不执行)
bcr sync /a /b --mode mirror --json             # 执行并输出结果
```

```json
{
  "schema": "sync.v1",
  "ok": true,
  "plan": [
    { "op": "copy", "rel": "src/a.rs", "from": "left", "size": 2048 },
    { "op": "delete", "rel": "tmp/old.log" },
    { "op": "skip", "rel": "same.txt", "reason": "identical" }
  ],
  "executed": { "copied": 1, "deleted": 1, "skipped": 1, "errors": 0 },
  "dry_run": true
}
```

### 3.4 report --json

`bcr report --format json --output -`(或 `--json` 直出):报告以 JSON 输出,
与 compare --json 同构,供 CI 消费。

### 3.5 其他子命令

- `compare3 --json` / `csv --json` / `merge --json` / `mp3tag --json` / `imgcmp --json`:
  各自输出 schema 化 JSON(status 枚举统一为字符串)
- 会话/Profile 操作本已是 TOML,不重复

## 4. ② 声明式任务清单(bcr task)

### 4.1 定位

- **纯数据**,不是语言:无变量、无表达式、无循环、无分支
- 场景:把常用操作串成清单,`bcr task run` 一键执行(替代 BC 脚本的 80% 简单场景)
- 复杂逻辑(条件/循环/错误处理)→ 用户用通用语言写(见③)

### 4.2 格式(JSON 或 TOML 均可)

```json
{
  "name": "nightly-sync",
  "silent": false,
  "steps": [
    { "cmd": "load", "session": "Nightly Sync" },
    { "cmd": "compare", "left": "/data/in", "right": "/data/out", "content": true },
    { "cmd": "report", "format": "txt", "output": "diff-%date%.txt" },
    { "cmd": "sync", "mode": "mirror", "direction": "left->right" }
  ]
}
```

### 4.3 执行语义

- 顺序执行;遇错即停(或 `"continue_on_error": true` 跳过)
- 动态变量仅保留数据层:`%date% %time% %1-%9 %env:VAR%`(纯字符串替换,无表达式)
- 退出码:最后一步结果(0/1/2);`"expect": "diff"` 可断言期望结果
- `bcr task check <file>`:JSON schema 校验 + 命令名/参数合法性(纯数据,校验简单可靠)
- `--dry-run`:打印将执行的步骤序列

### 4.4 与 BC 对比

| BC .bcscript | bcr task(纯数据) |
|---|---|
| 脚本语法要学 | JSON/TOML,零语法 |
| 校验靠跑 | `bcr task check` schema 校验 |
| 条件/循环(BC 也没有) | 不做——复杂逻辑走通用语言 |

## 5. ③ 通用语言绑定(用户自己的语言)

### 5.1 定位

bcr 是"可编程工具":任何语言通过 `subprocess + --json` 消费。
官方提供薄封装示例,不发明任何新语法。

### 5.2 Python 示例(官方封装,约 100 行)

```python
import bcr  # bcr.py 封装,内部 subprocess 调 CLI + 解析 JSON

r = bcr.compare("/data/in", "/data/out", content=True)
if r.stats.differ > 0:
    bcr.sync("/data/in", "/data/out", mode="mirror")
    bcr.report("txt", f"diff-{date}.txt")
```

- `bcr.py` 仅做:拼 CLI 参数、跑子进程、解析 JSON、返回对象
- 用户可改用 shell/JS/Go/任何语言——契约是 JSON,语言无关

### 5.3 为什么这样彻底克服 BC 缺点

| BC 缺点 | v3 解法 |
|---|---|
| 要学新语言 | **用你已会的语言**(Python/shell/JS) |
| 功能天花板(无循环/条件/函数) | 宿主语言全部能力:if/for/函数/异常/类型 |
| 调试困难 | Python 断点、pytest、日志、类型检查 |
| 生态封闭 | 复用通用生态:CI、通知、数据库、调度器 |
| 平台开关差异 | CLI 跨平台统一;宿主语言跨平台 |
| 无 lint | `bcr task check` + 宿主语言静态检查 |
| 机器解析难 | 稳定 JSON schema(版本化) |

## 6. 保留的 BC 优点(全部保留)

| BC 优点 | v3 实现 |
|---|---|
| 无人值守 | `--silent` + 退出码 0/1/2 |
| 报告生成 | `report --json` / txt / csv / html |
| 会话复用 | `load --session`(task 第一步) |
| 动态变量 | `%date% %time% %1-%9 %env:VAR%`(数据层字符串替换) |
| 跨平台 | CLI 三平台一致;JSON 契约平台无关 |

## 7. 架构

```
CLI(现有) ──加 --json──► JSON 契约
   │
   ├── compare/compare3/csv/merge/imgcmp/mp3tag → 结果 JSON
   ├── sync → 计划 JSON + 执行结果 JSON
   └── report → txt/csv/html/json

src/
  jsonout.rs      # 统一 JSON 输出:结果结构 → serde_json,错误包装,版本化 schema
  task.rs         # task 子命令:清单解析(JSON/TOML)、schema 校验、顺序执行、dry-run
  bindings/       # 官方语言封装示例(bcr.py 等,不参与编译,文档级)
```

- `jsonout.rs`:每个子命令的 JSON 输出收敛到一处,保证 schema 稳定
- `task.rs`:纯数据解释器(约 300 行),无表达式求值——这是"不做 DSL"的关键克制
- 复用现有 serde 派生:`CompareResult/SyncOp` 已 `Serialize`,直接输出

## 8. 跨平台

- JSON/TOML 契约本身跨平台
- CLI 已是三平台一致(路径归一化/CRLF 兼容已有)
- 任务清单里的路径:统一 `/`,CLI 层归一化
- CI:`tests/script_tests.sh` 用**同一份 task.json + 同一份 Python 示例脚本**在三平台跑

## 9. 测试策略

1. **单元测试**:
   - jsonout:各命令 JSON schema 字段/类型/退出码一致性
   - task:schema 校验(未知命令/缺参数/类型错误)、顺序执行、dry-run、expect 断言
2. **CLI 冒烟**:`bcr compare --json` 管道给 python 解析
3. **验收套件**:acceptance.sh 新增 JSON 契约用例(断言字段与退出码)
4. **跨平台**:三平台跑同一 task.json + bcr.py 示例

## 10. 分阶段实施

### Phase 1 — JSON 契约核心
- `jsonout.rs` 统一输出框架 + schema 版本化
- `compare --json`(核心,CompareResult 已可序列化,工作最小)
- `sync --json`(计划 + dry-run + 执行结果)
- 退出码与 JSON 一致性测试

### Phase 2 — 全命令契约
- compare3/csv/merge/imgcmp/mp3tag `--json`
- `report --json`
- 验收套件 JSON 用例

### Phase 3 — 任务清单
- `bcr task run/check` + JSON/TOML 解析 + schema 校验
- 动态变量(数据层替换)+ dry-run + continue_on_error + expect
- 示例 task 文件集

### Phase 4 — 语言绑定与文档
- `bindings/bcr.py` 官方封装 + 示例(compare→sync→report 全流程)
- 文档:`docs/automation.md`(JSON 契约参考 + 各语言示例)
- README 章节 + CHANGELOG

## 11. 风险与对策

| 风险 | 对策 |
|---|---|
| JSON schema 漂移破坏外部脚本 | schema 版本化(`compare.v1`),变更升版本并记录 |
| 用户嫌"还要自己写语言" | task 清单覆盖简单场景;文档给 Python/Shell 现成模板 |
| CLI --json 与文本输出混淆 | `--json` 时 stdout 只含 JSON;人类文本走 stderr(或 --json 下抑制) |
| 范围失控(又想加语言特性) | 明确克制:task 无表达式;需要逻辑 → 通用语言 |

## 12. 关键决策

1. **不发明脚本语言**——这是与 v1/v2 的本质区别
2. **JSON 是唯一契约**:机器消费稳定、人类可读、任意语言可解析
3. **task 是纯数据**:只有步骤列表 + 字符串替换,无表达式/循环/分支
4. **复杂自动化 = 通用语言 + --json**:用户用 Python/shell/JS,能力无上限
5. **官方薄封装**:bcr.py 只做子进程 + JSON 解析,不发明框架
