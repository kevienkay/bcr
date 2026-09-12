//! UI 主题样式引擎（P31）：集中管理视觉常量，对标 Beyond Compare 观感。
//!
//! - 行高/字号/间距/圆角统一常量
//! - 差异配色（仅左=红 / 仅右=绿 / 修改=黄，BC 语义）按深浅主题微调
//! - `apply()` 在应用启动时对 Dark/Light 两主题统一设置控件样式

use eframe::egui::{self, Color32, FontId};

/// 行高（文本对比/目录树/表格行统一）
pub const ROW_H: f32 = 22.0;
/// BC 5.2.5 设计稿：列表行高（文件夹/表格）——逐视图对齐时接入
#[allow(dead_code)]
pub const ROW_H_LIST: f32 = 26.0;
/// BC 5.2.5 设计稿：代码/十六进制行高——逐视图对齐时接入
#[allow(dead_code)]
pub const ROW_H_CODE: f32 = 20.0;
/// 等宽字号
pub const FONT_SIZE: f32 = 14.0;
/// 行号字号
pub const GUTTER_SIZE: f32 = 12.0;
/// 控件圆角
pub const CORNER: f32 = 4.0;
/// 工具栏/面板内边距（批次 2 主窗口布局使用）
#[allow(dead_code)]
pub const PANEL_PAD: f32 = 6.0;
/// 工具栏控件间距
pub const ITEM_GAP: f32 = 6.0;
/// 当前行左侧竖条宽度（BC 风格当前差异标记，批次 3 DiffTab 使用）
#[allow(dead_code)]
pub const CURRENT_BAR: f32 = 3.0;
/// P32-A1：左右面板之间空隙宽度（画差异连接线 + P58 内联覆盖箭头 ◀▶）
/// BC 5.2.5 设计稿：分隔槽 22px（原 26）
pub const MID_GAP: f32 = 22.0;

// ===== BC 5.2.5 设计稿布局阶梯（design-tokens.json → layout）=====
// 说明：这些是设计稿的布局 token 目录；逐视图对齐（P2/P3）时逐步接入。
// 未接入前统一 allow(dead_code)，与 theme.rs 既有 token 目录约定一致。
/// 顶部菜单栏高度
#[allow(dead_code)]
pub const MENUBAR_H: f32 = 30.0;
/// 会话标签栏高度
#[allow(dead_code)]
pub const TABBAR_H: f32 = 40.0;
/// 单个标签页高度
#[allow(dead_code)]
pub const TAB_H: f32 = 29.0;
/// 工具栏高度（图标 19 + 文字 10.5）
#[allow(dead_code)]
pub const TOOLBAR_H: f32 = 56.0;
/// 工具栏按钮尺寸（宽 × 高）
#[allow(dead_code)]
pub const TOOLBAR_BTN: [f32; 2] = [62.0, 46.0];
/// 工具栏图标字号
#[allow(dead_code)]
pub const TOOLBAR_ICON: f32 = 19.0;
/// 工具栏按钮文字字号
#[allow(dead_code)]
pub const TOOLBAR_LABEL: f32 = 10.5;
/// 视图模式分段控件高度
#[allow(dead_code)]
pub const SEG_H: f32 = 26.0;
/// 状态栏总高（两行）
#[allow(dead_code)]
pub const STATUSBAR_H: f32 = 48.0;
/// 状态栏单行高度（两行分格，已接入 status_bar）
pub const STATUSBAR_ROW_H: f32 = 24.0;
/// 差异色条宽度（中缝）
#[allow(dead_code)]
pub const DIFF_BAR: f32 = 3.0;

