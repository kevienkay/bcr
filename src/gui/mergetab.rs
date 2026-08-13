//! 三路合并标签页：BASE/LEFT/RIGHT 三栏渲染、冲突导航与解决、保存。

use super::common::*;
use crate::i18n::{fmt, t, Key as I18nKey};
use crate::mergeview::{build_merge_view, render_merged, MergeView, Resolution};
use eframe::egui::{self, Color32, Key, Pos2, Rect, Vec2};

pub struct MergeTab {
    pub base_path: String,
    pub left_path: String,
    pub right_path: String,
    pub view: MergeView,
    pub error: Option<String>,
    pub scroll: Vec2,
    /// 当前冲突块下标（conflict_rows 索引）
    pub conflict_idx: Option<usize>,
    pub label_l: String,
    pub label_r: String,
    /// 预览窗格滚动偏移
    pub preview_scroll: Vec2,
    pub show_preview: bool,
}

impl MergeTab {
    pub fn new(base: &str, left: &str, right: &str) -> Self {
        let mut t = MergeTab {
            base_path: base.to_string(),
            left_path: left.to_string(),
            right_path: right.to_string(),
            view: MergeView::default(),
            error: None,
            scroll: Vec2::ZERO,
            conflict_idx: None,
            label_l: "LEFT".to_string(),
            label_r: "RIGHT".to_string(),
            preview_scroll: Vec2::ZERO,
            show_preview: true,
        };
        t.reload();
        t
    }

    pub fn title(&self) -> String {
        fmt(
            I18nKey::MergeTitle,
            &[
                &basename(&self.base_path),
                &basename(&self.left_path),
                &basename(&self.right_path),
            ],
        )
    }

    pub fn reload(&mut self) {
        // P34：空路径守卫（空会话）
        if self.base_path.is_empty() && self.left_path.is_empty() && self.right_path.is_empty() {
            self.view = MergeView::default();
            self.conflict_idx = None;
            self.error = None;
            return;
        }
        let (b, l, r) = match (
            std::fs::read_to_string(&self.base_path),
            std::fs::read_to_string(&self.left_path),
            std::fs::read_to_string(&self.right_path),
        ) {
            (Ok(b), Ok(l), Ok(r)) => (b, l, r),
            (Err(e), _, _) => {
                self.error = Some(fmt(I18nKey::CannotRead, &[&self.base_path, &e.to_string()]));
                return;
            }
            (_, Err(e), _) => {
                self.error = Some(fmt(I18nKey::CannotRead, &[&self.left_path, &e.to_string()]));
                return;
            }
            (_, _, Err(e)) => {
                self.error = Some(fmt(
                    I18nKey::CannotRead,
                    &[&self.right_path, &e.to_string()],
                ));
                return;
            }
        };
        self.view = build_merge_view(&b, &l, &r);
        self.conflict_idx = None;
        self.error = None;
    }

    /// 当前冲突块在 blocks 中的索引（通过 conflict_block_indices 精确定位）
    pub(crate) fn current_conflict_block(&self) -> Option<usize> {
        self.conflict_idx
            .and_then(|k| self.view.conflict_block_indices.get(k).copied())
    }

    pub fn next_conflict(&mut self) {
        let n = self.view.conflict_rows.len();
        if n == 0 {
            return;
        }
        let next = match self.conflict_idx {
            Some(k) => (k + 1) % n,
            None => 0,
        };
        self.conflict_idx = Some(next);
        if let Some(&row) = self.view.conflict_rows.get(next) {
            self.jump_to_row(row);
        }
    }

    pub fn prev_conflict(&mut self) {
        let n = self.view.conflict_rows.len();
        if n == 0 {
            return;
        }
        let prev = match self.conflict_idx {
            Some(k) => (k + n - 1) % n,
            None => n - 1,
        };
        self.conflict_idx = Some(prev);
        if let Some(&row) = self.view.conflict_rows.get(prev) {
            self.jump_to_row(row);
        }
    }

    pub fn jump_to_row(&mut self, row: usize) {
        self.scroll.y = (row as f32 * ROW_H - 4.0 * ROW_H).max(0.0);
        self.scroll.x = 0.0;
    }

    pub fn resolve_current(&mut self, res: Resolution) {
        if let Some(bi) = self.current_conflict_block() {
            if let Some(blk) = self.view.blocks.get_mut(bi) {
                blk.resolution = res;
            }
        }
    }

