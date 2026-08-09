//! 并排 Diff 标签页：虚拟化渲染、行内高亮、搜索、差异/行号跳转。

use super::common::*;
use crate::i18n::{fmt, t, Key as I18nKey};
use crate::sideview::{build_rows, RowTag, SideRow, Stats, ViewOptions};
use eframe::egui::{self, Color32, Key, Pos2, Rect, Vec2};

/// 加载的文件
#[derive(Clone)]
pub struct LoadedFile {
    pub path: String,
    pub content: String,
    /// 解码信息（编码 + BOM），保存时按原编码回写
    pub encoding: crate::encoding::EncodingKind,
    pub had_bom: bool,
}

/// 搜索状态
#[derive(Default)]
pub struct SearchState {
    pub query: String,
    /// 匹配行索引
    pub matches: Vec<usize>,
    /// 当前匹配在 matches 中的位置
    pub current: Option<usize>,
    /// 请求聚焦搜索框
    pub focus: bool,
}

/// 行内编辑状态
#[derive(Clone, Copy)]
pub enum EditSide {
    Left,
    Right,
}

pub struct DiffTab {
    pub left: Option<LoadedFile>,
    pub right: Option<LoadedFile>,
    pub rows: Vec<SideRow>,
    pub stats: Stats,
    pub opts: ViewOptions,
    pub error: Option<String>,
    pub show_stats: bool,
    /// 滚动偏移（受控滚动，支持跳转）
    pub scroll: Vec2,
    /// 差异行索引（tag != Equal），供跳转
    pub diff_rows: Vec<usize>,
    pub diff_pos: Option<usize>,
    pub search: SearchState,
    /// 待跳转行号（1-based）
    pub goto_line: Option<usize>,
    pub goto_focus: bool,
    /// 编辑状态（编辑左侧/右侧内容）
    pub editing: Option<EditState>,
}

/// 编辑窗口状态
pub struct EditState {
    pub side: EditSide,
    pub path: String,
    pub content: String,
}

impl DiffTab {
    pub fn new() -> Self {
        DiffTab {
            left: None,
            right: None,
            rows: Vec::new(),
            stats: Stats::default(),
            opts: ViewOptions::default(),
            error: None,
            show_stats: true,
            scroll: Vec2::ZERO,
            diff_rows: Vec::new(),
            diff_pos: None,
            search: SearchState::default(),
            goto_line: None,
            goto_focus: false,
            editing: None,
        }
    }

    pub fn title(&self) -> String {
        match (&self.left, &self.right) {
            (Some(l), Some(r)) => format!(
                "{}: {} ↔ {}",
                t(I18nKey::DiffTitle),
                basename(&l.path),
                basename(&r.path)
            ),
            (Some(l), None) => format!("{}: {}", t(I18nKey::DiffTitle), basename(&l.path)),
            (None, Some(r)) => format!("{}: {}", t(I18nKey::DiffTitle), basename(&r.path)),
            (None, None) => t(I18nKey::DiffTitle).to_string(),
        }
    }

    pub fn load_pair(&mut self, l: &str, r: &str, opts: ViewOptions) {
        self.opts = opts;
        match (crate::encoding::read_text(l), crate::encoding::read_text(r)) {
            (Ok(lf), Ok(rf)) => {
                if lf.is_binary {
                    self.error = Some(fmt(I18nKey::BinaryFile, &[l]));
                    return;
                }
                if rf.is_binary {
                    self.error = Some(fmt(I18nKey::BinaryFile, &[r]));
                    return;
                }
                self.left = Some(LoadedFile {
                    path: l.to_string(),
                    content: lf.text,
                    encoding: lf.encoding,
                    had_bom: lf.had_bom,
                });
                self.right = Some(LoadedFile {
                    path: r.to_string(),
                    content: rf.text,
                    encoding: rf.encoding,
                    had_bom: rf.had_bom,
                });
                self.recompute();
                self.error = None;
            }
            (Err(e), _) => self.error = Some(fmt(I18nKey::CannotRead, &[l, &e.to_string()])),
            (_, Err(e)) => self.error = Some(fmt(I18nKey::CannotRead, &[r, &e.to_string()])),
        }
    }

