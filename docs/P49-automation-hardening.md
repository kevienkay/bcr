# P49 自动化补强 实施记录

用户指定做「自动化补强（P27 契约扩展新视图）+ kittest 菜单真点验证」，1 个功能提交 + 1 个 docs 收尾，全部推送 `origin/master`。

## P49-2 P27 契约扩展新视图（`e700e31`）

调研确认契约缺口：`bcr diff`/`bcr hex` 无 `--json`（已有契约：compare/compare3/sync/imgcmp/mp3tag/csv/merge）；媒体比较只有 GUI 无 CLI 子命令。

**`bcr diff --json`（diff.v1）**：

- `DiffArgs` 加 `--json`；`similar::DiffOp` → (tag, old_start, old_end, new_start, new_end) 五元组，tag ∈ equal/delete/insert/replace
- 输出 `{ schema: diff.v1, ok, command, args, result: { ops: [{tag, old_range, new_range}], has_differences }, warnings, error }`
- 退出码仍按差异语义（0=无差异，1=有差异）

**`bcr hex --json`（hex.v1）**：

- `HexArgs` 加 `--json`；流式分块模式内收集差异行（offset + 左右 hex 字符串），统计 diff_rows/diff_bytes
- 输出 `result: { rows: [{offset, left, right, diff}], stats: {diff_rows, diff_bytes, left_size, right_size}, has_differences }`
- 非 JSON 模式保持原流式渲染路径不变

**新子命令 `bcr media`（media.v1）**：

- `MediaArgs { left, right, --json }` + `run()`：复用 P43-6 自研容器头解析（WAV/MP3/FLAC），字段级差异（与 mp3tag 风格一致）
- 输出 `result: { left_format, right_format, fields: [{name, left, right, diff}], has_differences }`
- 文本模式：左右格式 + 差异字段列表；退出码 0/1/2
- main.rs 注册 `Commands::Media`

**测试**：jsonout.rs 新增 4 个信封单元测试（diff 有/无差异、hex、media 结构断言）；修复 3 处 `DiffArgs` 初始化缺 `json` 字段。

## P49-3 kittest 菜单真点验证（`e700e31`）

现有 4 个 kittest 只驱动 tab 级 UI（`Harness::new_ui(|ui| tab.ui(ui))`），菜单栏从未被真点过。新增 3 例：

- `menubar_session_new_text_creates_diff_tab`：驱动完整 `menu_bar(app, ui)`，点击「会话」→「新建文本对比」→ 断言创建 `Tab::Diff`
- `menubar_session_new_dir_creates_dir_tab`：→ `Tab::Dir`
- `menubar_session_new_merge_creates_merge_tab`：→ `Tab::Merge`

要点：菜单 popup 需多帧展开（click 后 `run_steps(2)` 再查子项）；子项文本用 `crate::i18n::t(Key)` 与渲染一致。

## 测试与质量

- 本地 **537 单元 + 4 kittest 全绿**（P49 新增 7 项）/ clippy 0 / fmt 干净
- 无新增 i18n key（复用现有 Key 枚举）