/// 差异色（BC 语义：仅左/删除=红，仅右/插入=绿，修改=黄）
/// P39-2b：对齐 BC 5.2.5 柔和色调（深色主题：淡红/淡绿/淡黄）
/// P50-fix：浅色主题下用深色系，保证白底上文字对比度（BC 浅色主题差异文字为深红/深绿/深黄）
pub fn diff_delete(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(226, 110, 110)
    } else {
        // BC 5.2.5 设计稿 diff-del-fg #E01E10（更饱和、更接近 BC）
        Color32::from_rgb(224, 30, 16)
    }
}
#[allow(dead_code)]
pub fn diff_insert(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(110, 196, 128)
    } else {
        Color32::from_rgb(40, 140, 80)
    }
}
pub fn diff_modify(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(224, 190, 96)
    } else {
        Color32::from_rgb(176, 140, 40)
    }
}

/// P39-2b：当前差异行左侧竖条（BC 蓝色系，代替原黄色）
pub fn current_bar(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(86, 148, 240)
    } else {
        Color32::from_rgb(40, 90, 200)
    }
}

/// 行级底色（BC 5.2.5 设计稿采样值）
/// 仅左/删除 = 红底 #FDE0DF；仅右/新增 = 绿底 #CCE1D8；修改行 = 琥珀底 #FBF0C8
///
/// P3：深色主题不吃浅色 pastel（会在深底上烧出亮块），改用同语义的半透明饱和色；
/// 与 `hl_*` 家族同一套取色口径（浅色 pastel / 深色 translucent）。
pub fn bg_left_only(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgba_unmultiplied(226, 110, 110, 70)
    } else {
        Color32::from_rgb(253, 224, 223)
    }
}
/// 文本比较「修改行」两侧底色（设计稿：均为琥珀 #FBF0C8）
pub fn bg_modified_l(dark: bool) -> Color32 {
    bg_modified(dark)
}
pub fn bg_modified_r(dark: bool) -> Color32 {
    bg_modified(dark)
}
/// 修改行底色（琥珀）：浅色取设计稿值，深色取半透明琥珀
fn bg_modified(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgba_unmultiplied(200, 160, 60, 70)
    } else {
        Color32::from_rgb(251, 240, 200)
    }
}
/// BC 5.2.5 设计稿 diff-ins-bg：新增/仅右行底色（独立 token，不复用 bg_modified_r）
pub fn diff_ins_bg(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgba_unmultiplied(110, 196, 128, 70)
    } else {
        Color32::from_rgb(204, 225, 216)
    }
}
pub fn bg_match() -> Color32 {
    Color32::from_rgba_unmultiplied(224, 190, 96, 32)
}
/// 当前差异行底色（比 match 更强的描边感）
pub fn bg_current() -> Color32 {
    Color32::from_rgba_unmultiplied(120, 170, 250, 60)
}
/// 选中单元格底色（P37-1c：CSV 表格单元格选中）
pub fn bg_select() -> Color32 {
    Color32::from_rgba_unmultiplied(120, 170, 250, 70)
}
/// 行内变更段高亮（浅色主题用更柔和的粉/绿 pastel 呼应 BC，深色保持原值）
pub fn hl_delete(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgba_unmultiplied(226, 110, 110, 150)
    } else {
        Color32::from_rgba_unmultiplied(244, 158, 158, 118)
    }
}
pub fn hl_insert(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgba_unmultiplied(110, 196, 128, 150)
    } else {
        Color32::from_rgba_unmultiplied(158, 210, 168, 118)
    }
}
pub fn hl_modify_l(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgba_unmultiplied(226, 120, 120, 160)
    } else {
        Color32::from_rgba_unmultiplied(244, 168, 160, 122)
    }
}
pub fn hl_modify_r(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgba_unmultiplied(120, 210, 138, 160)
    } else {
        Color32::from_rgba_unmultiplied(164, 216, 178, 122)
    }
}

/// BC 主强调色（复选框填充、滑块轨道、焦点/选中强调）。深浅主题各一套。
pub fn accent(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(86, 148, 240)
    } else {
        // BC 5.2.5 设计稿 accent #0078F0
        Color32::from_rgb(0, 120, 240)
    }
}

// ===== BC 5.2.5 设计稿新增 token（菜单 / 状态栏 / 面板）=====

