# P33 UI 重构 — 对标 Beyond Compare 5.2.5 真实界面

> 背景：用户指出 bcr GUI「做的很差」，本机已安装 Beyond Compare 5.2.5
> （`/Applications/Beyond Compare.app`，窗口「主页」运行中）。
> 参考来源：BC 官方帮助文档 231 张真实界面截图（OCR 提取布局）+ 应用二进制 UI 文案。
> 目标：把 bcr 的 GUI 从「egui 默认观感」重构为 BC 式的**菜单栏 + 分组工具栏 + 文件信息头 + 双栏主体 + 状态栏**结构。

## 〇、BC 5.2.5 真实运行界面实测（2026-08-13 解锁屏幕抓取）

本地运行 BC 5.2.5（`bcomp` 打开演示目录/文件），窗口 `-l 1541/674` 截屏 + 逐像素采样：

### 配色实测值（浅色主题，bcr 对齐目标）
| 元素 | 实测 RGB | bcr 对应 |
|------|---------|---------|
| 重要差异行背景（文本对比） | `rgb(253,224,223)` 浅红 | bg_modified_l / bg_left_only 调浅 |
| 内容区背景 | `rgb(255,255,255)` | 默认 |
| 行交替底色 | `rgb(247,249,248)` | 可选 zebra |
| 工具栏底色 | `rgb(240,244,245)` | Panel top 底色 |
| 孤儿（仅一侧存在）文字 | `rgb(83,44,199)` 紫 | 新增 orphan 色 |
| 差异/较新文字 | `rgb(246,39,16)` 红 | diff_delete 调校 |
| 相同文字 | 黑 | 默认 |
| 文件夹图标色 | `rgb(159,199,236)` 浅蓝 | DirTab 文件夹行 |
| 列头底色 | `rgb(251,252,252)` | DirTab 表头 |

### 布局实测
- 主页：菜单栏 `会话` 下拉（新建…）；主区左侧**会话类型大图标按钮**（文件夹比较/文件夹合并/文件夹同步/文本比较/文本合并/文本编辑/16进制比较/媒体比较/图片比较…），提示语「将文件夹或文件拖放到会话图标上」
- 文本比较：顶部 工具栏（主页/会话/上下文/重要/交换…）→ 文件头两列（路径 + 时间/大小/编码格式详情行）→ 双栏代码
- 文件夹比较：工具栏（主页/会话/结构/交换/停止）→ 双路径头 → 列头 `名称|大小|已修改` → 行（状态色文字）→ 底部状态栏（文件数/字节数）

## 一、BC 5.2.5 真实界面规范（OCR 实证）

### 1. 主页（Home View）
- 标题：`- Home - BeyondCompare`
- 菜单栏：`Session View Tools Help`
- **左侧 Sessions 管理面板**：保存的会话列表（可搜索、管理）
- **主区域**：会话类型大按钮（带图标，竖排网格）：
  `Folder Compare / Folder Merge / Folder Sync / Text Compare / Text Merge / Hex Compare / Media Compare / Picture Compare / Registry Compare / Table Compare / Version Compare`
- 每类下方显示最近的会话（如 `C:\Workspace\Cirrus\Mine ↔ Theirs`）
- `Open` / `Edit` 按钮；底部 `Auto-saved` 区 + 搜索框
- 右下：Knowledge Base / Support Forums 链接

### 2. 文本对比（Text Compare）
- 标题：`文件名 - Text Compare - Beyond Compare`
- 菜单栏：`Session File Edit Search View Tools Help`
- **工具栏（分组）**：
  `Home | Sessions | [All ▾ Diffs Context Minor Rules Format] | Copy | Next Section Prev Section | Swap Reload`
- **文件信息头（两列）**：
  - 左：`C:\Workspace\Cirrus\Mine\Text\TextFiltered.pas`
  - 右：`C:\Workspace\Cirrus\Theirs\Text\TextFiltered.pas`
  - 详情行：`3/21/2006 4:13:24 PM | 17,576 bytes Delphi Source • ANSI • PC`
