# P46 视图开关与导航补齐 实施记录

对标 Beyond Compare 5.2.5 各视图的视图/搜索/会话菜单剩余差距，5 个功能批次 + 1 个 docs 收尾，全部推送 `origin/master`。

## P46-1 TextEdit 视图开关（`b9c7576`）

**BC 对标**：文本编辑视图菜单 行号/语法加亮/自动换行/网页/文件信息/工具栏。

- TextEditTab 加 `show_line_numbers/show_wrap/show_file_info` 字段（pub(crate)）+ 初始化
- 预览模式（show_syntax）gutter 行号条件化（show_line_numbers 控制 gutter 宽度与 paint_line_no）
- 编辑模式自动换行：desired_width 条件化（换行=可用宽度，否则 INFINITY 横向滚动）——**egui 0.36 multiline TextEdit 无 wrap_mode API**（只影响 hint_text），改用 desired_width 实现
- 文件信息 TextEditStatus 条件化（show_file_info）
- View 菜单 TextEdit 分支 3 个 checkbox（复用 MenuLineNumbers/WordWrap，新增 MenuFileInfo）
- 1 个 i18n key ×10 语言

## P46-2 PatchTab 差异导航（`1cc2e7b`）

**BC 对标**：文本补丁搜索菜单 下一个/上一个差异（⇧⌥⌃↓/↑）、差异部分（⇧⌃↓/↑）。

- PatchTab 加 `diff_pos` 字段 + `diff_rows()`（RowTag≠Equal 行索引集合）+ `next_diff/prev_diff`（循环跳差异行）+ `next_diff_section/prev_diff_section`（连续差异行合并区块取块首）+ `jump_to` 滚动定位
- 快捷键 ⇧⌥⌃↓/↑ 差异、⇧⌃↓/↑ 差异部分（输入框聚焦不触发）
- Search 菜单 PatchTab 分支 4 项（复用 MenuNextDiff/MenuPrevDiff/MenuNextSection/MenuPrevSection）

## P46-3 hex 视图过滤与布局（`e46ba1d`）

**BC 对标**：16进制视图菜单 显示全部/差异/相同（1/2/3）、边并排/上-下布局。

- DiffTab 加 `HexViewFilter`（All/Diff/Same）+ `HexViewLayout`（SideBySide/TopBottom）枚举与 hex_filter/hex_layout 字段
- 渲染可见索引过滤（visible Vec 替代直接遍历）+ 上-下布局行高 2x
- 快捷键 1/2/3（hex 模式切换 hex_filter）
- View 菜单 hex 分支过滤 3 项 + 布局 2 项
- 5 个 i18n keys ×10 语言（HexFilterAll/Diff/Same + HexLayoutSideBySide/TopBottom）

## P46-4 DirTab 结构选项（`450326e`）

**BC 对标**：文件夹比较视图菜单 总是显示文件夹/比较文件和文件夹结构/仅比较文件。

- DirTab 加 `show_all_dirs`（关闭时隐藏空目录）+ `only_files`（walk 跳过目录行）字段 + walk 过滤参数
- View 菜单 DirTab 分支 2 个 checkbox（切换后 rebuild_tree）
- 2 个 i18n keys ×10 语言（DirShowAllDirs/DirOnlyFiles）
- walk 加 `#[allow(clippy::too_many_arguments)]`

## P46-5 文件夹同步视图 + 工作空间（`c1920d9`）

**BC 对标**：视图>图例（⇧L）；会话菜单 保存工作空间为.../加载工作空间。

- 图例快捷键 ⇧L（纯 shift 非 cmd，输入框聚焦不触发）
- DiffApp 加 `save_workspace/load_workspace`：标签布局 TOML 持久化（WsFile 包装结构规避 **toml 0.8 顶层数组 `unsupported rust type`**；支持 diff/dir/merge/image/csv/media 类型重建）
- Session 菜单加保存工作空间为.../加载工作空间（rfd 对话框）
- 2 个 i18n keys ×10 语言（MenuLoadWorkspace/MenuSaveWorkspaceAs）

## 测试与质量

- 每批：cargo test 全绿 → cargo fmt → cargo clippy --all-targets 0 警告 → 单提交推送
- 最终本地 **530 单元 + 4 kittest 全绿** / clippy 0 / fmt 干净
- P46 新增 i18n keys 共 **12 个 × 10 语言**（1+5+2+2+2）
- 新增测试 5 个 uikit