/// 菜单高亮底（白字）：#228EF4
#[allow(dead_code)]
pub fn menubar_hl(_dark: bool) -> Color32 {
    Color32::from_rgb(34, 142, 244)
}
/// 菜单置灰项前景：#B0B0B2（深色主题提亮）
#[allow(dead_code)]
pub fn menubar_disabled_fg(dark: bool) -> Color32 {
    if dark {
        Color32::from_gray(110)
    } else {
        Color32::from_rgb(176, 176, 178)
    }
}
/// 状态栏底：#DFE4EA
pub fn bg_status(dark: bool) -> Color32 {
    if dark {
        Color32::from_gray(30)
    } else {
        Color32::from_rgb(223, 228, 234)
    }
}
/// 主页左侧栏底：#DEE0E2
#[allow(dead_code)]
pub fn bg_sidebar(dark: bool) -> Color32 {
    if dark {
        Color32::from_gray(34)
    } else {
        Color32::from_rgb(222, 224, 226)
    }
}
/// 行内细线：#E0E4E5
#[allow(dead_code)]
pub fn line_soft(dark: bool) -> Color32 {
    if dark {
        Color32::from_gray(52)
    } else {
        Color32::from_rgb(224, 228, 229)
    }
}
/// 状态栏单元格分隔线：#C7CDD4
pub fn status_cell_sep(dark: bool) -> Color32 {
    if dark {
        Color32::from_gray(58)
    } else {
        Color32::from_rgb(199, 205, 212)
    }
}
/// 正文色：#1C1C1E
#[allow(dead_code)]
pub fn fg(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(230, 230, 232)
    } else {
        Color32::from_rgb(28, 28, 30)
    }
}
/// 次级文字：#4A4A4F
pub fn fg2(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(170, 170, 175)
    } else {
        Color32::from_rgb(74, 74, 79)
    }
}
/// 面板/标题栏底：#F0F4F5（浅色，对齐设计稿）
#[allow(dead_code)]
pub fn bg_window(dark: bool) -> Color32 {
    if dark {
        Color32::from_gray(38)
    } else {
        Color32::from_rgb(240, 244, 245)
    }
}

// ===== BC 5.2.5 设计稿 P2：图片遮罩 / 差异小地图 / 行对齐留白 =====

/// 差异小地图宽（设计稿 76px）
pub const MINIMAP_W: f32 = 76.0;

/// 行对齐留白 45° 斜纹线色（间距 4px；浅色 rgba(0,0,0,.055)）
pub fn hatch_line(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgba_unmultiplied(255, 255, 255, 26)
    } else {
        Color32::from_rgba_unmultiplied(0, 0, 0, 14)
    }
}

/// 图片差异遮罩：仅左图有 #E13C32
pub fn mask_left() -> Color32 {
    Color32::from_rgb(225, 60, 50)
}

/// 图片差异遮罩：仅右图有 #FFC83C
pub fn mask_right() -> Color32 {
    Color32::from_rgb(255, 200, 60)
}

/// 图片比较画布底 #292821
pub fn img_canvas() -> Color32 {
    Color32::from_rgb(41, 40, 33)
}

/// 行号颜色（P39-2b：适中灰，深浅主题都清晰）
pub const GUTTER: Color32 = Color32::from_gray(128);

/// P56-UI：zebra 条纹底色（目录/表格偶数行，仅黑白微差不喧宾夺主）
pub fn zebra_bg(dark: bool) -> Color32 {
    if dark {
        Color32::from_gray(28)
    } else {
        Color32::from_rgb(243, 244, 248)
    }
}

/// 状态徽标前景色（目录对比/合并视图，批次 3 使用）
/// P60 路线2：仅左红 · 仅右琥珀 · 已修改蓝
#[allow(dead_code)]
pub fn status_fg(ui: &egui::Ui, letter: char) -> Color32 {
    let dark = ui.visuals().dark_mode;
    match letter {
        'L' => diff_delete(dark),
        'R' => status_right(dark),
        'C' | 'M' => status_modified(dark),
        'B' => status_binary(dark),
        _ => ui.visuals().weak_text_color(),
    }
}