    pub fn load_left(&mut self, path: &str, opts: ViewOptions) {
        self.opts = opts;
        match crate::encoding::read_text(path) {
            Ok(tf) => {
                if tf.is_binary {
                    self.error = Some(fmt(I18nKey::BinaryFile, &[path]));
                    return;
                }
                self.left = Some(LoadedFile {
                    path: path.to_string(),
                    content: tf.text,
                    encoding: tf.encoding,
                    had_bom: tf.had_bom,
                });
                self.recompute();
                self.error = None;
            }
            Err(e) => self.error = Some(fmt(I18nKey::CannotRead, &[path, &e.to_string()])),
        }
    }

    pub fn load_right(&mut self, path: &str, opts: ViewOptions) {
        self.opts = opts;
        match crate::encoding::read_text(path) {
            Ok(tf) => {
                if tf.is_binary {
                    self.error = Some(fmt(I18nKey::BinaryFile, &[path]));
                    return;
                }
                self.right = Some(LoadedFile {
                    path: path.to_string(),
                    content: tf.text,
                    encoding: tf.encoding,
                    had_bom: tf.had_bom,
                });
                self.recompute();
                self.error = None;
            }
            Err(e) => self.error = Some(fmt(I18nKey::CannotRead, &[path, &e.to_string()])),
        }
    }

    pub fn recompute(&mut self) {
        let (l, r) = match (&self.left, &self.right) {
            (Some(l), Some(r)) => (l.content.as_str(), r.content.as_str()),
            _ => {
                self.rows.clear();
                self.stats = Stats::default();
                self.diff_rows.clear();
                self.diff_pos = None;
                return;
            }
        };
        let (rows, stats) = build_rows(l, r, self.opts);
        self.diff_rows = rows
            .iter()
            .enumerate()
            .filter(|(_, r)| r.tag != RowTag::Equal)
            .map(|(i, _)| i)
            .collect();
        self.diff_pos = None;
        self.rows = rows;
        self.stats = stats;
        self.update_search();
    }

    pub fn reload(&mut self) {
        let opts = self.opts;
        let l = self.left.as_ref().map(|f| f.path.clone());
        let r = self.right.as_ref().map(|f| f.path.clone());
        match (l, r) {
            (Some(l), Some(r)) => self.load_pair(&l, &r, opts),
            (Some(l), None) => self.load_left(&l, opts),
            (None, Some(r)) => self.load_right(&r, opts),
            (None, None) => {}
        }
    }

    // ---- 搜索 ----

    pub fn update_search(&mut self) {
        let q = self.search.query.trim();
        self.search.matches.clear();
        self.search.current = None;
        if q.is_empty() {
            return;
        }
        let lower = q.to_lowercase();
        for (i, row) in self.rows.iter().enumerate() {
            let hit = [row.left.as_ref(), row.right.as_ref()]
                .into_iter()
                .flatten()
                .any(|c| c.text.to_lowercase().contains(&lower));
            if hit {
                self.search.matches.push(i);
            }
        }
    }

    /// 跳到第 k 个匹配（k 为 matches 下标），返回是否成功
    pub fn goto_match(&mut self, k: usize) -> bool {
        if let Some(&row) = self.search.matches.get(k) {
            self.search.current = Some(k);
            self.jump_to_row(row);
            true
        } else {
            false
        }
    }

    pub fn next_match(&mut self) {
        if self.search.matches.is_empty() {
            return;
        }
        let next = match self.search.current {
            Some(k) => (k + 1) % self.search.matches.len(),
            None => 0,
        };
        self.goto_match(next);
    }

