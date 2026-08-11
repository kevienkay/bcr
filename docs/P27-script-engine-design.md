# P27 脚本自动化引擎（bcrscript）设计方案 v2

> 功能对标 Beyond Compare 脚本引擎,语法完全自主(不兼容 .bcscript);
> 现代脚本风格 + 完整控制流 + 结构化输出 + 跨平台一套脚本。

## 1. 设计原则

| 原则 | 说明 |
|---|---|
| **功能对标,语法自由** | 只对标 BC 脚本的*能力面*(加载/比较/同步/报告/无人值守/会话复用),语法 100% 自研,不受 BC 老派语法束缚 |
| **与现代 CLI 心智一致** | 脚本命令参数风格 = bcr CLI 风格(`--key value`),用户会 CLI 就会脚本 |
| **完整控制流** | 变量、if/else、foreach、break/continue、断言,不是"无编程能力的 DSL" |
| **纯内核** | 解释器直接调用现有纯逻辑模块,不启动 GUI,毫秒级启动 |
| **跨平台一套脚本** | 无平台开关差异,路径/换行自动归一化 |
| **可静态校验** | `bcr script check` 不执行即报语法/变量/类型错误(带行号) |

## 2. 语法总览(示例)

```
# 夜间同步脚本示例 — bcrscript v2
# 注释用 #;块用大括号;参数用 --key value

# ── 变量(全局)──
$src = "/data/in"
$dst = "/data/out"

# ── 加载并比较 ──
load --left $src --right $dst
compare --content

# ── 控制流:有差异才报告+同步 ──
if $last.diff > 0 {
    report --format txt --output "diff-%date%.txt"
    sync --mode mirror --direction left->right
    echo "同步完成: $last.ops 个文件"
} else {
    echo "无差异"
}

# ── 循环:处理左侧孤儿文件 ──
foreach $f in left-only {
    echo "orphan: $f"
}
```

## 3. 语言规范

### 3.1 词法

- 行式命令;一行一个命令(块关键字除外)
- 注释:`#` 到行尾(字符串内除外)
- 续行:行尾 `\`;或大括号/括号内隐式换行
- 字符串:双引号 `"..."`(支持转义 `\" \\ \n`);单引号 `'...'` 原样
- 数字:整数/浮点;布尔:`true false`
- 变量:`$name`(字母开头,含 `_`);内置状态变量以 `$last.` 前缀
- 动态变量:`%date% %time% %fn_time% %1-%9 %env:VAR%`
- 换行:兼容 `\n` 与 `\r\n`(Windows 脚本可直接跑)

### 3.2 命令与参数

```
命令 [位置参数...] [--key value]...
```

- 位置参数:字符串/数字/变量
- 命名参数:`--key value`;布尔开关:`--content`(无值即 true)
- 未知选项 → 报错(带行号),防止拼写错误静默生效
- 参数风格与 bcr CLI 对齐:`--mode` `--format` `--output` 等同名同义

### 3.3 变量

```
$name = 值                  # 赋值(值可为字面量/变量/表达式)
$name = $a + 1
$env:BCR_LANG = "zh"        # 环境变量读写
```

- 作用域:全局;`foreach` 循环变量为循环局部
- 未定义变量引用 → 编译期报错(`check` 阶段)
- 动态变量(运行期展开):
  - `%date%` → `2026-08-12`(本地时区)
  - `%time%` → `00:48:30`;`%fn_time%` → `00-48-30`
  - `%1`...`%9` → `bcr script run file.bcrscript a b c` 传入的位置参数
  - `%env:VAR%` → 环境变量(三平台一致语法)

### 3.4 表达式(用于 if/foreach/赋值)

- 算术:`+ - * / %`
- 比较:`== != > >= < <=`
- 逻辑:`and or not`
- 括号分组
- 内置函数(Phase 3):`count(<set>)` `exists(<path>)` `size(<path>)`

### 3.5 控制流

```
if 表达式 { ... } else { ... }        # else 可选,可嵌套
foreach $f in <集合> { ... }          # 集合:left-only right-only differences same all
    break / continue                  # 循环控制
assert 表达式 --message "..."         # 不满足即失败(带消息)
exit [code]                           # 提前结束,设退出码
```

