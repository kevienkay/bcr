# P37-1h 补丁视图（对标 BC Text Patch）

> 背景：BC-UI-study.md 实测补丁视图（`.patch`/`.diff` 文件自动进入）：
> 文件：打开方式/**应用补丁**
> 搜索：差异/差异部分导航/查找/**切换书签/转到书签/清除书签**/转到(L)
> 视图：可见空白/行号/语法加亮
> 界面：a/base.txt vs b/base.txt 差异对比 + "Lines added" 标记
> bcr 现状：无补丁视图。本批新增 patchview 解析 + PatchTab。

## 实施内容

### src/patchview.rs（新文件，纯逻辑可单测）
- `pub struct ParsedPatch { pub a_path: String, pub b_path: String, pub left: String, pub right: String, pub added: usize, pub removed: usize }`
- `pub fn is_patch_file(path: &str) -> bool`（扩展名 .patch/.diff）
- `pub fn parse_patch(text: &str) -> Option<ParsedPatch>`：
  - 解析 unified diff：`--- a/xxx`、`+++ b/xxx`、`@@ -l,c +l,c @@` hunk
  - 构建左侧文本（上下文 + `-` 行）与右侧文本（上下文 + `+` 行）
  - 统计 added/removed 行数
- 支持多 hunk 拼接

### src/gui/patchtab.rs（新文件）
- `pub struct PatchTab { path, left: String, right: String, parsed: Option<ParsedPatch>, error, apply_msg }`
- `new(path)`：读取文件 → parse_patch → 若成功用 build_rows 渲染差异
- 渲染：复用 DiffTab 式双栏虚拟化（行号 + 状态色 + 行内高亮），底部统计（added/removed）
- 工具栏：
  - 应用补丁（把右侧内容写回 b 侧路径；A2 模式 .bak 备份）
  - 打开补丁文件（对话框）
- 空会话：打开文件按钮 + 拖拽提示

### 路由（mod.rs + main.rs）
- Tab 枚举加 `Patch(PatchTab)`（title/ui/状态栏分支）
- 拖放/双文件路由：任一侧为 .patch/.diff 且另一侧为空 → PatchTab（拖拽单补丁文件）
- CLI：`bcr gui --patch <file>` 或复用 --edit 自动识别？加 `--patch` 参数

### i18n
- 新 key ×10 语言：PatchTitle / ApplyPatch / PatchApplied / PatchAdded / PatchRemoved

### 测试
- patchview 单元测试：标准 unified diff 解析、a/b 还原、added/removed 统计、多 hunk
- uikit 测试：PatchTab 打开显示差异不 panic；应用补丁写回文件
