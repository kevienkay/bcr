# P0 / P1 设计：编码检测、二进制检测、语法高亮、大文件支持

> 状态：设计稿 v1（2026-08-09）
> 范围：P0 = 编码检测 + 二进制检测；P1 = 语法高亮 + 大文件流式
> 原则：保持 Vfs trait 抽象不动（已返回 `Vec<u8>`，天然适合在此之上做解码层）；
> 所有新增能力在解码层统一接入，CLI 与 GUI 共用。

---

## P0：编码检测 + 二进制检测

### 0.1 现状与问题

| 位置 | 现状 | 问题 |
|---|---|---|
| `src/diff.rs::read_input` | `fs::read_to_string` | 非 UTF-8 直接报错 |
| `src/merge.rs::read_input` | `fs::read_to_string` | 同上 |
| `src/gui/difftab.rs::load_*` | `fs::read_to_string` | 同上 |
| `src/sideview.rs::build_rows` | 入参 `&str` | 上层解码，本身不需要改 |

中文环境（GBK 文件）、Windows 产物（UTF-16 带 BOM）、二进制文件（图片/压缩包）
目前全部无法处理或产生乱码。这是与 Beyond Compare 的最大差距。

### 0.2 设计

#### 新增模块 `src/encoding.rs`

```
pub enum EncodingKind { Utf8, Utf16Le, Utf16Be, Utf32Le, Utf32Be, Gbk, Big5, ShiftJis, Latin1, Other(&'static str) }

pub struct TextFile {
    pub text: String,
    pub encoding: EncodingKind,
    pub is_binary: bool,
    pub had_bom: bool,
}

pub fn detect(data: &[u8]) -> Detection;   // 嗅探：BOM → UTF-8 验证 → 多字节编码 → 二进制判定
pub fn read_text(path: &str) -> io::Result<TextFile>;        // 本地/虚拟后端统一入口
pub fn read_text_bytes(data: &[u8]) -> TextFile;             // 纯字节解码（测试/复用）
```

**检测顺序（确定性，不靠概率猜）**：

1. **BOM 嗅探**：`EF BB BF`(UTF-8)、`FF FE`(UTF-16LE)、`FE FF`(UTF-16BE)、`FF FE 00 00`(UTF-32LE)、`00 00 FE FF`(UTF-32BE)
2. **严格 UTF-8 验证**：`std::str::from_utf8` 全量通过 → UTF-8
3. **二进制判定**（在非 UTF-8 时先做）：
   - 前 8192 字节中 NUL 占比 ≥ 1% → 二进制
   - 或存在非文本控制字符（C0 除 `\t\n\r\f\b` 外）
4. **多字节编码尝试**（有启发式、可被 `BCR_ENCODING` 环境变量覆盖）：
   - UTF-16 无 BOM：偶/奇地址 NUL 分布判定 LE/BE
   - GBK/GB18030：`encoding_rs` 严格解码，替换字符比例 < 阈值则接受
   - Big5 / Shift_JIS 同法尝试
5. 全部失败 → Latin-1（永不失败，保底不 panic）

**依赖**：`encoding_rs`（WHATWG 标准实现，纯 Rust，GBK/GB18030/Big5/Shift_JIS/UTF-16 全覆盖，解码不可失败的 API 是 `decode_without_bom_handling`）+ `chardetng`（Mozilla 检测器，与 encoding_rs 同作者配套，用于第 4 步兜底增强）。两个都是轻量纯 Rust crate。

#### 接入点改造

| 位置 | 改动 |
|---|---|
| `src/diff.rs::read_input` | 改为 `encoding::read_text(path)`；`is_binary` → 输出 `bcr: 二进制文件: <path>` 并 exit 2；否则用 `text` 继续 |
| `src/merge.rs::read_input` | 同上（二进制文件禁止合并，exit 2） |
| `src/gui/difftab.rs::load_pair/load_left/load_right` | 改用 `read_text`；二进制 → 状态栏提示"二进制文件，不支持文本视图"；成功 → 保存 `TextFile`（含 encoding），编辑保存时按原编码回写 |
| `src/vfs/mod.rs` | 不动（read 已返回字节） |
| `src/sideview.rs` | 不动（入参已是 `&str`） |