    pub fn prev_match(&mut self) {
        if self.search.matches.is_empty() {
            return;
        }
        let n = self.search.matches.len();
        let prev = match self.search.current {
            Some(k) => (k + n - 1) % n,
            None => n - 1,
        };
        self.goto_match(prev);
    }

    // ---- 差异跳转 ----

    pub fn next_diff(&mut self) {
        if self.diff_rows.is_empty() {
            return;
        }
        let cur = self.rows.iter().position(|r| r.tag != RowTag::Equal);
        let base = match (self.diff_pos, cur) {
            (Some(p), _) => p,
            (None, Some(c)) => c,
            (None, None) => 0,
        };
        let next = self.diff_rows.iter().position(|&r| r > base).unwrap_or(0);
        self.diff_pos = Some(next);
        self.jump_to_row(self.diff_rows[next]);
    }

    pub fn prev_diff(&mut self) {
        if self.diff_rows.is_empty() {
            return;
        }
        let cur = self.rows.iter().position(|r| r.tag != RowTag::Equal);
        let base = match (self.diff_pos, cur) {
            (Some(p), _) => p,
            (None, Some(c)) => c,
            (None, None) => 0,
        };
        let n = self.diff_rows.len();
        let prev = self
            .diff_rows
            .iter()
            .rposition(|&r| r < base)
            .unwrap_or(n - 1);
        self.diff_pos = Some(prev);
        self.jump_to_row(self.diff_rows[prev]);
    }

    /// 滚动到指定行索引（虚拟化：设置 scroll.y）
    pub fn jump_to_row(&mut self, row: usize) {
        let y = row as f32 * ROW_H;
        // 尽量让目标行出现在视口中部偏上
        self.scroll.y = (y - 4.0 * ROW_H).max(0.0);
        self.scroll.x = 0.0;
    }

    pub fn handle_keys(&mut self, ui: &egui::Ui) {
        let ctrl = ui.input(|i| i.modifiers.command);
        if ui.input(|i| i.key_pressed(Key::F) && ctrl) {
            self.search.focus = true;
            return;
        }
        if ui.input(|i| i.key_pressed(Key::G) && ctrl) {
            self.goto_focus = true;
            return;
        }
        if ui.input(|i| i.key_pressed(Key::F7)) {
            if ui.input(|i| i.modifiers.shift) {
                self.prev_diff();
            } else {
                self.next_diff();
            }
        }
        if ui.input(|i| i.key_pressed(Key::Enter)) {
            self.next_match();
        }
        if ui.input(|i| i.key_pressed(Key::Escape)) {
            self.search.query.clear();
            self.update_search();
        }
    }

    // ---- 渲染 ----

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        self.handle_keys(ui);