- **主体**：左右两栏代码，行号 + 语法色 + 差异背景色（红=重要差异、蓝=不重要差异、浅红/浅蓝背景）
- 差异行间有连接线；当前差异行高亮

### 3. 文件夹对比（Folder Compare）
- 标题：`Cirrus - Folder Compare - Beyond Compare`
- 菜单栏：`Session Actions Edit Search View Tools Help`
- **工具栏**：`Home | Sessions | [All ▾ Diffs Same Structure Minor Rules] | Copy Expand Collapse Select Files Refresh`
- **过滤行**：`Filters: **` 输入框 + `Swap | Stop | Filters | Peek`
- **双路径头**：左 `C:\Workspace\Cirrus\Mine` 右 `C:\Workspace\Cirrus\Theirs`（各带浏览按钮）
- **列头（每侧）**：`Name | Size | Modified`
- 状态颜色：灰=未知/旧、黑=相同、紫=孤儿（仅一侧存在）、红=较新或不同；文件夹色提示子项差异
- 中央列图标表示内容比较结果

### 4. 表格对比 / 合并 / Hex
- Table：工具栏 `All/Diffs/Same/Minor/Rules/Format | Copy | Next Diff Prev Diff | Swap Reload`，双表并排，表头 `Qty Value Device`
- Folder Merge：三路径头 `Mine | Ancestor | Theirs`，工具栏多 `Merge To Output`
- Hex：工具栏 `All/Diffs/Same/Rules/Format | Copy | Next Diff Prev Diff | Swap Reload`，双栏 hex + ASCII

### 5. 共性规范
- 顶部固定：标题栏 → 菜单栏 → 工具栏 → 文件信息头 → 主体
- 底部状态栏（统计/当前项）
- 按钮带 tooltip（long hint）；图标 + 文字
- 间距紧凑、控件圆角小、行高 ~20-22px

## 二、bcr 现状 vs BC 差距清单

| # | 差距 | BC 5.2.5 | bcr 现状（P32 后） | 级别 |
|---|------|----------|-------------------|------|
| A1 | 菜单栏 | Session/File/Edit/Search/View/Tools/Help 标准菜单 | 无菜单栏，顶部一行扁平按钮（打开文件/目录/云盘/合并/Git/会话/规则） | 高 |
| A2 | 工具栏分组 | 显示过滤下拉组 + 复制 + 差异导航 + Swap/Reload 分组布局 | 工具栏按钮平铺（打开左/右、剪贴板、忽略项、编辑、撤销、重载、搜索），无 BC 式分组 | 高 |
| A3 | 文件信息头 | 左右文件名 + 路径 + **日期时间/大小/格式详情行** | 仅左右文件名头部（6af720b），无详情行 | 高 |
| A4 | 主页布局 | 左侧 Sessions 面板 + 会话类型大按钮竖排 + 最近会话 + Open/Edit | 居中标题 + 5 卡片网格（3+2）+ 底部按钮 | 高 |
| A5 | 文件夹对比列头 | Name/Size/Modified 列头 + 状态色（紫孤儿/红差异/黑相同/灰未知） | 树形视图无列头，状态徽标圆形底（L 红/R 蓝/C 黄） | 中 |
| A6 | 差异配色语义 | 红=重要差异、蓝=不重要差异；浅红/浅蓝行背景 | 红/绿/黄（仅左/仅右/修改），无重要/不重要之分 | 中 |
| A7 | 会话类型覆盖 | 11 种类型按钮 | 欢迎页 5 卡片（文本/文件夹/合并/图片/CSV），缺 Hex/同步/版本等入口 | 中 |
| A8 | 状态栏 | 当前项 + 统计 | 已有底部状态栏（路径/行数/统计），可保留打磨 | 低 |
| A9 | 工具栏 tooltip/图标 | 图标+文字+long hint | 部分有 tooltip，图标 emoji 混排 | 低 |
| A10 | 标签栏 | 标签页 + 关闭 | 已有标签栏 + 拖拽 + 关闭（P32-B6），可保留 | 低 |

