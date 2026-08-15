# P41：DirTab 选择操作 + 视图过滤扩展 + 展开/折叠全部

> 依据 `docs/BC-UI-study.md` 差距清单 D4/D5/D7（BC 文件夹比较 编辑/视图菜单实测）：
> - D4 视图过滤扩展：显示独有/较新项/不独有（bcr 有 All/Diff/LeftOnly/RightOnly/Same，缺"较新"维度）
> - D5 选择操作：选择较新项/独有项/反向选择
> - D7 展开/折叠全部：编辑菜单

## 现状（bcr DirTab）

- `ViewFilter`：All / Diff / LeftOnly / RightOnly / Moved / Same（缺 LeftNewer / RightNewer）
- `FileStatus`：Same / LeftOnly / RightOnly / Differ / Moved（无较新判断）
- 选择：单选 `selected: Option<usize>`（键盘上下 + 打开选中），无多选集合
- 折叠：`collapsed: HashSet<String>`（目录路径集合），有 toggle 无展开/折叠全部

## 批次

- **P41-1 展开/折叠全部（D7）**：
  - `DirTab::expand_all()`：清空 collapsed；`collapse_all()`：把全部目录路径加入 collapsed
  - Edit 菜单（DirTab 分支）加「展开全部」「折叠全部」
- **P41-2 视图过滤扩展（D4）**：
  - `ViewFilter` 加 `LeftNewer` / `RightNewer`（Differ 且 mtime 左新/右新）
  - 过滤实现：`FileStatus::Differ` + 比较 left/right mtime
  - 工具栏下拉 + 快捷键可选
- **P41-3 选择操作（D5）**：
  - 多选集合 `selected_set: HashSet<usize>`（flat 索引）+ 渲染高亮
  - Edit 菜单（DirTab 分支）：选择较新项 / 选择独有项 / 反向选择 / 全选 / 取消选择
  - 键盘：Shift+上下 扩展选择（可选）

## 文件改动

- `src/gui/dirtab.rs`：展开/折叠全部、ViewFilter 扩展、多选集合 + 选择操作
- `src/gui/menubar.rs`：Edit 菜单 DirTab 分支（展开/折叠全部、选择操作）
- `src/gui/common.rs`：选中行高亮绘制（如需要）
- `src/i18n.rs` + `src/i18n_tables.rs`：新 key × 10 语言
- `src/gui/uikit_tests.rs`：测试

每批本地 cargo test 全绿 → fmt/clippy → 单提交推送；全部完成后统一查 CI。
