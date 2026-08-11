# P28 差距消除方案 — 全面对标 Beyond Compare

> 背景：2026-08-12 差距分析（A 功能缺失 14 项 / B 部分差距 4 项 / C 工程限制 5 项），
> 殿下指示 A、B、C 全部消除。本文档为实施总方案，按批次推进，
> 每批完成即提交推送 + CI 三平台验证。

## 技术可行性核实（已确认）

| 事项 | 结论 |
|---|---|
| 7z 写入 | ✅ sevenz-rust2 0.7 提供 `en_funcs::compress/compress_to_path`，可写 |
| tar.bz2 写入 | ❌ bzip2-rs 仅解码无编码器 → 引入 `bzip2` crate（C 绑定）或保持只读 |
| FTPS | ✅ suppaftp 10 支持 `rustls` feature + `connect_secure` |
| FTP mtime | ✅ suppaftp `custom_command` 可发 MFMT（RFC 3659） |
| SFTP host key | ✅ russh 支持 known_hosts 校验 + 首次连接 TOFU 保存 |
| CAB/ISO 归档 | 待定：`cab` / `iso9660` crate（纯 Rust）；RAR 无纯 Rust 库 → 外部 unrar 命令或放弃 |
| 音频标签 | 自研：FLAC/OGG Vorbis comment（简单）、MP4 ilst atom（中等）、AAC 复用 ID3v2 前缀 |
| 大文件 diff | memmap2 已在依赖中 → mmap 只读映射 + 上限提升 |

## 批次划分与实施顺序

### 批次 1 — 低成本高频（GUI 为主）

- **A2 保存时自动备份**：GUI 文本/hex 编辑保存前，原文件先复制为 `<name>.bak`（BC 行为：保存前自动备份）
- **A3 剪贴板对比**：GUI「从剪贴板对比」按钮 + CLI `diff - file`（stdin 已有，补文档）；两剪贴板对比
- **A8 自动换行**：GUI 文本对比视图 word wrap 开关（BC5 特性）
- **B1 显示过滤**：GUI 目录对比工具栏加状态过滤下拉（全部/差异/仅左/仅右/新增/修改/匹配）
- **C4 FTP mtime**：FTP 后端 `set_mtime` 发 MFMT，失败静默降级

### 批次 2 — 文件夹合并核心（对标 Pro）

- **A1 三路文件夹合并 `merge3`**：`bcr merge3 BASE LEFT RIGHT -o OUT`——
  文本文件自动三路合并（复用 merge.rs 逻辑）、二进制/单侧文件直接复制、冲突标记输出；
  合并计划 dry-run；GUI 三路合并标签页联动
- **A7 忽略文件夹结构**：`--ignore-structure`，比较/同步时忽略目录层级按文件名对齐

### 批次 3 — 对比体验增强

- **A4 文本替换**：GUI 搜索框升级为查找+替换（替换单个/全部，CLI `diff` 不变）
- **A9 转换后比较**：`--convert` 选项（换行符归一 CRLF/LF/CR + 编码归一 UTF-8），比较前转换
- **A11 缩略图总览**：GUI 文本对比右侧迷你差异地图（点击跳转，BC thumbnail overview）
- **B4 符号链接跟随**：`--follow-symlinks` 选项（默认不跟随，跟随防死循环）

### 批次 4 — 报告与打印

- **A10 打印报告**：`compare --print` 调系统打印（macOS `lp` / Linux `lpr` / Windows 打印命令），
  失败时提示导出
- **B3 HTML 报告深度**：`--report-template <file>` 自定义 HTML 模板（占位符替换）；
  文件夹 HTML 报告每行差异链接到文件级 HTML 报告（`--report-link-files`）

### 批次 5 — 音频标签扩展

- **A5 FLAC/MP4/AAC/OGG 标签对比**：`bcr mp3tag` 扩展为通用音频标签比较器
  `bcr audiotag`（兼容旧命令名）：
  - FLAC/OGG：Vorbis comment 解析（`TITLE=xxx` 块）
  - MP4/M4A：ilst atom 解析（©nam/©ART/©alb 等）
  - AAC：ADTS 前 ID3v2 前缀（复用现有 ID3 解析）
  - 字段集对齐 BC：标题/艺术家/专辑/年份/流派/音轨/注释

### 批次 6 — 远程与安全

- **A6 FTPS**：`ftps://` 后端（implicit TLS，端口 990；`ftpes://` 显式 AUTH TLS 可选）
- **C3 SFTP host key 校验**：默认 TOFU——首次连接保存 host key 到 `~/.bcr-known-hosts`，
  后续校验；`--insecure` 跳过（兼容旧行为）；支持加载 `~/.ssh/known_hosts`
- **A13 SVN 后端**：`svn://` 走外部 `svn` 命令（list/cat/export 到缓存），未安装时报错提示
- **A14 第三方对比工具接入**：`~/.bcr-external.toml` 配置扩展名→外部命令，
  未知格式文件调外部工具对比（GUI 双击 + CLI `--external`）

### 批次 7 — 归档后端扩展

- **A12 CAB/ISO**：`cab://`（cab crate）、`iso://`（iso9660 crate）只读后端；
  RAR 用外部 `unrar`/`7z` 命令（未安装时报错提示）
- **C2 7z 可写**：sevenz-rust2 compress 实现 write/delete/set_mtime（全量重写+原子替换）；
  tar.bz2 引入 `bzip2` crate 实现写入（C 绑定，跨平台需预编译；若 CI 失败则保持只读并在 README 说明）

### 批次 8 — 性能与后台

- **C1 大文件内存优化**：文本 diff 改用 memmap2 只读映射（避免整文件拷贝），
  默认上限 64MB → 256MB（`--max-size` 仍可调）；README 更新已知限制
- **B2 后台多任务 + 暂停**：GUI 对比/同步放后台线程执行，进度条 + 暂停/继续/取消；
  多任务队列（可同时跑多个目录对比，BC 行为）

### C5 快速模式依赖 mtime

- 与 BC 行为一致（快速模式 mtime+size），非代码差距 → README 已知限制保留说明，不实施代码变更

## 质量门禁（每批）

1. `cargo test`（现有 319 + 新增用例）
2. `cargo clippy --all-targets -D warnings`（0 警告）
3. `cargo fmt --check`
4. 验收套件 tests/acceptance.sh + cross_platform.sh 全绿
5. 三平台 CI（ubuntu/windows/macos）
6. 每批 commit message 中文描述功能，如 `feat(P28a): ...`

## 风险与对策

| 风险 | 对策 |
|---|---|
| bzip2 crate 跨平台编译失败（C 绑定） | CI 先验证；失败则 tar.bz2 保持只读 + README 说明 |
| RAR 无纯 Rust 库 | 外部命令方案，未安装时友好报错（与 A14 共用机制） |
| merge3 与现有 merge 语义冲突 | 复用 compute_blocks/apply_regions 纯逻辑，新增独立 CLI 命令不破坏现有 |
| 音频标签格式差异大 | 字段缺失容错 + 二进制安全解析（防 panic） |
| GUI 后台线程与 egui 交互 | 线程只做计算，结果经 channel 回传 UI 线程；暂停用 AtomicBool |