/// 错误文本色（各 tab 错误显示统一，批次 3 接入）
#[allow(dead_code)]
pub fn error_color() -> Color32 {
    Color32::from_rgb(240, 110, 110)
}

// ===== P51 批次 1：语义化颜色收敛（替代各 tab 散落硬编码）=====
// P60（BC 5.2.5 设计稿 · 路线2）：文件夹四态语义色
//   仅左 = 红 #E01E10 · 仅右 = 琥珀 #A9761A · 已修改 = 蓝 #0A63C9 · 二进制不同 = 紫 #6A3FA0

/// 文件夹状态：仅左侧（红）
///
/// P3：深色主题下设计稿的深色前景（#E01E10 等）在深底上对比度不足，
/// 深色分支一律取同色相提亮版；浅色分支严格保持设计稿采样值。
pub fn status_left(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(226, 110, 110)
    } else {
        Color32::from_rgb(224, 30, 16)
    }
}
/// 文件夹状态：仅右侧（琥珀/黄）
pub fn status_right(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(217, 169, 58)
    } else {
        Color32::from_rgb(169, 118, 26)
    }
}
/// 文件夹状态：已修改 / 内容不同（蓝）
pub fn status_modified(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(110, 168, 240)
    } else {
        Color32::from_rgb(10, 99, 201)
    }
}
/// 文件夹状态：二进制不同（紫）
pub fn status_binary(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(169, 139, 224)
    } else {
        Color32::from_rgb(106, 63, 160)
    }
}
/// 文件夹状态行底色：仅左红 / 仅右琥珀 / 已修改蓝 / 二进制紫（设计稿 .drow 各级底）
pub fn bg_only_left(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgba_unmultiplied(226, 110, 110, 70)
    } else {
        Color32::from_rgb(253, 224, 223)
    }
}
pub fn bg_only_right(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgba_unmultiplied(200, 160, 60, 70)
    } else {
        Color32::from_rgb(251, 240, 200)
    }
}
pub fn bg_modified_row(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgba_unmultiplied(86, 148, 240, 70)
    } else {
        Color32::from_rgb(220, 233, 250)
    }
}
#[allow(dead_code)]
pub fn bg_binary_row(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgba_unmultiplied(160, 120, 220, 70)
    } else {
        Color32::from_rgb(239, 230, 248)
    }
}
/// 文件信息头背景（DiffTab 头部两栏）
#[allow(dead_code)]
pub fn head_bg(dark: bool) -> Color32 {
    if dark {
        Color32::from_gray(38)
    } else {
        Color32::from_rgb(232, 233, 238)
    }
}
/// 文件信息头前景（蓝色系，BC 观感）
#[allow(dead_code)]
pub fn head_fg(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(150, 190, 240)
    } else {
        Color32::from_rgb(60, 110, 190)
    }
}
/// 目录名/文件夹蓝（DirTab 树、主页标题）
pub fn folder_color(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(140, 180, 235)
    } else {
        Color32::from_rgb(60, 110, 190)
    }
}
/// 列头背景（DirTab 名称/大小/时间列头；浅色对标 BC #fbfcfc）
pub fn column_head_bg(dark: bool) -> Color32 {
    if dark {
        Color32::from_gray(42)
    } else {
        Color32::from_rgb(249, 250, 252)
    }
}
/// 统计色：相同（绿）——全局状态栏与 DiffTab 底部统计栏统一
pub fn stat_same(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(120, 190, 120)
    } else {
        Color32::from_rgb(50, 140, 80)
    }
}
/// 统计色：仅左/删除（红）
pub fn stat_delete(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(220, 120, 120)
    } else {
        Color32::from_rgb(190, 70, 70)
    }
}
/// 统计色：仅右/插入（绿）
pub fn stat_insert(dark: bool) -> Color32 {
    stat_same(dark)
}
/// 统计色：修改（黄）
pub fn stat_modify(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(220, 190, 110)
    } else {
        Color32::from_rgb(170, 130, 40)
    }
}
/// 行号栏（gutter）底色
pub fn gutter_bg(dark: bool) -> Color32 {
    if dark {
        Color32::from_gray(38)
    } else {
        Color32::from_rgb(240, 241, 245)
    }
}
/// 左右面板空隙（连接线区）底色，比 gutter 略深一档
pub fn mid_bg(dark: bool) -> Color32 {
    if dark {
        Color32::from_gray(24)
    } else {
        Color32::from_rgb(244, 245, 248)
    }
}
/// 无差异行空隙垂直分隔线
pub fn mid_sep(dark: bool) -> Color32 {
    if dark {
        Color32::from_gray(48)
    } else {
        Color32::from_rgb(214, 216, 222)
    }
}
/// 字符列标尺底色
pub fn ruler_bg(dark: bool) -> Color32 {
    if dark {
        Color32::from_gray(32)
    } else {
        Color32::from_rgb(246, 247, 250)
    }
}
/// 忽略行弱化底色
pub fn ignored_dim(dark: bool) -> Color32 {
    if dark {
        Color32::from_gray(42)
    } else {
        Color32::from_rgb(228, 229, 234)
    }
}
/// 折叠行提示条底色
pub fn fold_bg(dark: bool) -> Color32 {
    if dark {
        Color32::from_gray(26)
    } else {
        Color32::from_rgb(242, 243, 247)
    }
}
/// 主页卡片底色
pub fn card_bg(dark: bool) -> Color32 {
    if dark {
        Color32::from_gray(36)
    } else {
        Color32::from_rgb(250, 250, 252)
    }
}
/// 标签栏选中标签底色
pub fn tab_selected_bg(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(52, 58, 70)
    } else {
        Color32::from_rgb(228, 232, 240)
    }
}
/// 隔离提示条底色（黄褐系）
pub fn banner_isolate_bg(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(46, 42, 20)
    } else {
        Color32::from_rgb(255, 248, 210)
    }
}
/// 对齐提示条底色（青绿系）
pub fn banner_align_bg(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(22, 46, 42)
    } else {
        Color32::from_rgb(215, 248, 240)
    }
}
/// 合并冲突标记（未解决，黄）
pub fn conflict_color(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(240, 180, 60)
    } else {
        Color32::from_rgb(200, 140, 40)
    }
}
/// 合并已解决标记（绿）
pub fn resolved_color(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(110, 230, 120)
    } else {
        Color32::from_rgb(60, 160, 80)
    }
}
/// 图片差异/错误红
pub fn img_diff(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(230, 80, 80)
    } else {
        Color32::from_rgb(200, 60, 60)
    }
}
/// 图片相同绿
pub fn img_same(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(90, 190, 90)
    } else {
        Color32::from_rgb(50, 150, 70)
    }
}
/// 同步消息提示（黄）
pub fn sync_msg_color(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(230, 180, 80)
    } else {
        Color32::from_rgb(200, 150, 50)
    }
}
/// 补丁行/计划行提示（黄）
pub fn plan_color(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(230, 170, 60)
    } else {
        Color32::from_rgb(200, 140, 40)
    }
}
/// 选中叠加色（文本选区/补丁选中行，蓝色半透明）
pub fn selection_overlay() -> Color32 {
    Color32::from_rgba_unmultiplied(86, 148, 240, 60)
}
/// 同步计划：复制操作（蓝）
pub fn plan_copy(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(80, 160, 255)
    } else {
        Color32::from_rgb(40, 100, 220)
    }
}
/// 同步计划：合并操作（黄）
pub fn plan_merge(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(200, 160, 60)
    } else {
        Color32::from_rgb(170, 130, 40)
    }
}
/// 图片帧：普通帧边框（灰）
pub fn frame_normal(dark: bool) -> Color32 {
    if dark {
        Color32::from_gray(90)
    } else {
        Color32::from_gray(140)
    }
}
/// 合并冲突行 base 侧底色（灰红半透明）
pub fn merge_conflict_bg() -> Color32 {
    Color32::from_rgba_unmultiplied(120, 90, 90, 60)
}

