# P37-1n 文本编辑多文件查找（对标 BC Find in Files）

> 背景：UI 功能对齐审计剩余最后一项——BC 文本编辑的 **Find in Files**（在多个文件中查找）。
> 在目录内递归搜索文本，结果列表显示 文件:行号:内容，点击结果打开文件。

## BC 命令语义

| BC 菜单项 | 语义 |
|---|---|
| Edit → Find in Files | 在目录（含子目录）中搜索文本，列出匹配 文件/行，双击打开 |

## 实施内容

### textedit.rs
- 新增 `MultiFileSearch` 结构：
  - `dir: String`（搜索目录，默认当前文件目录）
  - `needle: String`（搜索词）
  - `results: Vec<FileHit>`（`{ path, line_no, line_text }`，仅文本文件，限制单文件最大行数）
  - `running: bool` / `done: bool`（同步执行，小目录即可）
- `search_files(&mut self)`：递归扫描 dir 下文件（跳过隐藏目录、.git/target/node_modules），
  逐文件按行找包含 needle 的行，最多收集 `MAX_HITS`（如 500）条防爆炸
- 查找/替换栏加「在文件中查找」按钮 → 弹窗（目录 + 搜索词 + 结果列表 + 点击打开）
- 点击结果 → `open(path)` 打开该文件（复用现有 open，跳转到该行）

### 测试
- textedit 单元测试：构造临时目录多文件，search_files 命中跨文件结果
- uikit 测试：按钮存在、弹窗渲染不 panic
