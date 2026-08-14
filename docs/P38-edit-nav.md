# P38-1d 文本对比编辑导航（对标 BC Next/Previous Edit）

> 背景：P38 深化批次第四项，BC 差距补充表 T7（编辑导航：下一个编辑/上一个编辑）。
> BC 搜索菜单 → 下一个编辑/上一个编辑：跳转到会话中已修改的行（复制/缩进/编辑产生的变更点）。

## BC 命令语义

| BC 菜单项 | 语义 |
|---|---|
| 搜索 → 下一个编辑 (Next Edit) | 跳转到下一个已编辑的行（相对当前） |
| 搜索 → 上一个编辑 (Previous Edit) | 跳转到上一个已编辑的行 |

## 实施内容

### difftab.rs
- 新增 `edited_anchors: Vec<(Option<usize>, Option<usize>)>`：已编辑行的行号锚点
  （left_no, right_no），编辑操作后追加，recompute 时重映射为当前行索引
- `mark_edited(row: &SideRow)`：记录锚点（去重）
- `next_edit()` / `prev_edit()`：在当前 diff_pos 行之后/之前找最近已编辑行，循环跳转
  （复用 jump_to_row 受控滚动）
- 在编辑操作点打标记：
  - `copy_block_at` / `copy_line_at`：目标侧受影响行
  - `indent_block`：块内两侧行
  - `replace_current` / `replace_all`：替换命中行
  - 行内编辑提交（inline_edit commit）
- 渲染：已编辑行右侧加小圆点标记（弱提示，不抢差异底色）

### menubar.rs
- Search 菜单 DiffTab 分支加「下一个编辑 / 上一个编辑」（i18n key：MenuNextEdit/MenuPrevEdit）

### i18n
- `MenuNextEdit` / `MenuPrevEdit` ×10 语言（先 grep 查重！）

### 测试
- 单元测试：编辑后锚点重映射正确；next_edit/prev_edit 循环跳转
- uikit 测试：Search 菜单项存在、点击后跳转
