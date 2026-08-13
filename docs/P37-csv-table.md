# P37-1c 表格视图补齐（对标 BC Table Compare）

> 背景：BC 帮助文档 `commandstextmerge.html` + 实测（BC-UI-study.md）确认
> 表格视图差距：复制单元格至右侧、删除/插入行列、修改、排序、隐藏相同列、列宽调整。
> bcr 已有：主键对齐、状态过滤、表头点击排序（P29）。本批补齐 3 项高频操作。

## BC 命令语义

| BC 菜单项 | 语义 |
|---|---|
| Copy Cell to Right Side（复制单元格至右侧） | 把左侧单元格内容复制到右侧对应单元格 |
| Hide Same Columns（隐藏相同列） | 视图选项：隐藏所有行都相同的列 |
| Adjust Column Sizes to Fit（调整列大小为适合大小） | 按内容自动调整列宽 |

## 实施内容

### csvcmp.rs
- 新增 `pub(crate) fn serialize_csv(table: &Table, delim: char) -> String`
  （RFC 4180 序列化：表头 + 数据行，字段含分隔符/引号/换行时加引号转义）

### CsvTab（csvtab.rs）
- 新增状态：
  - `selected: Option<(usize, usize)>`（对齐行下标, 列号）——单元格选择
  - `hide_same_cols: bool` ——隐藏相同列开关
  - `auto_fit: bool` ——列宽自适应开关
- **复制单元格至右侧**（A 类）：
  - 单元格点击选中（记录 aligned 下标 + 列号）
  - 右键菜单「复制单元格至右侧」：取 table_a 对应值 → 写入 table_b 对应行同列
    → serialize_csv 写回右侧文件（备份 .bak，复用 A2 模式）→ reload
- **隐藏相同列**（B 类视图）：
  - 计算每列是否「全部对齐行左右相等」→ 渲染时跳过这些列（表头 + 数据）
- **列宽自适应**（B 类视图）：
  - 按「表头 + 该列所有可见单元格」的最大字符宽度计算列宽（上限 320px）

### i18n
- 新 key ×10 语言：CopyCellRight / HideSameCols / FitColumns

### 测试
- csvcmp 单元测试：serialize_csv 往返（含引号/逗号/换行字段）
- CsvTab 单元测试：hide_same_cols 隐藏后可见列减少；copy_cell 写回后右侧文件更新
- uikit 测试：点击单元格 → 右键复制 → 右侧值更新