- `if` 可后置单命令:`if $last.diff > 0 then report --format txt ...`(语法糖,可选)
- 块必须大括号配对;parser 校验嵌套合法性

### 3.6 内置状态变量(每步命令后自动更新)

| 变量 | 含义 |
|---|---|
| `$last.exit` | 上一步退出码 |
| `$last.diff` | 上一步比较的差异文件数 |
| `$last.ops` | 上一步同步执行的操作数 |
| `$last.errors` | 上一步错误数 |
| `$last.status` | 上一步结果标记(`ok` / `diff` / `error`) |

### 3.7 命令集(映射现有模块)

| 命令 | 参数 | 说明 | 映射 |
|---|---|---|---|
| `load` | `--left --right` 或 `--session <name>` | 加载路径(任意 VFS 后端)或会话 | session.rs / vfs::open |
| `compare` | `--content --include G --exclude G --criteria k=v` | 目录/文件比较 | compare::run |
| `compare3` | `--base --left --right` | 三路文件夹对比 | compare3 |
| `sync` | `--mode update\|mirror\|two-way --direction left->right\|right->left --dry-run` | 方向性同步 | sync::build_plan/execute_op |
| `report` | `--format txt\|csv\|html\|json --output FILE --fields ...` | 报告生成 | report.rs / htmlreport.rs |
| `copyto` | `--from left\|right --file REL --to DEST` | 复制文件 | vfs::read/write |
| `move` | `--file REL --to DEST` | 移动/重命名 | vfs::rename |
| `delete` | `--file REL` | 删除 | vfs::delete |
| `select` | `--set differences\|orphans\|all` | 限定后续操作范围 | CompareResult 过滤 |
| `echo` | 文本(变量展开) | 输出 | — |
| `assert` | `--cond EXPR --message MSG` | 断言 | — |
| `exit` | `[code]` | 结束 | — |
| `error` | `--message MSG` | 主动失败(退出码 2) | — |

### 3.8 执行语义

- 顺序执行;**默认遇错即停**(错误含行号 + 命令 + 原因)
- `continue-on-error` 指令切换为跳过错误继续(运维场景)
- 每步后更新 `$last.*` 状态变量
- 结束退出码:显式 `exit` > 最后一次 compare/sync 结果 > 0
- `--silent` 全局开关:抑制 stdout 进度,保留错误与报告文件
- `--json` 全局开关:每个命令输出结构化结果到 stdout(供外部工具消费)

## 4. 架构设计

```
src/script/
  mod.rs        # 入口:run / check 子命令、文件读取、顶层编排、--silent/--json
  lexer.rs      # 词法:token 流(命令/参数/字符串/数字/变量/块/注释/行号)
  parser.rs     # 语法:命令+参数 → Stmt 树;if/foreach 块配对与嵌套校验;未知选项/未定义变量报错
  expr.rs       # 表达式:递归下降解析 + 求值(算术/比较/逻辑/函数)
  exec.rs       # 解释执行:遍历 Stmt 树,绑定现有模块,更新 $last.*
  vars.rs       # 变量表 + 动态变量展开(%date%/%1/%env:%)+ 环境变量读写
  report.rs     # report 命令分派(txt/csv/html/json)
```

- **parser 无状态产出指令树** → `check` 复用 parse + 变量引用检查,不执行
- **exec 绑定现有纯逻辑接口**:compare/sync 已是 `build_plan/execute_op` 分离;VFS 全后端可用
- **表达式解析器自研**(约 200 行递归下降),不引外部解析 crate
- **词法/语法分离**:lexer 带行号,报错精确到行

## 5. 跨平台策略

