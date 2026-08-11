# P27 脚本自动化引擎（bcrscript）设计方案

> 对标 Beyond Compare 的 .bcscript,保留其优点的同时克服缺点;
> 跨平台统一语法,一套脚本适配 Windows / macOS / Linux。

## 1. 设计目标

| # | 目标 | 对应 BC 优点 | 克服的 BC 缺点 |
|---|---|---|---|
| G1 | 命令面完整:加载/比较/同步/文件操作/报告全覆盖 | ✅ 27 命令能力 | — |
| G2 | 会话/Profile 复用:`load <session>` 零配置 | ✅ 会话深度集成 | — |
| G3 | 无人值守:`--silent` + 退出码表达结果 | ✅ /silent | — |
| G4 | 动态变量:`%date% %time% %1-%9` | ✅ 动态变量 | — |
| G5 | **真正的变量** `$name = value` | ❌ BC 无 | ✅ |
| G6 | **条件分支 if/else** | ❌ BC 无 | ✅ |
| G7 | **循环 foreach** | ❌ BC 无 | ✅ |
| G8 | **结构化输出 `--json`** | ❌ BC 无 | ✅ |
| G9 | **纯内核解释器,不依赖 GUI 进程** | ❌ BC 驱动 GUI | ✅ |
| G10 | **跨平台一套脚本,无开关差异** | ❌ BC 分 `/` 与 `-` | ✅ |
| G11 | 友好报错 + `bcr script check` 静态校验 | ❌ BC 无 lint | ✅ |

## 2. 与 BC 对比总览

| 维度 | BC .bcscript | bcrscript |
|---|---|---|
| 语法 | 行式命令 + `key:value`,`&` 续行 | 行式命令 + `key:value`,`\` 续行 |
| 变量 | ❌ 仅 `%1-%9` 位置参数 | ✅ `$name` 用户变量 + `%1-%9` + 动态变量 |
| 分支 | ❌ 无 | ✅ `if last-exit != 0 ... endif` |
| 循环 | ❌ 无 | ✅ `foreach f in left-only ... endfor` |
| 平台开关 | `/silent` vs `-silent` 分平台 | 统一 `--silent`(全部平台) |
| 内核 | 驱动 GUI 进程,启动慢 | 纯 Rust 内核,无 GUI 依赖,毫秒级启动 |
| 输出 | 文本报告,机器解析困难 | txt/csv/html + `--json` 结构化结果 |
| 校验 | 无 | `bcr script check` 静态检查 + 报错带行号 |
| 会话复用 | `load <session>` | 同款,复用现有 session/profile 存储 |

## 3. 脚本语言设计

### 3.1 文件与运行方式

- 扩展名:`.bcrscript`(纯文本,UTF-8,支持 BOM)
- 运行:`bcr script run <file.bcrscript> [args...]`(同义:`bcr runscript`)
- 校验:`bcr script check <file>`(不执行,输出语法/引用错误与行号)
- 静默:`--silent`(无进度输出,仅错误与报告;CI 友好)
- 退出码:同 CLI 约定 `0=无差异/成功,1=有差异,2=错误`;脚本可 `exit <code>` 覆盖

### 3.2 基础语法

```
# 注释(整行,# 开头)
load "C:\Data\In" "/data/out"     # 路径引号包裹;分隔符跨平台自动归一化
load "Nightly Sync"                # 加载已保存会话
criteria timestamp:2sec            # 比较准则
filter "-*.log -tmp/"              # 过滤规则(BC 风格 + 可重复)
compare                            # 执行目录比较
if last-diff > 0 then              # 条件:上一步差异数
    report txt output:"diff-%date%.txt"
endif
sync mirror:left->right            # 方向性同步
copyto left path:relative "C:\Backup"   # 文件操作
echo "done at %time%"
```

- 续行:行尾 `\`
- 大小写:命令不区分大小写(`LOAD` = `load`);变量名区分
- 引号:双引号包裹含空格的参数;`""` 表示空参数
- 路径归一化:解释器统一把 `\` 转 `/`(Windows 输入自动转换,脚本可在任一平台书写)

### 3.3 变量系统(BC 没有 → 我们的增强)

```
$src = "/data/in"
$dst = "/data/out"
$mode = "mirror"
load $src $dst
sync $mode:left->right
```

- 作用域:脚本级(全局);`foreach` 循环变量循环级
- 展开:`$name` 在任意参数位置展开
- 未定义变量:报错(带行号),防止拼写错误静默产生空参数
- 动态变量(兼容 BC):
  - `%date%` → `2026-08-12`(本地时区)
  - `%time%` → `HH:MM:SS`
  - `%fn_time%` → `HH-MM-SS`(文件名安全)
  - `%1`...`%9` → 命令行位置参数(运行脚本时传入)
- 内置状态变量(供 if 判断):
  - `last-exit` → 上一步退出码
  - `last-diff` → 上一步比较的差异文件数
  - `last-ops` → 上一步同步实际执行的操作数
  - `last-errors` → 上一步错误数

### 3.4 条件分支(BC 没有 → 我们的增强)

```
if last-diff > 0 then
    report txt output:"diff-%date%.txt"
