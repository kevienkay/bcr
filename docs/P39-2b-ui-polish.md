# P39-2b：UI 精修（字体 / 配色 / 图标 / gutter）

> 依据 `docs/P39-UI-study.md` B 表 UI 精修清单（样式层，可快速见效），
> 对齐 BC 5.2.5 实机观感。

## 字体（观感提升最大项）

- **等宽字体优先**：JetBrains Mono → 系统等宽（macOS SFNSMono/Menlo、Windows Consolas/Courier、Linux DejaVu Sans Mono/Liberation Mono），插入 Monospace 族首位
  - 行号 gutter / 代码内容 / hex 字节全部用等宽，观感对齐 BC
- **CJK fallback**：中/日/韩/阿拉伯字体追加到 Proportional + Monospace 末尾（10 语言可显示，原逻辑保留）
- 按平台探测路径，全部存在即加载

## diff 配色对齐 BC 柔和色调

- **前景**：删除淡红 rgb(226,110,110) / 插入淡绿 rgb(110,196,128) / 修改淡黄 rgb(224,190,96)（原高饱和 RGB 降饱和）
- **行级底色**（半透明，深浅通用）：仅左 rgba(246,96,96,40)、仅右 rgba(96,196,118,38)、修改 48、匹配 32
- **行内高亮段**同步降饱和
- **当前差异行竖条**：新增 `theme::current_bar()` 蓝色 rgb(86,148,240)，替换原黄色 diff_modify（BC 当前行竖条蓝色系）
- **当前行底色** bg_current 改蓝 rgba(120,170,250,60)

## 工具栏图标化（emoji → 矢量符号）

- `📋 剪贴板→左/右` → `⧉ 剪贴板→左/右`（与 DirTab 批量复制 ⧉ 风格统一）
- `🔧 外部工具对比` → `⚙ 外部工具对比`（与菜单 ⚙ 规则一致）
- 其余 emoji 按钮（⬆⬇⟳↻↩↪⇋⇵ 等）本已是矢量符号，保持

## 行号 gutter

- 行号颜色 `GUTTER` 118 → 128（深浅主题都清晰）
- 深色主题 gutter 底色 30 → 38（略亮于内容区，BC 浅灰底观感）

## 文件改动

- `src/gui/mod.rs`：`install_cjk_fonts` 重写为「等宽优先 + CJK fallback」
- `src/gui/theme.rs`：柔和配色 + `current_bar()` + GUTTER 微调
- `src/gui/difftab.rs`：竖条改蓝、gutter 深色底、剪贴板/外部工具图标
- `src/gui/dirtab.rs`：外部工具图标

本地 489 单元 + 4 kittest 全绿 / clippy 0 / fmt 干净。
