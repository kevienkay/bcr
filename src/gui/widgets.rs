//! BC 风格控件（自绘）——在 egui 内置控件之上封装一套统一观感的 widget。
//!
//! 颜色/间距/圆角全部取自 `super::theme`，图标取自 `super::icons`（矢量字体）。
//! 目标：工具栏按钮（图标+文字、扁平圆角、hover/active 底色）、纯图标按钮、
//! 单选 chip、会话卡片、分组分隔线等，全应用一致。

use super::{icons, theme};
use eframe::egui::{self, Align2, Color32, FontId, Margin, Pos2, Rect, RichText, Stroke, Ui, Vec2};

fn glyph_format(size: f32, color: Color32) -> egui::TextFormat {
    egui::TextFormat {
        font_id: icons::font(size),
        color,
        ..Default::default()
    }
}

fn text_format(size: f32, color: Color32) -> egui::TextFormat {
    egui::TextFormat {
        font_id: FontId::proportional(size),
        color,
        ..Default::default()
    }
}

fn icon_text_job(
    icon: Option<icons::Icon>,
    text: &str,
    size: f32,
    color: Color32,
) -> egui::WidgetText {
    let mut job = egui::text::LayoutJob::default();
    if let Some(ic) = icon {
        job.append(&ic.glyph().to_string(), 0.0, glyph_format(size, color));
        job.append("  ", 0.0, text_format(size, color));
    }
    job.append(text, 0.0, text_format(size, color));
    egui::WidgetText::LayoutJob(std::sync::Arc::new(job))
}

fn btn_style(ui: &Ui) -> (u8, Color32, Stroke, Color32) {
    let dark = ui.visuals().dark_mode;
    let corner = theme::CORNER as u8;
    let (fill, stroke) = if dark {
        (
            Color32::from_gray(42),
            Stroke::new(1.0, Color32::from_gray(64)),
        )
    } else {
        (
            Color32::from_gray(229),
            Stroke::new(1.0, Color32::from_gray(190)),
        )
    };
    (corner, fill, stroke, ui.visuals().text_color())
}

/// 工具栏按钮：可选图标 + 文字 + 悬停提示。BC 扁平圆角风格。
#[allow(dead_code)]
pub fn tool_button(
    ui: &mut Ui,
    icon: Option<icons::Icon>,
    text: &str,
    tooltip: &str,
) -> egui::Response {
    tool_button_enabled(ui, true, icon, text, tooltip)
}

/// 工具栏按钮（enabled 变体）：不可用时灰显。
#[allow(dead_code)]
pub fn tool_button_enabled(
    ui: &mut Ui,
    enabled: bool,
    icon: Option<icons::Icon>,
    text: &str,
    tooltip: &str,
) -> egui::Response {
    let (corner, fill, stroke, txt) = btn_style(ui);
    let resp = ui
        .add_enabled(
            enabled,
            egui::Button::new(icon_text_job(icon, text, 14.0, txt))
                .corner_radius(corner)
                .fill(fill)
                .stroke(stroke),
        )
        .on_hover_text(tooltip);
    // 无障碍/测试标签用纯文本（不含图标字形），避免与含同词的后台状态 label 冲突
    resp.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, text.to_owned())
    });
    resp
}

/// 纯图标按钮（替代 ▾/📁/✕/🔍）。用 tooltip 兼作无障碍标签，便于测试按功能名查询。
#[allow(dead_code)]
pub fn icon_button(ui: &mut Ui, icon: icons::Icon, tooltip: &str, size: f32) -> egui::Response {
    let (corner, fill, stroke, txt) = btn_style(ui);
    let resp = ui
        .add(
            egui::Button::new(
                RichText::new(icon.glyph().to_string())
                    .font(icons::font(size))
                    .color(txt),
            )
            .corner_radius(corner)
            .fill(fill)
            .stroke(stroke),
        )
        .on_hover_text(tooltip);
    resp.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tooltip.to_owned())
    });
    resp
}