else
    echo "no differences"
endif
```

- 支持比较运算符:`== != > >= < <=`
- 支持布尔组合:`and or not`
- 可判断变量、内置状态变量、字面量
- `if` 块必须 `endif`;`else` 可选
- 支持嵌套

### 3.5 循环(BC 没有 → 我们的增强)

```
# 对左侧独有的每个文件执行操作
foreach f in left-only
    echo "orphan: $f"
endfor

# 对报告中的每个差异文件
foreach f in differences
    copyto left file:$f output:"backup/"
endfor
```

- 集合来源:`left-only right-only differences same all`
- 循环变量 `$f` 为相对路径
- 支持 `break` / `continue`
- `endfor` 必须配对;循环内可嵌套 if

### 3.6 命令集(第一版,按现有能力映射)

| 分类 | 命令 | 说明 | 映射现有模块 |
|---|---|---|---|
| 加载 | `load <left> [right]` / `load <session>` | 加载路径或会话 | session.rs, vfs::open |
| 规则 | `criteria <k:v>` | content/timestamp/attrs/version 等 | compare.rs CompareArgs |
| 规则 | `filter <globs>` | 包含/排除 | fsscan::Filter |
| 比较 | `compare` | 执行目录/文件比较 | compare::run |
| 比较 | `compare3 <base> <left> <right>` | 三路文件夹 | compare3 |
| 同步 | `sync <update\|mirror\|two-way>:<l->r\|r->l>` | 方向性同步 | sync::build_plan/execute_op |
| 文件 | `copyto <left\|right> [path:relative\|file:<rel>] <dest>` | 复制 | vfs::read/write |
| 文件 | `move <rel> <dest>` | 移动 | vfs::rename |
| 文件 | `delete <rel>` | 删除 | vfs::delete |
| 报告 | `report <txt\|csv\|html\|json> output:<file> [fields:...]` | 生成报告 | report.rs / htmlreport.rs |
| 选择 | `select <files\|differences\|orphans>` | 限定后续操作范围 | CompareResult 过滤 |
| 流程 | `if/else/endif` `foreach/endfor` `break` `continue` | 控制流 | — |
| 输出 | `echo <text>` | 打印(含变量展开) | — |
| 流程 | `exit [code]` | 结束并设退出码 | — |
| 控制 | `--silent` | 静默模式(全局开关) | — |

### 3.7 执行语义

- 顺序执行;**默认遇错即停**(BC 行为),错误信息含行号与命令
- `continue-on-error` 指令可切换为"跳过错误继续"(运维场景常用)
- 每步执行后更新内置状态变量(`last-exit/last-diff/...`)
- 结束时退出码:
  1. 显式 `exit <code>` 优先
  2. 否则最后一次 `compare/sync` 的结果(0/1)
  3. 否则 0
- `--silent` 下仍输出报告文件与错误,仅抑制 stdout 进度

## 4. 架构设计

```
src/script/
  mod.rs        # 入口:run/check 子命令、文件读取、顶层编排
  lexer.rs      # 词法:命令、参数、引号、注释、续行、token 流(带行号)
  parser.rs     # 语法:命令 → 指令树;if/foreach 块配对与嵌套校验
  value.rs      # 变量表、动态变量展开(%date% 等)、状态变量
  exec.rs       # 解释执行:遍历指令树,绑定现有模块调用
  report.rs     # 报告命令分派(txt/csv/html/json)
