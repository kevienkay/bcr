# P39-2c：会话中心 + 报告生成 + 差异部分导航

> 依据 `docs/P39-UI-study.md` A 表功能缺口（P1 报告/差异部分导航，P0 会话 GUI），
> 补齐 BC 5.2.5 会话菜单核心能力。

## 会话中心 GUI（保存/加载/删除会话列表管理）

- 会话中心弹窗顶部新增**保存当前会话**入口（替代命令行 `bcr session save`）：
  - 会话名输入框 + 「保存当前会话」按钮（Enter 也可触发）
  - 从当前标签提取左右路径：DirTab 取 left/right；DiffTab 取已加载文件路径（`session_paths()`）
  - 保存到 `~/.bcr-sessions.toml`，下一帧自动刷新列表
- 列表原有能力保留：收藏星标 / ▶ 打开 / ✕ 删除 / 收藏优先 + 最近使用排序

## 报告生成（⌘P，文本/HTML）

- Session 菜单「报告…」+ 全局快捷键 `⌘P` → 报告弹窗：
  - 格式选择：文本 TXT / HTML
  - 实时预览（当前标签）：
    - **DirTab**：有 compare 结果 → `report::render_txt` 文本预览；保存时 HTML 走 `htmlreport::render_html`
    - **DiffTab**：`diff_report_preview()` 生成统计 + 差异行摘要文本报告
  - 「💾 保存报告…」→ rfd 保存对话框 → 写入文件 → 提示 `ReportSaved`
  - 错误（无结果/当前标签不支持）在弹窗内提示

## 差异部分导航（区块级跳转，BC ⇧⌃↓/↑）

- DiffTab 新增 `next_diff_section()` / `prev_diff_section()`：按 `diff_blocks`（连续差异行区块）跳转
  - 无当前位置 → 从第一块（或最后一块）开始；有 → 跳所在块的下一/上一块
  - `jump_to_section_row()` 同步 diff_pos 到对应 diff_rows 索引（保持竖条标记一致）
- 快捷键 `⇧⌃↓` / `⇧⌃↑`（输入框聚焦时不触发）
- Search 菜单加「下一差异部分 / 上一差异部分」

## 文件改动

- `src/gui/mod.rs`：会话中心保存当前会话、报告弹窗（⌘P）、`save_current_report`、`diff_report_preview`、`session_paths`
- `src/gui/difftab.rs`：`next_diff_section`/`prev_diff_section`/`jump_to_section_row` + `⇧⌃↓/↑` 快捷键
- `src/gui/menubar.rs`：Search 菜单差异部分项、Session 菜单报告项
- `src/i18n.rs` + `src/i18n_tables.rs`：6 个新 key × 10 语言（MenuReport/MenuNextSection/MenuPrevSection/ReportSaved/SessionSaveCurrent/SessionName）

## 测试（新增 2 个 uikit）

- `difftab_diff_section_navigation`：两区块跳转（0 → 3 → 0）
- `session_center_save_current_and_report_preview`：DiffTab 路径提取 + 报告预览含统计/标题

本地 491 单元 + 4 kittest 全绿 / clippy 0 / fmt 干净。
