# bcr — Beyond Compare 风格的文件对比工具（Rust）

Rust 实现的 Beyond Compare 替代品，当前完成 **M1：文本 diff** + **M2：文件夹对比** + **M3：三路合并** + **M4：同步引擎** + **M5：GUI** + **M6：虚拟文件系统** + **I18N：多语言** + **编码检测/二进制检测** + **语法高亮** + **P1：二进制 hex 对比** + **P2：移动/重命名检测** + **P3：流式读取** + **P4：HTML 报告与会话** + **P5：同步增强** + **P6：三路文件夹对比** + **P7：CSV/表格对比**。

## 功能

### 编码检测与二进制检测（P0）

- 所有文本入口（diff/merge/GUI）统一走 `encoding::decode`：**BOM 嗅探 → 严格 UTF-8 验证 → UTF-16 无 BOM 判定 → 二进制判定 → chardetng 多字节编码（GBK/Big5/Shift_JIS…）→ Latin-1 保底**，确定性检测、永不 panic
- 支持 UTF-8/UTF-16LE/BE/UTF-32LE/BE/GBK/Big5/Shift_JIS/Windows-1252 等（encoding_rs 全覆盖）
- 二进制判定：前 8192 字节 NUL 占比 ≥ 1% 或存在非文本控制字符 → `is_binary=true`；CLI diff/merge 对二进制文件报错 exit 2，GUI 自动切 hex 视图
- 强制指定：`--encoding <name>` 全局参数或 `BCR_ENCODING` 环境变量（跳过自动检测）
- 大小上限：`--max-size <MB>` 或 `BCR_MAX_SIZE`（默认 64MB，超限按文本加载报错，防 OOM）
- GUI 编辑保存**按原编码回写**（GBK 文件编辑后仍是 GBK，`encode_back` round-trip）

### 语法高亮（P1）

- syntect（Sublime 语法集，60+ 语言），按文件扩展名识别
- GUI 并排 Diff / 三路合并视图：**语法色管前景，diff 高亮管背景**，与 BC 一致分层
- CLI：`bcr diff --highlight` 输出 ANSI 语法色（需彩色输出）

### P7 CSV/表格对比

- `bcr csv LEFT RIGHT [--key id] [--delimiter ,] [--no-header]`：CSV/TSV 表格对比
- 对齐方式：`--key` 指定主键列（列名或列号）按主键对齐；缺省按行号对齐
- 列级 diff：每行内逐列比较，输出各列差异；RFC 4180 子集解析（双引号引用、`""` 转义、引号内可含分隔符与换行）
- `--delimiter` 支持自定义分隔符（`\t` 表示制表符）、`--no-header`、`--show-same`、`--summary`
- 退出码：0=无差异，1=有差异，2=错误

### P6 三路文件夹对比（compare3）

- `bcr compare3 BASE LEFT RIGHT`：三路目录树对比，10 种状态标记（仅一侧/两侧/三侧差异/冲突等）
- `--compare-content` 对大小相同的文件对做 blake3 哈希兜底（快速模式仅大小+修改时间）
- `--include/--exclude` glob 过滤（目录级剪枝）、`--show-same`、`--summary`
- 退出码：0=无差异，1=有差异，2=错误

### P5 同步引擎增强

- mirror/update 模式**移动检测**：源侧重命名/移动的文件识别为 `[MOVE] old -> new`（内容哈希一致），避免"复制+删除"误报
- mirror 模式**空目录清理 `[RMDIR]`**：镜像删除目标侧独有文件后，级联清理空目录（自底向上）
- 统计行新增 rename/rmdir 计数

### P20 细节增强（右键菜单 / 报告布局 / 缓存 / Profile 迁移）

- **目录对比右键菜单**：选中文件行右键 → 复制左侧/右侧路径、系统应用打开左右侧文件、在对比中打开（Enter）
- **报告布局**：`--report-title <标题>` 自定义报告标题、`--report-no-stats` 关闭统计行（txt/csv 通用）
- **比较结果缓存**：本地目录对比走 size+mtime 快照缓存（`~/.bcr-cache.toml`，LRU 64 条），目录未变化时秒开复用结果
- **Profile 迁移**：`bcr profile export <name> <file>` 导出独立 TOML；`bcr profile import <file> [--name <name>]` 导入（重名保护）