        // 搜索/跳转工具条
        egui::Panel::top("difftab_tools").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui.button(t(I18nKey::OpenLeft)).clicked() {
                    if let Some(p) = super::pick_file() {
                        self.load_left(&p, self.opts);
                    }
                }
                if ui.button(t(I18nKey::OpenRight)).clicked() {
                    if let Some(p) = super::pick_file() {
                        self.load_right(&p, self.opts);
                    }
                }
                ui.separator();
                ui.checkbox(&mut self.show_stats, t(I18nKey::StatsPanel))
                    .changed();
                ui.separator();
                if ui
                    .checkbox(&mut self.opts.ignore_whitespace, t(I18nKey::IgnoreWs))
                    .changed()
                {
                    self.recompute();
                }
                if ui
                    .checkbox(&mut self.opts.ignore_trailing, t(I18nKey::IgnoreTrailing))
                    .changed()
                {
                    self.recompute();
                }
                if ui
                    .checkbox(&mut self.opts.ignore_case, t(I18nKey::IgnoreCase))
                    .changed()
                {
                    self.recompute();
                }
                ui.separator();
                if ui.button(t(I18nKey::EditLeft)).clicked() {
                    if let Some(l) = &self.left {
                        self.editing = Some(EditState {
                            side: EditSide::Left,
                            path: l.path.clone(),
                            content: l.content.clone(),
                        });
                    }
                }
                if ui.button(t(I18nKey::EditRight)).clicked() {
                    if let Some(r) = &self.right {
                        self.editing = Some(EditState {
                            side: EditSide::Right,
                            path: r.path.clone(),
                            content: r.content.clone(),
                        });
                    }
                }
                ui.separator();
                if ui.button(t(I18nKey::Reload)).clicked() {
                    self.reload();
                }
                ui.separator();
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut self.search.query)
                        .hint_text(t(I18nKey::SearchHint))
                        .desired_width(220.0),
                );
                if self.search.focus {
                    resp.request_focus();
                    self.search.focus = false;
                }
                if resp.changed() {
                    self.update_search();
                }
                if ui
                    .button("⬆")
                    .on_hover_text(t(I18nKey::PrevMatch))
                    .clicked()
                {
                    self.prev_match();
                }
                if ui
                    .button("⬇")
                    .on_hover_text(t(I18nKey::NextMatch))
                    .clicked()
                {
                    self.next_match();
                }
                if let Some(k) = self.search.current {
                    ui.label(format!("{}/{}", k + 1, self.search.matches.len()));
                }
                ui.separator();
                let mut goto_text = self.goto_line.map(|l| l.to_string()).unwrap_or_default();
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut goto_text)
                        .hint_text(t(I18nKey::GotoHint))
                        .desired_width(70.0),
                );
                if self.goto_focus {
                    resp.request_focus();
                    self.goto_focus = false;
                }
                self.goto_line = goto_text.parse().ok();
                if (resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)))
                    || ui.button(t(I18nKey::Goto)).clicked()
                {
                    if let Some(line) = self.goto_line {
                        if line >= 1 {
                            self.jump_to_row(line - 1);
                        }
                    }
                }
                ui.separator();
                ui.label(fmt(
                    I18nKey::DiffCount,
                    &[
                        &self.diff_rows.len().to_string(),
                        &self.rows.len().to_string(),
                    ],
                ));
                if ui.button(t(I18nKey::NextDiff)).clicked() {
                    self.next_diff();
                }
                if ui.button(t(I18nKey::PrevDiff)).clicked() {
                    self.prev_diff();
                }
            });
        });

        // 错误弹窗
        if let Some(err) = self.error.clone() {
            egui::Window::new(t(I18nKey::Error))
                .collapsible(false)
                .resizable(false)
                .show(ui.ctx(), |ui| {
                    ui.colored_label(Color32::from_rgb(240, 110, 110), err);
                    if ui.button(t(I18nKey::Close)).clicked() {
                        self.error = None;
                    }
                });
        }

        // 编辑窗口
        if let Some(edit) = &mut self.editing {
            let side_name = match edit.side {
                EditSide::Left => t(I18nKey::SideLeft),
                EditSide::Right => t(I18nKey::SideRight),
            };
            let mut close = false;
            let mut save = false;
            egui::Window::new(format!("编辑{side_name}: {}", edit.path))
                .default_size([800.0, 600.0])
                .resizable(true)
                .show(ui.ctx(), |ui| {
                    egui::ScrollArea::both()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.add(
                                egui::TextEdit::multiline(&mut edit.content)
                                    .font(egui::TextStyle::Monospace)
                                    .desired_width(f32::INFINITY)
                                    .desired_rows(30)
                                    .code_editor(),
                            );
                        });
                    ui.horizontal(|ui| {
                        if ui.button(t(I18nKey::Save)).clicked() {
                            save = true;
                        }
                        if ui.button(t(I18nKey::Cancel)).clicked() {
                            close = true;
                        }
                        ui.label(t(I18nKey::SaveHint));
                    });
                    if ui.input(|i| i.modifiers.command && i.key_pressed(Key::S)) {
                        save = true;
                    }
                });
            if save {
                // 先克隆所需数据，避免同时借用 self.editing 与 self 方法
                let (path, side) = self
                    .editing
                    .as_ref()
                    .map(|e| (e.path.clone(), e.side))
                    .unwrap();
                let content = self.editing.as_ref().map(|e| e.content.clone()).unwrap();
                // 按原编码回写（保留 BOM 与编码，避免破坏 GBK/UTF-16 文件）
                let bytes = match side {
                    EditSide::Left => self.left.as_ref().map(|f| {
                        crate::encoding::encode_back(
                            &crate::encoding::TextFile {
                                text: String::new(),
                                encoding: f.encoding,
                                had_bom: f.had_bom,
                                is_binary: false,
                            },
                            &content,
                        )
                    }),
                    EditSide::Right => self.right.as_ref().map(|f| {
                        crate::encoding::encode_back(
                            &crate::encoding::TextFile {
                                text: String::new(),
                                encoding: f.encoding,
                                had_bom: f.had_bom,
                                is_binary: false,
                            },
                            &content,
                        )
                    }),
                };
                let write_res = match bytes {
                    Some(b) => std::fs::write(&path, b),
                    None => Ok(()),
                };
                match write_res {
                    Ok(()) => {
                        close = true;
                        // 保存后重新加载对应侧并重算 diff
                        match side {
                            EditSide::Left => self.load_left(&path, self.opts),
                            EditSide::Right => self.load_right(&path, self.opts),
                        }
                        self.error = Some(fmt(I18nKey::Saved, &[&path]));
                    }
                    Err(e) => {
                        self.error = Some(fmt(I18nKey::SaveFailed, &[&e.to_string()]));
                    }
                }
            }
            if close {
                self.editing = None;
            }
        }

        egui::CentralPanel::default().show(ui, |ui| {
            if self.rows.is_empty() {
                ui.centered_and_justified(|ui| {
                    ui.label(
                        egui::RichText::new(t(I18nKey::DiffEmptyHint))
                            .size(18.0)
                            .color(ui.visuals().weak_text_color()),
                    );
                });
                return;
            }

            let max_no_l = self
                .rows
                .iter()
                .filter_map(|r| r.left_no)
                .max()
                .unwrap_or(0);
            let max_no_r = self
                .rows
                .iter()
                .filter_map(|r| r.right_no)
                .max()
                .unwrap_or(0);
            let gutter_l = gutter_width(max_no_l);
            let gutter_r = gutter_width(max_no_r);
            // 内容宽度：撑满窗口一半，长行可横向滚动
            let avail = ui.available_width();
            let half = ((avail - gutter_l - gutter_r) / 2.0).max(200.0);
            let max_chars = self
                .rows
                .iter()
                .flat_map(|r| [r.left.as_ref(), r.right.as_ref()])
                .flatten()
                .map(|c| c.text.chars().count())
                .max()
                .unwrap_or(0);
            let content_w = half.max(max_chars as f32 * 8.5 + 24.0);
            let total_w = gutter_l + content_w + gutter_r + content_w;
            let fg = text_color(ui);

            // 匹配行集合（搜索高亮）
            let match_set: std::collections::HashSet<usize> =
                self.search.matches.iter().copied().collect();
            let current_match = self
                .search
                .current
                .and_then(|k| self.search.matches.get(k).copied());
            let rows = &self.rows;

            // 受控滚动 + 虚拟化渲染（统一走 common::show_rows）
            let out = super::show_rows(ui, rows.len(), ROW_H, |ui, range| {
                ui.set_min_width(total_w);
                for i in range {
                    let row = &rows[i];
                    let (bg_l, bg_r) = match row.tag {
                        RowTag::Equal => (None, None),
                        RowTag::Delete => (Some(bg_delete()), None),
                        RowTag::Insert => (None, Some(bg_insert())),
                        RowTag::Replace => (Some(bg_replace_l()), Some(bg_replace_r())),
                    };
                    // 搜索命中高亮
                    let (bg_l, bg_r) = if match_set.contains(&i) {
                        let c = if current_match == Some(i) {
                            bg_match_current()
                        } else {
                            bg_match()
                        };
                        (Some(bg_l.unwrap_or(c)), Some(bg_r.unwrap_or(c)))
                    } else {
                        (bg_l, bg_r)
                    };
                    let (hl_l, hl_r) = match row.tag {
                        RowTag::Replace => (Some(hl_replace_l()), Some(hl_replace_r())),
                        RowTag::Delete => (Some(hl_delete()), None),
                        RowTag::Insert => (None, Some(hl_insert())),
                        RowTag::Equal => (None, None),
                    };
                    paint_diff_row(
                        ui, row, gutter_l, gutter_r, content_w, bg_l, bg_r, hl_l, hl_r, fg,
                    );
                }
            });
            self.scroll = out.state.offset;

            // 底部统计栏
            if self.show_stats {
                ui.separator();
                ui.horizontal(|ui| {
                    let st = self.stats;
                    ui.label(format!("{} {}", t(I18nKey::StatSame), st.equal));
                    ui.colored_label(
                        Color32::from_rgb(240, 120, 120),
                        format!("{} {}", t(I18nKey::StatDelete), st.delete),
                    );
                    ui.colored_label(
                        Color32::from_rgb(120, 230, 130),
                        format!("{} {}", t(I18nKey::StatInsert), st.insert),
                    );
                    ui.colored_label(
                        Color32::from_rgb(235, 210, 100),
                        format!("{} {}", t(I18nKey::StatReplace), st.replace),
                    );
                    ui.separator();
                    match (&self.left, &self.right) {
                        (Some(l), Some(r)) => {
                            ui.label(format!("{}  ↔  {}", l.path, r.path));
                        }
                        (Some(l), None) => {
                            ui.label(fmt(I18nKey::NotOpenRight, &[&l.path]));
                        }
                        (None, Some(r)) => {
                            ui.label(fmt(I18nKey::NotOpenLeft, &[&r.path]));
                        }
                        (None, None) => {}
                    }
                });
            }
        });
    }
}