    pub fn save(&mut self) -> bool {
        let Some(path) = rfd::FileDialog::new()
            .set_file_name("merged.txt")
            .save_file()
        else {
            return false;
        };
        let (lines, unresolved) = render_merged(&self.view, &self.label_l, &self.label_r);
        let mut content = lines.join("\n");
        if !content.is_empty() {
            content.push('\n');
        }
        match std::fs::write(&path, content) {
            Ok(()) => {
                self.error = Some(fmt(
                    I18nKey::MergeSaved,
                    &[&path.display().to_string(), &unresolved.to_string()],
                ));
                true
            }
            Err(e) => {
                self.error = Some(fmt(I18nKey::SaveFailed, &[&e.to_string()]));
                false
            }
        }
    }

    /// P34：打开 BASE 文件（空会话填充）
    pub fn open_base(&mut self) {
        if let Some(p) = super::pick_file() {
            self.base_path = p;
            self.reload();
        }
    }

    /// P34：打开 LEFT 文件（空会话填充）
    pub fn open_left(&mut self) {
        if let Some(p) = super::pick_file() {
            self.left_path = p;
            self.reload();
        }
    }

    /// P34：打开 RIGHT 文件（空会话填充）
    pub fn open_right(&mut self) {
        if let Some(p) = super::pick_file() {
            self.right_path = p;
            self.reload();
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("mergetab_tools").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui.button(format!("⟳ {}", t(I18nKey::Reload))).clicked() {
                    self.reload();
                }
                if ui
                    .button(format!("💾 {}", t(I18nKey::SaveMerged)))
                    .clicked()
                {
                    self.save();
                }
                ui.separator();
                ui.checkbox(&mut self.show_preview, t(I18nKey::LivePreview))
                    .changed();
                ui.separator();
                ui.label(fmt(
                    I18nKey::ConflictsCount,
                    &[&self.view.conflicts.to_string()],
                ));
                if ui
                    .button(format!("⬇ {}", t(I18nKey::NextConflict)))
                    .on_hover_text("下一冲突 (F7)")
                    .clicked()
                {
                    self.next_conflict();
                }
                if ui
                    .button(format!("⬆ {}", t(I18nKey::PrevConflict)))
                    .on_hover_text("上一冲突 (Shift+F7)")
                    .clicked()
                {
                    self.prev_conflict();
                }
                ui.separator();
                if ui.button(format!("← {}", t(I18nKey::TakeLeft))).clicked() {
                    self.resolve_current(Resolution::Left);
                }
                if ui.button(format!("→ {}", t(I18nKey::TakeRight))).clicked() {
                    self.resolve_current(Resolution::Right);
                }
                if ui.button(format!("B {}", t(I18nKey::TakeBase))).clicked() {
                    self.resolve_current(Resolution::Base);
                }
                // 显示当前冲突块的解决状态（P31：已解决 ✓ 绿标）
                if let Some(bi) = self.current_conflict_block() {
                    if let Some(blk) = self.view.blocks.get(bi) {
                        ui.separator();
                        let (mark, color) = match blk.resolution {
                            Resolution::Auto => (
                                t(I18nKey::ResAuto).to_string(),
                                Color32::from_rgb(240, 180, 60),
                            ),
                            Resolution::Left | Resolution::Right | Resolution::Base => (
                                format!("✓ {}", t(I18nKey::Resolved)),
                                Color32::from_rgb(110, 230, 120),
                            ),
                        };
                        ui.colored_label(color, mark);
                    }
                }
            });
        });

        if let Some(err) = self.error.clone() {
            egui::Window::new(t(I18nKey::Hint))
                .collapsible(false)
                .resizable(false)
                .show(ui.ctx(), |ui| {
                    ui.label(err);
                    if ui.button(t(I18nKey::Close)).clicked() {
                        self.error = None;
                    }
                });
        }

        // 快捷键
        if ui.input(|i| i.key_pressed(Key::F7)) {
            if ui.input(|i| i.modifiers.shift) {
                self.prev_conflict();
            } else {
                self.next_conflict();
            }
        }

        // 底部实时预览窗格（显示保存将得到的结果，未解决冲突高亮）
        if self.show_preview {
            let (lines, unresolved) = render_merged(&self.view, &self.label_l, &self.label_r);
            let preview_lines: Vec<(&str, bool)> = lines
                .iter()
                .map(|l| {
                    (
                        l.as_str(),
                        l.starts_with("<<<<<<<")
                            || l.starts_with("=======")
                            || l.starts_with(">>>>>>>"),
                    )
                })
                .collect();
            egui::Panel::bottom("merge_preview").show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.strong(t(I18nKey::MergePreview));
                    ui.separator();
                    ui.label(fmt(I18nKey::MergeLines, &[&lines.len().to_string()]));
                    if unresolved > 0 {
                        ui.colored_label(
                            Color32::from_rgb(240, 180, 60),
                            fmt(I18nKey::MergeUnresolved, &[&unresolved.to_string()]),
                        );
                    } else {
                        ui.colored_label(
                            Color32::from_rgb(110, 230, 120),
                            t(I18nKey::MergeAllResolved),
                        );
                    }
                });
                let fg = text_color(ui);
                let out = super::show_rows(ui, preview_lines.len(), ROW_H, |ui, range| {
                    for idx in range {
                        let (text, is_marker) = preview_lines[idx];
                        let (rect, _) = ui.allocate_exact_size(
                            Vec2::new(ui.available_width().max(400.0), ROW_H),
                            egui::Sense::hover(),
                        );
                        if is_marker {
                            paint_bg(ui, rect, Some(bg_match_current()));
                        }
                        ui.painter().text(
                            Pos2::new(rect.left() + 4.0, rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            text,
                            egui::FontId::monospace(FONT_SIZE),
                            fg,
                        );
                    }
                });
                self.preview_scroll = out.state.offset;
            });
        }

        egui::CentralPanel::default().show(ui, |ui| {
            if self.view.rows.is_empty() && self.view.conflicts == 0 {
                ui.centered_and_justified(|ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new(t(I18nKey::MergeEmpty))
                                .size(16.0)
                                .color(ui.visuals().weak_text_color()),
                        );
                        ui.add_space(12.0);
                        // P34：分别打开 BASE/LEFT/RIGHT（BC 式：不强求一次选满三个）
                        ui.horizontal(|ui| {
                            if ui.button(t(I18nKey::OpenBase)).clicked() {
                                self.open_base();
                            }
                            if ui.button(t(I18nKey::OpenLeft)).clicked() {
                                self.open_left();
                            }
                            if ui.button(t(I18nKey::OpenRight)).clicked() {
                                self.open_right();
                            }
                        });
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new(t(I18nKey::DragHint))
                                .size(11.0)
                                .color(ui.visuals().weak_text_color()),
                        );
                    });
                });
                return;
            }

            let rows = &self.view.rows;
            let max_no = rows.iter().filter_map(|r| r.base_no).max().unwrap_or(0);
            let gutter = gutter_width(max_no);
            let avail = ui.available_width();
            let col_w = ((avail - gutter * 3.0) / 3.0).max(150.0);
            let total_w = gutter * 3.0 + col_w * 3.0;
            let fg = text_color(ui);

            let out = {
                let mut sa = egui::ScrollArea::both().auto_shrink([false, false]);
                sa = sa.vertical_scroll_offset(self.scroll.y);
                sa = sa.horizontal_scroll_offset(self.scroll.x);
                let syn_b = crate::highlight::syntax_for(&self.base_path);
                let syn_l = crate::highlight::syntax_for(&self.left_path);
                let syn_r = crate::highlight::syntax_for(&self.right_path);
                sa.show_rows(ui, ROW_H, rows.len(), |ui, range| {
                    ui.set_min_width(total_w);
                    let (bp, lp, rp) = (
                        self.base_path.clone(),
                        self.left_path.clone(),
                        self.right_path.clone(),
                    );
                    for i in range {
                        let row = &rows[i];
                        let (bg_b, bg_l, bg_r) = merge_row_bg(row);
                        let (hl_l, hl_r) = merge_row_hl(row);
                        let resp = paint_merge_row(
                            ui, row, gutter, col_w, bg_b, bg_l, bg_r, hl_l, hl_r, fg, syn_b, syn_l,
                            syn_r,
                        );
                        // P32-A4：行右键菜单（复制路径/打开所在位置/系统打开）
                        let (bp2, lp2, rp2) = (bp.clone(), lp.clone(), rp.clone());
                        resp.context_menu(|ui| {
                            if ui.button("复制 BASE 路径").clicked() {
                                ui.ctx().copy_text(bp2.clone());
                                ui.close();
                            }
                            if ui.button("复制左侧路径").clicked() {
                                ui.ctx().copy_text(lp2.clone());
                                ui.close();
                            }
                            if ui.button("复制右侧路径").clicked() {
                                ui.ctx().copy_text(rp2.clone());
                                ui.close();
                            }
                            ui.separator();
                            if ui.button("打开所在位置（BASE）").clicked() {
                                super::common::reveal_in_file_manager(&bp2);
                                ui.close();
                            }
                            if ui.button("打开所在位置（左）").clicked() {
                                super::common::reveal_in_file_manager(&lp2);
                                ui.close();
                            }
                            if ui.button("打开所在位置（右）").clicked() {
                                super::common::reveal_in_file_manager(&rp2);
                                ui.close();
                            }
                            ui.separator();
                            if ui.button("系统打开（BASE）").clicked() {
                                super::common::open_with_system_app(&bp2);
                                ui.close();
                            }
                            if ui.button("系统打开（左）").clicked() {
                                super::common::open_with_system_app(&lp2);
                                ui.close();
                            }
                            if ui.button("系统打开（右）").clicked() {
                                super::common::open_with_system_app(&rp2);
                                ui.close();
                            }
                        });
                    }
                })
            };
            self.scroll = out.state.offset;
        });
    }
}

