//! UI 主题样式引擎（P31）：集中管理视觉常量，对标 Beyond Compare 观感。
//!
//! - 行高/字号/间距/圆角统一常量
//! - 差异配色（仅左=红 / 仅右=绿 / 修改=黄，BC 语义）按深浅主题微调
//! - `apply()` 在应用启动时对 Dark/Light 两主题统一设置控件样式

use eframe::egui::{self, Color32, FontId};

/// 行高（文本对比/目录树/表格行统一）
pub const ROW_H: f32 = 22.0;
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
pub const MID_GAP: f32 = 26.0;

/// 差异色（BC 语义：仅左/删除=红，仅右/插入=绿，修改=黄）
/// P39-2b：对齐 BC 5.2.5 柔和色调（深色主题：淡红/淡绿/淡黄）
/// P50-fix：浅色主题下用深色系，保证白底上文字对比度（BC 浅色主题差异文字为深红/深绿/深黄）
pub fn diff_delete(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(226, 110, 110)
    } else {
        Color32::from_rgb(196, 60, 60)
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

/// 行级底色（半透明，深浅主题通用；BC 5.2.5 实测浅红/浅绿/浅黄）
pub fn bg_left_only() -> Color32 {
    Color32::from_rgba_unmultiplied(246, 96, 96, 40)
}
pub fn bg_right_only() -> Color32 {
    Color32::from_rgba_unmultiplied(96, 196, 118, 38)
}
pub fn bg_modified_l() -> Color32 {
    Color32::from_rgba_unmultiplied(246, 96, 96, 48)
}
pub fn bg_modified_r() -> Color32 {
    Color32::from_rgba_unmultiplied(96, 196, 118, 48)
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
/// 行内变更段高亮
pub fn hl_delete() -> Color32 {
    Color32::from_rgba_unmultiplied(226, 110, 110, 150)
}
pub fn hl_insert() -> Color32 {
    Color32::from_rgba_unmultiplied(110, 196, 128, 150)
}
pub fn hl_modify_l() -> Color32 {
    Color32::from_rgba_unmultiplied(226, 120, 120, 160)
}
pub fn hl_modify_r() -> Color32 {
    Color32::from_rgba_unmultiplied(120, 210, 138, 160)
}

/// 行号颜色（P39-2b：适中灰，深浅主题都清晰）
pub const GUTTER: Color32 = Color32::from_gray(128);

/// 状态徽标前景色（目录对比/合并视图，批次 3 使用）
#[allow(dead_code)]
pub fn status_fg(ui: &egui::Ui, letter: char) -> Color32 {
    let dark = ui.visuals().dark_mode;
    match letter {
        'L' => diff_delete(dark),
        'R' => Color32::from_rgb(110, 150, 240),
        'C' | 'M' => diff_modify(dark),
        _ => ui.visuals().weak_text_color(),
    }
}

/// 错误文本色（各 tab 错误显示统一，批次 3 接入）
#[allow(dead_code)]
pub fn error_color() -> Color32 {
    Color32::from_rgb(240, 110, 110)
}

// ===== P51 批次 1：语义化颜色收敛（替代各 tab 散落硬编码）=====

/// BC 状态徽标：孤儿（仅左/仅右）紫
pub fn status_orphan() -> Color32 {
    Color32::from_rgb(83, 44, 199)
}
/// BC 状态徽标：差异/移动红
pub fn status_differ() -> Color32 {
    Color32::from_rgb(246, 39, 16)
}
/// 文件信息头背景（DiffTab 头部两栏）
pub fn head_bg(dark: bool) -> Color32 {
    if dark {
        Color32::from_gray(38)
    } else {
        Color32::from_gray(230)
    }
}
/// 文件信息头前景（蓝色系，BC 观感）
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
        Color32::from_rgb(251, 252, 252)
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
        Color32::from_gray(238)
    }
}
/// 左右面板空隙（连接线区）底色，比 gutter 略深一档
pub fn mid_bg(dark: bool) -> Color32 {
    if dark {
        Color32::from_gray(24)
    } else {
        Color32::from_gray(244)
    }
}
/// 无差异行空隙垂直分隔线
pub fn mid_sep(dark: bool) -> Color32 {
    if dark {
        Color32::from_gray(48)
    } else {
        Color32::from_gray(210)
    }
}
/// 字符列标尺底色
pub fn ruler_bg(dark: bool) -> Color32 {
    if dark {
        Color32::from_gray(32)
    } else {
        Color32::from_gray(244)
    }
}
/// 忽略行弱化底色
pub fn ignored_dim(dark: bool) -> Color32 {
    if dark {
        Color32::from_gray(42)
    } else {
        Color32::from_gray(226)
    }
}
/// 折叠行提示条底色
pub fn fold_bg(dark: bool) -> Color32 {
    if dark {
        Color32::from_gray(26)
    } else {
        Color32::from_gray(240)
    }
}
/// 主页卡片底色
pub fn card_bg(dark: bool) -> Color32 {
    if dark {
        Color32::from_gray(36)
    } else {
        Color32::from_gray(250)
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

/// P58：桌面平台枚举（Windows / macOS / Linux）。
/// 用于顶部全局工具栏等需要按平台就地适配观感的部分（与 `install_cjk_fonts`
/// 用 `cfg!(target_os)` 探测字体同一模式）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PlatformKind {
    MacOs,
    Windows,
    Linux,
}

/// 当前运行平台（`cfg!(target_os)` 三选一；非 macOS/Windows 一律视为 Linux 族）。
pub fn current_platform() -> PlatformKind {
    if cfg!(target_os = "macos") {
        PlatformKind::MacOs
    } else if cfg!(target_os = "windows") {
        PlatformKind::Windows
    } else {
        PlatformKind::Linux
    }
}

/// P58：顶部全局工具栏的平台化样式（配色/分隔/行高/按钮尺寸）。
pub struct ToolbarStyle {
    /// 工具栏背景
    pub bg: Color32,
    /// 工具栏下缘分隔线
    pub bottom_stroke: egui::Stroke,
    /// 工具栏行高（保证三平台字体差异下高度一致、不溢出）
    pub row_height: f32,
    /// 图标按钮边长（紧凑原生观感）
    pub button_size: f32,
}

/// P58：按平台返回工具栏样式。macOS 用更接近系统工具栏的浅灰/深灰、
/// 下缘细分隔；Windows/Linux 用平坦面板底色 + 1px 下缘分割。
pub fn toolbar_style(dark: bool, p: PlatformKind) -> ToolbarStyle {
    match p {
        PlatformKind::MacOs => ToolbarStyle {
            bg: if dark {
                Color32::from_rgb(30, 30, 32)
            } else {
                Color32::from_rgb(235, 235, 237)
            },
            bottom_stroke: egui::Stroke::new(1.0, Color32::from_gray(if dark { 48 } else { 172 })),
            row_height: 34.0,
            button_size: 22.0,
        },
        PlatformKind::Windows | PlatformKind::Linux => ToolbarStyle {
            bg: if dark {
                Color32::from_rgb(32, 32, 34)
            } else {
                Color32::from_rgb(245, 245, 245)
            },
            bottom_stroke: egui::Stroke::new(1.0, Color32::from_gray(if dark { 52 } else { 200 })),
            row_height: 32.0,
            button_size: 21.0,
        },
    }
}

/// 对某主题应用统一样式（间距/圆角/选中态/面板层次）
fn apply_style(style: &mut egui::Style, dark: bool) {
    use egui::epaint::CornerRadius;
    use egui::Stroke;

    // 间距
    style.spacing.item_spacing = egui::vec2(ITEM_GAP, ITEM_GAP);
    style.spacing.button_padding = egui::vec2(10.0, 4.0);
    style.spacing.interact_size.y = 24.0;
    style.spacing.indent = 14.0;
    // 文本样式
    style
        .text_styles
        .insert(egui::TextStyle::Monospace, FontId::monospace(FONT_SIZE));
    style
        .text_styles
        .insert(egui::TextStyle::Body, FontId::proportional(FONT_SIZE));
    style
        .text_styles
        .insert(egui::TextStyle::Button, FontId::proportional(FONT_SIZE));
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
    if dark {
        style.visuals.panel_fill = Color32::from_gray(24);
        style.visuals.window_fill = Color32::from_gray(30);
        style.visuals.extreme_bg_color = Color32::from_gray(18);
        style.visuals.faint_bg_color = Color32::from_gray(34);
    } else {
        style.visuals.panel_fill = Color32::from_gray(248);
        style.visuals.window_fill = Color32::from_gray(252);
        style.visuals.extreme_bg_color = Color32::from_gray(240);
        style.visuals.faint_bg_color = Color32::from_gray(244);
        style.visuals.window_stroke = Stroke::new(1.0, Color32::from_gray(200));
    }
    // 选中态
    style.visuals.selection.bg_fill = if dark {
        Color32::from_rgb(46, 92, 160)
    } else {
        Color32::from_rgb(190, 214, 244)
    };
    style.visuals.selection.stroke = Stroke::new(1.0, Color32::from_gray(120));
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
        style.visuals.widgets.noninteractive.weak_bg_fill = Color32::from_gray(246);
        style.visuals.widgets.inactive.weak_bg_fill = Color32::from_gray(229);
        style.visuals.widgets.hovered.weak_bg_fill = Color32::from_gray(212);
        style.visuals.widgets.active.weak_bg_fill = Color32::from_gray(206);
        style.visuals.widgets.open.weak_bg_fill = Color32::from_gray(220);
    }
}

