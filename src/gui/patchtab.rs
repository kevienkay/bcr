//! P37-1h：补丁视图（对标 BC Text Patch，`.patch`/`.diff` 自动进入）。
//!
//! 解析补丁文件 → 左侧（旧）vs 右侧（新）双栏对比，
//! 支持应用补丁（把右侧内容写回 b 侧路径，A2 模式 .bak 备份）。

use super::common::*;
use crate::i18n::{fmt, t, Key as I18nKey};
use crate::patchview::{parse_patch, ParsedPatch};
use crate::sideview::{build_rows, RowTag, SideRow, ViewOptions};
use eframe::egui::{self, Color32, Pos2, Rect, Vec2};

/// 补丁标签页
pub struct PatchTab {
    path: String,
    /// 解析结果（状态栏展示统计）
    pub(crate) parsed: Option<ParsedPatch>,
    /// 渲染行（build_rows 结果）
    rows: Vec<SideRow>,
    /// 错误信息（测试访问）
    pub(crate) error: Option<String>,
    /// 应用补丁结果消息
    msg: Option<String>,
    scroll: Vec2,
    /// 打开补丁文件对话框请求
    open_req: bool,
    /// 应用补丁请求
    apply_req: bool,
}

impl PatchTab {
    pub fn new(path: &str) -> Self {
        let mut t = PatchTab {
            path: String::new(),
            parsed: None,
            rows: Vec::new(),
            error: None,
            msg: None,
            scroll: Vec2::ZERO,
            open_req: false,
            apply_req: false,
        };
        t.open(path);
        t
    }

