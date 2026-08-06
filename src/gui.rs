//! M5 GUI：egui 并排 Diff 视图。
//!
//! 启动方式：`bcr gui [LEFT] [RIGHT]`，或打开后通过菜单/拖放加载文件。
//! 渲染基于 [`crate::sideview`] 的数据模型，GUI 本身不做 diff 计算。

use crate::sideview::{build_rows, Cell, RowTag, Stats, ViewOptions};
use eframe::egui::{
    self, Align2, Color32, FontId, Pos2, Rect, RichText, ScrollArea, Sense, TextFormat, TextStyle,
    Vec2,
};

/// 差异状态 → 底色（深色主题下的半透明红/绿）
const BG_DELETE: Color32 = Color32::from_rgb(80, 24, 28);
const BG_INSERT: Color32 = Color32::from_rgb(22, 64, 30);
const BG_PLACEHOLDER: Color32 = Color32::from_rgb(30, 34, 44);
const HL_DELETE: Color32 = Color32::from_rgb(150, 46, 52);
const HL_INSERT: Color32 = Color32::from_rgb(40, 120, 55);

const FG: Color32 = Color32::from_gray(215);
const FG_DIM: Color32 = Color32::from_gray(130);
const ROW_H: f32 = 20.0;
const FONT_SIZE: f32 = 14.0;

/// GUI 子命令参数
#[derive(clap::Args, Debug)]
pub struct GuiArgs {
    /// 左侧文件（可选，可在 GUI 中打开）
    pub left: Option<String>,

    /// 右侧文件（可选，可在 GUI 中打开）
    pub right: Option<String>,

    /// 忽略所有空白差异
    #[arg(long)]
    pub ignore_whitespace: bool,

    /// 忽略行尾空白差异
    #[arg(long)]
    pub ignore_trailing: bool,

    /// 忽略大小写差异
    #[arg(long)]
    pub ignore_case: bool,
}

/// 运行 GUI 事件循环（阻塞），返回进程退出码
pub fn run(args: &GuiArgs) -> i32 {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 820.0])
            .with_title("bcr — 并排 Diff"),
        ..Default::default()
    };

    let mut app = DiffApp::new();
    let opts = ViewOptions {
        ignore_whitespace: args.ignore_whitespace,
        ignore_trailing: args.ignore_trailing,
        ignore_case: args.ignore_case,
    };
    match (&args.left, &args.right) {
        (Some(l), Some(r)) => app.load_pair(l, r, opts),
        (Some(l), None) => app.load_left(l, opts),
        (None, Some(r)) => app.load_right(r, opts),
        (None, None) => {}
    }

    match eframe::run_native(
        "bcr",
        options,
        Box::new(move |cc| {
            // 等宽字体加大，便于代码阅读
            for theme in [egui::Theme::Dark, egui::Theme::Light] {
                cc.egui_ctx.style_mut_of(theme, |style| {
                    style
                        .text_styles
                        .insert(TextStyle::Monospace, FontId::monospace(FONT_SIZE));
                });
            }
            Ok(Box::new(app))
        }),
    ) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("bcr: GUI 启动失败: {e}");
            2
        }
    }
}

struct DiffApp {
    left: Option<LoadedFile>,
    right: Option<LoadedFile>,
    rows: Vec<crate::sideview::SideRow>,
    stats: Stats,
    opts: ViewOptions,
    error: Option<String>,
    show_stats: bool,
}

struct LoadedFile {
    path: String,
    content: String,
}

impl DiffApp {
    fn new() -> Self {
        DiffApp {
            left: None,
            right: None,
            rows: Vec::new(),
            stats: Stats::default(),
            opts: ViewOptions::default(),
            error: None,
            show_stats: true,
        }
    }

    fn load_pair(&mut self, l: &str, r: &str, opts: ViewOptions) {
        self.opts = opts;
        match (std::fs::read_to_string(l), std::fs::read_to_string(r)) {
            (Ok(lc), Ok(rc)) => {
                self.left = Some(LoadedFile { path: l.to_string(), content: lc });
                self.right = Some(LoadedFile { path: r.to_string(), content: rc });
                self.recompute();
                self.error = None;
            }
            (Err(e), _) => self.error = Some(format!("无法读取 {l}: {e}")),
            (_, Err(e)) => self.error = Some(format!("无法读取 {r}: {e}")),
        }
    }

    fn load_left(&mut self, path: &str, opts: ViewOptions) {
        self.opts = opts;
        match std::fs::read_to_string(path) {
            Ok(c) => {
                self.left = Some(LoadedFile { path: path.to_string(), content: c });
                self.recompute();
                self.error = None;
            }
            Err(e) => self.error = Some(format!("无法读取 {path}: {e}")),
        }
    }

    fn load_right(&mut self, path: &str, opts: ViewOptions) {
        self.opts = opts;
        match std::fs::read_to_string(path) {
            Ok(c) => {
                self.right = Some(LoadedFile { path: path.to_string(), content: c });
                self.recompute();
                self.error = None;
            }
            Err(e) => self.error = Some(format!("无法读取 {path}: {e}")),
        }
    }

