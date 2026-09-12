# P62 · BC 5.2.5 设计稿对齐收口（P3）

> 设计基准：`design/BC菜单和状态栏/`（入口 `交接/实现交接.md`、`交接/design-tokens.json`、`screens/*.html`）
> 本文记录 **P3 收口**阶段的主题取色政策、验收口径与仍未接线的项。

## 一、主题取色政策（本轮确立）

设计稿只提供**浅色**采样值（`design-tokens.json` 的 `colors[].spec` 逐像素取自 BC 截图）。
深色主题没有设计参照，因此约定：

| 类别 | 浅色 | 深色 |
|---|---|---|
| 差异/状态**行底色** | 严格取设计稿采样值（如修改行 `#FBF0C8`） | 同语义**半透明饱和色**（如 `rgba(200,160,60,70)`） |
| 差异/状态**前景色** | 取设计稿采样值（如仅左 `#E01E10`） | 同色相**提亮版**（如 `rgb(226,110,110)`），保证深底对比度 |
| 布局常量、遮罩色、图片画布底 | 两套主题共用（画布底本身即深色 `#292821`） | 同左 |

取色口径与既有 `hl_delete/hl_insert/hl_modify_*` 家族一致：**浅色 pastel / 深色 translucent**。

实现落点：`src/gui/theme.rs`。差异/状态语义色一律带 `dark: bool` 参数；调用方透传
`ui.visuals().dark_mode`（`common.rs` 的兼容别名与 `status_color`、`theme::status_fg` 做中转）。

### 新增 dark 分支的 12 个函数（P62）

`bg_left_only`、`bg_modified_l`、`bg_modified_r`、`diff_ins_bg`、
`status_left`、`status_right`、`status_modified`、`status_binary`、
`bg_only_left`、`bg_only_right`、`bg_modified_row`、`bg_binary_row`。

## 二、验收口径

```bash
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

- 视觉回归：`BCR_SNAP_DIR=<dir> cargo test --bin bcr gui::ui_snap -- --ignored`
  （无头快照，含 `*_light` 与深色两套，不依赖屏幕录制权限）。
- 基准图：`ui_review/baseline-2026-09-12/`（12 张 PNG + `analyze.py` 输出）。
- 与设计稿的**像素级对照**（画板转 PNG + 窗口 1:1 截图 + 色带/主色量化）见
  `~/Downloads/bcr界面核对-2026-09-12/`（2026-09-12 那轮的交付）。

## 三、仍未接线 / 未覆盖（下一轮候选）

1. **工具栏高度**：`theme::TOOLBAR_H`(56) / `TABBAR_H`(40) / `MENUBAR_H`(30) / `STATUSBAR_H`(48)
   仍是带 `#[allow(dead_code)]` 的 token 目录，**未被绘制代码引用**。实测窗口截图的工具栏约
   40px、状态栏约 58px，与设计稿 56 / 48 有差。接线需同时改 9 个 tab 的工具栏绘制
   （`csvtab / difftab / dirtab / foldermergetab / imagetab / mergetab / patchtab / textedit` 等
   各自的 `SHOW_TOOLBAR` 分支），属**明显视觉变更**，需先决策再动。
2. **菜单展开态核对**：`screens/menus.html` 3 张画板（16 条置灰规则）在 macOS 走原生菜单栏，
   窗口级截图拍不到，需全屏截图 + 辅助功能驱动。
3. **状态栏高度**：`STATUSBAR_H`(48) 未接线，实际两行为 58px（`STATUSBAR_ROW_H`=24 已接入）。
4. **各屏第 2 张画板**：文本比较「交换两边」、文件夹比较「压缩包」等变体尚未逐张核对。

## 四、相关文档

- 设计稿入口与源码映射：`design/BC菜单和状态栏/交接/实现交接.md`
- 历史 UI 研究：`docs/BC-UI-study.md`、`docs/P33-ui-bc5.md`、`docs/P39-UI-study.md`