### P16 会话收藏与文本 diff 细节

- **会话收藏**：GUI 会话中心支持 ★ 收藏标记与最近使用时间排序（收藏优先），打开会话自动记录最近使用
- **编辑撤销/重做**：GUI 行内编辑框内置 Ctrl+Z / Ctrl+Y（egui TextEdit undoer）
- **No newline 标记**：`bcr diff` 对不以换行结尾的文件输出 `\ No newline at end of file`（GNU diff 兼容）
- **CRLF 忽略**：`bcr diff --ignore-crlf` 归一化行尾 CR/LF 差异（CLI 与 GUI 共用）

### P15 报告字段定制

- `bcr compare A B --txt r.txt --csv r.csv --report-fields status,path,size,mtime,moved`：选择报告字段（默认全部）
- 文本报告每行展示所选字段；CSV 表头/列随字段变化（mtime 输出 `YYYY-MM-DD HH:MM:SS`）

### P14 细节打磨（GUI 目录对比）

- **自动刷新**：目录对比每 2 秒自动重扫（无需手动点刷新）
- **手动对齐**：工具栏「⇱ 手动对齐」弹窗，左右各选一个文件配对打开并排 diff（支持不同文件名）
- **批量操作**：「批量复制→右」把全部差异/仅左侧文件复制到右侧；「批量删除右侧」删除右侧全部差异文件（操作后自动重算）

### P13 hex 编辑

- GUI 二进制视图（hex 对比）**双击行进入编辑**：十六进制输入框（如 `01 0a ff`），Ctrl+S 写回对应偏移并重建对比
- 与文本行内编辑（✏️）并列，补齐二进制文件的修改能力

### P12 文件属性与符号链接

- `bcr compare A B --compare-attrs`：比较 **Unix 权限位** 与 **符号链接目标**（默认仅大小+时间+内容）
- 内容一致但属性不同 → 判为 `[C]` 差异并在输出追加 `↳ 属性不同（权限/符号链接）`
- 符号链接参与扫描（记录目标路径，读链接自身元数据，不跟随避免悬挂链接/死循环）
- 非 Unix 平台或远程/压缩包后端无权限信息时自动跳过属性比较，不误报

### P11 报告格式（文本/CSV）

- `bcr compare A B --txt report.txt`：文本报告（统计 + 差异条目表 + 两侧大小）
- `bcr compare A B --csv report.csv`：CSV 报告（表头 `status,path,left_size,right_size,moved_to`，统计追加为 `#` 注释行，Excel/脚本可直接处理）
- 与 `--html` 平级，可同时导出多种格式

### P10 比较规则 Profile

- `bcr profile save <name> [--include ...] [--exclude ...] [--ignore-whitespace] [--ignore-trailing] [--ignore-case] [--encoding ...] [--compare-content] [--no-detect-moves]`：把过滤/忽略/编码等规则打包为可复用 Profile（`~/.bcr-profiles.toml`）
- `bcr profile list` / `bcr profile delete <name>`：列出/删除
- `bcr compare A B --profile <name>` / `bcr diff A B --profile <name>`：合并规则（命令显式参数优先于 Profile 默认值）
- GUI 会话中心：工具栏「会话中心」列出已保存会话（`~/.bcr-sessions.toml`），一键打开目录对比或删除
- **GUI 规则面板**：工具栏「规则」打开管理窗口——左侧列出全部规则集，右侧可视化编辑（include/exclude/忽略选项/编码/哈希/移动检测），支持**保存修改**、**应用**到当前目录对比、**删除**

### P4 HTML 报告与会话保存

- `bcr compare A B --html report.html`：导出自包含 HTML 对比报告（内嵌 CSS，浏览器直接打开），含统计摘要 + 差异条目表 + 移动标记
- `bcr session save <name> <left> <right> [--compare-content] [--include ...] [--exclude ...]`：把比较配置持久化为会话（`~/.bcr-sessions.toml`）
- `bcr session list` / `bcr session run <name>` / `bcr session delete <name>`：列出/复跑/删除会话

### P3 分块流式读取