/// 工具栏分组弱分隔线。
#[allow(dead_code)]
pub fn separator(ui: &mut Ui) {
    ui.separator();
}

/// BC 风格复选框：圆角框 + 选中品牌蓝填充 + 白色勾选。返回 Response（`.changed()`/`.clicked()` 可用）。
#[allow(dead_code)]
pub fn check(ui: &mut Ui, checked: &mut bool, text: &str) -> egui::Response {
    use egui::StrokeKind;
    let dark = ui.visuals().dark_mode;
    let box_size = 15.0;
    let text_color = ui.visuals().text_color();
    let galley =
        ui.painter()
            .layout_no_wrap(text.to_string(), FontId::proportional(14.0), text_color);
    let total_w = box_size + 6.0 + galley.size().x + 4.0;
    let (rect, mut resp) = ui.allocate_exact_size(egui::vec2(total_w, 20.0), egui::Sense::click());
    if resp.clicked() {
        *checked = !*checked;
        resp.mark_changed();
    }
    let box_rect = Rect::from_min_size(
        Pos2::new(rect.left() + 2.0, rect.center().y - box_size / 2.0),
        Vec2::splat(box_size),
    );
    let accent = theme::accent(dark);
    if *checked {
        ui.painter().rect_filled(box_rect, 4.0, accent);
        ui.painter().text(
            box_rect.center(),
            Align2::CENTER_CENTER,
            icons::Icon::Equal.glyph().to_string(),
            icons::font(10.5),
            Color32::WHITE,
        );
    } else {
        ui.painter()
            .rect_filled(box_rect, 4.0, Color32::TRANSPARENT);
        let border = if dark {
            Color32::from_gray(90)
        } else {
            Color32::from_gray(170)
        };
        ui.painter()
            .rect_stroke(box_rect, 4.0, Stroke::new(1.0, border), StrokeKind::Inside);
        if resp.hovered() {
            let hover = if dark {
                Color32::from_gray(150)
            } else {
                Color32::from_gray(110)
            };
            ui.painter()
                .rect_stroke(box_rect, 4.0, Stroke::new(1.5, hover), StrokeKind::Inside);
        }
    }
    let text_pos = Pos2::new(
        box_rect.right() + 6.0,
        rect.center().y - galley.size().y / 2.0,
    );
    ui.painter().galley(text_pos, galley, text_color);
    // 无障碍/测试标签：以 Checkbox 角色 + 文案暴露
    resp.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Checkbox, true, text.to_owned())
    });
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// BC 风格下拉组合框：主题化按钮（选中文本 + Phosphor caret-down）+ 弹出菜单。
/// `add_contents` 在弹出菜单里渲染可选项（一般用 `selectable_label`）。
#[allow(dead_code)]
pub fn combo(
    ui: &mut Ui,
    selected_text: &str,
    tooltip: &str,
    add_contents: impl FnOnce(&mut Ui),
) -> egui::Response {
    let txt = ui.visuals().text_color();
    let mut job = egui::text::LayoutJob::default();
    job.append(selected_text, 0.0, text_format(14.0, txt));
    job.append("  ", 0.0, text_format(14.0, txt));
    job.append(
        &icons::Icon::Next.glyph().to_string(),
        0.0,
        glyph_format(12.0, txt),
    );
    let resp = ui
        .menu_button(
            egui::WidgetText::LayoutJob(std::sync::Arc::new(job)),
            |ui| {
                add_contents(ui);
            },
        )
        .response
        .on_hover_text(tooltip);
    // 让 accesskit/测试把它当作 ComboBox
    resp.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::ComboBox, true, selected_text.to_owned())
    });
    resp
}

