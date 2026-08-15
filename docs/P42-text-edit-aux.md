# P42：文本比较编辑补齐 + 视图辅助（转换文件 / 剪贴板 ⌘V / 标尺 / 图例日志工具栏开关）

> 依据 `docs/BC-UI-study.md`（文本比较编辑菜单/视图菜单实测）与
> `docs/P39-UI-study.md` A 表（P1 打开剪贴板比较 ⌘V、P2 文本编辑打开剪贴板）：
> - 编辑菜单：转换文件（Trim Trailing Whitespace / Tabs to Spaces / Line Endings）
> - 视图菜单：标尺（字符列标尺）、图例(L)、日志、工具栏开关
> - 文件菜单：打开剪贴板比较 ⌘V

## 现状

- 文本编辑（TextEditTab）已有转换文件（convert_trim/convert_tabs/convert_line_ending）；**文本比较（DiffTab）没有**
- 剪贴板比较：DiffTab 有 load_clipboard_left/right + File 菜单项，**无 ⌘V 快捷键**；TextEdit 无剪贴板入口
- 标尺 / 图例 / 日志 / 工具栏开关：均未实现

## 批次

- **P42-1 文本比较转换文件**：
  - DiffTab 加 `convert_trim()` / `convert_tabs()` / `convert_line_ending(to_crlf)`（复用 TextEdit 语义：.bak 备份 + 编码回写 + 撤销快照 + 重载）
  - Edit 菜单 DiffTab 分支加「转换文件」子菜单（Trim 行尾空白 / Tabs to Spaces / CRLF↔LF）
- **P42-2 剪贴板比较 ⌘V + 文本编辑打开剪贴板**：
  - DiffTab `⌘V` → load_clipboard_right（File 菜单「打开剪贴板比较」已有左/右，补快捷键）
  - TextEditTab 加「打开剪贴板」入口（File 菜单转发）
- **P42-3 标尺（字符列标尺）**：
  - DiffTab 加 `show_ruler`：内容区顶部绘制字符列标尺（10/20/.../100 刻度），View 菜单 checkbox
- **P42-4 图例 / 日志 / 工具栏开关**：
  - View 菜单加「图例」弹窗（BC 图例：差异色/状态徽标含义说明）
  - View 菜单加「日志」开关（打开日志面板，记录最近操作/错误）
  - View 菜单加「工具栏」开关（隐藏/显示工具栏，View 菜单自身保留）

## 文件改动

- `src/gui/difftab.rs`：转换文件方法、⌘V 快捷键、标尺渲染、工具栏开关
- `src/gui/textedit.rs`：打开剪贴板
- `src/gui/menubar.rs`：Edit「转换文件」子菜单、File 剪贴板项、View 图例/日志/工具栏
- `src/gui/mod.rs`：日志面板（如需要）
- `src/i18n.rs` + `src/i18n_tables.rs`：新 key × 10 语言
- `src/gui/uikit_tests.rs`：测试

每批本地 cargo test 全绿 → fmt/clippy → 单提交推送；全部完成后统一查 CI。