- 内容比较（compare/sync `--compare-content`、移动检测）走 `Vfs::hash` 流式 blake3，内存 O(64KB)，300MB 文件实测内存 ~10MB
- `bcr hex` 分块渲染（64KB/块），超大二进制文件不占内存

### P2 重命名/移动检测（Detect Moves）

- `bcr compare A B` 默认开启：仅左侧与仅右侧中内容哈希一致的文件对合并为 `[M] old -> new`（跨子目录移动同样识别）
- `--detect-moves false` 关闭；`--summary` 输出移动/重命名对数

### P1 十六进制对比（二进制文件）

- `bcr hex LEFT RIGHT`：逐字节对比，差异行 `!` 标记 + 偏移 + 两侧 hex/ASCII，退出码 0/1/2
- GUI 检测到二进制文件自动切换 hex 视图（DiffTab 内渲染，差异行高亮）

### P8 图片对比（多帧版）

- `bcr imgcmp a.png b.png`：逐像素差异统计（差异像素数/百分比），退出码 0/1/2
- GUI ImageTab：并排渲染 + 差异叠加图 + 缩放（0.05x~8x）+ **适应窗口**（fit-to-window，按可用区自动计算缩放）
- **GIF/WebP 动图多帧对比**：帧导航（⏮◀▶⏭）+ 帧计数 + 底部**缩略图条**（点击跳转，差异帧红色边框标记）
- **逐帧差异定位**：⏮!/!▶ 跳到上一个/下一个**差异帧**（帧级差异预计算，循环导航）;「🎯 定位差异」按差异包围盒自动缩放并滚动到差异区域
- 静态图（PNG/JPEG/BMP）为单帧，行为不变

### I18N 多语言支持

- 支持 10 种语言：中文、English、Deutsch、日本語、한국어、Español、Português、العربية、Русский、Français
- CLI：`bcr --lang de ...` 全局参数，或环境变量 `BCR_LANG=de`（未指定时按系统 `LANG` 推断，默认中文）
- GUI：工具栏语言下拉框即时切换，持久化到 `~/.bcr-gui.toml`
- 覆盖全部 CLI 输出（错误消息/统计行/同步标签）与 GUI 文案（菜单/工具栏/标签页/Git 弹窗）
- 翻译表由宏保证穷尽：新增文案若缺少任一语言翻译会编译失败

### M6 虚拟文件系统（`zip://` / `tar://` / `7z://` / `sftp://` / `ftp://`）

- compare/sync 的路径参数支持虚拟后端，可跨后端混合对比：
  - `zip://path/to/archive.zip`：把 ZIP 压缩包当作目录树（可读写：write/delete/set_mtime 全量重写）
  - `tar://path/x.tar` / `tar://x.tar.gz` / `tar://x.tar.xz`：tar 及压缩变体（**可读写**：write/delete/rename/set_mtime 全量重写；tar.bz2 无纯 Rust 编码器保持只读）
  - `7z://path/x.7z`：7-Zip 压缩包（只读，全量解压进内存）
  - `sftp://[user[:pass]@]host[:port]/remote/path`：SFTP 远程目录（可读写，含 mtime 保留）
  - `ftp://[user[:pass]@]host[:port]/remote/path`：FTP 远程目录（可读写；无标准 mtime 设置命令，同步建议 `--compare-content`）
  - 普通路径仍为本地目录，可任意组合（本地 vs zip、zip vs tar.gz、本地 vs sftp、sftp vs ftp 等）
- 例：`bcr compare src/ "zip://backup.zip" --compare-content`、`bcr sync local/ "sftp://alice@nas/srv" --mode mirror --dry-run`、`bcr compare src/ "tar://backup.tar.gz"`、`bcr sync pub/ "ftp://mirror@files.example.com:/srv/pub" --mode update`
- 内部通过 [`Vfs`] trait 统一抽象（scan/read/write/delete/set_mtime），CLI 与 GUI 共用
- 注意：SFTP 首次连接不校验 host key（适用于受信环境）；7z 与 tar.bz2 后端只读，写入会报错；tar/7z 全量解压进内存，超大归档建议用 zip 或本地目录；FTP 被动模式，支持匿名登录（user=anonymous）

### M5 GUI（`bcr gui`）— 完整版

