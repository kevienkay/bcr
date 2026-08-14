# P37-1m 文本对比行级复制（对标 BC 编辑菜单 Copy Line）

> 背景：UI 功能对齐审计剩余——BC 文本对比编辑菜单：
> 复制到右边/复制到左边（整块，P35-A1 已做）、**复制行到右边/复制行到左边**（行级）、
> 对齐方式（Align With）、隔离（Isolate）。
> 本批补行级复制（Copy Line）。

## BC 命令语义

| BC 菜单项 | 语义 |
|---|---|
| Copy Line to Right / Left | 复制当前行到另一侧对应位置（行级，替换该行内容） |

## 实施内容

### difftab.rs
- 新增 `pub fn copy_line_to(&mut self, row_idx: usize, target: EditSide) -> bool`：
  - 取当前行（row_idx 对应 rows 中的对齐行）源侧文本 → 写入目标侧对应行
  - 写回目标文件（A2 模式 .bak + 编码回写 + EditSnapshot 撤销，复用 copy_block_to 的写回路径）
- 右键菜单加「复制行到右侧 / 复制行到左侧」（行级，区别于现有整块复制）

### 测试
- difftab 单元测试：copy_line_to 修改目标侧该行内容 + 文件写回
- uikit 测试：右键菜单项存在
