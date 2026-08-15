# bcr — Beyond Compare 风格的文件对比工具（Rust）

Rust 实现的 Beyond Compare 替代品，当前完成 **M1：文本 diff** + **M2：文件夹对比** + **M3：三路合并** + **M4：同步引擎** + **M5：GUI** + **M6：虚拟文件系统（14 种后端）** + **I18N：多语言** + **P0：编码检测/二进制检测** + **P1：语法高亮 + hex 对比** + **P2：移动检测** + **P3：流式读取** + **P4：HTML 报告与会话** + **P5-P7：同步增强/compare3/CSV** + **P8：图片对比（多帧/差异定位）** + **P9：GUI 同步面板** + **P10：Profile 规则** + **P11-P12：报告/属性** + **P13-P16：hex 编辑/细节/收藏** + **P17-P23：缓存/报告定制/WebDAV/内容过滤/拖放排序** + **P24-P26：MP3 标签比较/版本比较模式/GUI 云盘浏览** + **P27：自动化（JSON 契约 + Python 绑定 bcr.py + 纯数据任务清单 bcr task）** + **P28：全面对标收口（三路文件夹合并/忽略结构/符号链接跟随/转换后比较/替换/缩略图/打印/HTML 模板/通用音频标签/FTPS/SFTP host key 校验/SVN/外部工具/CAB/ISO/7z 可写/大文件 mmap/后台任务）** + **P29：CSV 表格 GUI** + **P30：三平台安装包（dmg/zip/tar.gz/deb）** + **P31：UI 精修（主题引擎/状态栏/标签栏）** + **P32：UI 差距消除（差异连接线/内联编辑/撤销重做/右键菜单全覆盖/差异块折叠/会话类型起始页/快捷键系统化/过滤面板/状态栏信息/标签拖拽/hex 差异导航）** + **P33：对标 BC 5 真实界面重构（标准菜单栏 Session/File/Edit/Search/View/Tools/Help + 工具栏分组 + 文件信息头两行化 + BC 式主页会话面板 + 文件夹列头与配色对齐）** + **P34：交互模型对齐（空会话优先 + 拖拽导入填充）** + **P35：文本对比核心交互（复制差异块到另一侧/交换左右/视图过滤 All-Diff-Same-Context/显示空白符）** + **P36：文件夹对比交互补齐（交换两边/逐文件操作复制-删除-排除/视图过滤快捷键 1-2-3）** + **P37：全视图对标补齐（三路合并顺序合并+导航/表格复制单元格-隐藏相同列-列宽自适应/hex 地址与字节序格式/图片旋转翻转+容差-不匹配范围-混合模式/文件夹同步立即同步-独自离开-批量双向/文本编辑视图/补丁视图/文件夹合并 GUI）** + **P38：文本对比深化（隔离 Isolate/手动对齐 Align/缩进调整/编辑导航 Next-Prev Edit/复制文件并打开下一差异 + macOS 双击启动 GUI + 主页卡片点击修复）** + **P39：BC 5.2.5 全视图对标收口（设置对话框 ⌘, 集中管理/新建标签 ⌘T 与窗口 ⌘N/快捷键系统化 ⌘L-⌘G-⇧⌘G-⌥⌘S-⌥⌘C-1-2-3/会话中心保存当前会话/报告生成 ⌘P 文本-HTML/差异部分导航 ⇧⌃↓↑/细节三模式 文本-16进制-对齐/上-下与网页布局/主页卡片 hover 精修/书签 ⌘⌥⌃0-9/替换菜单 ⇧⌘F/忽略不重要差异/保存快照/比较文件使用视图切换 + UI 精修（等宽字体优先/CJK fallback/柔和 diff 配色/蓝色当前行竖条））** + **P40：工具栏精简（对标 BC 一行 6 组——DiffTab 34→~18 控件：剪贴板/编辑左/右/撤销重做/忽略×3/自动换行/显示空白/hex 选项/缩略图收进 Edit+View 菜单；DirTab include/exclude 收进过滤面板）** + **P41：DirTab 选择操作与过滤扩展（展开/折叠全部；视图过滤加仅左侧/右侧较新 LeftNewer-RightNewer（Differ+mtime）；多选集合 selected_set + 选择较新项/独有项/反向选择/全选/取消选择）** + **P42：文本比较编辑补齐与视图辅助（转换文件 Trim/Tabs/CRLF 作用于两侧（ConvertMode 纯函数复用 TextEdit/DiffTab）；剪贴板比较 ⌘V + 文本编辑打开剪贴板；字符列标尺；图例弹窗/日志面板/工具栏全局开关 SHOW_TOOLBAR）** + **P43：导航/选区/媒体（DirTab 导航历史 后退-前进-上一层-比较父文件夹；DiffTab 文本选区 当前差异块选为选区/选区→剪贴板→右侧；替换导航 下一-上一替换；差异文件导航 循环跳差异文件；会话菜单补齐 合并文件-和输出比较；信息弹窗 当前标签统计；媒体比较（简化版）WAV-MP3-FLAC 容器头元数据对比）** + **P44：剩余差距补齐（窗口菜单 ⌘]/⌘[ 切换标签-⌘⇧W 关闭所有-⌘M 最小化；文本比较快捷键 ⌘A 对齐-]/[ 缩进-⌘E 选区查找；文本合并冲突采用 ⇧←/⇧→-⌘B/⇧⌘B 顺序合并-⌘⇧⌃↓↑ 冲突导航；会话/文件菜单补齐 ⌥⌘O 打开会话-⌘R 重新比较-⌘⇧S 保存文件为-已锁定-打开方式（系统应用/在 Finder 显示）；工具菜单导出/导入设置-恢复出厂默认-编辑文本文件-查看补丁；视图开关 DiffTab 行号/语法加亮 + CsvTab ⇧⌃↩ 修改-⌘⌥⌃↩/⌥⌃↩ 前后插行-排序对话框；搜索补齐 DirTab 查找文件名 ⌘F-TextEdit 在多个文件中查找 ⌘⇧F）** + **P45：深层交互补齐（文本合并行级采用 ⌥⇧←/→ 采用左/右行-中心行；文件夹合并视图过滤 1-7 全部-更改-冲突-左变-右变-可合并-未变化；文件夹比较过滤扩展 独有-不独有-差异但无独有-左较新+左独有-右较新+右独有；图片比较 重置差异偏移-比较元数据弹窗；表格 后面插入列；HEX 复制到右边 ⇧⌃→；补丁 选择选择内容；文本编辑 使用选择内容查找 ⌘E）** + **P46：视图开关与导航补齐（TextEdit 行号/自动换行/文件信息开关；PatchTab 差异导航 ⇧⌥⌃↓↑-⇧⌃↓↑；hex 视图过滤 1/2/3 + 边并排/上-下布局；DirTab 结构选项 总是显示文件夹/仅比较文件；图例 ⇧L 快捷键；工作空间 保存/加载标签布局 TOML）** + **P47：优化与 UI 精修（CI 稳定性根治——Windows 构建依赖弃 chocolatey 改预装 cmake + 官方直链 nasm，消除反复 499/504 超时；工具栏图标化——Diff/TextEdit/Patch 打开-保存-转换按钮统一矢量符号；Diff 状态栏 BC 分区——路径弱色/彩色统计（相同绿-仅左红-仅右绿-修改黄）/编码·大小右对齐）**。

## 安装

### macOS

1. 从 [Releases](https://github.com/kevienkay/bcr/releases) 下载 `bcr-<版本>-macos-arm64.dmg`
2. 双击打开 dmg，将 `bcr.app` 拖入「应用程序」文件夹
3. 首次启动若被 Gatekeeper 拦截：右键 bcr.app →「打开」（个人项目未签名，属预期行为）
4. 命令行工具：`/Applications/bcr.app/Contents/MacOS/bcr --help`（可选：软链到 `~/.local/bin`）

### Windows

1. 下载 `bcr-<版本>-windows-x86_64.zip`，解压到任意目录（如 `C:\bcr`）
2. 双击 `bcr.exe` 启动 GUI，或打开命令行执行 `bcr.exe --help`
3. （可选）将目录加入 PATH，即可在任意终端使用 `bcr`

### Linux

**deb（Debian/Ubuntu）**：

```bash
sudo dpkg -i bcr-<版本>-linux-x86_64.deb
bcr --help
```

**tar.gz（通用）**：

```bash
tar xzf bcr-<版本>-linux-x86_64.tar.gz -C ~/.local/bin
bcr --help
```

> 校验完整性：下载后对照 Release 中的 `SHA256SUMS` 执行 `sha256sum -c`。

### 源码 / Cargo

```bash
cargo install --git https://github.com/kevienkay/bcr --tag v0.1.0
bcr --help
```

### Python 绑定（自动化）

```bash
pip install bcr            # PyPI（发布后）
# 或本地安装：
pip install ./bindings
```

Python 绑定 `bcr.py` 纯标准库，提供 `bcr.compare()/sync()/compare3()/csv()/merge()/mp3tag()/imgcmp()`
等类型化 API（详见 `docs/automation.md`）。

## 快速开始

### GUI（推荐）

```bash
bcr gui                      # 打开图形界面
bcr gui 左侧文件 右侧文件     # 直接对比两个文件
bcr gui --merge BASE LEFT RIGHT   # 三路合并
```

### 命令行

```bash
bcr diff 文件A 文件B          # 文本 diff（退出码 0=无差异 1=有差异 2=错误）
bcr compare 目录A 目录B       # 文件夹对比
bcr sync 目录A 目录B          # 目录同步（--dry-run 预览）
bcr merge3 BASE LEFT RIGHT -o OUT   # 三路文件夹合并
bcr csv 表A.csv 表B.csv       # CSV 表格对比
bcr compare 目录A 目录B --json # JSON 契约输出（自动化用）
```

常用选项：`--ignore-whitespace` 忽略空白、`--ignore-case` 忽略大小写、
`--compare-content` 内容比较、`--max-size` 大文件上限、`--lang zh|en|de|ja|...` 切换语言。
完整命令参考：`bcr --help` 与 `docs/automation.md`（自动化/JSON 契约）。

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

### P23 细节增强（WebDAV / 内容过滤 / 报告排序分组 / 拖放排序）

- **WebDAV 后端**：`webdav://` / `webdavs://` 可读写（PROPFIND/GET/PUT/DELETE/MKCOL/MOVE，Basic Auth，HTTP/HTTPS，PROPFIND depth=1 递归扫描）
- **内容过滤规则**：`bcr diff A B --ignore-lines <regex>` 忽略匹配正则的行（版本号/时间戳行等，可重复；GUI 并排视图共用）
- **报告排序/分组**：`--report-sort path|status|size` 排序（size = 差异大小降序）+ `--report-group` 状态分组（文本报告）
- **拖放排序**：GUI 多文件/目录拖入按文件名排序，行为可预测

### P24 特殊格式比较器（MP3 标签）

- `bcr mp3tag <left.mp3> <right.mp3>`：**字段级标签对比**（标题/艺术家/专辑/年份/流派/音轨/注释）
- 自研 ID3v1 + ID3v2 解析（UTF-8 / UTF-16 LE/BE / ISO-8859-1 编码，COMM 注释帧），无外部依赖
- 退出码：0=标签一致，1=有差异，2=错误；GUI 打开双 MP3 自动切换标签比较视图
- 例：`bcr mp3tag old.mp3 new.mp3 --show-same`

### P25 版本比较模式

- `bcr compare L R --compare-version`：按**文件版本号**判断差异（对标 BC 的 version 比较）
- 从 FileVersion / ProductVersion 字段提取版本号（ASCII 与 UTF-16LE），回退匹配内容中首个版本号模式
- 版本号按段数值比较（1.2.3 == 1.2.3.0，1.10 > 1.9）；无版本号的文件对回退快速模式（size+mtime）
- 适用：exe/dll/驱动等带版本资源的文件目录对比

### P26 GUI 云盘浏览

- GUI 工具栏「☁ 云盘」：输入远程目录 URL（webdav:// webdavs:// s3:// onedrive:// dropbox:// sftp:// ftp://）或本地路径
- 扫描列出顶层条目，点选进入子目录，确认后打开目录对比标签页（CLI/GUI 共用的 10 种后端全可用）

### P27 自动化（JSON 契约 + Python 绑定 + 任务清单）

- **全命令 `--json` 契约**：compare / sync / compare3 / csv / merge / mp3tag / imgcmp 输出版本化 schema JSON（`compare.v1` 等），stdout 只出 JSON，错误走 stderr，退出码 0/1/2 语义不变
- **Python 绑定 `bindings/bcr.py`**：纯标准库（零第三方依赖），类型化 dataclass 返回值，`bcr.compare()/sync()/compare3()/csv()/merge()/mp3tag()/imgcmp()` 全覆盖；`BCR_BIN` 环境变量指定二进制路径
- **任务清单 `bcr task`**：JSON/TOML 纯数据步骤清单（load/compare/compare3/csv/merge/sync/report/echo/exit），`%date%` `%time%` `%fn_time%` `%env:VAR%` `%1-%9` 动态变量，compare/sync 有差异不中止、遇错即停（或 continue_on_error），`bcr task check` 只校验不执行
- 完整参考：`docs/automation.md`（契约 + API + 场景示例）

### P32 UI 差距消除（对标 BC 4 全量盘点，A 类核心交互 + B 类次要差距）

- **差异连接线（A1）**：DiffTab 左右面板间空隙 + 差异行水平连接线（红/绿/黄按差异类型），单 ScrollArea 双栏渲染天然同步滚动
- **直接内联编辑（A2）+ 撤销/重做（A6）**：双击行就地编辑（Enter 提交 / ESC 取消）；编辑/替换入撤销栈（Ctrl+Z / Ctrl+Y，工具栏 ↩/↪）
- **右键菜单全覆盖（A4）**：DiffTab/CsvTab/ImageTab/MergeTab 行级 context_menu（复制路径/打开所在位置/系统打开/交换/忽略），跨平台文件定位 `reveal_in_file_manager`
- **差异块折叠（A5）**：块首 ▾ 折叠，占位行「N 行已折叠」点击展开
- **标记忽略差异（B5）**：右键忽略/取消忽略，从导航与统计排除（会话级）
- **会话类型起始页（A7）**：欢迎页网格卡片（文本/文件夹/三路合并/图片/CSV），独立会话入口
- **快捷键系统化（B1）**：F6/F7 差异跳转、F5 重载/刷新、F2 重命名（DirTab，vfs 跨后端）、Ctrl+W 关标签；tooltip 带快捷键提示
- **DirTab 过滤面板（B2）**：左侧可折叠 SidePanel（扩展名/大小范围/修改时间 YYYY-MM-DD），与工具栏联动
- **状态栏信息（B3）**：当前标签路径、行数/行列数、选中项数
- **标签拖拽重排（B6）**：标签栏拖拽换位，active 保持指向
- **hex 差异导航（B4）**：二进制 hex 视图补 F6/F7 差异行循环跳转

### P31 UI 精修（对标 BC 视觉）

- **主题引擎 `src/gui/theme.rs`**：集中视觉常量（行高 22/圆角 4/间距统一），差异配色对齐 BC（仅左红/仅右绿/修改黄）按深浅主题微调，启动时全局应用
- **主窗口**：底部全局状态栏（当前标签统计：Diff 行级/Dir 目录/Csv 表格/Merge 冲突/Image 帧差异）、标签栏美化（当前标签 strong + 关闭按钮 hover 红色）
- **DiffTab**：当前差异行 BC 风格左侧竖条标记
- **DirTab**：目录名文件夹色（浅蓝）、文件行状态徽标（圆形底替代纯文本 `[L]`）
- **CsvTab**：表头底色区分内容区
- **MergeTab**：冲突行左侧黄色竖条标记

### P29 CSV 表格 GUI（对标 BC 表格视图）

- **CsvTab 标签页**：并排渲染左右表格，行按主键（或行号）对齐，行级状态着色（相同/仅左/仅右/修改）
- **单元格级差异高亮**：修改的列左右两侧同时着色（左红右黄）
- **工具栏**：主键下拉（表头列名/行号对齐）、分隔符（`,`/`\t`）、显示相同开关、状态过滤下拉（全部/仅差异/仅左/仅右/仅修改/仅相同）
- **表头点击排序**：按任意列升/降序（纯显示排序，不改对齐数据）
- **入口路由**：目录对比双击/手动对齐/打开对比/拖放/CLI 启动，两侧均为 .csv/.tsv/.tab 自动进表格视图
- 结构化对比 API：`csvcmp::align_tables` 返回逐行状态 + 变化列（CLI 文本输出契约 csv.v1 不变）

### P28 全面对标收口（A/B/C 差距消除）

- **A1 三路文件夹合并 `merge3`**：`bcr merge3 BASE LEFT RIGHT -o OUT` —— 文本自动三路合并（冲突标记）、二进制冲突复制左侧并标记、单侧复制/删除，`--dry-run`/`--json`
- **A2 保存自动备份**：GUI 文本/hex 编辑保存前自动复制 `<name>.bak`
- **A3 剪贴板对比**：GUI「剪贴板→左/右」按钮（arboard 读系统剪贴板）
- **A4 文本替换**：GUI 查找升级为替换/全部替换（按原编码回写 + 自动备份）
- **A5 通用音频标签**：mp3tag 升级为魔数自动识别 MP3/FLAC/OGG/MP4/AAC（Vorbis comment + MP4 ilst 自研解析）
- **A6 FTPS**：`ftps://` implicit TLS 后端（suppaftp rustls，端口默认 990）
- **A7 忽略文件夹结构**：compare/sync `--ignore-structure` 按文件名跨目录对齐
- **A8 自动换行**：GUI 文本对比 word wrap（BC5 特性）
- **A9 转换后比较**：`diff --convert` 统一换行符（CRLF/CR→LF）再比较
- **A10 打印报告**：`compare --print` 调系统打印（lp/lpr/PowerShell Out-Printer）
- **A11 缩略图总览**：GUI 文本对比右侧迷你差异地图（按状态着色，点击跳转）
- **A12 CAB/ISO 归档**：`cab://`（纯 Rust）+ `iso://`（外部 7z/bsdtar）只读后端
- **A13 SVN 后端**：`svn://` 外部 svn 命令（list/cat/info，认证参数，未安装友好报错）
- **A14 第三方对比工具**：`~/.bcr-external.toml` 扩展名映射外部命令，`diff --external`
- **B1 显示过滤**：GUI 目录对比按状态过滤下拉（全部/差异/仅左/仅右/仅移动/仅相同）
- **B2 后台多任务**：GUI 对比/同步放后台线程（进度条 + 暂停/继续/取消）
- **B3 HTML 报告深度**：`--report-template` 自定义模板 + `--report-link-files` 差异条目链接文件级报告
- **B4 符号链接跟随**：compare/sync `--follow-symlinks`（Filter 开关 + 扫描跟随防死循环）
- **C1 大文件内存**：文本读取改 memmap2 只读映射，默认上限 64MB→256MB
- **C2 7z 可写**：sevenz-rust2 LZMA2 编码全量重写（write/delete/rename/set_mtime + 原子替换）
- **C3 SFTP host key 校验**：TOFU 首次保存 `~/.bcr-known-hosts` 后续校验（含 `~/.ssh/known_hosts` 通配匹配），`sftp+insecure://` 兼容跳过
- **C4 FTP mtime**：MFMT（RFC 3659）设置，服务器不支持静默降级

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

### M6 虚拟文件系统（`zip://` / `tar://` / `7z://` / `sftp://` / `ftp://` / `webdav://` / `s3://` / `onedrive://` / `dropbox://`）

- compare/sync 的路径参数支持虚拟后端，可跨后端混合对比：
  - `zip://path/to/archive.zip`：把 ZIP 压缩包当作目录树（可读写：write/delete/set_mtime 全量重写）
  - `tar://path/x.tar` / `tar://x.tar.gz` / `tar://x.tar.xz`：tar 及压缩变体（**可读写**：write/delete/rename/set_mtime 全量重写；tar.bz2 无纯 Rust 编码器保持只读）
  - `7z://path/x.7z`：7-Zip 压缩包（只读，全量解压进内存）
  - `sftp://[user[:pass]@]host[:port]/remote/path`：SFTP 远程目录（可读写，含 mtime 保留）
  - `ftp://[user[:pass]@]host[:port]/remote/path`：FTP 远程目录（可读写；无标准 mtime 设置命令，同步建议 `--compare-content`）
  - `webdav://[user[:pass]@]host[:port]/remote/path` / `webdavs://...`：WebDAV 远程目录（可读写：PROPFIND/GET/PUT/DELETE/MKCOL/MOVE，Basic Auth）
  - `s3://bucket[/prefix]`：Amazon S3 对象存储（可读写；凭证 AWS_ACCESS_KEY_ID/AWS_SECRET_ACCESS_KEY，区域 AWS_REGION，MinIO 用 AWS_ENDPOINT）
  - `onedrive://path`：OneDrive（可读写；token 用 BCR_ONEDRIVE_TOKEN 或 ~/.bcr-cloud.toml）
  - `dropbox://path`：Dropbox（可读写；token 用 BCR_DROPBOX_TOKEN 或 ~/.bcr-cloud.toml）
  - 普通路径仍为本地目录，可任意组合（本地 vs zip、本地 vs sftp、本地 vs s3、onedrive vs dropbox 等）
- 例：`bcr compare src/ "zip://backup.zip" --compare-content`、`bcr sync local/ "sftp://alice@nas/srv" --mode mirror --dry-run`、`bcr compare src/ "tar://backup.tar.gz"`、`bcr sync pub/ "ftp://mirror@files.example.com:/srv/pub" --mode update`、`bcr compare local/ "s3://my-bucket/backups" --compare-content`、`bcr sync docs/ "onedrive://backup" --mode update`
- 内部通过 [`Vfs`] trait 统一抽象（scan/read/write/delete/set_mtime），CLI 与 GUI 共用
- **云凭证配置**（OneDrive/Dropbox 需 OAuth access token）：环境变量 `BCR_ONEDRIVE_TOKEN` / `BCR_DROPBOX_TOKEN`，或 `~/.bcr-cloud.toml`：

```toml
[onedrive]
token = "eyJ0eXAiOiJKV1Qi..."

[dropbox]
token = "sl.B2i5..."
```
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

- 文本 diff 整文件读入内存，超过 `--max-size` 上限（默认 256MB，C1 mmap 优化）报错；超大文件内容比较走 P3 流式哈希
- 二进制文件已做检测：CLI diff/merge 报错 exit 2；GUI 自动切 hex 视图
- M5 目录对比的 glob 过滤在 GUI 中以逗号分隔输入；拖放仅支持本地文件
- M6 tar.bz2 后端只读（无纯 Rust 编码器）；tar/7z 全量解压进内存，超大归档建议 zip 或本地；FTP 无标准 mtime 设置命令（已用 MFMT 扩展，服务器不支持时静默降级，同步建议 `--compare-content`）；SVN/ISO/RAR 走外部命令（7z/bsdtar/svn），未安装时友好报错
- 快速模式依赖 mtime，跨文件系统/拷贝场景建议用 `--compare-content` 保证准确（与 BC 行为一致）
- M3 三处 stdin 不能同时用（`-` 只能出现一次）
- 与 git 的行为差异：两侧对**相邻行**的独立修改，bcr 按经典 diff3 语义无冲突合并，git 保守判冲突
- sync 快速模式下无法检测“mtime 相同但内容不同”，two-way 冲突检测需 `--compare-content`
- 语法高亮仅在 GUI 与 `diff --highlight` 生效；CLI compare/sync 输出无语法色
- SFTP host key 校验默认 TOFU（首次连接保存到 `~/.bcr-known-hosts`）；`sftp+insecure://` 可跳过
