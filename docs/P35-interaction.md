# P35 交互深度对齐 BC — 文本对比核心操作

> 用户反馈：UI 交互太粗糙。本机已装 Beyond Compare 5.2.5，学它的 UI 跟交互。
> 参考：BC 官方帮助文档（`/Applications/Beyond Compare.app/Contents/Resources/en.lproj/Help/`）完整命令参考（commandsbc/commandstext/commandsdir…），逐条对比 bcr 现状。

## 差距分析（BC 命令参考 vs bcr 现状）

### 文本对比（核心，最高频）

| # | BC 交互 | bcr 现状 | 判定 |
|---|---------|----------|------|
| A1 | Copy to Other Side（复制差异块/选中行到另一侧，工具栏+右键+快捷键） | 无（右键只有复制路径/打开/忽略） | ❌ 缺失，最痛 |
| A2 | Swap Sides（交换左右） | 无（ImageTab 有，DiffTab 无） | ❌ 缺失 |
| A3 | View 过滤（Show All/Diff/Same/Context/None） | DiffTab 无过滤下拉（DirTab 有 ViewFilter） | ❌ 缺失 |
| A4 | Visible Whitespace（显示空白符） | 无 | ❌ 缺失 |
| B1 | Side-by-Side / Over-Under 布局切换 | 只有并排 | ❌ 缺失 |
| B2 | Bookmark（0-9 书签） | 无 | ❌ 缺失 |
| B3 | Next/Prev Edit（跳到编辑行） | 无 | ❌ 缺失 |
| B4 | 会话操作（Clear Session / Locked / Swap） | 无 | ❌ 缺失 |
| C1 | Align With / Isolate（手动对齐） | 无 | ❌ 高级 |
| C2 | Compare Parent Folders / Merge Files（派生会话） | 无 | ❌ 高级 |
| — | Reload / Recompare / 内联编辑 / 折叠 / 忽略 / 缩略图 / 行号 / Goto / 搜索替换 / 剪贴板 / 撤销重做 | 已有 | ✅ |

### 目录对比

| # | BC 交互 | bcr 现状 | 判定 |
|---|---------|----------|------|
| C3 | Copy File to Right/Left（逐文件复制）+ favor 操作 | 有后台同步，无简单逐文件复制 | ⚠️ 部分 |

## 实施计划（先 A 类，每项一提交）

1. **批次 1**：A1 Copy to Other Side（复制差异块到另一侧）——工具栏按钮 + 右键菜单 + 快捷键
2. **批次 2**：A2 Swap Sides（交换左右）——工具栏按钮 + 快捷键
3. **批次 3**：A3 View 过滤下拉（All/Diff/Same/Context）+ A4 Visible Whitespace
4. **批次 4**（后续）：B1 布局切换 / B2 书签 / B3 编辑导航 / B4 会话操作
5. **批次 5**（后续）：C1 手动对齐 / C2 派生会话 / C3 目录对比

## 验收
- 每批测试全绿 / clippy 0 / fmt 干净 / CI 三平台绿
- 新增 uikit 交互测试
