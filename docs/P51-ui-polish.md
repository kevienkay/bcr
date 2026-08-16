# P51 UI 精修美化 实施记录

用户指定做「UI 精修美化」规划并按顺序开工（颜色收敛 → 工具栏/状态栏 → 菜单快捷键 → 主页/行细节 → i18n 收敛），
5 个功能批次 + 1 个 docs 收尾，全部推送 `origin/master`。

## 批次 1：颜色收敛（theme.rs 扩展 + 全文件替换）

- theme.rs 新增 25 个语义函数：
  - `status_orphan/status_differ`（BC 紫/红状态徽标）
  - `head_bg/head_fg/folder_color/column_head_bg`（信息头/文件夹蓝/列头）
  - `stat_same/stat_delete/stat_insert/stat_modify`（**统一 difftab 底部与全局状态栏统计色差异**）
  - `gutter_bg/mid_bg/mid_sep/ruler_bg/ignored_dim/fold_bg`（行渲染底色）
  - `card_bg/tab_selected_bg/banner_isolate_bg/banner_align_bg`（主页/标签栏/提示条）
  - `conflict_color/resolved_color/img_diff/img_same/sync_msg_color/plan_color/plan_copy/plan_merge/frame_normal/merge_conflict_bg/selection_overlay`
- 替换范围：difftab（34 处）/mod（16 处）/dirtab/csvtab/mergetab/imagetab/mediatab/patchtab/foldermergetab/textedit/common 全部散落 Color32
- 统计色统一：difftab 底部统计栏 `240,120,120/120,230,130/235,210,100` → 与状态栏 `stat_*` 一致
- 错误色统一：`240,110,110/230,100,100/235,90,90` 三种 → `theme::error_color()` 唯一
- 顺带清理：difftab `hex_bytes_text` 中未使用的死代码颜色（`let _ = c`）删除
- 移除 8 个未使用 Color32 import（cargo fix）

## 批次 2：工具栏统一 + 状态栏 BC 分区扩展

- DiffTab 工具栏：移除编辑组后重复的 `ui.separator(); ui.separator();` → 单分隔符
- CsvTab：刷新按钮 `重新加载` → `⟳ 重新加载`（带 hover 提示，对齐 DirTab 符号集）
- MergeTab：空状态打开 BASE/LEFT/RIGHT 按钮加 📂 图标
- **状态栏 8 个 tab 全部改为 BC 分区**（对齐 DiffTab 既有 P47 分区）：
  - 左：路径弱色（文件名，Dir 双路径 / Csv 双路径 / Merge 三路径 / Image 双路径 / TextEdit 单路径 / Patch 单路径 / FolderMerge 双路径 / Media 双路径）
  - 中：彩色统计（Dir/Csv 同绿-仅左红-仅右绿-修改黄；Merge 冲突黄/全解决绿；Image 差异红/相同绿；Patch 新增绿-删除红；FolderMerge 复制蓝-合并黄-冲突红；Media 差异红/相同绿）
  - 右：右对齐元数据（Dir 选中项数；Csv 行列数）
- 新增 i18n key `StatsSelected`（"选中"，10 语言）；删除不再使用的 `StatsPanel`（10 语言表同步清理）

## 批次 3：菜单快捷键显示

- menubar.rs 新增 `sc(mac, win)` 平台辅助函数（macOS ⌘ 系 / Windows-Linux Ctrl 系）+ `menu_item()` 封装（`Button::shortcut_text`）
- 覆盖菜单项快捷键：
  - Session：新建标签页 ⌘T / 新建窗口 ⌘N / 保存会话 ⌥⌘S / 打开会话 ⌥⌘O / 重新比较 ⌘R / 清除会话 ⌥⌘C / 报告 ⌘P
  - File：保存文件为 ⌘⇧S
  - Edit：撤销 ⌘Z / 重做 ⌘Y / 对齐 ⌘A / 增加缩进 ] / 减少缩进 [
  - Search：查找 ⌘F / 选区查找 ⌘E / 替换 ⇧⌘F / 查找下一 ⌘G / 查找上一 ⇧⌘G / 转到行 ⌘L / 下一差异 F6 / 上一差异 F7 / 下一区块 ⇧⌃↓ / 上一区块 ⇧⌃↑ / 重新加载 F5 / 查找文件名 ⌘F / 多文件查找 ⌘⇧F
  - View：设置 ⌘, / 图例 ⇧L / 书签切换 ⌘⌥⌃0-9 / 书签跳转 ⌘0-9
  - Window：下一标签 ⌘] / 上一标签 ⌘[ / 最小化 ⌘M / 关闭所有 ⌘⇧W

## 批次 4：主页与行级细节

- **Sessions 面板**：列表项改 Frame 包裹（圆角 + 内边距），hover 半透明高亮（bg_match）+ 整行可点打开；右侧 ✕ 删除按钮（hover 红色提示），删除后 `session::save_all` 持久化；ScrollArea `auto_shrink([false, true])`
- **卡片网格自适应**：固定 4 列 → 按可用宽度 `((avail+10)/(card_w+10)).floor()` 计算列数（min 170px/列），换行条件同步
- **DiffTab 行 hover 高亮**：左右内容区在 `resp.hovered() && !ignored` 时叠加半透明 `bg_match()`；SideBySide 与上-下布局（paint_diff_row_v）同步

## 批次 5：i18n 硬编码收敛

- 新增 15 个 i18n key（10 语言全翻译）：
  - `OpenLeftFile/OpenRightFile`（打开左侧/右侧文件，工具栏 hover + 右键）
  - `CopyLeftPath/CopyRightPath`（复制左/右侧路径，右键菜单）
  - `ReloadHint`（重新加载 (F5)）、`DeleteSession`（删除会话）
  - `LineCount`（行数，信息弹窗）
  - `RevealLeft/RevealRight`（打开所在位置）、`SystemOpenLeft/SystemOpenRight`（系统打开）
  - `PlanFirst`（请先生成计划）、`NoDiffToCopy/NoDiffToDelete`（没有可复制/删除的差异文件）
  - `ClipboardUnavailable`（无法读取系统剪贴板）
- 替换范围：difftab/csvtab/dirtab/imagetab/mergetab/textedit/foldermergetab/mod 中 30+ 处硬编码

## 测试与质量

- 本地 **547 单元 + 4 kittest 全绿** / clippy 0 / fmt 干净
- 无新增依赖；i18n 变更同步 10 语言表（en/zh/de/ja/ko/es/pt/ar/ru/fr）
- 移除未使用 key `StatsPanel`（枚举 + 10 语言翻译），clippy 无 dead_code 告警
