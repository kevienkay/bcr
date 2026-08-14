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
/// P32-A1：左右面板之间空隙宽度（画差异连接线）
pub const MID_GAP: f32 = 14.0;

/// 差异色（BC 语义：仅左/删除=红，仅右/插入=绿，修改=黄）
/// P39-2b：对齐 BC 5.2.5 柔和色调（浅色：删除 rgb(253,224,223) 系淡红/插入淡绿/修改淡黄）
pub fn diff_delete() -> Color32 {
    Color32::from_rgb(226, 110, 110)
}
#[allow(dead_code)]
pub fn diff_insert() -> Color32 {
    Color32::from_rgb(110, 196, 128)
}
pub fn diff_modify() -> Color32 {
    Color32::from_rgb(224, 190, 96)
}

/// P39-2b：当前差异行左侧竖条（BC 蓝色系，代替原黄色）
pub fn current_bar() -> Color32 {
    Color32::from_rgb(86, 148, 240)
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
    match letter {
        'L' => diff_delete(),
        'R' => Color32::from_rgb(110, 150, 240),
        'C' | 'M' => diff_modify(),
        _ => ui.visuals().weak_text_color(),
    }
}

/// 错误文本色（各 tab 错误显示统一，批次 3 接入）
#[allow(dead_code)]
pub fn error_color() -> Color32 {
    Color32::from_rgb(240, 110, 110)
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
    style.visuals.widgets.hovered.bg_stroke = border;
}

/// 应用主题样式（启动时调用，对 Dark/Light 两套都设置）
pub fn apply(ctx: &egui::Context) {
    for theme in [egui::Theme::Dark, egui::Theme::Light] {
        ctx.style_mut_of(theme, |style| {
            apply_style(style, theme == egui::Theme::Dark);
        });
    }
}