- `bcr gui [LEFT] [RIGHT]`：egui 桌面应用，三种标签页，多标签管理
  - **并排 Diff**：左右并排渲染，行内字符级高亮，虚拟化渲染（支持超大文件），同步滚动
    - 行内编辑：✏️ 编辑左侧/右侧（Ctrl+S 保存后自动重算 diff）
    - 搜索高亮（Ctrl+F）、行号跳转（Ctrl+G）、差异跳转（F7/Shift+F7）
  - **目录对比**：**树形视图**（▶/▼ 折叠子目录、缩进层级），`[L]/[R]/[C]` 状态 + 两侧大小，
    glob 过滤，**键盘导航**（↑↓ 选择、←→ 折叠、Enter 打开）、双击打开并排 Diff
  - **三路合并**：BASE/LEFT/RIGHT 三栏渲染，冲突块导航（F7/Shift+F7），取左/取右/取 BASE 解决，
    **底部实时预览**（未解决冲突高亮并输出 git 风格标记），保存合并结果
- 工具栏：忽略空白/行尾空白/大小写（即时重算）、重新加载、统计栏开关
- **主题切换（系统/深色/浅色）+ 设置持久化**：窗口大小、忽略选项、统计栏开关、主题均存入 `~/.bcr-gui.toml`，下次启动自动恢复
- **Git 集成**：🐙 弹窗展示 difftool/mergetool 配置，一键复制，可直接 `git difftool --tool=bcr` / `git mergetool --tool=bcr`
- 拖放文件/目录加载；`bcr gui --merge BASE LEFT RIGHT` 直接打开三路合并

### M4 目录同步（`bcr sync`）

- `bcr sync <LEFT> <RIGHT>`，三种模式：
  - `--mode update`（默认）：单向复制新增/更新（源较新才覆盖），不删除
  - `--mode mirror`：单向镜像，源为准无条件覆盖，并删除目标侧独有文件
  - `--mode two-way`：双向，新增/更新以 mtime 新者胜；mtime 相同且内容不同（需 `--compare-content`）报冲突跳过
- `--reverse` 反转方向（默认 LEFT → RIGHT）、`--dry-run` 只预览不执行
- 复制保留源 mtime（幂等：同步后再跑无操作）、自动建目录、`--include/--exclude` 过滤、`--summary`
- 退出码：0=成功，1=有冲突/有计划(dry-run)，2=错误

### M3 三路合并（`bcr merge`）

- `bcr merge <BASE> <LEFT> <RIGHT>`：经典 diff3 算法，两侧变更基于 base 行号归并
- 合并规则：单侧修改取该侧；两侧相同修改无冲突；两侧不同修改输出 git 风格冲突块
- 冲突标记：`<<<<<<< LEFT / ======= / >>>>>>> RIGHT`，可用 `-L` 自定义标签
- `-o` 输出到文件（有冲突也写出，供人工处理）、`-` 支持 stdin
- 退出码：0=无冲突，1=有冲突，2=错误（可直接作为 `git mergetool`）

### M2 文件夹对比（`bcr compare`）

- 递归对比两个目录树，输出差异状态列表：`[L]` 仅左侧 / `[R]` 仅右侧 / `[C]` 内容不同 / `[S]` 相同
- 双模式：默认按 `大小+修改时间` 快速比较；`--compare-content` 对大小相同的文件对做 blake3 哈希深度比对
- 过滤规则：`--include <glob>` 白名单、`--exclude <glob>` 黑名单（可重复，目录级剪枝）
- `--show-same` 显示相同文件、`--summary` 输出统计
- git 兼容退出码：0=无差异，1=有差异，2=错误

### M1 文本 diff（`bcr diff`）

- 两个文本文件的 unified diff 输出（兼容 GNU diff / git apply 格式）
- 行内差异高亮（intra-line diff，字符级二次比对）
- 双算法：`--algo myers` / `--algo patience`（默认 patience，更适合代码）
- 忽略选项：`--ignore-whitespace` / `--ignore-trailing` / `--ignore-case`
- 语法高亮：`--highlight`（ANSI 语法色，需彩色输出）
- `--color auto|always|never`（默认按 TTY 自动）
- `-L` 自定义标签、`-` 从 stdin 读取
- git 兼容退出码：**0=无差异，1=有差异，2=错误**