/// 应用主题样式（启动时调用，对 Dark/Light 两套都设置）
pub fn apply(ctx: &egui::Context) {
    for theme in [egui::Theme::Dark, egui::Theme::Light] {
        ctx.style_mut_of(theme, |style| {
            apply_style(style, theme == egui::Theme::Dark);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_platform_maps_to_three_kinds() {
        // current_platform() 依当前编译目标返回三种平台之一；恰好落在枚举内。
        let p = current_platform();
        let _ = match p {
            PlatformKind::MacOs | PlatformKind::Windows | PlatformKind::Linux => true,
        };
        // 枚举共有三平台且互斥（Copy/PartialEq 保证可比）
        assert_ne!(PlatformKind::MacOs, PlatformKind::Windows);
        assert_ne!(PlatformKind::Windows, PlatformKind::Linux);
        assert_ne!(PlatformKind::Linux, PlatformKind::MacOs);
    }

    #[test]
    fn toolbar_style_diffs_by_platform_and_theme() {
        for p in [
            PlatformKind::MacOs,
            PlatformKind::Windows,
            PlatformKind::Linux,
        ] {
            for dark in [false, true] {
                let s = toolbar_style(dark, p);
                // 三平台三主题都应有非零行高与按钮尺寸、稳定底色
                assert!(s.row_height > 0.0);
                assert!(s.button_size > 0.0);
                assert!(s.bottom_stroke.width > 0.0);
                assert_ne!(s.bg, Color32::TRANSPARENT);
            }
        }
        // macOS 与 Windows 行高/配色应不同（平台差异化生效）
        let mac_light = toolbar_style(false, PlatformKind::MacOs);
        let win_light = toolbar_style(false, PlatformKind::Windows);
        assert_ne!(mac_light.row_height, win_light.row_height);
        assert_ne!(mac_light.bg, win_light.bg);
        // 深/浅主题底色应不同（深浅差异化生效）
        assert_ne!(
            toolbar_style(true, PlatformKind::Linux).bg,
            toolbar_style(false, PlatformKind::Linux).bg
        );
        // 同平台同主题应稳定（纯函数）
        assert_eq!(
            toolbar_style(true, PlatformKind::MacOs).bg,
            toolbar_style(true, PlatformKind::MacOs).bg
        );
    }
}
