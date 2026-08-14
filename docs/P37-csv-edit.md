# P37-1l 表格行列操作（对标 BC Table Compare 编辑菜单）

> 背景：UI 功能对齐审计剩余差距——BC 表格编辑菜单：
> 复制到右边/复制单元格至右侧/删除行/删除列/插入行/插入列/修改/排序
> bcr 已有：复制单元格至右侧、隐藏相同列、列宽自适应、表头排序（P37-1c）。
> 本批补齐：删除行/删除列/插入行/插入列/修改单元格。

## BC 命令语义

| BC 菜单项 | 语义 |
|---|---|
| Delete Row / Delete Column | 删除选中行 / 选中列（两侧同时操作） |
| Insert Row / Insert Column | 在选中位置插入空行 / 空列 |
| Edit (Modify) | 修改当前单元格值 |

## 实施内容

### CsvTab（csvtab.rs）
- 新增方法（复用 serialize_csv 写回 + .bak 备份 + reload）：
  - `delete_row(rel: &str)`：删除选中对齐行对应的两侧原始行（a 侧与 b 侧）
  - `insert_row(rel: &str)`：在选中行前插入空行（两侧）
  - `delete_col(col: usize)`：删除两侧表的列
  - `insert_col(col: usize)`：在列前插入空列（两侧）
  - `set_cell(value: String)`：修改选中单元格（写回当前侧？——按 BC 语义修改当前侧；bcr 简化：修改两侧中选中侧）
- 右键菜单加：删除行 / 插入行 / 删除列 / 插入列 / 修改单元格（需选中单元格）
- 单元格选中已有（selected），请求收集模式（闭包外执行）

### i18n
- 新 key ×10 语言：CsvDeleteRow / CsvInsertRow / CsvDeleteCol / CsvInsertCol / CsvEditCell

### 测试
- csvtab 单元测试：delete_row 两侧删除 + 文件写回；insert_col 列插入；set_cell 修改后文件更新
- uikit 测试：右键菜单项存在
