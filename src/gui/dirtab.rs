//! 目录对比标签页：树形差异视图（可折叠）+ 键盘导航 + 双击打开并排 Diff。

use super::common::*;
use crate::compare::{compare_dirs, CompareResult, FileStatus};
use crate::fsscan::Filter;
use crate::i18n::{fmt, t, Key as I18nKey};
use eframe::egui::{self, Color32, Key, Pos2, Vec2};
use std::collections::HashSet;

/// 目录标签页
pub struct DirTab {
    pub left: String,
    pub right: String,
    pub compare_content: bool,
    pub includes: String,
    pub excludes: String,
    pub show_same: bool,
    /// 仅显示差异文件
    pub only_diff: bool,
    pub result: Option<CompareResult>,
    pub error: Option<String>,
    pub scroll: Vec2,
    /// 请求打开并排 diff（rel 相对路径，由主应用拼完整路径）
    pub open_diff: Option<String>,
    /// 折叠的目录路径集合（空字符串表示根）
    pub(crate) collapsed: HashSet<String>,
    /// 选中的展平行索引
    pub(crate) selected: Option<usize>,
    /// 展平后的行
    pub(crate) flat: Vec<FlatRow>,
    /// 需要滚动到选中行的标记
    scroll_to_selected: bool,
}

/// 展平后的树行
pub(crate) struct FlatRow {
    pub(crate) depth: usize,
    pub(crate) name: String,
    pub(crate) path: String,
    pub(crate) is_dir: bool,
    /// 目录是否展开
    pub(crate) expanded: bool,
    /// 文件在 entries 中的索引
    pub(crate) entry: Option<usize>,
}

impl DirTab {
    pub fn new(left: &str, right: &str) -> Self {
        DirTab {
            left: left.to_string(),
            right: right.to_string(),
            compare_content: false,
            includes: String::new(),
            excludes: String::new(),
            show_same: false,
            only_diff: true,
            result: None,
            error: None,
            scroll: Vec2::ZERO,
            open_diff: None,
            collapsed: HashSet::new(),
            selected: None,
            flat: Vec::new(),
            scroll_to_selected: false,
        }
    }

    pub fn title(&self) -> String {
        fmt(
            I18nKey::DirTitle,
            &[&basename(&self.left), &basename(&self.right)],
        )
    }

    pub fn refresh(&mut self) {
        let filter = match Filter::new(&split_globs(&self.includes), &split_globs(&self.excludes)) {
            Ok(f) => f,
            Err(e) => {
                self.error = Some(fmt(I18nKey::FilterError, &[&e.to_string()]));
                self.result = None;
                self.flat.clear();
                return;
            }
        };
        match compare_dirs(
            std::path::Path::new(&self.left),
            std::path::Path::new(&self.right),
            &filter,
            self.compare_content,
        ) {
            Ok(r) => {
                for w in &r.warnings {
                    self.error = Some(w.clone());
                }
                self.result = Some(r);
            }
            Err(e) => {
                self.error = Some(fmt(I18nKey::ScanFailed, &[&e.to_string()]));
                self.result = None;
            }
        }
        self.rebuild_tree();
    }

    /// 从结果重建树并展平
    fn rebuild_tree(&mut self) {
        self.flat.clear();
        let Some(r) = &self.result else { return };
        let mut visible: Vec<&crate::compare::FileEntry> = r.entries.iter().collect();
        if self.only_diff {
            visible.retain(|e| e.status != FileStatus::Same);
        }
        if visible.is_empty() {
            self.selected = None;
            return;
        }
        // 构建树：按 '/' 分段
        #[derive(Default)]
        struct Node {
            dirs: std::collections::BTreeMap<String, Node>,
            files: Vec<(String, usize)>, // (name, entry_idx)
        }
        let mut root = Node::default();
        for (idx, e) in visible.iter().enumerate() {
            let parts: Vec<&str> = e.rel.split('/').collect();
            let mut node = &mut root;
            for (i, part) in parts.iter().enumerate() {
                if i + 1 == parts.len() {
                    node.files.push((part.to_string(), idx));
                } else {
                    node = node.dirs.entry(part.to_string()).or_default();
                }
            }
        }
        // 展平（跳过折叠目录）
        let mut out: Vec<FlatRow> = Vec::new();
        fn walk(
            node: &Node,
            path: &str,
            depth: usize,
            collapsed: &HashSet<String>,
            visible: &[&crate::compare::FileEntry],
            out: &mut Vec<FlatRow>,
        ) {
            for (dir_name, child) in &node.dirs {
                let dir_path = if path.is_empty() {
                    dir_name.clone()
                } else {
                    format!("{path}/{dir_name}")
                };
                let expanded = !collapsed.contains(&dir_path);
                out.push(FlatRow {
                    depth,
                    name: format!("{dir_name}/"),
                    path: dir_path.clone(),
                    is_dir: true,
                    expanded,
                    entry: None,
                });
                if expanded {
                    walk(child, &dir_path, depth + 1, collapsed, visible, out);
                }
            }
            for (name, idx) in &node.files {
                let _ = visible;
                out.push(FlatRow {
                    depth,
                    name: name.clone(),
                    path: String::new(),
                    is_dir: false,
                    expanded: false,
                    entry: Some(*idx),
                });
            }
        }
        walk(&root, "", 0, &self.collapsed, &visible, &mut out);
        self.flat = out;
        if self.selected.is_none() && !self.flat.is_empty() {
            self.selected = Some(0);
        }
    }

