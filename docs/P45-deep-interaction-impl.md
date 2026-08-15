# P45 深层交互补齐 实施记录

对标 Beyond Compare 5.2.5 各视图专属菜单（`docs/ui-study/bc-menus-*.txt`）在 P44 之后的剩余差距，5 个功能批次 + 1 个 docs 收尾，全部推送 `origin/master`。

## P45-1 文本合并行级采用（`bca8038`）

**BC 对标**：编辑菜单 采用左边的行 ⌥⇧← / 中心行 / 右边行 ⌥⇧→。

- `mergeview.rs`：BlockInfo 加 `line_res: Vec<Option<Resolution>>`（Conflict 块逐行覆盖）；render_merged 行级优先输出（设置行按行取左/中/右，未设置行跟随块 resolution）
- MergeTab 加 `cur_line`（渲染点击行记录，借局部变量避免借用冲突）+ `take_line(res)`（当前行所在冲突块对应行设置 line_res）+ `line_takes`（测试用，cfg(test)）
- 快捷键 ⌥⇧←/→ 采用左/右行；Edit 菜单 MergeTab 分支加 3 项（采用左边的行/中心行/右边行）
- **坑**：egui TextEdit 编辑光标用 `CharIndex` 新类型（`.0` 取 usize）；测试先 `next_conflict()` 再逐行 take_line（冲突块仅 2 行，start+2 越界）
- 3 个 i18n keys ×10 语言（MenuTakeLeftLine/MenuTakeCenterLine/MenuTakeRightLine）

## P45-2 文件夹合并视图过滤（`1ea9e69`）

**BC 对标**：视图菜单 显示全部 1/更改 2/冲突 3/左边变化 4/右边变化 5/可合并 6/未变化项 7。

- FolderMergeTab 加 `MergeFilter` 枚举 + `view_filter` 字段 + `filter_matches`（op/from/conflicted 匹配）+ 渲染可见索引过滤（visible Vec 替代直接 plan 遍历）
- 快捷键 1-7（egui_wants_keyboard_input 守卫）；View 菜单 FolderMerge 分支 7 项过滤
- 8 个 i18n keys ×10 语言（MergeFilterAll~Unchanged + MenuFilter）

## P45-3 文件夹比较视图过滤扩展（`7960f14`）

**BC 对标**：视图菜单 显示独有/不独有/差异但无独有项/左边较新和左边独有项/右边较新和右边独有项。

- DirTab ViewFilter 加 5 个变体：`Orphans`（左+右独有）、`NonOrphans`（排除独有）、`DiffNoOrphans`（Differ+Moved）、`LeftNewerOrOrphan`、`RightNewerOrOrphan` + 过滤逻辑
- View 菜单 DirTab 分支 10 项过滤（All/Diff/Same/独有/不独有/差异无独有/左较新/右较新/组合 2 项，复用现有 ViewLeftNewer/ViewRightNewer keys）
- 5 个 i18n keys ×10 语言（DirFilterOrphans/NonOrphans/DiffNoOrphans/LeftNewerOrOrphan/RightNewerOrOrphan）

## P45-4 图片比较补齐（`1a16bd9`）

**BC 对标**：视图菜单 重置差异偏移 / 比较元数据。

- ImageTab 加 `reset_diff_offset`（滚动归零 + 有差异时请求定位第一差异）+ `compare_meta`（show_meta_compare 弹窗：格式/尺寸/大小/帧数 左右对比，差异字段红色）+ `show_meta_compare` 字段
- View 菜单 ImageTab 分支 2 项
- 2 个 i18n keys ×10 语言（ImgResetOffset/ImgCompareMeta）

## P45-5 表格/HEX/补丁/文本编辑补齐（`b5e7d83`）

**BC 对标**：表格 在后面插入列；16进制 复制到右边 ⇧⌃→；补丁 选择选择内容 D；文本编辑 使用选择内容进行查找 ⌘E。

- CsvTab 加 `insert_col_after`（后面插入列）+ Edit 菜单项 + `select_row_col`（测试用，cfg(test)）
- DiffTab 加 `hex_copy_to_right`（HEX 复制到右边：当前差异行左侧字节写入右侧文件 + .bak 备份 + load_pair 重建）+ ⇧⌃→ 快捷键
- PatchTab 加 `selection` + `select_selection`（第一个差异块 Delete/Insert/Replace 选为选区）+ 选区蓝色高亮渲染（Color32::from_rgba_unmultiplied(86,148,240,60)）+ Edit 菜单项
- TextEdit 加 `sel_range`（渲染用 edit.show() 捕获 TextEditOutput.cursor_range，CharIndex.0 取 usize）+ `find_selection`（⌘E 选区文本填入查找框）+ Search 菜单项 + ⌘E 快捷键

## 测试与质量

- 每批：cargo test 全绿 → cargo fmt → cargo clippy --all-targets 0 警告 → 单提交推送
- 最终本地 **525 单元 + 4 kittest 全绿** / clippy 0 / fmt 干净
- P45 新增 i18n keys 共 **21 个 × 10 语言**（3+8+5+2+2+MenuFilter=21）
- 新增测试 5 个 uikit