    fn recompute(&mut self) {
        let (l, r) = match (&self.left, &self.right) {
            (Some(l), Some(r)) => (l.content.as_str(), r.content.as_str()),
            _ => {
                self.rows.clear();
                self.stats = Stats::default();
                return;
            }
        };
        let (rows, stats) = build_rows(l, r, self.opts);
        self.rows = rows;
        self.stats = stats;
    }

    fn reload(&mut self) {
        let paths = (self.left.as_ref().map(|f| f.path.clone()),
                     self.right.as_ref().map(|f| f.path.clone()));
        match paths {
            (Some(l), Some(r)) => self.load_pair(&l, &r, self.opts),
            (Some(l), None) => self.load_left(&l, self.opts),
            (None, Some(r)) => self.load_right(&r, self.opts),
            (None, None) => {}
        }
    }

    fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped: Vec<std::path::PathBuf> = ctx
            .input(|i| {
                i.raw
                    .dropped_files
                    .iter()
                    .map(|f| f.path().to_path_buf())
                    .collect()
            });
        if dropped.is_empty() {
            return;
        }
        match (dropped.get(0), dropped.get(1)) {
            (Some(l), Some(r)) => {
                self.load_pair(&l.to_string_lossy(), &r.to_string_lossy(), self.opts);
            }
            (Some(p), None) => {
                if self.left.is_none() {
                    self.load_left(&p.to_string_lossy(), self.opts);
                } else {
                    self.load_right(&p.to_string_lossy(), self.opts);
                }
            }
            _ => {}
        }
    }
}

fn pick_file() -> Option<String> {
    let path = rfd::FileDialog::new().pick_file()?;
    Some(path.to_string_lossy().into_owned())
}

impl eframe::App for DiffApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // 拖放文件加载
        self.handle_dropped_files(ui.ctx());

        // 顶部工具条
        egui::Panel::top("toolbar").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui.button("打开左侧…").clicked() {
                    if let Some(p) = pick_file() {
                        self.load_left(&p, self.opts);
                    }
                }
                if ui.button("打开右侧…").clicked() {
                    if let Some(p) = pick_file() {
                        self.load_right(&p, self.opts);
                    }
                }
                if ui.button("打开两个文件…").clicked() {
                    if let (Some(a), Some(b)) = (pick_file(), pick_file()) {
                        self.load_pair(&a, &b, self.opts);
                    }
                }
                ui.separator();
                ui.checkbox(&mut self.show_stats, "统计栏");
                ui.separator();
                if ui.checkbox(&mut self.opts.ignore_whitespace, "忽略空白").changed() {
                    self.recompute();
                }
                if ui.checkbox(&mut self.opts.ignore_trailing, "忽略行尾空白").changed() {
                    self.recompute();
                }
                if ui.checkbox(&mut self.opts.ignore_case, "忽略大小写").changed() {
                    self.recompute();
                }
                ui.separator();
                if ui.button("重新加载").clicked() {
                    self.reload();
                }
            });
        });

        // 底部统计栏
        if self.show_stats {
            egui::Panel::bottom("status").show(ui, |ui| {
                ui.horizontal(|ui| {
                    let st = self.stats;
                    ui.label(RichText::new(format!("相同 {}", st.equal)).color(FG_DIM));
                    ui.label(RichText::new(format!("删除 {}", st.delete)).color(Color32::from_rgb(240, 110, 110)));
                    ui.label(RichText::new(format!("插入 {}", st.insert)).color(Color32::from_rgb(110, 230, 120)));
                    ui.label(RichText::new(format!("修改 {}", st.replace)).color(Color32::from_rgb(235, 200, 100)));
                    ui.separator();
                    match (&self.left, &self.right) {
                        (Some(l), Some(r)) => {
                            ui.label(format!("{}  ↔  {}", l.path, r.path));
                        }
                        (Some(l), None) => {
                            ui.label(format!("{}  ↔  (未打开右侧)", l.path));
                        }
                        (None, Some(r)) => {
                            ui.label(format!("(未打开左侧)  ↔  {}", r.path));
                        }
                        (None, None) => {
                            ui.label("打开两个文件开始对比（或直接拖放文件到窗口）");
                        }
                    }
                });
            });
        }

        // 错误提示
        if let Some(err) = self.error.clone() {
            egui::Window::new("错误")
                .collapsible(false)
                .resizable(false)
                .show(ui.ctx(), |ui| {
                    ui.colored_label(Color32::from_rgb(240, 110, 110), err);
                    if ui.button("关闭").clicked() {
                        self.error = None;
                    }
                });
        }

        // 主体：并排滚动区
        egui::CentralPanel::default().show(ui, |ui| {
            if self.rows.is_empty() {
                ui.centered_and_justified(|ui| {
                    ui.label(
                        RichText::new("打开两个文件开始并排对比\n命令行：bcr gui left.txt right.txt\n或将文件拖入窗口")
                            .size(18.0)
                            .color(FG_DIM),
                    );
                });
                return;
            }

            // 计算布局尺寸
            let max_no_l = self.rows.iter().filter_map(|r| r.left_no).max().unwrap_or(0);
            let max_no_r = self.rows.iter().filter_map(|r| r.right_no).max().unwrap_or(0);
            let no_w = 8.0 * (max_no_l.max(max_no_r).max(1)).to_string().len() as f32 + 14.0;

            // 内容列宽：至少占窗口一半，长行撑开滚动区
            let avail = ui.available_width();
            let half = ((avail - 2.0 * no_w) / 2.0).max(300.0);
            let max_chars = self
                .rows
                .iter()
                .flat_map(|r| [r.left.as_ref(), r.right.as_ref()])
                .flatten()
                .map(|c| c.text.chars().count())
                .max()
                .unwrap_or(0);
            let content_w = half.max(max_chars as f32 * 8.5 + 20.0);
            let total_w = 2.0 * no_w + 2.0 * content_w;

            ScrollArea::both()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.set_min_width(total_w);
                    for row in &self.rows {
                        paint_row(ui, row, no_w, content_w);
                    }
                });
        });
    }
}

