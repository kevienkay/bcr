//! P37-1i：文件夹合并标签页（对标 BC Folder Merge）。
//!
//! BASE/LEFT/RIGHT 三目录 + 输出目录：生成合并计划（build_merge3_plan），
//! 列表展示 copy/merge/conflict/delete/same 状态，执行后写入输出目录。

use crate::i18n::{fmt, t, Key as I18nKey};
use crate::merge3::{build_merge3_plan, execute_plan, Merge3PlanItem, Merge3Stats};
use eframe::egui::{self, Color32, RichText};

/// 文件夹合并标签页
pub struct FolderMergeTab {
    pub base: String,
    pub left: String,
    pub right: String,
    pub out: String,
    /// 合并计划（生成后缓存）
    pub(crate) plan: Option<Vec<Merge3PlanItem>>,
    /// 计划统计
    pub(crate) stats: Merge3Stats,
    pub error: Option<String>,
    pub msg: Option<String>,
    /// 滚动偏移
    pub(crate) scroll: egui::Vec2,
    /// 生成计划请求
    gen_req: bool,
    /// 执行合并请求
    exec_req: bool,
}

impl FolderMergeTab {
    pub fn new(base: &str, left: &str, right: &str, out: &str) -> Self {
        let mut t = FolderMergeTab {
            base: base.to_string(),
            left: left.to_string(),
            right: right.to_string(),
            out: out.to_string(),
            plan: None,
            stats: Merge3Stats::default(),
            error: None,
            msg: None,
            scroll: egui::Vec2::ZERO,
            gen_req: false,
            exec_req: false,
        };
        t.reload();
        t
    }

