# P39-2d：细节三模式 + 布局 + 主页卡片 + 书签快捷键

> 依据 `docs/P39-UI-study.md`（4.4 细节模式 / 布局；A 表 P2 项；B 表第 3 条），
> 补齐 BC 5.2.5 视图菜单「细节」「布局」「书签」三大能力。

## 细节三模式（BC 视图菜单「细节」）

- DiffTab 新增 `detail_mode: DiffDetailMode`（Text/Hex/Align，默认 Text）
  - **文本细节**：默认行渲染
  - **16进制细节**：对文本文件也强制构建字节网格（复用 hexview::build_hex_rows），
    走 hex 渲染分支（含地址列/字节/ASCII、hex 差异导航）
  - **对齐方式细节**：对齐模式提示条 + 手动对齐行标记（P38-1b 既有能力）
- View 菜单「细节」子菜单切换；`set_detail_mode()` 文本文件切 Hex 时自动构建 hex 数据

## 布局（BC 视图菜单「布局」）

- DiffTab 新增 `layout: DiffLayout`（SideBySide / TopBottom / Web，默认并排）
  - **边并排**：原有左右两栏（各半宽 + 差异连接线）
  - **上-下**：左内容上半、右内容下半堆叠，行高 2*ROW_H（`row_h()`），
    各带 gutter 行号；当前行竖条整行高；双击上半=左、下半=右
  - **网页**：单栏全宽流式（同垂直堆叠渲染，供后续区分）
- 渲染循环宽度/行高按布局计算（`content_w`/`total_w`/`row_h()`）
- View 菜单「布局」子菜单切换；`set_layout()`

## 主页卡片精修（B 表第 3 条）

- 主页 8 张会话卡片：hover 蓝色描边高亮（`rect_stroke` 1.5px rgb(86,148,240)）+ 圆角 8 + PointingHand 光标
- 深色主题下卡片底色统一 gray(36)，浅色 gray(250)

## 书签 0-9 快捷键（BC ⌘⌥⌃0-9）

- DiffTab 新增 `bookmarks: HashMap<u8, usize>`（编号 → 渲染行索引）
  - `toggle_bookmark(no)`：当前顶部行绑定/取消（参照 PatchTab P37-1k）
  - `goto_bookmark(no)`：滚动到书签行并同步 diff_pos 高亮
  - `clear_bookmarks()`：清除全部
- 快捷键：`⌘⌥⌃0-9` 切换书签 / `⌘0-9` 跳转书签（输入框聚焦时不触发）
- View 菜单「书签」分组：切换 / 转到 / 清除

## 文件改动

- `src/gui/difftab.rs`：DiffDetailMode/DiffLayout 枚举、字段、书签方法、set_detail_mode/set_layout/row_h、handle_keys 书签快捷键、渲染循环布局宽度/行高、paint_diff_row 布局参数、新增 paint_diff_row_v 垂直绘制
- `src/gui/menubar.rs`：View 菜单「细节」「布局」「书签」子菜单
- `src/gui/mod.rs`：主页卡片 hover 描边精修
- `src/i18n.rs` + `src/i18n_tables.rs`：12 个新 key × 10 语言

## 测试（新增 2 个 uikit）

- `difftab_bookmarks_toggle_goto_clear`：切换/取消/跳转/清除 + ⌘⌥⌃1 键盘切换
- `difftab_detail_mode_hex_and_layout`：Hex 细节构建字节网格、布局行高切换

本地 493 单元 + 4 kittest 全绿 / clippy 0 / fmt 干净。
