# P39-2e：剩余项（替换菜单 / 忽略不重要差异 / 保存快照 / 比较文件使用）

> 依据 `docs/P39-UI-study.md` 快捷键速查表 + 2.3 工具菜单 + 2.1 会话菜单，
> 补齐 P39-2 最后一批差距。

## 替换菜单项（⇧⌘F）

- DiffTab 新增 `search.replace_focus`（替换框聚焦请求）+ `focus_replace()` 方法
- 快捷键 `⇧⌘F` 聚焦替换框（修复：原 ⌘F 分支先命中 shift 组合，合并为同分支内判断 shift）
- Search 菜单加「替换…」入口

## 忽略不重要差异（BC View > Ignore Minor Differences）

- View 菜单「忽略不重要差异」checkbox：一键同步 DiffTab 四个忽略选项
  （ignore_whitespace / ignore_trailing / ignore_case / ignore_crlf）并 recompute

## 保存快照（BC Tools > Save Snapshot）

- Tools 菜单加「保存快照」：打开会话中心（GUI 保存当前对比为命名会话）

## 比较文件使用（BC Session > Compare Using，视图切换）

- Session 菜单加「比较文件使用」子菜单（当前 DiffTab 有左右文件时显示）：
  - 文本对比 / 16进制对比 / 图片对比 / 表格对比
  - `reopen_as_text` / `reopen_as_hex`（强制 Hex 细节）/ `reopen_as_image` / `reopen_as_csv`
  - 复用设置中的忽略选项与 show_stats

## 文件改动

- `src/gui/difftab.rs`：replace_focus + focus_replace + ⇧⌘F 快捷键
- `src/gui/menubar.rs`：Search「替换…」、View「忽略不重要差异」、Tools「保存快照」、Session「比较文件使用」
- `src/gui/mod.rs`：reopen_as_text/hex/image/csv 视图切换方法
- `src/i18n.rs` + `src/i18n_tables.rs`：4 个新 key × 10 语言（MenuReplace/IgnoreMinor/MenuSnapshot/MenuCompareUsing）

## 测试（新增 3 个 uikit）

- `difftab_replace_focus_via_shift_cmd_f`：⇧⌘F 聚焦替换框（TextInput 第 3 个）
- `ignore_minor_toggles_all_options`：四忽略开关同开 + recompute
- `compare_using_reopen_views`：文本 → 图片/表格/hex 视图切换

本地 496 单元 + 4 kittest 全绿 / clippy 0 / fmt 干净。