    pub(crate) fn toggle_dir(&mut self, path: &str) {
        if self.collapsed.contains(path) {
            self.collapsed.remove(path);
        } else {
            self.collapsed.insert(path.to_string());
        }
        self.rebuild_tree();
    }

    pub(crate) fn open_selected(&mut self) {
        let Some(idx) = self.selected else { return };
        let Some(row) = self.flat.get(idx) else {
            return;
        };
        let (is_dir, path, entry) = (row.is_dir, row.path.clone(), row.entry);
        if is_dir {
            self.toggle_dir(&path);
        } else if let Some(ei) = entry {
            if let Some(r) = &self.result {
                self.open_diff = Some(r.entries[ei].rel.clone());
            }
        }
    }

    /// 键盘导航：上下选择、左右折叠、回车打开
    fn handle_keys(&mut self, ui: &egui::Ui) {
        if self.flat.is_empty() {
            return;
        }
        let n = self.flat.len();
        let sel = self.selected.unwrap_or(0);
        if ui.input(|i| i.key_pressed(Key::ArrowDown)) {
            self.selected = Some((sel + 1).min(n - 1));
            self.scroll_to_selected = true;
        }
        if ui.input(|i| i.key_pressed(Key::ArrowUp)) {
            self.selected = Some(sel.saturating_sub(1));
            self.scroll_to_selected = true;
        }
        if ui.input(|i| i.key_pressed(Key::ArrowRight)) {
            if let Some(row) = self.flat.get(sel) {
                if row.is_dir && !row.expanded {
                    let p = row.path.clone();
                    self.toggle_dir(&p);
                }
            }
        }
        if ui.input(|i| i.key_pressed(Key::ArrowLeft)) {
            if let Some(row) = self.flat.get(sel) {
                if row.is_dir && row.expanded {
                    let p = row.path.clone();
                    self.toggle_dir(&p);
                }
            }
        }
        if ui.input(|i| i.key_pressed(Key::Enter)) {
            self.open_selected();
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("dirtab_tools").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui.button(t(I18nKey::Refresh)).clicked() {
                    self.refresh();
                }
                ui.separator();
                if ui
                    .checkbox(&mut self.compare_content, t(I18nKey::ContentHash))
                    .changed()
                {
                    self.refresh();
                }
                if ui
                    .checkbox(&mut self.only_diff, t(I18nKey::OnlyDiff))
                    .changed()
                {
                    self.rebuild_tree();
                }
                if ui
                    .checkbox(&mut self.show_same, t(I18nKey::ShowSame))
                    .changed()
                    && !self.only_diff
                {
                    self.rebuild_tree();
                }
                ui.separator();
                let mut inc = self.includes.clone();
                let r1 = ui.add(
                    egui::TextEdit::singleline(&mut inc)
                        .hint_text(t(I18nKey::IncludeGlob))
                        .desired_width(150.0),
                );
                let mut exc = self.excludes.clone();
                let r2 = ui.add(
                    egui::TextEdit::singleline(&mut exc)
                        .hint_text(t(I18nKey::ExcludeGlob))
                        .desired_width(150.0),
                );
                if (r1.changed() && r1.lost_focus())
                    || (r2.changed() && r2.lost_focus())
                    || ui.button(t(I18nKey::ApplyFilter)).clicked()
                {
                    self.includes = inc;
                    self.excludes = exc;
                    self.refresh();
                }
                ui.separator();
                if let Some(r) = &self.result {
                    let s = r.stats;
                    ui.label(fmt(
                        I18nKey::DirStats,
                        &[
                            &s.same.to_string(),
                            &s.left_only.to_string(),
                            &s.right_only.to_string(),
                            &s.differ.to_string(),
                        ],
                    ));
                }
            });
        });

        if let Some(err) = self.error.clone() {
            egui::Window::new(t(I18nKey::Hint))
                .collapsible(false)
                .resizable(false)
                .show(ui.ctx(), |ui| {
                    ui.colored_label(Color32::from_rgb(240, 110, 110), err);
                    if ui.button(t(I18nKey::Close)).clicked() {
                        self.error = None;
                    }
                });
        }

        self.handle_keys(ui);

        egui::CentralPanel::default().show(ui, |ui| {
            if self.result.is_none() && self.error.is_none() {
                self.refresh();
            }
            if self.flat.is_empty() {
                ui.centered_and_justified(|ui| {
                    ui.label(
                        egui::RichText::new(t(I18nKey::NoDiff))
                            .size(16.0)
                            .color(ui.visuals().weak_text_color()),
                    );
                });
                return;
            }

            let fg = text_color(ui);
            let mut pending_open: Option<String> = None;
            let mut pending_toggle: Option<String> = None;
            let mut scroll_to_sel = self.scroll_to_selected;
            self.scroll_to_selected = false;
            let selected = self.selected;

            let out = super::show_rows(ui, self.flat.len(), ROW_H, |ui, range| {
                for idx in range {
                    let row = &self.flat[idx];
                    let (rect, resp) = ui.allocate_exact_size(
                        Vec2::new(ui.available_width().max(400.0), ROW_H),
                        egui::Sense::click(),
                    );
                    let is_sel = selected == Some(idx);
                    let bg = if is_sel {
                        Some(bg_match_current())
                    } else if resp.hovered() {
                        Some(bg_match())
                    } else {
                        None
                    };
                    paint_bg(ui, rect, bg);
                    let indent = row.depth as f32 * 16.0;
                    let x0 = rect.left() + 4.0 + indent;

                    if row.is_dir {
                        let arrow = if row.expanded { "▼" } else { "▶" };
                        ui.painter().text(
                            Pos2::new(x0, rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            arrow,
                            egui::FontId::proportional(12.0),
                            fg,
                        );
                        ui.painter().text(
                            Pos2::new(x0 + 16.0, rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            &row.name,
                            egui::FontId::proportional(14.0),
                            fg,
                        );
                        if resp.clicked() {
                            pending_toggle = Some(row.path.clone());
                        }
                    } else if let Some(ei) = row.entry {
                        if let Some(e) = self.result.as_ref().and_then(|r| r.entries.get(ei)) {
                            let letter = e.status.letter();
                            let color = status_color(ui, letter);
                            ui.painter().text(
                                Pos2::new(x0, rect.center().y),
                                egui::Align2::LEFT_CENTER,
                                format!("[{letter}]"),
                                egui::FontId::monospace(14.0),
                                color,
                            );
                            ui.painter().text(
                                Pos2::new(x0 + 44.0, rect.center().y),
                                egui::Align2::LEFT_CENTER,
                                &row.name,
                                egui::FontId::monospace(14.0),
                                fg,
                            );
                            // 两侧大小
                            let size_text = match (&e.left, &e.right) {
                                (Some(l), Some(r)) => format!("{}B → {}B", l.size, r.size),
                                (Some(l), None) => format!("{}B → -", l.size),
                                (None, Some(r)) => format!("- → {}B", r.size),
                                (None, None) => String::new(),
                            };
                            if !size_text.is_empty() {
                                ui.painter().text(
                                    Pos2::new(rect.right() - 8.0, rect.center().y),
                                    egui::Align2::RIGHT_CENTER,
                                    size_text,
                                    egui::FontId::monospace(12.0),
                                    ui.visuals().weak_text_color(),
                                );
                            }
                            if resp.double_clicked() {
                                pending_open = Some(e.rel.clone());
                            }
                            if resp.clicked() {
                                self.selected = Some(idx);
                            }
                        }
                    }
                    // 键盘选中后滚动到该行
                    if is_sel && scroll_to_sel {
                        ui.scroll_to_rect(rect, Some(egui::Align::Center));
                        scroll_to_sel = false;
                    }
                }
            });
            self.scroll = out.state.offset;
            if let Some(p) = pending_toggle {
                self.toggle_dir(&p);
            }
            if pending_open.is_some() {
                self.open_diff = pending_open;
            }
        });
    }
}

fn split_globs(s: &str) -> Vec<String> {
    s.split(',')
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

fn basename(p: &str) -> String {
    std::path::Path::new(p)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.to_string())
}
