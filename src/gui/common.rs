//! GUI 公共绘制辅助：颜色、单元格渲染（含行内高亮）、虚拟化行高常量。
//!
//! 颜色与视觉常量已收敛到 `super::theme`（P31），此处保留兼容别名并复用。

use crate::sideview::Cell;
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Vec2};

/// 虚拟化行高
pub const ROW_H: f32 = super::theme::ROW_H;
/// 等宽字体大小
pub const FONT_SIZE: f32 = super::theme::FONT_SIZE;

/// 差异底色（兼容别名，指向 theme 的统一配色）
pub fn bg_delete() -> Color32 {
    super::theme::bg_left_only()
}
pub fn bg_insert() -> Color32 {
    super::theme::bg_right_only()
}
pub fn bg_replace_l() -> Color32 {
    super::theme::bg_modified_l()
}
pub fn bg_replace_r() -> Color32 {
    super::theme::bg_modified_r()
}
pub fn bg_match() -> Color32 {
    super::theme::bg_match()
}
pub fn bg_match_current() -> Color32 {
    super::theme::bg_current()
}
/// 行内高亮（变更段背景）
pub fn hl_delete() -> Color32 {
    super::theme::hl_delete()
}
pub fn hl_insert() -> Color32 {
    super::theme::hl_insert()
}
pub fn hl_replace_l() -> Color32 {
    super::theme::hl_modify_l()
}
pub fn hl_replace_r() -> Color32 {
    super::theme::hl_modify_r()
}
/// 行号颜色
pub const GUTTER: Color32 = super::theme::GUTTER;

/// 绘制一行单元格的背景
pub fn paint_bg(ui: &egui::Ui, rect: Rect, bg: Option<Color32>) {
    if let Some(c) = bg {
        ui.painter().rect_filled(rect, 0.0, c);
    }
}

/// 在单元格内绘制带行内高亮的文本（LayoutJob 分段着色）
/// syntax 非空时叠加语法前景色（diff 语义管背景，语法管前景）
pub fn paint_cell(
    ui: &egui::Ui,
    rect: Rect,
    cell: Option<&Cell>,
    fg: Color32,
    hl: Option<Color32>,
    syntax: Option<&'static syntect::parsing::SyntaxReference>,
) {
    let Some(cell) = cell else { return };
    // 语法分段：字节偏移 -> (r,g,b)
    let syntax_segs = syntax.map(|s| crate::highlight::highlight_line(&cell.text, s));
    let mut job = egui::text::LayoutJob::default();
    let mut off = 0usize; // 当前字节偏移（cell.text 内）
    for (seg, changed) in &cell.segments {
        let seg_start = off;
        let seg_end = off + seg.len();
        let color = syntax_segs
            .as_ref()
            .and_then(|segs| {
                segs.iter()
                    .find(|(s, l, _)| *s < seg_end && *s + *l > seg_start)
                    .map(|(_, _, rgb)| Color32::from_rgb(rgb.0, rgb.1, rgb.2))
            })
            .unwrap_or(fg);
        job.append(
            seg,
            0.0,
            egui::TextFormat {
                font_id: FontId::monospace(FONT_SIZE),
                color,
                background: if *changed {
                    hl.unwrap_or(Color32::TRANSPARENT)
                } else {
                    Color32::TRANSPARENT
                },
                ..Default::default()
            },
        );
        off = seg_end;
    }
    let galley = ui.painter().layout_job(job);
    let y = rect.center().y - galley.size().y / 2.0;
    ui.painter()
        .galley(Pos2::new(rect.left() + 4.0, y), galley, fg);
}

/// 绘制行号（右对齐在 gutter 内）
pub fn paint_line_no(ui: &egui::Ui, rect: Rect, no: Option<usize>) {
    if let Some(n) = no {
        ui.painter().text(
            Pos2::new(rect.right() - 4.0, rect.center().y),
            Align2::RIGHT_CENTER,
            n.to_string(),
            FontId::monospace(super::theme::GUTTER_SIZE),
            GUTTER,
        );
    }
}

/// 计算行号列宽
pub fn gutter_width(max_no: usize) -> f32 {
    let digits = max_no.max(1).to_string().len() as f32;
    digits * 8.0 + 16.0
}

/// 主题下的前景文本色
pub fn text_color(ui: &egui::Ui) -> Color32 {
    ui.visuals().text_color()
}

/// 虚拟化渲染行（row_h 为实际行高；内部关闭 item_spacing 垂直间距保证对齐）
pub fn show_rows<R>(
    ui: &mut egui::Ui,
    total: usize,
    row_h: f32,
    add: impl FnOnce(&mut egui::Ui, std::ops::Range<usize>) -> R,
) -> egui::scroll_area::ScrollAreaOutput<R> {
    // 临时关闭 item spacing，使行高精确 = row_h
    let prev = ui.spacing().item_spacing.y;
    ui.spacing_mut().item_spacing.y = 0.0;
    let out = egui::ScrollArea::both()
        .auto_shrink([false, false])
        .show_rows(ui, row_h, total, add);
    ui.spacing_mut().item_spacing.y = prev;
    out
}

/// 带初始滚动偏移的虚拟化渲染（B4：hex 差异导航跳转用）
pub fn show_rows_offset<R>(
    ui: &mut egui::Ui,
    total: usize,
    row_h: f32,
    offset: egui::Vec2,
    add: impl FnOnce(&mut egui::Ui, std::ops::Range<usize>) -> R,
) -> egui::scroll_area::ScrollAreaOutput<R> {
    let prev = ui.spacing().item_spacing.y;
    ui.spacing_mut().item_spacing.y = 0.0;
    let out = egui::ScrollArea::both()
        .auto_shrink([false, false])
        .vertical_scroll_offset(offset.y)
        .horizontal_scroll_offset(offset.x)
        .show_rows(ui, row_h, total, add);
    ui.spacing_mut().item_spacing.y = prev;
    out
}

/// 状态色（目录对比/合并视图用）
pub fn status_color(ui: &egui::Ui, letter: char) -> Color32 {
    match letter {
        'L' => super::theme::diff_delete(),
        'R' => Color32::from_rgb(110, 150, 240),
        'C' | 'M' => super::theme::diff_modify(),
        _ => ui.visuals().weak_text_color(),
    }
}

pub fn vec2(x: f32, y: f32) -> Vec2 {
    Vec2::new(x, y)
}

/// P32-A4：用系统默认应用打开文件（跨平台）
pub fn open_with_system_app(path: &str) {
    #[cfg(target_os = "macos")]
    let cmd = "open";
    #[cfg(target_os = "windows")]
    let cmd = "explorer";
    #[cfg(all(unix, not(target_os = "macos")))]
    let cmd = "xdg-open";
    #[cfg(not(any(unix, windows)))]
    let cmd = "open";
    let _ = std::process::Command::new(cmd).arg(path).spawn();
}

/// P32-A4：在文件管理器中定位文件（macOS open -R / Windows explorer /select / Linux xdg-open 父目录）
pub fn reveal_in_file_manager(path: &str) {
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open")
            .args(["-R", path])
            .spawn();
    }
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("explorer")
            .args(["/select,", path])
            .spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        // Linux 无原生定位；打开父目录
        let parent = std::path::Path::new(path)
            .parent()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.to_string());
        let _ = std::process::Command::new("xdg-open").arg(parent).spawn();
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = std::process::Command::new("open")
            .args(["-R", path])
            .spawn();
    }
}