## 使用

```bash
cargo build --release

# 多语言：--lang 或 BCR_LANG（zh/en/de/ja/ko/es/pt/ar/ru/fr）
bcr --lang en diff old.rs new.rs
BCR_LANG=de bcr compare src/ backup/ --summary
bcr gui --lang ja old.rs new.rs

# 编码：--encoding 或 BCR_ENCODING（utf-8/utf-16le/gbk/big5/shift_jis 等，默认自动检测）
bcr --encoding gbk diff 中文旧文件.txt 中文新文件.txt
BCR_ENCODING=utf-16le bcr diff a.txt b.txt

# 文本大小上限（默认 64MB，防 OOM）
bcr --max-size 128 diff big1.log big2.log

# GUI 并排 Diff 视图（语法高亮 + 行内编辑）
bcr gui old.rs new.rs
bcr gui --ignore-whitespace old.rs new.rs

# 预览同步计划（不执行）
bcr sync src/ backup/ --mode mirror --dry-run

# 单向更新（左→右）
bcr sync src/ backup/ --mode update

# 双向同步，内容级冲突检测
bcr sync laptop/ nas/ --mode two-way --compare-content --summary

# 三路合并（无冲突自动合并）
bcr merge base.py left.py right.py -o merged.py

# 冲突时输出标记块，退出码 1
# <<<<<<< LEFT / ======= / >>>>>>> RIGHT

# 作为 git mergetool 使用（M3 后支持）
git mergetool --tool=bcr

# 目录对比（快速模式）
bcr compare old-dir new-dir

# 深度内容对比 + 统计
bcr compare old-dir new-dir --compare-content --summary

# 排除 build 目录和日志，只看源码差异
bcr compare src/ src-copy/ --exclude 'target/**' --exclude '*.log'

# 作为 git difftool 使用
# 基本对比
bcr diff old.rs new.rs

# 忽略空白 + 强制颜色
bcr diff --ignore-whitespace --color=always old.rs new.rs

# 语法高亮
bcr diff --highlight old.rs new.rs

# stdin 对比
printf 'a\nb\n' | bcr diff - new.txt -L stdin -L file

# 十六进制对比（二进制文件，P1）
bcr hex old.bin new.bin
bcr hex old.bin new.bin --show-same

# 目录对比 + 移动/重命名检测（P2，默认开启）
bcr compare old-dir new-dir
bcr compare old-dir new-dir --detect-moves false

# 三路文件夹对比（P6）
bcr compare3 base/ left/ right/
bcr compare3 base/ left/ right/ --compare-content --summary

# CSV/表格对比（P7）
bcr csv a.csv b.csv
bcr csv a.csv b.csv --key id --summary
bcr csv a.tsv b.tsv --delimiter '\t' --no-header

# 导出 HTML 对比报告（P4）
bcr compare old-dir new-dir --html report.html

# 会话保存/复跑/删除（P4）
bcr session save backup old-dir new-dir --compare-content --exclude 'target/**'
bcr session list
bcr session run backup
bcr session delete backup

# 作为 git difftool 使用
git difftool --tool=bcr
```

git 配置：

```ini
[difftool "bcr"]
    cmd = bcr diff "$LOCAL" "$REMOTE" -L "$LOCAL" -L "$REMOTE"
```

## 架构