**CLI 参数**：新增全局 `--encoding <name>`（`bcr --encoding gbk diff a b`），显式指定时跳过检测第 2-4 步；`BCR_ENCODING` 环境变量同样生效（与 `BCR_LANG` 风格一致）。

#### 测试计划（`tests/encoding.rs` 或模块内单测）

- [ ] UTF-8 无 BOM / 带 BOM 正确识别
- [ ] UTF-16LE/BE 带 BOM 与无 BOM（NUL 分布判定）
- [ ] GBK 中文文件正确解码（构造字节序列）
- [ ] 二进制：PNG 头、NUL 密集文件 → `is_binary=true`
- [ ] `--encoding` 强制覆盖
- [ ] GUI 编辑保存编码 round-trip（GBK 文件编辑后仍为 GBK）

### 0.3 工作量

- `src/encoding.rs` ≈ 350 行（含测试）
- 接入点改动 ≈ 150 行
- 依赖 +2（encoding_rs, chardetng），无编译负担

---

## P1：语法高亮

### 1.1 目标

GUI 并排 Diff / 三路合并视图按语言着色；CLI 可选 `--highlight` 输出 ANSI 语法色。
与现有"diff 语义高亮"（changed 背景）分层共存：**语法色管前景，diff 高亮管背景**（与 BC 一致）。

### 1.2 方案选型

| 候选 | 优点 | 缺点 | 结论 |
|---|---|---|---|
| **syntect** | 纯 Rust；Sublime 语法集 60+ 语言；一次加载全局复用；与 egui 集成成熟 | 首次编译 +30~60s；语法集体积 | ✅ 选它 |
| tree-sitter | 精确 AST | 每种语言一个 crate，重，接入成本高 | ❌ 对比场景杀鸡用牛刀 |

`syntect` 配置：`default-features = false, features = ["default-syntaxes", "default-themes"]`。

### 1.3 设计

#### 新增模块 `src/highlight.rs`

```
static SYNTAXES: OnceLock<SyntaxSet>;
static THEMES: OnceLock<ThemeSet>;

pub fn parse_syntax(path: &str) -> Option<&SyntaxReference>;   // 按扩展名/文件名，无匹配 → None（纯文本）
pub fn highlight_line(line: &str, syntax: &SyntaxReference, theme: &Theme) -> Vec<(usize, usize, Color32)>;
// 返回 (byte_start, byte_len, color)，GUI 拼 LayoutJob；CLI 拼 ANSI 序列
```

**GUI 接入**（`src/gui/common.rs::paint_cell`）：
- 行渲染时先取语法分段前景色，再叠 diff 语义背景（changed 高亮不受影响）
- 虚拟化只对可见行调用 `highlight_line`，性能无压力
- 文件编辑/重新加载时 SyntaxReference 缓存失效重建（按 (path, mtime) 缓存即可）

**CLI 接入**（`src/render.rs`，可选）：
- `--highlight` 开关，默认关（保持现有输出完全兼容）
- ANSI 256 色映射：语法色 → `\x1b[38;5;Nm`，与现有红/绿 diff 标记叠加时语法色让位

#### 测试计划

- [ ] 常见语言（rs/py/js/ts/go/json/yaml/md）高亮不 panic、输出分段拼回原文一致
- [ ] 无扩展名/未知类型 → 回退纯文本
- [ ] diff changed 背景与语法前景共存（GUI 无头测试断言 LayoutJob 分段）
- [ ] CLI `--highlight` 输出含 ANSI 且 diff 退出码不变

### 1.4 工作量

