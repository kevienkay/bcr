//! GUI 公共绘制辅助：颜色、单元格渲染（含行内高亮）、虚拟化行高常量。
//!
//! 颜色与视觉常量已收敛到 `super::theme`（P31），此处保留兼容别名并复用。

use crate::sideview::Cell;
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Vec2};

/// P42-4：工具栏全局开关（BC View>工具栏，各 tab 工具栏渲染统一受控）
pub static SHOW_TOOLBAR: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

/// P50：统一对话框——不透明背景（修复 Frame::new() 默认 fill 透明导致的透视）
/// + 屏幕居中显示（BC 风格弹窗）。所有弹窗应经此创建。
pub fn dialog_window<'a>(
    ctx: &egui::Context,
    title: impl Into<egui::WidgetText>,
) -> egui::Window<'a> {
    let style = ctx.style_of(ctx.theme());
    egui::Window::new(title)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .frame(
            egui::Frame::window(&style)
                .corner_radius(8.0)
                .inner_margin(egui::Margin::same(16))
                .fill(style.visuals.window_fill()),
        )
}

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
/// 选中单元格底色（P37-1c）
pub fn bg_select() -> Color32 {
    super::theme::bg_select()
}
/// 行内高亮（变更段背景）
pub fn hl_delete(dark: bool) -> Color32 {
    super::theme::hl_delete(dark)
}
pub fn hl_insert(dark: bool) -> Color32 {
    super::theme::hl_insert(dark)
}
pub fn hl_replace_l(dark: bool) -> Color32 {
    super::theme::hl_modify_l(dark)
}
pub fn hl_replace_r(dark: bool) -> Color32 {
    super::theme::hl_modify_r(dark)
}
/// 行号颜色
pub const GUTTER: Color32 = super::theme::GUTTER;

/// P52-2：统一空状态面板（BC 观感）——大号柔和色图标底片 + 标题 + 可选提示与操作行。
/// 各 tab 空会话（未选择文件/目录）复用，取代裸文字提示。
///
/// 注意：不用 `centered_and_justified`——其 main_justify 会把子组拉伸到全高、
/// 内容仍从顶部堆叠（实测不垂直居中）。这里先测量文本高度算出内容总高，
/// 再用顶部占位实现真正的垂直居中。
pub fn empty_state(
    ui: &mut egui::Ui,
    icon: &str,
    icon_color: Color32,
    title: &str,
    hint: &str,
    actions: impl FnOnce(&mut egui::Ui),
) {
    let avail = ui.available_rect_before_wrap();
    // 测量标题/提示文本高度（与实际渲染一致，避免估算偏差）
    let title_g = ui.painter().layout_no_wrap(
        title.to_string(),
        egui::FontId::proportional(16.0),
        ui.visuals().text_color(),
    );
    let hint_g = if hint.is_empty() {
        None
    } else {
        Some(ui.painter().layout_no_wrap(
            hint.to_string(),
            egui::FontId::proportional(11.0),
            ui.visuals().weak_text_color(),
        ))
    };
    let hint_h = hint_g.as_ref().map(|g| 4.0 + g.size().y).unwrap_or(0.0);
    let btn_h = 28.0;
    let content_h = 64.0 + 14.0 + title_g.size().y + hint_h + 14.0 + btn_h;
    let top = ((avail.height() - content_h) / 2.0).max(0.0);

    ui.vertical_centered(|ui| {
        ui.add_space(top);
        // 大号图标底片：64px 圆角色块 + 32px 同色符号（与主页卡片同风格）
        let (rect, _) = ui.allocate_exact_size(egui::vec2(64.0, 64.0), egui::Sense::hover());
        ui.painter().rect_filled(
            rect,
            14.0,
            Color32::from_rgba_unmultiplied(icon_color.r(), icon_color.g(), icon_color.b(), 38),
        );
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            icon,
            egui::FontId::proportional(32.0),
            icon_color,
        );
        ui.add_space(14.0);
        ui.label(
            egui::RichText::new(title)
                .size(16.0)
                .color(ui.visuals().weak_text_color()),
        );
        if let Some(_g) = hint_g {
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(hint)
                    .size(11.0)
                    .color(ui.visuals().weak_text_color()),
            );
        }
        ui.add_space(14.0);
        actions(ui);
    });
}

