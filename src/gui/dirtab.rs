//! 目录对比标签页：差异文件列表 + 点击打开并排 Diff。

use super::common::*;
use crate::compare::{compare_dirs, CompareResult, FileStatus};
use crate::fsscan::Filter;
use eframe::egui::{self, Color32, Pos2, Vec2};

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
        }
    }

    pub fn title(&self) -> String {
        format!(
            "目录: {} ↔ {}",
            basename(&self.left),
            basename(&self.right)
        )
    }

    pub fn refresh(&mut self) {
        let filter = match Filter::new(
            &split_globs(&self.includes),
            &split_globs(&self.excludes),
        ) {
            Ok(f) => f,
            Err(e) => {
                self.error = Some(format!("过滤规则错误: {e}"));
                self.result = None;
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
                self.error = Some(format!("扫描失败: {e}"));
                self.result = None;
            }
        }
    }

    /// 过滤后的展示条目
    fn entries(&self) -> Vec<&crate::compare::FileEntry> {
        let Some(r) = &self.result else { return vec![] };
        let mut v: Vec<_> = r.entries.iter().collect();
        if self.only_diff {
            v.retain(|e| e.status != FileStatus::Same);
        } else if !self.show_same {
            v.retain(|e| e.status != FileStatus::Same);
        }
        v
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("dirtab_tools").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui.button("刷新").clicked() {
                    self.refresh();
                }
                ui.separator();
                if ui.checkbox(&mut self.compare_content, "内容比对(哈希)").changed() {
                    self.refresh();
                }
                if ui.checkbox(&mut self.only_diff, "仅显示差异").changed() {
                    // 无需刷新，只是过滤视图
                }
                if ui.checkbox(&mut self.show_same, "显示相同").changed() {
                    // 仅当 only_diff=false 时生效
                }
                ui.separator();
                let mut inc = self.includes.clone();
                let r1 = ui.add(
                    egui::TextEdit::singleline(&mut inc)
                        .hint_text("包含 glob（逗号分隔）")
                        .desired_width(160.0),
                );
                let mut exc = self.excludes.clone();
                let r2 = ui.add(
                    egui::TextEdit::singleline(&mut exc)
                        .hint_text("排除 glob（逗号分隔）")
                        .desired_width(160.0),
                );
                if (r1.changed() && r1.lost_focus())
                    || (r2.changed() && r2.lost_focus())
                    || ui.button("应用过滤").clicked()
                {
                    self.includes = inc;
                    self.excludes = exc;
                    self.refresh();
                }
                ui.separator();
                if let Some(r) = &self.result {
                    let s = r.stats;
                    ui.label(format!(
                        "相同 {} / 仅左 {} / 仅右 {} / 不同 {}",
                        s.same, s.left_only, s.right_only, s.differ
                    ));
                }
            });
        });

        if let Some(err) = self.error.clone() {
            egui::Window::new("提示")
                .collapsible(false)
                .resizable(false)
                .show(ui.ctx(), |ui| {
                    ui.colored_label(Color32::from_rgb(240, 110, 110), err);
                    if ui.button("关闭").clicked() {
                        self.error = None;
                    }
                });
        }

        egui::CentralPanel::default().show(ui, |ui| {
            if self.result.is_none() && self.error.is_none() {
                self.refresh();
            }
            let entries = self.entries();
            if entries.is_empty() {
                ui.centered_and_justified(|ui| {
                    ui.label(
                        egui::RichText::new("无差异文件（或目录为空）")
                            .size(16.0)
                            .color(ui.visuals().weak_text_color()),
                    );
                });
                return;
            }

            let fg = text_color(ui);
            let mut pending_open: Option<String> = None;
            let out = super::show_rows(ui, entries.len(), ROW_H, |ui, range| {
                for idx in range {
                    let e = entries[idx];
                    let (rect, resp) = ui.allocate_exact_size(
                        Vec2::new(ui.available_width().max(400.0), ROW_H),
                        egui::Sense::click(),
                    );
                    let bg = if resp.hovered() || resp.is_pointer_button_down_on() {
                        Some(bg_match())
                    } else {
                        None
                    };
                    paint_bg(ui, rect, bg);
                    let letter = e.status.letter();
                    let color = status_color(ui, letter);
                    ui.painter().text(
                        Pos2::new(rect.left() + 4.0, rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        format!("[{letter}]"),
                        egui::FontId::monospace(14.0),
                        color,
                    );
                        ui.painter().text(
                            Pos2::new(rect.left() + 48.0, rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            &e.rel,
                            egui::FontId::monospace(14.0),
                            fg,
                        );
                        // 两侧大小（元数据已由 compare_dirs 填充）
                        let size_text = match (&e.left, &e.right) {
                            (Some(l), Some(r)) => format!("{}B → {}B", l.size, r.size),
                            (Some(l), None) => format!("{}B → -", l.size),
                            (None, Some(r)) => format!("- → {}B", r.size),
                            (None, None) => String::new(),
                        };
                        if !size_text.is_empty() {
                            let tw = ui
                                .painter()
                                .layout_no_wrap(
                                    size_text.clone(),
                                    egui::FontId::monospace(12.0),
                                    ui.visuals().weak_text_color(),
                                )
                                .size()
                                .x;
                            ui.painter().text(
                                Pos2::new(rect.right() - tw - 8.0, rect.center().y),
                                egui::Align2::LEFT_CENTER,
                                size_text,
                                egui::FontId::monospace(12.0),
                                ui.visuals().weak_text_color(),
                            );
                        }
                        // 双击打开并排 diff（只记录 rel，闭包外处理）
                        if resp.double_clicked() {
                            pending_open = Some(e.rel.clone());
                        }
                    }
            });
            self.scroll = out.state.offset;
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
