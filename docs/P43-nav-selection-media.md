# P43：导航历史 + 选区操作 + 会话菜单补齐 + 媒体比较

> 依据 `docs/BC-UI-study.md`（会话菜单：后退/前进/浏览文件夹/上一层/比较父文件夹/合并文件/和输出比较；
> 编辑菜单：选择选择内容(D)/把选择内容和剪贴板比较；搜索菜单：下一个/上一个替换、差异文件导航）与
> `docs/P39-UI-study.md` A 表剩余项（P2 文本编辑打开剪贴板已做；P3 媒体比较）。

## 现状与剩余差距

| 差距 | BC 位置 | bcr 现状 | 处理 |
|---|---|---|---|
| 后退/前进/上一层/浏览文件夹 | 会话菜单（文件夹比较） | 无历史栈 | P43-1 |
| 比较父文件夹 | 会话菜单（全视图） | 无 | P43-1 |
| 选择选择内容(D)/把选择内容和剪贴板比较 | 编辑菜单（文本比较） | 无选区概念 | P43-2 |
| 下一个/上一个替换 | 搜索菜单（文本比较） | 有替换框无导航 | P43-3 |
| 差异文件（子会话）导航 | 搜索菜单（文件夹比较） | 有 flat 无「下一差异文件」 | P43-3 |
| 合并文件 | 会话菜单（文本比较） | DiffTab 无入口 | P43-4 |
| 和输出比较 | 会话菜单（文件夹合并/同步） | 无 | P43-4 |
| 信息（各视图统计信息弹窗） | 会话菜单（全视图） | 无独立信息弹窗 | P43-5 |
| 媒体比较（音视频） | 新建会话 | bcr 无 | P43-6（简化版：音视频元数据/帧信息比较） |

## 批次

- **P43-1 导航历史（DirTab 后退/前进/上一层/比较父文件夹）**：
  - `DirTab` 加历史栈 `history: Vec<(String, String)>` + `history_pos`；`navigate(paths)` 入栈，`back()`/`forward()` 出栈
  - Session 菜单（DirTab 分支）：后退/前进/上一层（跳父目录）/比较父文件夹（打开父目录对比）
  - 快捷键：⌥← / ⌥→（或 ⌘[ / ⌘]）
- **P43-2 文本选区操作（T6）**：
  - DiffTab 加选区：`selection: Option<(usize, usize)>`（起始/结束行），鼠标拖选或双击行范围
  - Edit 菜单（DiffTab 分支）：选择选择内容(D)（把当前差异块选为选区）、把选择内容和剪贴板比较（选区文本 → 剪贴板 → load_clipboard_right）
  - 选区高亮渲染
- **P43-3 替换导航 + 差异文件导航**：
  - DiffTab：`next_replace()`/`prev_replace()`（在 search.matches 中跳转并自动替换？BC 是导航到替换点）——实现为「下一/上一匹配并进入替换状态」
  - DirTab：`next_diff_file()`/`prev_diff_file()`（flat 中跳转差异文件并选中）
  - Search 菜单加对应项
- **P43-4 会话菜单补齐**：
  - DiffTab 分支「合并文件」：把当前左右文件作为 BASE/LEFT 开三路合并（或两路→merge 语义，补 RIGHT 选择）
  - FolderMerge/Sync 分支「和输出比较」：输出目录 vs 一侧目录对比
- **P43-5 信息弹窗**：
  - Session 菜单「信息」：当前标签统计（Diff 行数/编码/大小、Dir 条目数/统计、Csv 行列、Merge 冲突数等）
  - 独立 `show_info` 窗口
- **P43-6 媒体比较（简化版）**：
  - `mediacmp` 模块：读音视频文件元数据（时长/编码/码率/尺寸/帧率——用 ffprobe 外部命令或纯容器解析）对比
  - 新建会话「媒体比较」入口 + GUI 标签（元数据并排 + 差异标记）
  - 优先级最低，可先做元数据对比（mp3tag 思路扩展到音视频容器头）

## 文件改动

- `src/gui/dirtab.rs`：历史栈 + 导航方法
- `src/gui/difftab.rs`：选区 + 替换导航
- `src/gui/mod.rs`：信息弹窗、合并文件入口
- `src/gui/menubar.rs`：Session/Edit/Search 菜单分支补项
- `src/mediacmp.rs`（新）+ `src/gui/mediatab.rs`（新）：媒体比较
- `src/i18n.rs` + `src/i18n_tables.rs`：新 key × 10 语言
- `src/gui/uikit_tests.rs`：测试

每批本地 cargo test 全绿 → fmt/clippy → 单提交推送；全部完成后统一查 CI。