/// 绘制一行单元格的背景
pub fn paint_bg(ui: &egui::Ui, rect: Rect, bg: Option<Color32>) {
    if let Some(c) = bg {
        ui.painter().rect_filled(rect, 0.0, c);
    }
}

/// 把空白符替换为可见符号（P35-A4）：空格 → ·，制表符 → →
pub fn visible_ws(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            ' ' => out.push('·'),
            '\t' => out.push('→'),
            _ => out.push(c),
        }
    }
    out
}

/// 在单元格内绘制带行内高亮的文本（LayoutJob 分段着色）
/// syntax 非空时叠加语法前景色（diff 语义管背景，语法管前景）
/// x_off：横向滚动偏移（P33 长行栏内滑动查看），文本 x -= x_off，clip 保持栏内
/// show_ws：显示空白符（P35-A4，此时禁用语法高亮避免字节偏移错位）
#[allow(clippy::too_many_arguments)]
pub fn paint_cell(
    ui: &egui::Ui,
    rect: Rect,
    cell: Option<&Cell>,
    fg: Color32,
    hl: Option<Color32>,
    syntax: Option<&'static syntect::parsing::SyntaxReference>,
    x_off: f32,
    show_ws: bool,
) {
    let Some(cell) = cell else { return };
    // 语法分段：字节偏移 -> (r,g,b)（show_ws 时禁用，避免空白符号替换错位）
    let syntax_segs = if show_ws {
        None
    } else {
        syntax.map(|s| crate::highlight::highlight_line(&cell.text, s))
    };
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
        let display = if show_ws {
            visible_ws(seg)
        } else {
            seg.clone()
        };
        job.append(
            &display,
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
    // 裁剪到单元格内：长行在栏内截断（BC 式左右两页，不溢出到中线/对侧）
    let painter = ui.painter().with_clip_rect(rect);
    // P33：文本按横向滚动偏移平移（栏内滑动查看长行）
    painter.galley(Pos2::new(rect.left() + 4.0 - x_off, y), galley, fg);
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

/// 状态色（目录对比/合并视图用，P33 对齐 BC 语义）
/// BC 5.2.5 实测：孤儿（仅一侧）= 紫 rgb(83,44,199)；差异/较新 = 红 rgb(246,39,16)；相同 = 黑；未知/未扫 = 灰
pub fn status_color(ui: &egui::Ui, letter: char) -> Color32 {
    match letter {
        // 仅左侧/仅右侧 = 孤儿（BC 紫）
        'L' | 'R' => super::theme::status_orphan(),
        // 内容不同/移动 = 差异（BC 红）
        'C' | 'M' => super::theme::status_differ(),
        // 相同 = 默认文本色（BC 黑）
        'S' => ui.visuals().text_color(),
        // 未知/其他 = 弱色（BC 灰）
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

/// P37-1j：用第三方外部工具对比两侧文件（~/.bcr-external.toml 扩展名映射）。
///
/// 任一侧扩展名有映射则执行；返回 None（成功或未配置）或 Some(错误消息)。
pub fn external_compare(left: &str, right: &str) -> Option<String> {
    let tools = crate::external::ExternalTools::load();
    let template = tools.command_for(left).or_else(|| tools.command_for(right));
    let Some(t) = template else {
        return None; // 未配置该扩展名：不执行也不报错
    };
    match crate::external::ExternalTools::run(t, left, right) {
        Some(_) => None,
        None => Some("外部工具启动失败（命令不存在或执行出错）".to_string()),
    }
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