```
src/main.rs     CLI 入口（clap 子命令分发，全局 --lang/--encoding/--max-size）
src/diff.rs     M1 参数解析、输入读取、diff 引擎（similar::capture_diff_slices）
src/render.rs   M1 unified 渲染：hunk 分组、行内高亮、ANSI 着色
src/encoding.rs P0 编码检测与二进制检测：decode 检测链、TextFile、按原编码回写
src/highlight.rs P1 语法高亮：syntect 主题/语法识别/行高亮
src/compare.rs  M2 目录扫描（walkdir）、双模式比较、glob 过滤、状态输出
src/compare3.rs P6 三路文件夹对比：BASE/LEFT/RIGHT、10 状态标记
src/csvcmp.rs   P7 CSV/表格对比：主键对齐、列级 diff、引号字段解析
src/merge.rs    M3 三路合并：diff3 归并（collect_block + apply_regions）、冲突标记
src/fsscan.rs   共享扫描/过滤/哈希模块（本地实现，compare 与 sync 共用）
src/sync.rs     M4 同步引擎：三模式计划生成、dry-run、mtime 保留复制、移动检测、空目录清理
src/sideview.rs M5 并排 diff 数据模型：行级 ops 展开为并排行（行号+行内高亮），纯逻辑可单测
src/mergeview.rs M5 三路合并视图模型：块级对齐 + 冲突标记 + 解决选择
src/hexview.rs  P1 hex 对比数据模型：行构建/渲染（分块，支持超大文件）
src/htmlreport.rs P4 HTML 报告渲染
src/session.rs  P4 会话持久化（~/.bcr-sessions.toml）
src/gui/         M5 egui 窗口：mod.rs（多标签/主题/持久化）、difftab（并排+搜索+跳转+hex）、
                 dirtab（目录导航）、mergetab（三路合并）、common（虚拟化渲染/着色）
src/vfs/        M6 虚拟文件系统：mod.rs（Vfs trait + LocalVfs + 路径解析）、zip.rs（ZIP 只读）、
                archive.rs（tar/gz/bz2/xz/7z 只读，全量解压）、sftp.rs（russh 纯 Rust SFTP）
src/i18n.rs     I18N：Lang 枚举 + Key 枚举 + 全局语言 + t()/fmt()
src/i18n_tables.rs I18N 翻译表（10 语言 × 全量 Key，宏保证穷尽）
```

关键设计：

- **比较键与输出分离**：忽略选项作用于归一化后的"比较键"，输出始终保留原始行，不会因忽略空白而丢内容
- **两级 diff**：行级 diff 定位变更行，变更行对再跑字符级 diff 得到行内高亮区间
- **hunk 分组**：仅按变更 op 之间的间隔决定是否断开（间隔 > 2×3 行上下文则新开 hunk）
- **编码/二进制统一入口**：所有文本入口共用 `encoding::decode` 检测链，CLI 与 GUI 行为一致，编辑保存按原编码回写

## Roadmap

- [x] M1 文本 diff
- [x] M2 文件夹对比（walkdir + blake3 + 过滤规则）
- [x] M3 三路合并 + 冲突标记
- [x] M4 同步引擎（镜像/双向/更新 + dry-run 预览）
- [x] M5 GUI（egui 并排 Diff 视图）
- [x] M6 远程/压缩包适配层（SFTP / ZIP 虚拟 FS）
- [x] M6b 归档后端扩展（tar/tar.gz/tar.bz2/tar.xz/7z 只读）
- [x] P0 编码检测 + 二进制检测
- [x] P1 语法高亮
- [x] P1 二进制 hex 对比
- [x] P2 移动/重命名检测
- [x] P3 分块流式读取
- [x] P4 HTML 报告 + 会话保存
- [x] P5 同步增强（移动检测 + 空目录清理）
- [x] P6 三路文件夹对比（compare3）
- [x] P7 CSV/表格对比

## 已知限制

- 文本 diff 整文件读入内存，超过 `--max-size` 上限（默认 64MB）报错；超大文件内容比较走 P3 流式哈希
- 不处理 "No newline at end of file" 标记
- 二进制文件已做检测：CLI diff/merge 报错 exit 2；GUI 自动切 hex 视图
- M5 目录对比的 glob 过滤在 GUI 中以逗号分隔输入；拖放仅支持本地文件
- M6 7z 与 tar.bz2 后端只读（写入/删除会报错）；tar/7z 全量解压进内存，超大归档建议 zip 或本地；SFTP 首次连接不校验 host key，且依赖网络可达性；FTP 无标准 mtime 设置命令（同步建议 `--compare-content`）
- 快速模式依赖 mtime，跨文件系统/拷贝场景建议用 `--compare-content` 保证准确
- M3 三处 stdin 不能同时用（`-` 只能出现一次）
- 与 git 的行为差异：两侧对**相邻行**的独立修改，bcr 按经典 diff3 语义无冲突合并，git 保守判冲突
- sync 快速模式下无法检测“mtime 相同但内容不同”，two-way 冲突检测需 `--compare-content`
- 语法高亮仅在 GUI 与 `diff --highlight` 生效；CLI compare/sync 输出无语法色