/// 主页会话卡片图标色（7 类：文本/文件夹/三路合并/图片/CSV/Hex/媒体）。
/// 配合"彩色图标底片"（半透明色块 + 同色符号）使用，弥补 egui 无彩色 emoji
/// 字形的限制——单色 NotoEmoji 符号经此着色后获得品牌色观感。
pub fn card_icon_colors() -> [Color32; 7] {
    [
        Color32::from_rgb(96, 158, 240),  // 0 文本对比（蓝）
        Color32::from_rgb(74, 184, 160),  // 1 文件夹对比（青）
        Color32::from_rgb(238, 158, 74),  // 2 三路合并（橙）
        Color32::from_rgb(168, 122, 230), // 3 图片对比（紫）
        Color32::from_rgb(104, 186, 108), // 4 CSV 表格（绿）
        Color32::from_rgb(92, 118, 222),  // 5 Hex 对比（靛）
        Color32::from_rgb(226, 118, 178), // 6 媒体比较（粉）
    ]
}

/// 对某主题应用统一样式（间距/圆角/选中态/面板层次）
fn apply_style(style: &mut egui::Style, dark: bool) {
    use egui::epaint::CornerRadius;
    use egui::Stroke;

    // 间距（4/8 密度网格：8 的整数/一半，避免随意值）
    style.spacing.item_spacing = egui::vec2(ITEM_GAP, ITEM_GAP);
    style.spacing.button_padding = egui::vec2(10.0, 5.0);
    style.spacing.interact_size.y = 26.0;
    style.spacing.indent = 14.0;
    // 字号分级（Heading/正文/按钮/等宽/Small）——BC 用字号+粗细做层级而非艳色
    style
        .text_styles
        .insert(egui::TextStyle::Heading, FontId::proportional(17.0));
    style
        .text_styles
        .insert(egui::TextStyle::Monospace, FontId::monospace(FONT_SIZE));
    style
        .text_styles
        .insert(egui::TextStyle::Body, FontId::proportional(FONT_SIZE));
    style
        .text_styles
        .insert(egui::TextStyle::Button, FontId::proportional(FONT_SIZE));
    style
        .text_styles
        .insert(egui::TextStyle::Small, FontId::proportional(11.5));
    // 圆角
    for w in [
        &mut style.visuals.widgets.noninteractive,
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
        &mut style.visuals.widgets.open,
    ] {
        w.corner_radius = CornerRadius::same(CORNER as u8);
    }
    // 面板层次：深色下用更深的背景区分工具栏/内容区
    // P56-UI 精修：窗口统一圆角（BC/原生桌面窗感），深浅主题圆角一致
    style.visuals.window_corner_radius = CornerRadius::same(8);
    style.visuals.window_fill = if dark {
        Color32::from_gray(30)
    } else {
        Color32::from_rgb(252, 252, 254)
    };
    if dark {
        style.visuals.panel_fill = Color32::from_gray(24);
        style.visuals.extreme_bg_color = Color32::from_gray(18);
        style.visuals.faint_bg_color = Color32::from_gray(34);
        style.visuals.window_stroke = Stroke::new(1.0, Color32::from_gray(56));
    } else {
        // Pxx-浅色：BC 纸面观感——轻微冷白 + 面板间细描边，层次靠同色系微差
        style.visuals.panel_fill = Color32::from_rgb(248, 249, 252);
        style.visuals.extreme_bg_color = Color32::from_rgb(240, 242, 246);
        style.visuals.faint_bg_color = Color32::from_rgb(245, 246, 249);
        style.visuals.window_stroke = Stroke::new(1.0, Color32::from_rgb(206, 208, 216));
    }
    // 选中态
    style.visuals.selection.bg_fill = if dark {
        Color32::from_rgb(52, 102, 180)
    } else {
        Color32::from_rgb(186, 212, 246)
    };
    style.visuals.selection.stroke = Stroke::new(1.0, Color32::from_gray(120));
    // BC 蓝色强调（Slider 填充轨、ComboBox、超链接等默认控件统一走这个主色）
    style.visuals.hyperlink_color = if dark {
        Color32::from_rgb(86, 148, 240)
    } else {
        Color32::from_rgb(40, 90, 200)
    };
    // 输入框底色（TextEdit 默认靠此与面板区分，形成"内嵌输入区"观感）
    style.visuals.text_edit_bg_color = Some(if dark {
        Color32::from_gray(15)
    } else {
        Color32::from_rgb(255, 255, 255)
    });
    // 按钮边框
    let border = if dark {
        Stroke::new(1.0, Color32::from_gray(64))
    } else {
        Stroke::new(1.0, Color32::from_gray(190))
    };
    style.visuals.widgets.inactive.bg_stroke = border;
    // P56-5：BC 式细浮动滚动条（悬浮覆盖内容、不挤占空间，观感更简洁专业）
    style.spacing.scroll = egui::style::ScrollStyle {
        floating: true,
        bar_width: 8.0,
        floating_width: 3.0,
        bar_inner_margin: 2.0,
        bar_outer_margin: 0.0,
        handle_min_length: 20.0,
        ..Default::default()
    };
    style.visuals.widgets.hovered.bg_stroke = border;
    // P55 按钮背景（BC 工具栏观感：浅色主题按钮浅灰圆角底，深色主题深灰底）
    if dark {
        style.visuals.widgets.noninteractive.weak_bg_fill = Color32::from_gray(34);
        style.visuals.widgets.inactive.weak_bg_fill = Color32::from_gray(42);
        style.visuals.widgets.hovered.weak_bg_fill = Color32::from_gray(58);
        style.visuals.widgets.active.weak_bg_fill = Color32::from_gray(50);
        style.visuals.widgets.open.weak_bg_fill = Color32::from_gray(46);
    } else {
        // Pxx-浅色：按钮底色略偏冷白到浅灰蓝，hover/active 清晰可辨
        style.visuals.widgets.noninteractive.weak_bg_fill = Color32::from_rgb(247, 248, 251);
        style.visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(233, 234, 239);
        style.visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(219, 221, 228);
        style.visuals.widgets.active.weak_bg_fill = Color32::from_rgb(211, 213, 222);
        style.visuals.widgets.open.weak_bg_fill = Color32::from_rgb(225, 227, 234);
    }
    // 菜单/弹层：统一圆角 + 柔和阴影（下拉/右键菜单、tooltip）
    style.visuals.menu_corner_radius = CornerRadius::same(5);
    style.visuals.popup_shadow = egui::epaint::Shadow {
        offset: [2, 5],
        blur: 8,
        spread: 0,
        color: if dark {
            Color32::from_black_alpha(110)
        } else {
            Color32::from_black_alpha(30)
        },
    };
    // 对话框：柔化阴影（深浅一致）
    style.visuals.window_shadow = egui::epaint::Shadow {
        offset: [3, 8],
        blur: 12,
        spread: 0,
        color: if dark {
            Color32::from_black_alpha(120)
        } else {
            Color32::from_black_alpha(28)
        },
    };
}

/// 应用主题样式（启动时调用，对 Dark/Light 两套都设置）
pub fn apply(ctx: &egui::Context) {
    for theme in [egui::Theme::Dark, egui::Theme::Light] {
        ctx.style_mut_of(theme, |style| {
            apply_style(style, theme == egui::Theme::Dark);
        });
    }
}