#[allow(clippy::too_many_arguments)] // egui 行绘制参数较多，保持扁平可读
fn paint_diff_row(
    ui: &mut egui::Ui,
    row: &SideRow,
    gutter_l: f32,
    gutter_r: f32,
    content_w: f32,
    bg_l: Option<Color32>,
    bg_r: Option<Color32>,
    hl_l: Option<Color32>,
    hl_r: Option<Color32>,
    fg: Color32,
) {
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(gutter_l + content_w + gutter_r + content_w, ROW_H),
        egui::Sense::hover(),
    );
    let x = rect.left();
    let y = rect.top();

    // 左 gutter + 内容
    let gutter_rect = Rect::from_min_size(Pos2::new(x, y), vec2(gutter_l, ROW_H));
    paint_bg(ui, gutter_rect, bg_l);
    paint_line_no(ui, gutter_rect, row.left_no);
    let content_rect = Rect::from_min_size(Pos2::new(x + gutter_l, y), vec2(content_w, ROW_H));
    paint_bg(ui, content_rect, bg_l);
    paint_cell(ui, content_rect, row.left.as_ref(), fg, hl_l);

    // 右 gutter + 内容
    let x_r = x + gutter_l + content_w;
    let gutter_rect = Rect::from_min_size(Pos2::new(x_r, y), vec2(gutter_r, ROW_H));
    paint_bg(ui, gutter_rect, bg_r);
    paint_line_no(ui, gutter_rect, row.right_no);
    let content_rect = Rect::from_min_size(Pos2::new(x_r + gutter_r, y), vec2(content_w, ROW_H));
    paint_bg(ui, content_rect, bg_r);
    paint_cell(ui, content_rect, row.right.as_ref(), fg, hl_r);
}

fn basename(p: &str) -> String {
    std::path::Path::new(p)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.to_string())
}
