# P29 CSV 表格 GUI（对标 Beyond Compare 表格视图）

## 背景

P28 已将 A/B/C 23 项差距清零，剩余差距中**唯一有实质价值的是 CSV 表格 GUI**：
BC 5 的表格对比以**并排表格视图**呈现（行对齐、单元格级差异着色、可排序/筛选），
而 bcr 目前只有 CLI 文本输出（`bcr csv`），GUI 双击 CSV 文件走的是普通文本 diff。

## 目标

新增 `CsvTab` 标签页，对标 BC 表格对比：

- 并排渲染左右两张表（表头 + 数据行），行按主键或行号对齐
- **单元格级差异高亮**：修改的列在左右两侧同时着色（左红右黄，同文本 diff 语义）
- **行级状态着色**：`[L]` 仅左 / `[R]` 仅右 / `[M]` 修改 / `[S]` 相同
- 工具栏：
  - 对齐主键下拉（表头列名选择，复用 `--key` 语义）
  - 分隔符选择（`,` / `\t` / 自定义，复用 `--delimiter`）
  - 显示相同行开关（复用 `--show-same`）
  - 状态过滤下拉（全部/差异/仅左/仅右/修改/相同，复用 B1 ViewFilter 模式）
- **表头点击排序**（BC 表格视图核心交互；纯显示排序不改数据）
- 统计栏（相同/仅左/仅右/修改，复用 RowStats）

## 设计

### 1. csvcmp 结构化对比 API（批次 1）

现有 `compare_csv` 返回**渲染文本行**（`Vec<String>`），GUI 无法复用。
新增结构化 API，文本输出路径保持不变（CLI 契约不动）：

```rust
/// 行对齐结果（供 GUI 渲染）
pub(crate) struct AlignedRow {
    pub a_no: Option<usize>,   // 左侧行号（None = 仅右侧）
    pub b_no: Option<usize>,   // 右侧行号（None = 仅左侧）
    pub status: RowStatus,     // Same / LeftOnly / RightOnly / Modified
    pub changed_cols: Vec<usize>, // 修改的列索引（Modified 时）
}

pub(crate) enum RowStatus { Same, LeftOnly, RightOnly, Modified }

/// 结构化对比：按 key（或行号）对齐两表，返回逐行状态 + 变化列
pub(crate) fn align_tables(
    a: &Table, b: &Table, key: Option<&str>,
) -> (Vec<AlignedRow>, RowStats)
```

- `Table` / `parse_csv` / `key_of` 改为 `pub(crate)` 供 GUI 使用
- 内部复用现有对齐逻辑（key 对齐 + 行号对齐），仅改变输出形态
- 单元测试：对齐正确性（key 对齐/行号对齐/仅左/仅右/修改列集合）

### 2. CsvTab 标签页（批次 2，新增 `src/gui/csvtab.rs`）

```rust
pub(crate) struct CsvTab {
    left: String, right: String,
    table_a: Option<crate::csvcmp::Table>,
    table_b: Option<crate::csvcmp::Table>,
    aligned: Vec<AlignedRow>,   // 对比结果
    stats: RowStats,
    key: String,                // 当前主键（空 = 行号对齐）
    delimiter: String,          // "," 或 "\t"
    show_same: bool,
    filter: ViewFilter,         // 复用 dirtab 的枚举
    sort_col: Option<(usize, bool)>, // 排序列 + 升/降
}
```

- `new(left, right)`：读文件 → 解析 → 对齐（后台线程，复用 B2 模式？先同步，CSV 一般不大；若大文件再后台）
- `ui(ui)`：
  - 工具栏（key 下拉、分隔符、显示相同、过滤下拉、重新对比按钮）
  - 表头行：列名按钮（点击排序，带 ▲/▼ 指示）；行号列 + 状态列
  - 数据行：`ScrollArea` 虚拟化渲染（复用 dirtab 的 flat 模式）
  - 单元格：Modified 的列左右双色高亮；LeftOnly 行整行左色；RightOnly 整行右色
  - 统计栏
- 状态过滤与排序在渲染层做（不重排数据）

### 3. GUI 接线（批次 3）

- `Tab` 枚举加 `Csv(CsvTab)`；`title()` / `ui()` 分发补分支
- **打开入口**（对标图片路由）：
  - DirTab 双击/打开对比时：若两侧均为 `.csv`/`.tsv` 且非图片 → `CsvTab` 而非 DiffTab
  - `open_diff` / `open_pair` 路由处加扩展名判断（`is_csv_file()`）
  - 拖放：两个 CSV 文件 → CsvTab
- i18n：新增 `CsvTabTitle`、`CsvCol/Status/Key/Delimiter/Filter` 等 Key（zh/en 双语表）
- README 完成列表补 P29 + 已知限制调整；CHANGELOG 补 P29 章节

## 批次计划

| 批次 | 内容 | 测试 |
|---|---|---|
| 1 | csvcmp 结构化 API（align_tables + RowStatus）+ pub(crate) 暴露 | 单元测试：对齐/状态/变化列 |
| 2 | CsvTab 标签页（渲染/工具栏/排序/过滤/高亮） | 无头测试：构建/过滤/排序/统计 |
| 3 | GUI 接线（Tab 枚举/入口路由/i18n/README/CHANGELOG） | GUI 头测：路由、标题、i18n |

每批完成即跑质量门禁（cargo test + clippy -D warnings + fmt）并提交推送。

## 风险与取舍

- CSV 大文件：先同步解析；超 256MB 文本走 `--max-size` 保护（复用 encoding 读文件）
- 列排序只影响显示顺序，行对齐基于原始数据——与 BC 行为一致
- 单元格差异高亮用整格背景色（egui Table 无内联 diff 需求），不做字符级 diff
- CLI `bcr csv` 文本输出契约（csv.v1 JSON）完全不动