```

- **lexer/parser/exec 三层分离**:parser 产出无状态的 `Vec<Stmt>`,可被 check 复用(静态校验 = parse + 变量引用检查,不执行)
- **exec 绑定现有纯逻辑接口**:compare/sync 已是 `build_plan/execute_op` 分离,脚本引擎直接调用,无 GUI 依赖
- **VFS 全后端可用**:load 任意路径(本地/zip/sftp/ftp/webdav/s3/onedrive/dropbox),脚本天然继承 10 种后端
- **不引入外部解析依赖**:自研手写递归下降解析器(约 600 行),与项目自研风格一致(参考 mp3tag/webdav 先例)

## 5. 跨平台策略(G10)

| 痛点(BC) | bcrscript 方案 |
|---|---|
| Windows 用 `/switch`,mac 用 `-switch` | 统一 `--silent`,平台无关 |
| 路径分隔符差异 | 解释器统一归一化 `\`→`/`;输出到文件系统时由 vfs 层处理 |
| 脚本里写死 `C:\...` | 动态变量 `%1-%9` + 环境变量展开 `$env:HOME`(三平台一致语法) |
| CRLF/LF 差异 | 解析器按行拆分时兼容 `\n` 与 `\r\n`(类似 diff 的 CRLF 处理) |
| CI 三平台验证 | 验收套件新增 `tests/script_tests.sh`:同一份 .bcrscript 在 ubuntu/windows/macos 各跑一遍,断言退出码与报告内容 |

## 6. 测试策略

1. **单元测试**(lexer/parser/exec,纯逻辑):
   - token 化:引号/注释/续行/行号
   - 解析:if/endif 配对、foreach 嵌套、非法嵌套报错
   - 变量:展开、未定义报错、动态变量格式化
   - 执行:compare/sync/report 端到端(tempdir 构造目录)
2. **CLI 测试**:`bcr script run` / `bcr script check` 冒烟
3. **验收套件**:`tests/acceptance.sh` 新增脚本用例(约 10-15 条)
4. **跨平台**:CI 三平台跑同一套脚本(新增 `tests/script_tests.sh`)

## 7. 分阶段实施计划

### Phase 1 — MVP(核心闭环)
- lexer + parser(命令/参数/注释/续行)
- 变量 `$name` + `%1-%9` + `%date% %time% %fn_time%`
- 命令:`load compare sync report(txt/csv/html) echo exit`
- 状态变量 `last-exit last-diff`
- `bcr script run` + `--silent` + 退出码
- 单元测试 + 验收用例 + 三平台 CI

### Phase 2 — 控制流
- `if/else/endif`、`foreach/endfor`、`break/continue`
- 状态变量扩充(`last-ops last-errors`)
- `continue-on-error`
- 解析器块配对/嵌套校验 + `bcr script check`

### Phase 3 — 完整命令面
- `criteria filter select copyto move delete`
- `compare3`、会话加载 `load <session>`、Profile 应用
- `--json` 结构化输出(供外部工具消费)
- 文档:`docs/script-reference.md` 命令参考 + 示例脚本集

### Phase 4 — 打磨
- `bcr script check` 完善(未定义变量/死代码/行号提示)
- 示例脚本:`nightly-sync.bcrscript`、`diff-report.bcrscript` 等
- README 脚本章节 + CHANGELOG

## 8. 风险与对策

| 风险 | 对策 |
|---|---|
| 自研解析器 bug 导致脚本误执行 | lexer/parser 单测覆盖 + `bcr script check` 先行;执行前可 `--dry-run` 打印指令树 |
| 与 BC 语法不兼容,迁移成本 | 命令名/参数风格刻意对齐 BC(`load sync report filter criteria`),`%date%` 等动态变量同名;差异仅在**增强**(变量/if/foreach) |
| 脚本在 Windows 路径/换行出问题 | 路径归一化 + CRLF 容忍 + 三平台验收脚本兜底 |
| 范围过大拖延 | 按 Phase 推进,Phase 1 即可独立交付价值(相当于 BC 基础脚本能力) |

## 9. 关键决策记录

1. **自研解析器**,不引 crates(如 pest/nom):项目一贯风格,依赖可控,解析逻辑简单(行式命令)
2. **命令语法对齐 BC**,增强才用新语法:降低用户从 BC 迁移的心理成本,文档可直接对照
3. **纯内核执行**,不启动 GUI:`bcr script run` 是独立子命令,与 `bcr gui` 完全解耦
4. **扩展名 .bcrscript**:明确区分 BC 的 .bcscript,避免混用
5. **--json 输出**:是 bcr 相对 BC 的差异化卖点,Phase 3 必须落地