/// 绘制一行：左行号 | 左内容 | 右行号 | 右内容
fn paint_row(
    ui: &mut egui::Ui,
    row: &crate::sideview::SideRow,
    no_w: f32,
    content_w: f32,
) {
    let (bg_l, bg_r) = match row.tag {
        RowTag::Equal => (None, None),
        RowTag::Delete => (Some(BG_DELETE), Some(BG_PLACEHOLDER)),
        RowTag::Insert => (Some(BG_PLACEHOLDER), Some(BG_INSERT)),
        RowTag::Replace => (Some(BG_DELETE), Some(BG_INSERT)),
    };
    let (hl_l, hl_r) = match row.tag {
        RowTag::Replace => (Some(HL_DELETE), Some(HL_INSERT)),
        _ => (None, None),
    };

    let (rect, _) = ui.allocate_exact_size(Vec2::new(2.0 * no_w + 2.0 * content_w, ROW_H), Sense::hover());
    let y0 = rect.top();

    // 左行号
    paint_bg(ui, Rect::from_min_size(Pos2::new(rect.left(), y0), Vec2::new(no_w, ROW_H)), bg_l);
    if let Some(no) = row.left_no {
        paint_text(ui, Pos2::new(rect.left() + 4.0, y0), &no.to_string(), FG_DIM, 12.0);
    }
    // 左内容
    let l_rect = Rect::from_min_size(Pos2::new(rect.left() + no_w, y0), Vec2::new(content_w, ROW_H));
    paint_bg(ui, l_rect, bg_l);
    if let Some(cell) = &row.left {
        paint_cell(ui, l_rect, cell, FG, hl_l);
    }

    // 右行号
    let r_no_x = rect.left() + no_w + content_w;
    paint_bg(ui, Rect::from_min_size(Pos2::new(r_no_x, y0), Vec2::new(no_w, ROW_H)), bg_r);
    if let Some(no) = row.right_no {
        paint_text(ui, Pos2::new(r_no_x + 4.0, y0), &no.to_string(), FG_DIM, 12.0);
    }
    // 右内容
    let r_rect = Rect::from_min_size(Pos2::new(r_no_x + no_w, y0), Vec2::new(content_w, ROW_H));
    paint_bg(ui, r_rect, bg_r);
    if let Some(cell) = &row.right {
        paint_cell(ui, r_rect, cell, FG, hl_r);
    }
}

fn paint_bg(ui: &egui::Ui, rect: Rect, bg: Option<Color32>) {
    if let Some(c) = bg {
        ui.painter().rect_filled(rect, 0.0, c);
    }
}

fn paint_text(ui: &egui::Ui, pos: Pos2, text: &str, color: Color32, size: f32) {
    ui.painter().text(
        pos,
        Align2::LEFT_TOP,
        text,
        FontId::monospace(size),
        color,
    );
}

/// 绘制带行内高亮的单元格（使用 LayoutJob 分段着色）
fn paint_cell(ui: &egui::Ui, rect: Rect, cell: &Cell, fg: Color32, hl: Option<Color32>) {
    let mut job = egui::text::LayoutJob::default();
    for (seg, changed) in &cell.segments {
        job.append(
            seg,
            0.0,
            TextFormat {
                font_id: FontId::monospace(FONT_SIZE),
                color: fg,
                background: if *changed { hl.unwrap_or(Color32::TRANSPARENT) } else { Color32::TRANSPARENT },
                ..Default::default()
            },
        );
    }
    let galley = ui.painter().layout_job(job);
    // 垂直居中
    let y = rect.center().y - galley.size().y / 2.0;
    ui.painter().galley(Pos2::new(rect.left() + 6.0, y), galley, fg);
}
