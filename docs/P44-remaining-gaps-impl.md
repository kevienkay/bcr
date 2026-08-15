# P44 剩余差距补齐 实施记录

对标 Beyond Compare 5.2.5 菜单树（`docs/ui-study/bc-menus-*.txt`，11 份/2668 行）全量扫描后的剩余差距，7 个功能批次 + 1 个 docs 收尾，全部推送 `origin/master`。

## P44-1 窗口菜单 + 标签切换（`7fb859d`）

**BC 对标**：窗口菜单 选择下一个标签页 ⌘]/⌘[、最小化 ⌘M、关闭所有窗口 ⌘⇧W。

- DiffApp 加 `next_tab/prev_tab`（循环切换标签）+ `close_all_tabs`（清空所有标签回主页）
- 全局快捷键：⌘]/⌘[ 切换标签、⌘⇧W 关闭所有窗口、⌘M 最小化（`ViewportCommand::Minimized(true)`）
- 新增 Window 菜单（选择下一/上一标签页/最小化/关闭所有窗口，空标签时禁用）
- 5 个 i18n keys ×10 语言（MenuWindow/MenuNextTab/MenuPrevTab/MenuMinimize/MenuCloseAllWindows）
- **坑**：egui 0.36 Key 枚举用 `OpenBracket/CloseBracket`（不是 BracketLeft/Right）

## P44-2 文本比较快捷键（`6ca46ab`）

**BC 对标**：编辑 对齐方式... ⌘A / 增加缩进 ] / 减少缩进 [；搜索 使用选择内容进行查找 ⌘E。

- DiffTab 加 `align_current`（当前差异块左侧行与右侧当前行对齐）、`indent_current`（当前差异块 ±4 空格，复用 indent_block）、`find_selection`（选区文本填入查找框并聚焦）
- 快捷键：⌘A 对齐、]/[ 缩进、⌘E 选区查找（输入框聚焦时不触发）
- Edit 菜单 DiffTab 分支加对齐方式/增加缩进/减少缩进；Search 菜单加使用选择内容进行查找
- 4 个 i18n keys ×10 语言

## P44-3 文本合并快捷键（`69be495`）

**BC 对标**：编辑 冲突 采用左边 ⇧←/中心/右边 ⇧→、采用左边然后右边 ⌘B、右边然后左边 ⇧⌘B；搜索 清除冲突区段下一、冲突部分导航。

- MergeTab 快捷键：⇧←/⇧→ 采用左/右（块级 resolve_current）、⌘B 左后右、⇧⌘B 右后左、⌘⇧⌃↓/↑ 冲突部分导航（与 F7/⇧F7 等效）
- Edit 菜单 MergeTab 分支加冲突采用 5 项（采用左边/中心/右边/左后右/右后左）
- **坑**：`resolve_current` 依赖 conflict_idx 定位，测试需先 `next_conflict()` 再采用
- 5 个 i18n keys ×10 语言

## P44-4 会话/文件菜单补齐（`6313f64`）

**BC 对标**：会话 打开会话 ⌥⌘O、重新比较文件 ⌘R、已锁定；文件 保存文件为 ⌘⇧S、打开方式（关联应用/在查找器中显示）。

- 全局快捷键：⌥⌘O 打开会话中心、⌘R 重新比较（`reload_current` 转发 Diff/Merge/Dir/FolderMerge/Csv）、⌘⇧S 保存文件为（TextEdit save_as rfd 另存）
- Session 菜单加打开会话/重新比较文件/已锁定（DiffTab `locked` checkbox 防编辑）
- File 菜单加保存文件为 + 打开方式子菜单（open_with_system_app/reveal_in_file_manager，DiffTab 左右文件）
- 6 个 i18n keys ×10 语言

## P44-5 工具菜单补齐（`cdf27dd`）

**BC 对标**：工具 导出设置/导入设置/恢复出厂默认/编辑文本文件/查看补丁。

- Settings 加 `export_to/import_from/reset_defaults`（TOML 序列化）
- tools_menu 加导出/导入设置（rfd 对话框）、恢复出厂默认、编辑文本文件/查看补丁（TextEdit/PatchTab 空标签入口）
- **坑**：menubar.rs 无 TextEditTab/PatchTab 导入，用全路径 `super::textedit::TextEditTab`
- 5 个 i18n keys ×10 语言

## P44-6 视图开关 + 表格快捷键（`cefc947`）

**BC 对标**：视图 行号/语法加亮；表格 修改 ⇧⌃↩、前面插入行 ⌘⌥⌃↩、后面插入行 ⌥⌃↩、排序...。

- DiffTab 加 `show_line_numbers/show_syntax` 开关（gutter 宽度与 syn 传参条件化），View 菜单 checkbox
- CsvTab 快捷键：⇧⌃↩ 修改（open_cell_edit 预填选中单元格）、⌘⌥⌃↩ 前插行、⌥⌃↩ 后插行（新增 `insert_row_after`）
- 排序对话框（Edit 菜单排序...：选左右侧列 + 升/降序，parse_sort_col/sort_label）
- Edit 菜单 CsvTab 分支（排序/修改/前插/后插/删除行）
- 5 个 i18n keys ×10 语言

## P44-7 搜索补齐（`ec6c1d1`）

**BC 对标**：文件夹比较 查找文件名 ⌘F；文本编辑 在多个文件中查找 ⌘⇧F。

- DirTab 加 `find_name` 字段 + rebuild 过滤（不区分大小写匹配基名）+ 过滤面板输入框
- Search 菜单 DirTab 分支加查找文件名（展开过滤面板）
- TextEdit 加 `open_find_in_files`（默认当前文件目录 + 弹窗），Search 菜单入口
- **坑**：TextEdit `search_dir` 私有，封装为 pub(crate) 方法
- 2 个 i18n keys ×10 语言

## 测试与质量

- 每批：cargo test 全绿 → cargo fmt → cargo clippy --all-targets 0 警告 → 单提交推送
- 最终本地 **520 单元 + 4 kittest 全绿** / clippy 0 / fmt 干净
- P44 新增 i18n keys 共 **30 个 × 10 语言**
- 新增测试 7 个 uikit