    pub fn title(&self) -> String {
        if self.path.is_empty() {
            t(I18nKey::PatchTitle).to_string()
        } else {
            let name = std::path::Path::new(&self.path)
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| self.path.clone());
            format!("🧩 {}", name)
        }
    }

    /// 打开补丁文件并解析
    pub fn open(&mut self, path: &str) {
        match std::fs::read_to_string(path) {
            Ok(text) => match parse_patch(&text) {
                Some(p) => {
                    self.path = path.to_string();
                    self.parsed = Some(p.clone());
                    let (rows, _) = build_rows(&p.left, &p.right, ViewOptions::default());
                    self.rows = rows;
                    self.error = None;
                    self.msg = None;
                }
                None => {
                    self.error = Some(format!("{}: 不是有效的 unified diff 格式", path));
                }
            },
            Err(e) => {
                self.error = Some(format!("读取 {} 失败: {}", path, e));
            }
        }
    }

    /// 打开文件对话框
    pub fn open_dialog(&mut self) {
        if let Some(p) = super::pick_file() {
            self.open(&p);
        }
    }

    /// 是否为空会话
    pub fn is_empty(&self) -> bool {
        self.path.is_empty()
    }

    /// 应用补丁：把右侧（新）内容写回 b 侧路径；b 侧路径为空时写回原补丁路径。
    /// A2 模式 .bak 备份。返回 (目标路径, 是否成功)。
    pub fn apply(&mut self) -> Option<(String, bool)> {
        let p = self.parsed.clone()?;
        let target = if p.b_path.is_empty() {
            self.path.clone()
        } else {
            // b 侧路径可能带子目录：优先相对于补丁所在目录解析
            let parent = std::path::Path::new(&self.path)
                .parent()
                .map(|d| d.to_path_buf())
                .unwrap_or_default();
            let joined = parent.join(&p.b_path);
            if joined.exists() || parent.as_os_str().is_empty() {
                joined.to_string_lossy().into_owned()
            } else {
                p.b_path.clone()
            }
        };
        let _ = std::fs::copy(&target, format!("{target}.bak"));
        match std::fs::write(&target, &p.right) {
            Ok(()) => {
                self.msg = Some(fmt(I18nKey::PatchApplied, &[&target]));
                Some((target, true))
            }
            Err(e) => {
                self.msg = Some(format!("应用补丁失败: {}", e));
                Some((target, false))
            }
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("patch_tools").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui.button(t(I18nKey::OpenFile)).clicked() {
                    self.open_dialog();
                }
                if ui
                    .button(format!("⚡ {}", t(I18nKey::ApplyPatch)))
                    .on_hover_text("把右侧（新）内容写回目标文件")
                    .clicked()
                {
                    self.apply_req = true;
                }
                ui.separator();
                if let Some(p) = &self.parsed {
                    ui.label(fmt(I18nKey::PatchAdded, &[&p.added.to_string()]));
                    ui.label(fmt(I18nKey::PatchRemoved, &[&p.removed.to_string()]));
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
            if self.path.is_empty() {
                ui.centered_and_justified(|ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new(t(I18nKey::PatchTitle))
                                .size(16.0)
                                .color(ui.visuals().weak_text_color()),
                        );
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            if ui.button(t(I18nKey::OpenFile)).clicked() {
                                self.open_dialog();
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

            // 双栏渲染：左侧（旧）/ 右侧（新）
            let rows = &self.rows;
            let total = rows.len();
            if total == 0 {
                ui.centered_and_justified(|ui| {
                    ui.label(
                        egui::RichText::new(t(I18nKey::DiffEmptyHint))
                            .size(14.0)
                            .color(ui.visuals().weak_text_color()),
                    );
                });
                return;
            }
            let max_no_l = rows.iter().filter_map(|r| r.left_no).max().unwrap_or(0);
            let max_no_r = rows.iter().filter_map(|r| r.right_no).max().unwrap_or(0);
            let gutter = gutter_width(max_no_l.max(max_no_r));
            let avail = ui.available_width();
            let half = ((avail - gutter * 2.0) / 2.0).max(200.0);
            let fg = text_color(ui);
            let out = super::show_rows(ui, total, ROW_H, |ui, range| {
                ui.set_min_width(gutter * 2.0 + half * 2.0);
                for i in range {
                    let row = &rows[i];
                    let (rect, _) = ui.allocate_exact_size(
                        Vec2::new(gutter * 2.0 + half * 2.0, ROW_H),
                        egui::Sense::hover(),
                    );
                    // 状态底色
                    let bg = match row.tag {
                        RowTag::Delete => Some(bg_replace_l()),
                        RowTag::Insert => Some(bg_replace_r()),
                        _ => None,
                    };
                    // 左列
                    let lr = Rect::from_min_size(rect.min, vec2(gutter + half, ROW_H));
                    paint_bg(ui, lr, bg);
                    paint_line_no(
                        ui,
                        Rect::from_min_size(rect.min, vec2(gutter, ROW_H)),
                        row.left_no,
                    );
                    paint_cell(
                        ui,
                        Rect::from_min_size(
                            Pos2::new(rect.left() + gutter, rect.top()),
                            vec2(half, ROW_H),
                        ),
                        row.left.as_ref(),
                        fg,
                        None,
                        None,
                        0.0,
                        false,
                    );
                    // 右列
                    let rr = Rect::from_min_size(
                        Pos2::new(rect.left() + gutter + half, rect.top()),
                        vec2(gutter + half, ROW_H),
                    );
                    paint_bg(ui, rr, bg);
                    paint_line_no(
                        ui,
                        Rect::from_min_size(
                            Pos2::new(rect.left() + gutter + half, rect.top()),
                            vec2(gutter, ROW_H),
                        ),
                        row.right_no,
                    );
                    paint_cell(
                        ui,
                        Rect::from_min_size(
                            Pos2::new(rect.left() + gutter * 2.0 + half, rect.top()),
                            vec2(half, ROW_H),
                        ),
                        row.right.as_ref(),
                        fg,
                        None,
                        None,
                        0.0,
                        false,
                    );
                }
            });
            self.scroll = out.state.offset;
        });

        // 请求处理（闭包外执行，借用安全）
        if self.open_req {
            self.open_dialog();
            self.open_req = false;
        }
        if self.apply_req {
            self.apply();
            self.apply_req = false;
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
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&p, content).unwrap();
        p.to_str().unwrap().to_string()
    }

    #[test]
    fn open_parses_and_renders_rows() {
        let d = tempdir().unwrap();
        let p = write(
            d.path(),
            "a.patch",
            "--- a/src/a.txt\n+++ b/src/a.txt\n@@ -1,3 +1,3 @@\n line1\n-old line\n+new line\n line3\n",
        );
        let t = PatchTab::new(&p);
        assert!(t.error.is_none(), "解析不应出错: {:?}", t.error);
        let parsed = t.parsed.as_ref().unwrap();
        assert_eq!(parsed.left, "line1\nold line\nline3");
        assert_eq!(parsed.right, "line1\nnew line\nline3");
        assert_eq!(parsed.added, 1);
        assert_eq!(parsed.removed, 1);
        // 有渲染行（含差异）
        assert!(t.rows.iter().any(|r| r.tag != RowTag::Equal));
    }

    #[test]
    fn apply_writes_right_side_to_b_path() {
        let d = tempdir().unwrap();
        // 补丁同目录存在 b 路径文件
        write(d.path(), "a.txt", "line1\nold line\nline3\n");
        let patch = write(
            d.path(),
            "a.patch",
            "--- a/a.txt\n+++ b/a.txt\n@@ -1,3 +1,3 @@\n line1\n-old line\n+new line\n line3\n",
        );
        let mut t = PatchTab::new(&patch);
        let (target, ok) = t.apply().expect("应返回结果");
        assert!(ok, "应用应成功");
        assert!(target.ends_with("a.txt"), "目标应为 a.txt: {}", target);
        assert_eq!(
            fs::read_to_string(&target).unwrap(),
            "line1\nnew line\nline3"
        );
        // .bak 备份存在
        assert!(fs::metadata(format!("{target}.bak")).is_ok());
    }

    #[test]
    fn reject_non_patch_file() {
        let d = tempdir().unwrap();
        let p = write(d.path(), "x.txt", "just text\n");
        let t = PatchTab::new(&p);
        assert!(t.error.is_some(), "非补丁文件应报错");
        assert!(t.parsed.is_none());
    }
}
