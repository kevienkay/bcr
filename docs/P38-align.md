# P38-1b 文本对比对齐方式（对标 BC Align With）

> 背景：P38 深化批次第二项，BC 差距补充表 T2（手动强制行对齐）。
> 当 diff 引擎自动对齐不满意时，用户可手动把左侧某行与右侧某行强制对齐。

## BC 命令语义

| BC 菜单项 | 语义 |
|---|---|
| 右键 → 对齐方式 (Align With) | 先选源行，再点目标行，强制两行对齐（内容可不同） |
| 右键 → 清除对齐 (Clear Alignments) | 移除全部手动对齐 |

## 实施内容

### difftab.rs
- 新增字段：
  - `manual_aligns: Vec<(usize, usize)>`：手动对齐对（左侧行号 1-based, 右侧行号 1-based）
  - `align_pick: Option<(EditSide, usize)>`：对齐模式（源侧 + 源行号，等待点击目标行）
- 方法：
  - `start_align(side, row_no)`：进入对齐模式（记录源侧+行号）
  - `finish_align(target_row_no)`：点击目标行完成对齐 → manual_aligns.push((ln, rn))
  - `clear_aligns()`：清空全部手动对齐
- `recompute()`：build_rows 后应用 `apply_manual_aligns` 后处理：
  - 对每个 (ln, rn)：定位 left_no=ln 的行与 right_no=rn 的行
  - 若两行已并排（同索引）跳过；否则移除两行，在较前位置插入 Replace 行（左=源行内容, 右=目标行内容，tag=Replace）
  - 用 left_no/right_no 作稳定锚点，逐个处理
- 右键菜单：差异行 →「对齐方式…」进入对齐模式；有 manual_aligns 时 →「清除对齐」
- 对齐模式：顶部提示条「请点击另一侧行完成对齐 [✕ 取消]」；点击另一侧行 → finish_align
- 手动对齐行状态：用 Replace 底色 + 行号两侧并排显示

### 测试
- sideview/difftab 单元测试：
  - apply_manual_aligns：删除块+插入块各一行 → 对齐后合并为 Replace 行
  - 已并排的对跳过；行号锚点稳定
- uikit 测试：右键菜单含「对齐方式」；对齐模式提示条渲染不 panic