| 痛点(BC) | bcrscript v2 |
|---|---|
| Windows `/silent` vs mac `-silent` | 统一 `--silent`,无平台开关 |
| 路径分隔符 | 解释器归一化 `\`→`/`;写文件系统由 vfs 层处理 |
| 脚本写死 `C:\...` | `%env:VAR%` + 位置参数 + 变量,三平台同一套脚本 |
| CRLF/LF | 词法按行拆分兼容 `\n` 与 `\r\n` |
| CI 验证 | `tests/script_tests.sh`:同一份 .bcrscript 在 ubuntu/windows/macos 各跑一遍,断言退出码与报告 |

## 6. 与 BC 能力对标表

| BC 脚本能力 | bcrscript v2 对应 | 超越点 |
|---|---|---|
| load 会话/路径 | `load --session / --left --right` | VFS 10 后端全支持 |
| criteria/filter 规则 | `compare --criteria --include --exclude` | 与 CLI 参数同义 |
| sync 方向同步 | `sync --mode --direction` | 同 |
| 各类 report | `report --format txt/csv/html/json` | **+json 结构化** |
| copy/move/delete | `copyto / move / delete` | 同 |
| /silent 无人值守 | `--silent` | 纯内核,无 GUI 开销 |
| 退出码 | 0/1/2 约定 + `exit` | 同 |
| %date% 等动态变量 | `%date% %time% %fn_time%` | 同 |
| 位置参数 %1-%9 | `%1-%9` | 同 |
| —(BC 无变量) | `$name` 变量 | ✅ 新增 |
| —(BC 无分支) | `if/else` | ✅ 新增 |
| —(BC 无循环) | `foreach/break/continue` | ✅ 新增 |
| —(BC 无校验) | `bcr script check` 静态检查 | ✅ 新增 |
| —(BC 无断言) | `assert` | ✅ 新增 |

## 7. 测试策略

1. **单元测试**:
   - lexer:引号/注释/续行/CRLF/行号
   - parser:块配对、嵌套、未知选项、未定义变量报错
   - expr:优先级、比较、逻辑、函数
   - exec:端到端(tempdir 目录树 + compare/sync/report 断言)
   - vars:动态变量展开、环境变量
2. **CLI 冒烟**:`bcr script run` / `bcr script check` / `--silent` / `--json`
3. **验收套件**:`tests/acceptance.sh` 新增脚本用例(10-15 条)
4. **跨平台**:CI 三平台 `tests/script_tests.sh` 跑同一套脚本

## 8. 分阶段实施

### Phase 1 — MVP(核心闭环)
- lexer + parser(命令/参数/字符串/注释/续行/块)
- 变量 `$name` + 动态变量 `%date% %time% %fn_time% %1-%9`
- 命令:`load compare sync report(txt/csv/html) echo exit`
- 状态变量 `$last.exit $last.diff`
- `bcr script run` + `--silent` + 退出码
- 单元测试 + 验收 + 三平台 CI

### Phase 2 — 控制流
- `if/else`、`foreach/break/continue`、表达式求值
- 状态变量扩充(`$last.ops $last.errors $last.status`)
- `continue-on-error`
- `bcr script check` 静态校验(块配对/变量引用/未知选项)

### Phase 3 — 完整命令面
- `criteria/filter/select/copyto/move/delete/compare3/assert/error`
- `--json` 结构化输出;`--dry-run` 预览
- `load --session` 会话复用 + Profile 应用
- 文档:`docs/script-reference.md` + 示例脚本集

### Phase 4 — 打磨
- check 增强(死代码/类型检查提示)
- 示例脚本:`nightly-sync` / `diff-report` / `backup-archive`
- README 章节 + CHANGELOG

## 9. 风险与对策

| 风险 | 对策 |
|---|---|
| 自研解析器 bug | lexer/parser/expr 单测覆盖;`check` 先行;`--dry-run` 打印指令树 |
| 语法设计过度/不足 | 语法最小集定稿(MVP)+ 示例驱动设计(先写示例脚本再定语法) |
| Windows 路径/换行问题 | 归一化 + CRLF 容忍 + 三平台验收脚本 |
| 与 CLI 参数漂移 | 命令参数直接复用 CLI 的 clap 定义思路,同名单参数 |

## 10. 关键决策

1. **语法完全自主**,不兼容 .bcscript:现代行式命令 + `--key value` + 大括号块
2. **命令参数风格 = CLI 参数风格**:用户学习成本趋近于零
3. **控制流是语言一等公民**:变量/if/foreach/assert,对标 BC 的"能力"而非"语法"
4. **自研解析器**(lexer/parser/expr 共约 600-800 行),不引外部解析 crate
5. **--json 是核心卖点**:脚本结果可直接被 CI/外部工具消费(BC 做不到)
6. **纯内核执行**,与 GUI 解耦
