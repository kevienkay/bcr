# P40：工具栏精简（P35-B0 落地）

> 依据 `docs/BC-UI-study.md` 六、工具栏精简对照：
> 「BC 工具栏一行 6 组，bcr 工具栏 30+ 控件塞一行 → 低频操作（剪贴板/编辑/搜索/替换）应收进菜单。」
> BC 文本比较工具栏参考：`Home | Sessions | [All▾ Diffs Context Minor Rules Format] | Copy | Next Section Prev Section | Swap Reload`

## 现状盘点（DiffTab 工具栏 34 控件，src/gui/difftab.rs:1771 起）

| 组 | 控件 | 处理 |
|---|---|---|
| 打开 | 打开左侧 / 打开右侧 | 保留（高频） |
| 打开 | ⧉ 剪贴板→左 / →右 | **收菜单**（File 菜单已有 MenuClipLeft/Right） |
| 显示 | 视图过滤下拉 All/Diff/Same/Context | 保留（BC All▾ 组） |
| 显示 | 统计栏 checkbox | **收菜单**（View 菜单已有 MenuStats） |
| 显示 | 忽略空白/行尾/大小写 3 个 checkbox | **收菜单**（设置对话框已集中管理） |
| 显示 | 自动换行 / 显示空白 checkbox | **收菜单**（View 菜单新增） |
| 显示 | hex 显示选项（地址格式/值格式 ComboBox + 显示地址） | 仅 hex 模式显示，收 View 菜单 |
| 显示 | 缩略图 checkbox | **收菜单**（View 菜单已有 MenuThumb） |
| 编辑 | 编辑左侧 / 编辑右侧 | **收菜单**（Edit 菜单新增） |
| 编辑 | 撤销 / 重做 | **收菜单**（Edit 菜单已有 MenuUndo/Redo） |
| 操作 | 复制到右侧 / 左侧 | 保留（BC Copy 组） |
| 操作 | 重新加载 / 交换两侧 | 保留（BC Swap Reload 组） |
| 导航 | 差异计数 label | 保留（紧凑） |
| 导航 | 下一差异 / 上一差异 | 保留（BC Next Section Prev Section） |
| 导航 | 跳转行输入框 + 转到按钮 | 保留（紧凑） |
| 搜索 | 搜索框 + 上一/下一匹配 + 计数 | 保留（常用） |
| 搜索 | 替换框 + 替换/全部替换按钮 | 保留（紧凑，BC 搜索组） |

目标：34 → ~18 控件，BC 式 6 组布局（打开 | 显示过滤 | 编辑 | 操作 | 导航 | 搜索替换）。

## 批次

- **P40-1 DiffTab 工具栏精简**：删剪贴板/编辑/忽略/显示选项等低频控件，菜单补位（Edit 菜单编辑左/右、View 菜单自动换行/显示空白/忽略选项），工具栏重排为 6 组；hex 模式选项移 View 菜单
- **P40-2 其他视图工具栏检查**：DirTab/CsvTab/ImageTab 等如有同样堆积同样精简（按需）
- **P40-3 收尾**：README/CHANGELOG + 统一查 CI