/// 单选 chip（分段按钮）：选中高亮圆角，未选中弱色。用于设置页语言/格式、视图过滤。
#[allow(dead_code)]
pub fn chip(ui: &mut Ui, selected: bool, text: &str) -> egui::Response {
    let dark = ui.visuals().dark_mode;
    let fill = if selected {
        theme::tab_selected_bg(dark)
    } else if dark {
        Color32::from_gray(38)
    } else {
        Color32::from_gray(244)
    };
    let text = if selected {
        RichText::new(text).color(ui.visuals().strong_text_color())
    } else {
        RichText::new(text).color(ui.visuals().text_color())
    };
    ui.add(
        egui::Button::new(text)
            .corner_radius(theme::CORNER as u8)
            .fill(fill),
    )
}

/// 纯图标标签（不交互，仅显示）。如标题/状态。
#[allow(dead_code)]
pub fn icon_label(ui: &mut Ui, icon: icons::Icon, size: f32, color: Color32) {
    ui.label(
        RichText::new(icon.glyph().to_string())
            .font(icons::font(size))
            .color(color),
    );
}

/// 标签 chip（BC 式）：彩色类型图标 + 文本 + 关闭按钮。
/// 返回 (主响应, 关闭点击)。选中标签高亮底色+圆角；未选中弱色。
#[allow(dead_code)]
pub fn tab_chip(
    ui: &mut Ui,
    icon: icons::Icon,
    icon_color: Color32,
    text: &str,
    selected: bool,
    close_tooltip: &str,
) -> (egui::Response, bool) {
    let dark = ui.visuals().dark_mode;
    let tab_bg = if selected {
        theme::tab_selected_bg(dark)
    } else {
        Color32::TRANSPARENT
    };
    let text = if selected {
        RichText::new(text).strong()
    } else {
        RichText::new(text)
    };
    let resp = egui::Frame::new()
        .fill(tab_bg)
        .corner_radius(6.0)
        .inner_margin(Margin::symmetric(8, 4))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(icon.glyph().to_string())
                        .font(icons::font(13.0))
                        .color(icon_color),
                );
                let _ = ui.selectable_label(selected, text);
            });
        })
        .response
        .interact(egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    let close_color = if resp.hovered() {
        theme::stat_delete(dark)
    } else if selected {
        ui.visuals().strong_text_color()
    } else {
        ui.visuals().weak_text_color()
    };
    let close_resp = ui
        .add(
            egui::Button::new(
                RichText::new(icons::Icon::Close.glyph().to_string())
                    .font(icons::font(11.0))
                    .color(close_color),
            )
            .small()
            .corner_radius(theme::CORNER as u8),
        )
        .on_hover_text(close_tooltip);
    (resp, close_resp.clicked())
}

/// 会话/欢迎卡片：彩色图标底片 + 标题 (+ 副标题)。返回可交互响应。
#[allow(dead_code)]
pub fn card(
    ui: &mut Ui,
    icon: &str,
    icon_color: Color32,
    title: &str,
    subtitle: &str,
) -> egui::Response {
    let dark = ui.visuals().dark_mode;
    let txt = ui.visuals().text_color();
    let sub = ui.visuals().weak_text_color();
    egui::Frame::new()
        .corner_radius(theme::CORNER as u8)
        .fill(theme::card_bg(dark))
        .inner_margin(Margin::same(12))
        .show(ui, |ui| {
            ui.set_width(200.0);
            ui.vertical(|ui| {
                let (rect, _) = ui.allocate_exact_size(Vec2::new(40.0, 40.0), egui::Sense::hover());
                ui.painter().rect_filled(
                    rect,
                    9.0,
                    Color32::from_rgba_unmultiplied(
                        icon_color.r(),
                        icon_color.g(),
                        icon_color.b(),
                        38,
                    ),
                );
                ui.painter().text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    icon,
                    FontId::proportional(22.0),
                    icon_color,
                );
                ui.add_space(8.0);
                ui.label(RichText::new(title).size(13.0).color(txt).strong());
                if !subtitle.is_empty() {
                    ui.label(RichText::new(subtitle).size(10.5).color(sub));
                }
            });
        })
        .response
        .interact(egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}
