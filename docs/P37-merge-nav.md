# P37-1b 三路合并导航补齐（对标 BC Text Merge 搜索菜单）

> 背景：P37-1a 已完成「顺序合并」（Take Left Then Right / Right Then Left）。
> 本批对照 BC 帮助文档 `commandstextmerge.html` 实测命令语义，补齐剩余导航差距。

## BC 命令语义（帮助文档原文）

| BC 菜单项 | 语义 |
|---|---|
| Clear Conflict Section, Next | 清除当前区段冲突并定位到下一冲突区段 |
| Next / Previous Conflict Section | 定位下一/上一冲突区段（bcr 已有 ⬇/⬆ + F7） |
| Next / Previous Difference | 定位下一/上一差异文本（非 Context 块） |
| Next / Previous Left Taken | 定位到下一处「采用了左侧」的行范围 |
| Next / Previous Right Taken | 定位到下一处「采用了右侧」的行范围 |

## 实施内容

### mergeview.rs
- `MergeView` 增加 `diff_rows: Vec<usize>` + `diff_block_indices: Vec<usize>`
  （所有非 Context 块：LeftOnly/RightOnly/Same/Conflict 的起始行与 blocks 下标，
  与 conflict_rows 结构对齐，构建时填充）

### mergetab.rs（工具栏导航组）
- `next_diff()` / `prev_diff()`：diff_rows 循环跳转
- `next_taken_left()` / `prev_taken_left()`：resolution ∈ {Left, LeftThenRight} 的块循环跳转
- `next_taken_right()` / `prev_taken_right()`：resolution ∈ {Right, RightThenLeft} 的块循环跳转
- `clear_conflict_next()`：当前冲突块若未解决（Auto）则取左侧（默认），再跳到下一冲突区段
- 工具栏加「✖ 清除冲突并跳下一」+「◀ 采用左导航」+「▶ 采用右导航」按钮
  （采用导航为循环：再点继续下一处）

### menubar.rs（Search 菜单 MergeTab 分支）
- search_menu 增加 `with_merge_tab` 转发：下一/上一差异、清除冲突区段下一、
  下一/上一左采用、下一/上一右采用（与 DiffTab 菜单共存）

### i18n
- 新 key ×10 语言：ClearConflictNext / NextDiff / PrevDiff /
  NextLeftTaken / PrevLeftTaken / NextRightTaken / PrevRightTaken

### 测试
- mergeview 单元测试：diff_rows 覆盖非 Context 块
- mergetab uikit 测试：清除冲突下一 / 采用导航循环跳转
