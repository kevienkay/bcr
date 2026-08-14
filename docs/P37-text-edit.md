# P37-1g 文本编辑视图（对标 BC Text Edit）

> 背景：BC-UI-study.md 实测文本编辑视图（`bcomp -edit 文件`）：
> 文件：打开文件(O)/打开剪贴板(V)/打开方式/保存文件(S)
> 编辑：撤销重做/缩进/剪切复制粘贴/转换文件
> 搜索：编辑导航/查找替换/使用选择内容查找/转到(L)
> 视图：可见空白/行号/语法加亮
> bcr 现状：无独立单文件编辑视图（只有 DiffTab 双栏内联编辑）。本批新增 TextEditTab。

## 实施内容

### src/gui/textedit.rs（新文件）
- `pub struct TextEditTab { path: String, content: String, error, scroll, undo_stack, redo_stack, search, show_ws, syntax, encoding, had_bom }`
- 核心：
  - `open(path)` / `save()`（编码回写 + A2 模式 .bak 备份；Ctrl+S 保存）
  - 撤销/重做（EditSnapshot 复用思路：整文件快照栈，Ctrl+Z/Ctrl+Y）
  - 查找/替换（复用 common SearchState 风格：查找下一个/全部替换）
  - 转换文件（BC 编辑菜单 Convert File）：
    - Trim 行尾空白
    - Tabs to Spaces（4 空格）
    - 行尾风格：CRLF ↔ LF
  - 渲染：单栏虚拟化行编辑（egui TextEdit 每行）或 TextEdit::multiline 整体编辑 + 语法高亮预览；
    行号 gutter + 语法高亮（复用 highlight::syntax_for）+ 可见空白开关（复用 visible_ws）
- 空会话：打开文件按钮（P34 风格）

### 路由
- `GuiArgs` 加 `#[arg(long = "edit")] pub edit: Option<String>`（单文件文本编辑）
- main.rs CLI 分发 `bcr -e file`？直接 `--edit <file>` → GUI 打开 TextEditTab
- Session 菜单加「文本编辑」入口（新建文本编辑会话，空面板打开文件）

### mod.rs
- Tab 枚举加 `TextEdit(TextEditTab)` + title/ui/状态栏分支 + 拖拽填充（空 TextEdit 拖文件 → open）

### i18n
- 新 key ×10 语言：TextEditTitle / OpenFile / SaveFile / ConvertTrim / ConvertTabs / ConvertCrlf / ConvertLf

### 测试
- textedit 单元测试：转换函数（trim/tabs/crlf）、保存 .bak、撤销重做
- uikit 测试：空会话打开按钮；输入后 Ctrl+S 保存（模拟保存请求标志）