fn merge_row_bg(
    row: &crate::mergeview::MergeRow,
) -> (Option<Color32>, Option<Color32>, Option<Color32>) {
    use crate::mergeview::BlockKind;
    if row.in_conflict {
        // 冲突行：base 灰红、left 红、right 绿
        return (
            Some(Color32::from_rgba_unmultiplied(120, 90, 90, 60)),
            Some(bg_replace_l()),
            Some(bg_replace_r()),
        );
    }
    // 单侧修改行：仅该侧着色，便于快速定位
    match row.kind {
        BlockKind::LeftOnly | BlockKind::Same => (None, Some(bg_replace_l()), None),
        BlockKind::RightOnly => (None, None, Some(bg_replace_r())),
        BlockKind::Context | BlockKind::Conflict => (None, None, None),
    }
}

fn merge_row_hl(row: &crate::mergeview::MergeRow) -> (Option<Color32>, Option<Color32>) {
    if row.in_conflict {
        (Some(hl_replace_l()), Some(hl_replace_r()))
    } else {
        (None, None)
    }
}

#[allow(clippy::too_many_arguments)] // egui 行绘制参数较多，保持扁平可读
fn paint_merge_row(
    ui: &mut egui::Ui,
    row: &crate::mergeview::MergeRow,
    gutter: f32,
    col_w: f32,
    bg_b: Option<Color32>,
    bg_l: Option<Color32>,
    bg_r: Option<Color32>,
    hl_l: Option<Color32>,
    hl_r: Option<Color32>,
    fg: Color32,
    syn_b: Option<&'static syntect::parsing::SyntaxReference>,
    syn_l: Option<&'static syntect::parsing::SyntaxReference>,
    syn_r: Option<&'static syntect::parsing::SyntaxReference>,
) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(
        Vec2::new(gutter * 3.0 + col_w * 3.0, ROW_H),
        egui::Sense::click(),
    );
    let x = rect.left();
    let y = rect.top();

    // P31 冲突行：左侧竖条标记（黄色，BC 风格）
    if row.in_conflict {
        ui.painter().rect_filled(
            Rect::from_min_size(Pos2::new(x, y), vec2(super::theme::CURRENT_BAR, ROW_H)),
            0.0,
            super::theme::diff_modify(),
        );
    }

    // BASE 列
    let g = Rect::from_min_size(Pos2::new(x, y), vec2(gutter, ROW_H));
    paint_bg(ui, g, bg_b);
    paint_line_no(ui, g, row.base_no);
    let c = Rect::from_min_size(Pos2::new(x + gutter, y), vec2(col_w, ROW_H));
    paint_bg(ui, c, bg_b);
    paint_cell(ui, c, row.base.as_ref(), fg, None, syn_b, 0.0);

    // LEFT 列
    let xl = x + gutter + col_w;
    let g = Rect::from_min_size(Pos2::new(xl, y), vec2(gutter, ROW_H));
    paint_bg(ui, g, bg_l);
    paint_line_no(ui, g, None);
    let c = Rect::from_min_size(Pos2::new(xl + gutter, y), vec2(col_w, ROW_H));
    paint_bg(ui, c, bg_l);
    paint_cell(ui, c, row.left.as_ref(), fg, hl_l, syn_l, 0.0);

    // RIGHT 列
    let xr = xl + gutter + col_w;
    let g = Rect::from_min_size(Pos2::new(xr, y), vec2(gutter, ROW_H));
    paint_bg(ui, g, bg_r);
    paint_line_no(ui, g, None);
    let c = Rect::from_min_size(Pos2::new(xr + gutter, y), vec2(col_w, ROW_H));
    paint_bg(ui, c, bg_r);
    paint_cell(ui, c, row.right.as_ref(), fg, hl_r, syn_r, 0.0);
    resp
}

fn basename(p: &str) -> String {
    std::path::Path::new(p)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.to_string())
}