- `src/highlight.rs` ≈ 200 行
- GUI 接入 ≈ 120 行；CLI 接入 ≈ 80 行
- 依赖 +1（syntect），三端 CI 编译时间增加约 30-60s（可接受，不切 feature gate）

---

## P1：大文件支持

### 2.1 现状

所有文本路径都是 `read_to_string` 全量读入 + `similar` 全量行 diff。超大文件（>100MB）
内存翻倍（原文 + 行集合 + diff ops），存在 OOM 风险。目录对比快速模式不受影响（只读元数据）。

### 2.2 分层方案（务实地分阶段，不一次做真流式）

| 阶段 | 内容 | 说明 |
|---|---|---|
| **A（必做）** | 大小阈值保护 | 读文件前 `metadata.len()`，> 阈值（默认 64MB，`--max-size` 可调）→ CLI 报错提示"文件过大，请用 compare"；GUI 提示并拒绝加载。防 OOM 底线 |
| **B（推荐）** | mmap 读取本地文件 | `memmap2` 已在依赖树（eframe 传递依赖），提升为直接依赖即可；本地后端 `read` 改为 mmap + `copy_to_slice`，减少一次内核拷贝。zip/sftp 保持整读 |
| **C（远期，不在本次）** | 真流式/窗口 diff | 需要对 similar 做块级拆分或换增量算法，工作量数倍于 A+B，收益仅在超大文件场景，暂缓 |

### 2.3 改动点

- `src/vfs/mod.rs`：`LocalVfs::read` 改用 memmap（`unsafe { Mmap::map(&file) }` → 复制到 Vec，注意生命周期）；其他后端不动
- `src/diff.rs` / `src/merge.rs` / `src/gui/difftab.rs`：加载前统一走 `ensure_size_ok(path)` 检查（阈值从 `--max-size` 或常量取）
- CLI 错误消息 + GUI 状态栏提示文案进 i18n 表（新 Key 需补全 10 语言，翻译宏保证穷尽）

### 2.4 测试计划

- [ ] 阈值内正常；超阈值 CLI exit 2 + 提示
- [ ] `--max-size` 可调（测试用 1KB 阈值 + 2KB 文件）
- [ ] mmap 读取与普通读取结果逐字节一致（本地后端）
- [ ] GUI 超阈值拒绝加载提示

### 2.5 工作量

- 阈值保护 ≈ 80 行；mmap 改造 ≈ 60 行；依赖 +0（提升 memmap2）
- i18n 新文案：约 4 个 Key × 10 语言

---

## 依赖变更汇总

| crate | 用途 | 影响 |
|---|---|---|
| `encoding_rs` | 编码解码（GBK/UTF-16 等） | 纯 Rust，轻 |
| `chardetng` | 非 UTF-8 编码检测兜底 | 纯 Rust，轻 |
| `syntect` | 语法高亮 | 编译 +30-60s |
| `memmap2` | 大文件 mmap | 已在依赖树，零新增 |

## 风险与对策

1. **编码误判**：二进制/编码边界模糊 → 检测结果可被 `BCR_ENCODING` / `--encoding` 强制覆盖；判定阈值用测试固化
2. **syntect 编译时间**：CI 三端各 +30-60s → 接受；若在意可后续切 feature gate
3. **GUI 编辑保存编码 round-trip**：P0 先保证"读取正确 + 保存按原编码回写"，UTF-8 文件行为与现在完全一致
4. **mmap 安全性**：只读映射 + 立即复制为 Vec，不长期持有映射（避免文件被外部修改导致的 SIGBUS 面）

## 建议实施顺序

1. `encoding.rs` + 接入 + 测试（P0 完整）
2. 大文件 A（阈值保护，小改动先落地）
3. 大文件 B（mmap）
4. `highlight.rs` + GUI 接入
5. CLI `--highlight` + i18n 文案 + 三端 CI 验证
