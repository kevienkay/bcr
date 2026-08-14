# P37-1j 外部工具 GUI 入口（消除审计空壳点）

> 背景：UI 功能对齐审计发现——Tools→外部工具弹窗是纯说明（0 按钮），
> GUI 右键没有「外部工具对比」入口（功能只在 CLI `bcr diff --external`）。
> BC 的文件菜单有「打开方式」子菜单可调第三方工具。本批补 GUI 入口。

## 实施内容

### src/gui/common.rs
- 新增 `pub fn external_compare(left: &str, right: &str) -> Option<String>`：
  - 加载 `ExternalTools`，任一侧扩展名有映射 → `run(template, left, right)` 执行
  - 返回 None（成功或无映射）或 Some(错误消息)

### DiffTab 右键菜单
- 「打开左侧文件/打开右侧文件」下方加「🔧 外部工具对比」：
  - 任一侧扩展名有外部工具映射时显示
  - 点击执行 external_compare（闭包外处理请求）

### DirTab 右键菜单
- 「在对比中打开 (Enter)」附近加「🔧 外部工具对比」：
  - 对选中文件用左右完整路径调用 external_compare
  - 无映射时不显示（或显示但禁用）

### 测试
- external_compare：有映射时返回 None（命令执行成功/失败不阻塞）、无映射时 None
- uikit：右键菜单在无映射时不出现「外部工具对比」；有映射时出现（用假 HOME + 临时 toml）