    pub fn title(&self) -> String {
        if self.base.is_empty() && self.left.is_empty() && self.right.is_empty() {
            t(I18nKey::FolderMergeTitle).to_string()
        } else {
            let n = |p: &str| {
                std::path::Path::new(p)
                    .file_name()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| p.to_string())
            };
            format!(
                "🗂 {} ↔ {} ↔ {}",
                n(&self.base),
                n(&self.left),
                n(&self.right)
            )
        }
    }

    /// P34：空会话守卫
    pub fn is_empty(&self) -> bool {
        self.base.is_empty() && self.left.is_empty() && self.right.is_empty()
    }

    /// 生成合并计划（BASE/LEFT/RIGHT → OUT）
    pub fn reload(&mut self) {
        self.error = None;
        if self.is_empty() {
            self.plan = None;
            self.stats = Merge3Stats::default();
            return;
        }
        let (b, l, r) = match (
            crate::vfs::open(&self.base),
            crate::vfs::open(&self.left),
            crate::vfs::open(&self.right),
        ) {
            (Ok(b), Ok(l), Ok(r)) => (b, l, r),
            (Err(e), _, _) => {
                self.error = Some(format!("打开 BASE {} 失败: {}", self.base, e));
                self.plan = None;
                return;
            }
            (_, Err(e), _) => {
                self.error = Some(format!("打开 LEFT {} 失败: {}", self.left, e));
                self.plan = None;
                return;
            }
            (_, _, Err(e)) => {
                self.error = Some(format!("打开 RIGHT {} 失败: {}", self.right, e));
                self.plan = None;
                return;
            }
        };
        let filter = crate::fsscan::Filter::new(&[], &[]).unwrap_or_else(|_| {
            // 空模式不可能失败，兜底用无过滤
            crate::fsscan::Filter::new(&[], &[]).unwrap()
        });
        match build_merge3_plan(b.as_ref(), l.as_ref(), r.as_ref(), &filter, false) {
            Ok((plan, stats)) => {
                self.plan = Some(plan);
                self.stats = stats;
                self.msg = None;
            }
            Err(e) => {
                self.error = Some(format!("生成计划失败: {}", e));
            }
        }
    }

    /// 打开 BASE 目录（空会话填充）
    pub fn open_base(&mut self) {
        if let Some(p) = super::pick_dir() {
            self.base = p;
            self.reload();
        }
    }

    /// 打开 LEFT 目录
    pub fn open_left(&mut self) {
        if let Some(p) = super::pick_dir() {
            self.left = p;
            self.reload();
        }
    }

    /// 打开 RIGHT 目录
    pub fn open_right(&mut self) {
        if let Some(p) = super::pick_dir() {
            self.right = p;
            self.reload();
        }
    }

    /// 选择输出目录
    pub fn open_out(&mut self) {
        if let Some(p) = super::pick_dir() {
            self.out = p;
        }
    }

    /// 执行合并计划到输出目录（复用 execute_plan）
    pub fn execute(&mut self) {
        let Some(plan) = self.plan.clone() else {
            self.msg = Some("请先生成计划".to_string());
            return;
        };
        if self.out.is_empty() {
            self.msg = Some("请选择输出目录".to_string());
            return;
        }
        // 输出目录不存在时自动创建（CLI 语义：merge3 也会创建输出目录）
        if let Err(e) = std::fs::create_dir_all(&self.out) {
            self.msg = Some(format!("创建输出目录失败: {}", e));
            return;
        }
        let (b, l, r, o) = match (
            crate::vfs::open(&self.base),
            crate::vfs::open(&self.left),
            crate::vfs::open(&self.right),
            crate::vfs::open(&self.out),
        ) {
            (Ok(b), Ok(l), Ok(r), Ok(o)) => (b, l, r, o),
            _ => {
                self.msg = Some("打开目录失败".to_string());
                return;
            }
        };
        match execute_plan(
            b.as_ref(),
            l.as_ref(),
            r.as_ref(),
            o.as_ref(),
            &plan,
            similar::Algorithm::Patience,
            false,
        ) {
            Ok(conflicts) => {
                let s = self.stats;
                self.msg = Some(fmt(
                    I18nKey::MergeStats,
                    &[
                        &s.copied.to_string(),
                        &s.merged.to_string(),
                        &s.deleted.to_string(),
                        &conflicts.to_string(),
                    ],
                ));
            }
            Err(e) => {
                self.msg = Some(format!("执行合并失败: {}", e));
            }
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("foldermerge_tools").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui
                    .button(format!("🗂 {}", t(I18nKey::GeneratePlan)))
                    .on_hover_text("重新扫描三目录并生成合并计划")
                    .clicked()
                {
                    self.gen_req = true;
                }
                if ui
                    .button(format!("⚡ {}", t(I18nKey::ExecuteMerge)))
                    .on_hover_text("把合并结果写入输出目录")
                    .clicked()
                {
                    self.exec_req = true;
                }
                ui.separator();
                let s = self.stats;
                ui.label(fmt(
                    I18nKey::MergeStats,
                    &[
                        &s.copied.to_string(),
                        &s.merged.to_string(),
                        &s.deleted.to_string(),
                        &s.conflicts.to_string(),
                    ],
                ));
                ui.separator();
                if !self.out.is_empty() {
                    ui.label(format!("→ {}", self.out));
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
        if let Some(m) = self.msg.clone() {
            egui::Window::new(t(I18nKey::Hint))
                .collapsible(false)
                .resizable(false)
                .show(ui.ctx(), |ui| {
                    ui.label(m);
                    if ui.button(t(I18nKey::Close)).clicked() {
                        self.msg = None;
                    }
                });
        }

        egui::CentralPanel::default().show(ui, |ui| {
            if self.is_empty() {
                ui.centered_and_justified(|ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(t(I18nKey::FolderMergeTitle))
                                .size(16.0)
                                .color(ui.visuals().weak_text_color()),
                        );
                        ui.add_space(12.0);
                        // P34：分别打开 BASE/LEFT/RIGHT/OUT
                        ui.horizontal(|ui| {
                            if ui.button("BASE…").clicked() {
                                self.open_base();
                            }
                            if ui.button("LEFT…").clicked() {
                                self.open_left();
                            }
                            if ui.button("RIGHT…").clicked() {
                                self.open_right();
                            }
                            if ui.button(t(I18nKey::MergeOut)).clicked() {
                                self.open_out();
                            }
                        });
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new(t(I18nKey::DragHint))
                                .size(11.0)
                                .color(ui.visuals().weak_text_color()),
                        );
                    });
                });
                return;
            }

            let Some(plan) = &self.plan else {
                ui.centered_and_justified(|ui| {
                    ui.label("点击「生成计划」扫描三目录");
                });
                return;
            };
            let fg = ui.visuals().text_color();
            let out = super::show_rows(ui, plan.len(), super::theme::ROW_H, |ui, range| {
                for i in range {
                    let item = &plan[i];
                    let (rect, _) = ui.allocate_exact_size(
                        egui::Vec2::new(ui.available_width().max(300.0), super::theme::ROW_H),
                        egui::Sense::hover(),
                    );
                    // 操作徽标颜色
                    let (mark, color) = match item.op.as_str() {
                        "copy" => (
                            if item.conflicted {
                                t(I18nKey::PlanConflict)
                            } else {
                                "copy"
                            },
                            if item.conflicted {
                                Color32::from_rgb(230, 80, 80)
                            } else {
                                Color32::from_rgb(80, 160, 255)
                            },
                        ),
                        "merge" => ("merge", Color32::from_rgb(200, 160, 60)),
                        "delete" => ("delete", Color32::from_rgb(230, 80, 80)),
                        _ => ("same", Color32::from_gray(120)),
                    };
                    ui.painter().text(
                        egui::Pos2::new(rect.left() + 6.0, rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        mark,
                        egui::FontId::monospace(12.0),
                        color,
                    );
                    ui.painter().text(
                        egui::Pos2::new(rect.left() + 76.0, rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        &item.rel,
                        egui::FontId::monospace(super::theme::FONT_SIZE),
                        fg,
                    );
                    if let Some(from) = &item.from {
                        ui.painter().text(
                            egui::Pos2::new(rect.right() - 60.0, rect.center().y),
                            egui::Align2::RIGHT_CENTER,
                            format!("← {}", from),
                            egui::FontId::monospace(12.0),
                            ui.visuals().weak_text_color(),
                        );
                    }
                }
            });
            self.scroll = out.state.offset;
        });

        // 请求处理（闭包外执行）
        if self.gen_req {
            self.reload();
            self.gen_req = false;
        }
        if self.exec_req {
            self.execute();
            self.exec_req = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn write(dir: &std::path::Path, name: &str, content: &str) -> String {
        let p = dir.join(name);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, content).unwrap();
        p.to_str().unwrap().to_string()
    }

    #[test]
    fn plan_detects_copy_merge_delete() {
        let d = tempdir().unwrap();
        // base: a.txt 相同；left: l.txt 仅左；right: r.txt 仅右；c.txt 两侧不同 → 冲突/合并
        let base = d.path().join("base");
        let left = d.path().join("left");
        let right = d.path().join("right");
        fs::create_dir_all(&base).unwrap();
        fs::create_dir_all(&left).unwrap();
        fs::create_dir_all(&right).unwrap();
        write(&base, "a.txt", "same\n");
        write(&left, "a.txt", "same\n");
        write(&right, "a.txt", "same\n");
        write(&left, "l.txt", "L\n");
        write(&right, "r.txt", "R\n");
        write(&base, "c.txt", "base\n");
        write(&left, "c.txt", "left\n");
        write(&right, "c.txt", "right\n");

        let t = FolderMergeTab::new(
            base.to_str().unwrap(),
            left.to_str().unwrap(),
            right.to_str().unwrap(),
            d.path().join("out").to_str().unwrap(),
        );
        assert!(t.error.is_none(), "不应出错: {:?}", t.error);
        let plan = t.plan.as_ref().expect("应生成计划");
        assert!(plan
            .iter()
            .any(|i| i.rel == "l.txt" && i.op == "copy" && i.from.as_deref() == Some("left")));
        assert!(plan
            .iter()
            .any(|i| i.rel == "r.txt" && i.op == "copy" && i.from.as_deref() == Some("right")));
        // c.txt 两侧都改 → merge 或 conflict
        assert!(plan
            .iter()
            .any(|i| i.rel == "c.txt" && (i.op == "merge" || i.conflicted)));
    }

    #[test]
    fn execute_writes_output_dir() {
        let d = tempdir().unwrap();
        let base = d.path().join("base");
        let left = d.path().join("left");
        let right = d.path().join("right");
        let out = d.path().join("out");
        fs::create_dir_all(&base).unwrap();
        fs::create_dir_all(&left).unwrap();
        fs::create_dir_all(&right).unwrap();
        write(&base, "a.txt", "same\n");
        write(&left, "a.txt", "same\n");
        write(&right, "a.txt", "same\n");
        write(&left, "l.txt", "L\n");

        let mut t = FolderMergeTab::new(
            base.to_str().unwrap(),
            left.to_str().unwrap(),
            right.to_str().unwrap(),
            out.to_str().unwrap(),
        );
        t.execute();
        // merge3 语义：Same 文件不复制到输出，输出只含变更文件
        // 输出目录出现 l.txt（仅左侧复制）
        assert!(out.join("l.txt").exists(), "输出应有 l.txt（仅左侧复制）");
        assert_eq!(fs::read_to_string(out.join("l.txt")).unwrap(), "L\n");
        // 相同文件 a.txt 不出现在输出（除非预先复制 base）
        assert!(!out.join("a.txt").exists(), "相同文件不应复制到输出");
    }
}