## 三、实施方案（分 4 批）

### 批次 1：全局框架 — 标准菜单栏 + 工具栏分组（A1/A2）
- `src/gui/menubar.rs` 新增：BC 式菜单栏 `Session | File | Edit | Search | View | Tools | Help`
  - Session：新建文本/文件夹/合并/图片/CSV/Hex 会话、保存会话、设置
  - File：打开左/右文件、打开目录、打开合并、云盘、剪贴板
  - Edit：撤销/重做（转发 DiffTab）、替换（转发）
  - Search：查找/下一差异/上一差异（F6/F7）
  - View：主题（系统/深/浅）、语言下拉、统计栏开关、缩略图开关
  - Tools：Git 配置、会话中心、规则、外部工具
  - Help：关于、快捷键说明
- 每个 Tab 的工具栏改为 BC 式分组：
  - 左组：显示过滤（`All ▾ Diffs Same Minor Rules Format` 语义 → 映射到 bcr 的 ViewFilter/选项）
  - 中组：操作（Copy/Next/Prev/Swap/Reload 按 tab 类型）
  - 右组：搜索
- 用 `egui::MenuBar` + `ui.separator()` 分区

### 批次 2：文件信息头 + 主页改造（A3/A4）
- DiffTab 文件信息头补详情行：左侧/右侧各显示
  `路径（monospace 弱色）` + `时间 | 大小 | 编码格式`（复用 encoding 检测结果）
- 主页重构为 BC 式：
  - 左侧窄面板：最近会话/自动保存列表（点按打开，复用 session 持久化）
  - 主区：会话类型大按钮竖排（文本/文件夹/三路合并/文件夹合并/图片/CSV/Hex/同步/版本），带图标 + 标题 + 简述
  - 底部：打开文件/打开目录按钮 + 搜索框
  - 拖放：拖到某类型按钮 → 直接开对应会话

### 批次 3：文件夹对比列头 + 配色对齐（A5/A6）
- DirTab 每侧加列头 `名称 | 大小 | 修改时间`（两侧对齐），树形缩进保留
- 状态色改为 BC 语义：
  - 孤儿（仅左/仅右）= 紫；差异 = 红；相同 = 黑/默认；未知/未扫 = 灰
  - 文件夹行按子项状态着色（红/紫混合提示）
- 文本对比补「不重要差异」概念：`--ignore-unimportant` 语义暂缓，先对齐行背景（浅红=重要差异、浅蓝=次要）——**具体以 BC 实际截图取色为准**

### 批次 4：收尾打磨（A7-A10）
- 欢迎页会话类型补 Hex/同步入口（映射到现有 hex 路由 / sync CLI 已存在）
- 工具栏图标统一 + tooltip 全量补 long hint
- 状态栏按 tab 展示当前项（当前行号/列、选中数）
- README/CHANGELOG 记录；全部 uikit 测试适配新布局（菜单路径变化）
- 每批提交 + 测试全绿 + CI 三平台验证

## 四、验收标准
- 每批：`cargo test` 全绿（现 392）+ clippy 0 警告 + fmt 干净 + CI 三平台绿
- uikit 测试适配：菜单项触发、工具栏按钮、主页会话卡片、DirTab 列头
- 视觉对照：与 BC 截图逐屏对比（菜单/工具栏/文件头/双栏/状态栏结构一致）

## 五、风险与说明
- egui 无原生菜单栏，用 `egui::MenuBar`（Panel::top 内 menubar）实现标准菜单外观
- BC 的「重要/不重要差异」需要 diff 引擎支持重要度标记（现有 `--ignore-lines` 可作基础），本轮先做配色与结构对齐，重要度标记列为后续项
- 语言 i18n：新增菜单/会话类型 key × 10 语言（宏保证穷尽，注意上次 pt 漏插教训）
- uikit 测试中按钮 label 变化需同步更新（get_by_label_contains 兜底）
