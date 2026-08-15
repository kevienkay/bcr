# P43 导航/选区/媒体 实施记录

对标 Beyond Compare 5.2.5，6 个功能批次 + 1 个 docs 收尾，全部推送 `origin/master`。

## P43-1 导航历史（`e23f0e9`）

**BC 对标**：会话菜单 后退/前进/上一层/比较父文件夹（DirTab 分支）。

- DirTab 加 `history: Vec<(String, String)>` + `history_pos`，实现 `navigate/back/forward/up_level/compare_parent`
- **关键决策**：`navigate` 首次调用时把当前路径对作为历史起点入栈（保证 back 可回退）；空会话（`new("","")`）不入栈
- Session 菜单 DirTab 分支加 4 个导航项
- 新增 13 个 i18n keys（MenuBack/MenuForward/MenuUpLevel/MenuCompareParent/MenuSelectSelection/MenuSelectionToClipboard/MenuNextReplace/MenuPrevReplace/MenuNextDiffFile/MenuPrevDiffFile/MenuMergeFiles/MenuCompareWithOutput/MenuInfo）×10 语言（P43 全批预插，未用 key 加 `#[allow(dead_code)]` 待对应批次用上）

## P43-2 文本选区操作（`951dd40`）

**BC 对标**：编辑菜单 选择选择内容（Select Selection）/ 把选择内容和剪贴板比较（Compare Selection with Clipboard）。

- DiffTab 加 `selection: Option<(usize, usize)>`（rows 索引范围）+ `replace_nav` 字段（P43-3 用）
- `select_selection`：当前差异块选为选区；`selection_to_clipboard`：选区文本经 `write_clipboard_temp` 写临时文件 → `load_right`；`selection_text`（测试用）
- 渲染：选区行叠加蓝色高亮（`Color32::from_rgba_unmultiplied(86,148,240,60)`）
- Edit 菜单 DiffTab 分支加两项
- **修复 P42-1 遗留 bug**：转换文件子菜单误放 DirTab 分支内，重构为独立 DiffTab 分支

## P43-3 替换导航 + 差异文件导航（`34f0daa`）

**BC 对标**：搜索菜单 下一/上一替换；文件夹对比 下一/上一差异文件。

- DiffTab `next_replace/prev_replace`：复用 next_match/prev_match 跳匹配 + 设 `search.replace_focus = true`（聚焦替换框）
- DirTab `next_diff_file/prev_diff_file/move_diff_file`：isize 取模循环遍历 flat，找 status != Same 的文件，选中 + `scroll_to_selected = true`
- Search 菜单 DiffTab 分支加替换导航、DirTab 分支加差异文件导航

## P43-4 会话菜单补齐（`5c27726`）

**BC 对标**：会话菜单 合并文件（文本比较）/ 和输出比较（文件夹合并）。

- mod.rs 加 `reopen_as_merge`（当前 Diff 左右文件进三路合并，BASE 留空 `MergeTab::new("", l, r)`）和 `compare_with_output`（FolderMerge 的 out vs left 开 DirTab）
- Session 菜单加「合并文件」（DiffTab 分支 cur_paths 后）+「和输出比较」（FolderMerge 分支，修复 collapsible_if）
- 测试修正：MergeTab 路径字段是 `left_path/right_path`（不是 label_l/label_r）

## P43-5 信息弹窗（`b711c5d`）

**BC 对标**：会话菜单 信息（当前标签统计）。

- DiffApp 加 `show_info: bool`；Session 菜单加「信息」项
- `info_window`（抽独立方法便于测试直接调用）：按当前标签类型显示统计
  - Diff：视图/左右文件路径/编码/大小/行数/差异行/相同/仅左/仅右/修改
  - Dir：目录路径/条目/相同/仅左/仅右/差异/移动
  - Merge：BASE/左右路径/冲突/行数
  - Image：路径/帧/差异像素；Csv：行数/列数；TextEdit：标题/字符/行数；Patch：标题；FolderMerge：输出/左右；Media：左右/差异字段
- 注意各 tab 字段可见性：CsvTab 用 `row_count()/col_count()`（key 私有）、TextEdit 用 `pub(crate) content`、Patch 用 `title()`

## P43-6 媒体比较（简化版）（`ae2b643`）

**BC 对标**：媒体比较会话（简化版 = 纯音视频元数据对比，不涉及解码）。

- `src/mediacmp.rs`（自研容器头解析，无外部依赖）：
  - WAV：RIFF + fmt 块 → 声道/采样率/位深/字节率，时长 = data 大小 / 字节率
  - MP3：MPEG 帧头同步字 → 码率表估算，时长 ≈ 文件大小 / 码率（CBR 近似）
  - FLAC：fLaC + STREAMINFO → 采样率/声道/位深/总采样数 → 时长
  - 其他格式：退化为 文件大小 + 扩展名
  - `compare_media()` 字段级对比（与 P24 mp3tag 风格一致）
- `src/gui/mediatab.rs`：MediaTab 双栏元数据并排 + 差异字段红色标记 + 重新加载/交换两侧 + 空会话左右打开 + `is_media_file` 扩展名兜底
- 集成：主页第 9 张卡片 🎵（SessionMedia/SessionMediaDesc）、Session 菜单新建媒体比较、拖放两文件自动识别、状态栏媒体差异字段数
- 新增 2 个 i18n keys ×10 语言

## 测试与质量

- 每批：cargo test 全绿 → cargo fmt → cargo clippy --all-targets 0 警告 → 单提交推送
- 最终本地 **513 单元 + 4 kittest 全绿** / clippy 0 / fmt 干净
- 新增 i18n keys 共 **15 个 × 10 语言**（13 预插 + 2 媒体）
- 新增测试：P43-1~5 各 1 个 uikit + P43-6 的 2 个 mediacmp 单测 + 1 个 uikit
