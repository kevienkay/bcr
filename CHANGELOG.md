# Changelog

bcr — Beyond Compare 风格的文件对比工具（Rust 实现）。本文件遵循
[Keep a Changelog](https://keepachangelog.com/zh-CN/1.0.0/) 格式，
版本号遵循 [SemVer](https://semver.org/lang/zh-CN/)。

## [Unreleased]

## [0.2.0] - 2026-08-12

### UI 差距消除（P32，对标 Beyond Compare 4 全量盘点，A类核心交互 + B类次要差距）

- **差异连接线（A1）**：DiffTab 左右面板间空隙 + 差异行水平连接线（红/绿/黄按差异类型着色），等行弱色分隔；单 ScrollArea 双栏渲染天然同步滚动（A3）
- **直接内联编辑（A2）+ 撤销/重做（A6）**：DiffTab 双击行就地编辑（Enter 提交 / ESC 取消）；编辑/替换入撤销栈（Ctrl+Z / Ctrl+Y 或 Ctrl+Shift+Z，工具栏 ↩/↪ 按钮）
- **右键菜单全覆盖（A4）**：DiffTab/CsvTab/ImageTab/MergeTab 全部行级 context_menu（复制路径/打开所在位置/系统打开/交换左右/忽略差异），DirTab 扩展；`open_with_system_app` 与 `reveal_in_file_manager` 跨平台（macOS open / Windows explorer / Linux xdg-open）提升到 common 复用
- **差异块折叠（A5）**：DiffTab 差异块行首 ▾ 折叠箭头，折叠后隐藏中间行显示「N 行已折叠」占位，点击展开
- **标记忽略差异（B5）**：差异行右键「忽略此行/此块」→ 从导航/统计排除（会话级，右键可取消），弱化显示
- **会话类型起始页（A7）**：欢迎页扩展为网格卡片（文本对比/文件夹对比/三路合并/图片对比/CSV 表格），新增 `open_image_compare` / `open_csv_compare` 独立会话入口，10 语言 i18n
- **快捷键系统化（B1）**：DiffTab F6 下一差异/F7 上一差异（循环跳转，修正索引语义）、F5 重载；DirTab F2 重命名（vfs 跨后端弹窗）+ F5 刷新；全局 Ctrl+W 关闭当前标签；按钮 tooltip 带快捷键提示
- **DirTab 过滤/显示面板（B2）**：左侧可折叠 SidePanel——扩展名（逗号分隔）、大小范围（字节）、修改时间范围（YYYY-MM-DD，自研公历转换）过滤，与工具栏联动，清除/计数显示
- **状态栏信息（B3）**：底部状态栏补当前标签路径（DiffTab 文件名）、行数/行列数（DiffTab 行数、CsvTab 行×列）、选中项数（DirTab 选中 x/y）
- **标签拖拽重排（B6）**：标签栏拖拽换位（`move_tab` 保持 active 指向原标签）
- **独立 Hex 差异导航（B4）**：二进制自动切换 hex 对比视图已有基础上补 F6/F7 差异行循环跳转（滚动定位，`show_rows_offset` 支持初始偏移）
- **工具栏图标细节（B7）**：按钮 tooltip 统一补快捷键提示（如「⟳ 重新加载 (F5)」「下一差异 (F6)」）
- **质量**：每批新增 uikit 测试（egui_kittest 驱动真实标签页交互，含键盘事件），累计 392+ 测试全绿；clippy 0 警告；fmt 干净

### UI 精修（P31，对标 Beyond Compare 视觉）

- **主题引擎**：新增 `src/gui/theme.rs`——集中视觉常量（行高 22、圆角 4、间距统一），差异配色对齐 BC（仅左红/仅右绿/修改黄）按深浅主题微调，启动时对 Dark/Light 两套主题全局应用
- **主窗口**：底部全局状态栏（当前标签统计：Diff 行级 / Dir 目录 / Csv 表格 / Merge 冲突 / Image 帧差异）；标签栏美化（当前标签 strong 高亮 + 关闭按钮 hover 变红）
- **DiffTab**：当前差异行 BC 风格左侧竖条标记（CURRENT_BAR 3px，黄色）
- **DirTab**：目录名文件夹色（浅蓝，深浅主题区分）；文件行状态徽标（圆形底色 + 状态字母，替代纯文本 `[L]`）
- **CsvTab**：表头底色与内容区区分；新增 `stats()` 访问器供状态栏
- **MergeTab**：冲突行左侧黄色竖条标记

### 优化（按差距分析优先级推进）

- **归档内存保护**：tar/7z 全量解压加上限（默认 1 GiB，`BCR_MAX_ARCHIVE_SIZE` 环境变量可调），超限报 `OutOfMemory` 错误而非 OOM 崩溃；`read_tar_with_limit` 支持注入上限（测试用）
- **benchmarks**：新增 `benches/core.rs`（criterion，黑盒 CLI 方式）——文本 diff（1K/10K/50K 行）、文件夹对比（100/1K/5K 文件）、CSV 对齐（1K/10K 行）、同步计划（100/1K 文件）；`cargo bench` 可运行，`cargo bench --bench core -- --test` 验证模式
- **测试稳定性修复**：`dirtab_collapse_hides_children` Windows 偶发失败（默认 `ViewFilter::Diff` 把两侧相同文件滤掉 → flat 为空 → `unwrap()` panic）——测试显式设置 `ViewFilter::All`，消除平台时序依赖
- **基线数据**（本机 arm64）：diff 1K 行 ≈6.8ms / 10K ≈129ms / 50K ≈2.57s；compare 100 文件 ≈57ms

### 文档（P30 收尾）

- **README 新增「安装」章节**：macOS dmg（拖入应用程序/未签名说明）、Windows zip（解压+PATH）、Linux deb/tar.gz（dpkg -i / 解压到 ~/.local/bin），含 SHA256SUMS 校验指引；补充 cargo install（git tag）与 Python 绑定 pip 安装方式
- **README 新增「快速开始」章节**：GUI 与命令行常用用法（diff/compare/sync/merge3/csv/--json），常用选项说明
- **README 已知限制修正**：删除“不处理 No newline 标记”过期条目（P16 已实现 `\ No newline at end of file`，与功能列表矛盾）

### 分发（P30 扩展）

- **应用图标**：新增 `assets/icon.png`（1024px，PIL 生成“双面板差异条”设计）+ `bcr.icns`（iconutil，macOS）+ `bcr.ico`（多尺寸，Windows）
- **macOS 打包**：Info.plist 加 CFBundleIconFile，icns 拷入 Resources
- **Linux 打包**：deb 含 256px 图标（/usr/share/icons/hicolor）+ bcr.desktop 桌面入口（Exec=bcr gui）
- **Windows 打包**：zip 内含 bcr.ico

## [0.1.0] - 2026-08-11

首个里程碑版本：完整对标 Beyond Compare 核心场景的对比/合并/同步工具。

### 里程碑 M1-M6（核心）

- **M1 文本 diff**：unified 输出 + 行内高亮（Myers/Patience 双算法）+ 忽略空白/行尾/大小写 + git 兼容退出码
- **M2 文件夹对比**：walkdir 递归 + blake3 深度模式 + glob 过滤 + 状态输出
- **M3 三路合并**：diff3 归并 + git 风格冲突标记 + git mergetool 兼容
- **M4 同步引擎**：update/mirror/two-way 三模式 + dry-run 预览 + mtime 保留
- **M5 GUI**：egui 桌面应用（并排 Diff/目录树/三路合并多标签）+ 虚拟化渲染 + 搜索/跳转 + 主题/设置持久化 + Git 集成
- **M6 虚拟文件系统**：Vfs trait 统一抽象 + 跨后端混合对比

### P0-P7（增强）

- **P0** 编码检测与二进制检测（GBK/UTF-16/UTF-32/BOM + 二进制识别）
- **P1** 语法高亮（syntect）+ 二进制 hex 对比（CLI `bcr hex` + GUI 自动切换）
- **P2** 重命名/移动检测（内容哈希匹配 `[M]` 标记）
- **P3** 分块流式读取（大文件内存 O(64KB)）
- **P4** HTML 对比报告 + 会话保存/恢复（session save/list/run/delete）
- **P5** 同步增强：mirror/update 移动检测 `[MOVE]` + mirror 空目录清理 `[RMDIR]`
- **P6** 三路文件夹对比（compare3，10 状态标记）
- **P7** CSV/表格对比（主键/行号对齐、列级 diff、引号字段解析）

### P8-P16（GUI/工作流完善）

- **P8** 图片对比（imgcmp 逐像素 diff + GUI ImageTab 并排/叠加/缩放；GIF/WebP 多帧导航 + 缩略图条 + fit-to-window + 🎯 差异区域定位）
- **P9** GUI 同步操作面板（build_plan/execute_op 纯逻辑接口 + 同步向导 + 单项操作）
- **P10** 比较规则 Profile（save/list/delete + `--profile` 合并 + GUI 规则面板）
- **P11** 报告格式（--txt/--csv）+ 字段定制（--report-fields）
- **P12** 文件属性与符号链接（--compare-attrs）
- **P13** hex 编辑（GUI 二进制视图双击行编辑）
- **P14** 细节打磨（自动刷新 2s / 手动对齐 / 批量复制/删除）
- **P15** 文本 diff 细节（`\ No newline at end of file` + --ignore-crlf）
- **P16** 会话收藏（★ + 最近使用排序）

### P17-P23（性能/远程/报告深度）

- **P17** 比较结果缓存（size+mtime 快照，LRU 64 条，命中秒开）
- **P18** 报告布局模板（--report-title / --report-no-stats）
- **P19** Profile export/import（迁移/分享）
- **P20** 目录对比右键菜单（复制路径/系统应用打开）
- **P21** 内容过滤规则（--ignore-lines 正则忽略差异行）
- **P22** 报告深度定制（--report-sort / --report-group 状态分组）
- **P23** GUI 拖放排序

### P24-P27（特殊格式 / 版本比较 / 云盘浏览 / 自动化）

- **P24** MP3 标签比较器（自研 ID3v1/v2 解析，UTF-8/UTF-16/ISO-8859-1，`bcr mp3tag` 字段级差异）
- **P25** 版本比较模式（--compare-version，从 FileVersion/ProductVersion 提取，UTF-16/ASCII，段数值比较）
- **P26** GUI 云盘浏览（☁ 窗口输入 webdav/s3/onedrive/dropbox/sftp/ftp URL，扫描并打开目录对比）
- **P27** 自动化：全命令 `--json` 契约（compare.v1/sync.v1/compare3.v1/csv.v1/merge.v1/mp3tag.v1/imgcmp.v1）+ Python 绑定 `bindings/bcr.py`（纯标准库 dataclass API）+ 纯数据任务清单 `bcr task run/check`（JSON/TOML，%date%/%time%/%fn_time%/%env:VAR%/%1-%9 动态变量，差异不中止语义）

### P29（CSV 表格 GUI）

- **CsvTab 标签页**：并排渲染左右表格，行按主键/行号对齐，行级状态着色 + 单元格级差异高亮（左红右黄）
- **工具栏**：主键下拉、分隔符（`,`/`\t`）、显示相同、状态过滤（全部/仅差异/仅左/仅右/仅修改/仅相同）
- **表头点击排序**（升/降，纯显示排序）+ 统计栏
- **入口路由**：目录双击/手动对齐/打开对比/拖放/CLI 启动自动识别 .csv/.tsv/.tab 进表格视图
- **结构化 API**：`csvcmp::align_tables` 返回逐行状态 + 变化列（RowStatus/AlignedRow）；CLI 文本输出契约 csv.v1 不变
- i18n：10 语言新增 CsvTitle/CsvKeyCol/CsvDelimiter/CsvRowAlign/CsvFilter* 条目

### P28（全面对标收口，A/B/C 差距消除）

- **A1** 三路文件夹合并 `merge3`（BASE/L/R → 输出目录，文本自动三路合并 + 冲突标记，--dry-run/--json）
- **A2** 保存自动备份（GUI 编辑保存前生成 .bak）
- **A3** 剪贴板对比（GUI 剪贴板→左/右，arboard 跨平台）
- **A4** 文本替换（GUI 查找升级为替换/全部替换，按原编码回写）
- **A5** 通用音频标签（mp3tag 魔数自动识别 MP3/FLAC/OGG/MP4/AAC，Vorbis comment + MP4 ilst 自研解析）
- **A6** FTPS 后端（ftps:// implicit TLS，端口默认 990）
- **A7** 忽略文件夹结构（compare/sync --ignore-structure 按文件名跨目录对齐）
- **A8** 自动换行（GUI word wrap，BC5 特性）
- **A9** 转换后比较（diff --convert 统一换行符）
- **A10** 打印报告（compare --print，lp/lpr/PowerShell Out-Printer）
- **A11** 缩略图总览（GUI 文本对比右侧迷你差异地图，点击跳转）
- **A12** CAB/ISO 归档后端（cab:// 纯 Rust + iso:// 外部 7z/bsdtar）
- **A13** SVN 后端（svn:// 外部 svn 命令，只读）
- **A14** 第三方对比工具（~/.bcr-external.toml 扩展名映射，diff --external）
- **B1** 目录状态过滤（GUI 下拉：全部/差异/仅左/仅右/仅移动/仅相同）
- **B2** 后台多任务（GUI 对比/同步后台线程 + 进度条 + 暂停/继续/取消）
- **B3** HTML 报告深度（--report-template 模板 + --report-link-files 文件级链接）
- **B4** 符号链接跟随（compare/sync --follow-symlinks）
- **C1** 大文件内存优化（memmap2 只读映射，默认上限 64MB→256MB）
- **C2** 7z 可写（sevenz-rust2 LZMA2 全量重写 + 原子替换）
- **C3** SFTP host key 校验（TOFU 保存 ~/.bcr-known-hosts，sftp+insecure:// 兼容跳过）
- **C4** FTP mtime（MFMT 扩展设置，不支持时静默降级）

### 后端扩展（M6b-M6h）

- **M6b** tar/tar.gz/tar.bz2/tar.xz/7z 只读后端（`tar://` / `7z://`）
- **M6c** ZIP 后端可写（write/delete/set_mtime/rename 全量重写+原子替换）
- **M6d** FTP 后端（可读写，匿名登录/被动模式）
- **M6e** tar 可写（tar/tar.gz/tar.xz 全量重写）
- **M6f** WebDAV 后端（PROPFIND/GET/PUT/DELETE/MKCOL/MOVE，Basic Auth）
- **M6g** Amazon S3 后端（`s3://`，rust-s3，MinIO endpoint）
- **M6h** OneDrive（Graph API）+ Dropbox（Dropbox API）云存储后端

### 工程/质量

- **I18N**：10 语言支持（zh/en/de/ja/ko/es/pt/ar/ru/fr），翻译表宏保证穷尽
- **CI**：GitHub Actions 三端矩阵（ubuntu/windows/macos）fmt/clippy 门禁 + 验收测试 + 跨平台专项
- **验收套件**：142 用例（tests/acceptance.sh + cross_platform.sh）
- **单元测试**：275 用例

### 文档

- README：完整功能矩阵 + 使用示例 + 云凭证配置说明
- docs/P0-P1-design.md：编码检测/语法高亮设计稿
- docs/P27-python-binding-design.md：自动化方案 v3 设计稿
- docs/automation.md：自动化指南（JSON 契约参考 + bcr.py API + 任务清单 + 场景示例）

[0.1.0]: https://github.com/kevienkay/bcr/releases/tag/v0.1.0
[0.2.0]: https://github.com/kevienkay/bcr/releases/tag/v0.2.0
