//! P37-1g：文本编辑视图（对标 BC Text Edit，`bcomp -edit 文件`）。
//!
//! 单文件编辑器：打开/保存（编码回写 + A2 模式 .bak 备份）、撤销/重做、
//! 查找/替换、转换文件（Trim 行尾空白 / Tabs to Spaces / CRLF↔LF）、
//! 语法高亮预览 + 可见空白 + 行号 gutter。

use super::common::*;
use crate::i18n::{fmt, t, Key as I18nKey};
use eframe::egui::{self, Color32, Key, Pos2, Rect, Vec2};

/// 文本编辑标签页
pub struct TextEditTab {
    path: String,
    /// 编辑缓冲区（测试直接读写）
    pub(crate) content: String,
    error: Option<String>,
    /// 撤销栈（整文件快照）
    undo_stack: Vec<String>,
    /// 重做栈（整文件快照）
    redo_stack: Vec<String>,
    /// 查找词
    search: String,
    /// 替换词
    replace: String,
    /// 查找/替换栏是否展开
    show_search: bool,
    /// 可见空白（空格→·、制表符→→）
    show_ws: bool,
    /// 语法高亮预览模式（只读渲染高亮；关闭时直接编辑）
    show_syntax: bool,
    /// 文件编码（保存回写用）
    encoding: crate::encoding::EncodingKind,
    /// 原文件是否带 BOM
    had_bom: bool,
    /// 滚动偏移（预览模式用）
    scroll: Vec2,
    /// 保存请求（Ctrl+S 或按钮）
    save_req: bool,
}

impl TextEditTab {
    pub fn new(path: &str) -> Self {
        let mut t = TextEditTab {
            path: String::new(),
            content: String::new(),
            error: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            search: String::new(),
            replace: String::new(),
            show_search: false,
            show_ws: false,
            show_syntax: false,
            encoding: crate::encoding::EncodingKind::Utf8,
            had_bom: false,
            scroll: Vec2::ZERO,
            save_req: false,
        };
        t.open(path);
        t
    }

    pub fn title(&self) -> String {
        if self.path.is_empty() {
            t(I18nKey::TextEditTitle).to_string()
        } else {
            let name = std::path::Path::new(&self.path)
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| self.path.clone());
            format!("✏️ {}", name)
        }
    }

    /// P34：打开文件（空会话填充 / 拖拽）
    pub fn open(&mut self, path: &str) {
        match crate::encoding::read_text(path) {
            Ok(tf) => {
                if tf.is_binary {
                    self.error = Some(format!("{}: 二进制文件，请用 hex 对比查看", path));
                    return;
                }
                self.path = path.to_string();
                self.content = tf.text.clone();
                self.encoding = tf.encoding;
                self.had_bom = tf.had_bom;
                self.error = None;
                self.undo_stack.clear();
                self.redo_stack.clear();
            }
            Err(e) => {
                self.error = Some(format!("{}: {}", path, e));
            }
        }
    }

    /// 打开文件对话框（空会话）
    pub fn open_dialog(&mut self) {
        if let Some(p) = super::pick_file() {
            self.open(&p);
        }
    }

    /// 是否为空会话
    pub fn is_empty(&self) -> bool {
        self.path.is_empty()
    }

    /// 保存：编码回写 + A2 模式 .bak 备份。返回是否成功。
    pub fn save(&mut self) -> bool {
        if self.path.is_empty() {
            return false;
        }
        let _ = std::fs::copy(&self.path, format!("{}.bak", self.path));
        let bytes = crate::encoding::encode_back(
            &crate::encoding::TextFile {
                text: String::new(),
                encoding: self.encoding,
                had_bom: self.had_bom,
                is_binary: false,
            },
            &self.content,
        );
        match std::fs::write(&self.path, bytes) {
            Ok(()) => {
                self.error = None;
                true
            }
            Err(e) => {
                self.error = Some(format!("保存失败: {}", e));
                false
            }
        }
    }

    /// 压入撤销快照（修改内容前调用）
    fn push_snapshot(&mut self) {
        self.undo_stack.push(self.content.clone());
        if self.undo_stack.len() > 100 {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }

    pub fn undo(&mut self) {
        if let Some(prev) = self.undo_stack.pop() {
            self.redo_stack.push(self.content.clone());
            self.content = prev;
        }
    }

    pub fn redo(&mut self) {
        if let Some(next) = self.redo_stack.pop() {
            self.undo_stack.push(self.content.clone());
            self.content = next;
        }
    }

    /// BC Convert File：Trim 行尾空白（逐行保留各自行尾风格 CRLF/LF）
    pub fn convert_trim(&mut self) {
        let mut new = String::new();
        for line in self.content.split_inclusive('\n') {
            let (body, ending) = if let Some(stripped) = line.strip_suffix("\r\n") {
                (stripped, "\r\n")
            } else if let Some(stripped) = line.strip_suffix('\n') {
                (stripped, "\n")
            } else {
                (line, "")
            };
            new.push_str(body.trim_end());
            new.push_str(ending);
        }
        if new != self.content {
            self.push_snapshot();
            self.content = new;
        }
    }

    /// BC Convert File：Tabs to Spaces（4 空格）
    pub fn convert_tabs(&mut self) {
        let new = self.content.replace('\t', "    ");
        if new != self.content {
            self.push_snapshot();
            self.content = new;
        }
    }

    /// BC Convert File：行尾风格（to_crlf = true → CRLF；false → LF）
    pub fn convert_line_ending(&mut self, to_crlf: bool) {
        let new = if to_crlf {
            self.content.replace("\r\n", "\n").replace('\n', "\r\n")
        } else {
            self.content.replace("\r\n", "\n")
        };
        if new != self.content {
            self.push_snapshot();
            self.content = new;
        }
    }

    /// 查找下一个匹配：返回匹配行索引（0-based）；未找到 None
    pub fn find_next(&self) -> Option<usize> {
        if self.search.is_empty() {
            return None;
        }
        self.content.lines().position(|l| l.contains(&self.search))
    }

    /// 全部替换：返回替换次数
    pub fn replace_all(&mut self) -> usize {
        if self.search.is_empty() {
            return 0;
        }
        let n = self.content.matches(&self.search).count();
        if n > 0 {
            self.push_snapshot();
            self.content = self.content.replace(&self.search, &self.replace);
        }
        n
    }

    /// 行数（状态栏）
    pub fn line_count(&self) -> usize {
        if self.content.is_empty() {
            0
        } else {
            self.content.lines().count()
        }
    }

    /// 字符数（状态栏）
    pub fn char_count(&self) -> usize {
        self.content.chars().count()
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("textedit_tools").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui.button(t(I18nKey::OpenFile)).clicked() {
                    self.open_dialog();
                }
                if ui
                    .button(t(I18nKey::SaveFile))
                    .on_hover_text("Ctrl+S")
                    .clicked()
                {
                    self.save_req = true;
                }
                ui.separator();
                if ui
                    .add_enabled(!self.undo_stack.is_empty(), egui::Button::new("↩ 撤销"))
                    .on_hover_text("Ctrl+Z")
                    .clicked()
                {
                    self.undo();
                }
                if ui
                    .add_enabled(!self.redo_stack.is_empty(), egui::Button::new("↪ 重做"))
                    .on_hover_text("Ctrl+Y")
                    .clicked()
                {
                    self.redo();
                }
                ui.separator();
                if ui.button("🔍 查找/替换").clicked() {
                    self.show_search = !self.show_search;
                }
                ui.separator();
                // BC Convert File
                if ui
                    .button(t(I18nKey::ConvertTrim))
                    .on_hover_text("移除每行行尾空白")
                    .clicked()
                {
                    self.convert_trim();
                }
                if ui
                    .button(t(I18nKey::ConvertTabs))
                    .on_hover_text("制表符 → 4 空格")
                    .clicked()
                {
                    self.convert_tabs();
                }
                if ui
                    .button(t(I18nKey::ConvertCrlf))
                    .on_hover_text("行尾 → CRLF")
                    .clicked()
                {
                    self.convert_line_ending(true);
                }
                if ui
                    .button(t(I18nKey::ConvertLf))
                    .on_hover_text("行尾 → LF")
                    .clicked()
                {
                    self.convert_line_ending(false);
                }
                ui.separator();
                ui.checkbox(&mut self.show_ws, t(I18nKey::VisibleWs));
                ui.checkbox(&mut self.show_syntax, t(I18nKey::TextEditSyntax));
                ui.separator();
                if !self.path.is_empty() {
                    ui.label(fmt(
                        I18nKey::TextEditStatus,
                        &[
                            self.encoding.name(),
                            &self.line_count().to_string(),
                            &self.char_count().to_string(),
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

        // 查找/替换栏
        if self.show_search {
            egui::Panel::top("textedit_search").show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("查找");
                    ui.add(egui::TextEdit::singleline(&mut self.search).desired_width(140.0));
                    if ui.button("下一处").clicked() {
                        if let Some(line) = self.find_next() {
                            self.scroll.y = line as f32 * ROW_H;
                        }
                    }
                    ui.separator();
                    ui.label("替换");
                    ui.add(egui::TextEdit::singleline(&mut self.replace).desired_width(140.0));
                    if ui.button("全部替换").clicked() {
                        let n = self.replace_all();
                        self.error = Some(format!("已替换 {} 处", n));
                    }
                });
            });
        }

        // 快捷键：Ctrl+S 保存 / Ctrl+Z 撤销 / Ctrl+Y 重做
        if ui.input(|i| i.modifiers.command && i.key_pressed(Key::S)) {
            self.save_req = true;
        }
        if ui.input(|i| i.modifiers.command && i.key_pressed(Key::Z)) {
            self.undo();
        }
        if ui.input(|i| i.modifiers.command && i.key_pressed(Key::Y)) {
            self.redo();
        }

        egui::CentralPanel::default().show(ui, |ui| {
            if self.path.is_empty() {
                ui.centered_and_justified(|ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new(t(I18nKey::TextEditTitle))
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

            // 语法高亮预览模式：只读虚拟化行渲染（行号 + 高亮 + 可见空白）
            if self.show_syntax {
                let lines: Vec<&str> = self.content.lines().collect();
                let max_no = lines.len();
                let gutter = gutter_width(max_no);
                let fg = text_color(ui);
                let syn = crate::highlight::syntax_for(&self.path);
                let out =
                    super::show_rows_offset(ui, lines.len(), ROW_H, self.scroll, |ui, range| {
                        for i in range {
                            let (rect, _) = ui.allocate_exact_size(
                                Vec2::new(ui.available_width().max(200.0), ROW_H),
                                egui::Sense::hover(),
                            );
                            paint_line_no(
                                ui,
                                Rect::from_min_size(rect.min, vec2(gutter, ROW_H)),
                                Some(i + 1),
                            );
                            let cell = crate::sideview::Cell {
                                text: lines[i].to_string(),
                                segments: vec![(lines[i].to_string(), false)],
                            };
                            let crect = Rect::from_min_size(
                                Pos2::new(rect.left() + gutter, rect.top()),
                                vec2((rect.width() - gutter).max(50.0), ROW_H),
                            );
                            paint_cell(ui, crect, Some(&cell), fg, None, syn, 0.0, self.show_ws);
                        }
                    });
                self.scroll = out.state.offset;
                if self.save_req {
                    self.save();
                    self.save_req = false;
                }
                return;
            }

            // 编辑模式：multiline TextEdit
            let prev = ui.spacing().item_spacing.y;
            ui.spacing_mut().item_spacing.y = 0.0;
            let out = egui::ScrollArea::both()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let edit = egui::TextEdit::multiline(&mut self.content)
                        .font(egui::TextStyle::Monospace)
                        .desired_width(f32::INFINITY)
                        .desired_rows(30)
                        .lock_focus(true)
                        .code_editor();
                    ui.add(edit);
                });
            ui.spacing_mut().item_spacing.y = prev;
            let _ = out;
            if self.save_req {
                self.save();
                self.save_req = false;
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn write(dir: &std::path::Path, name: &str, content: &str) -> String {
        let p = dir.join(name);
        fs::write(&p, content).unwrap();
        p.to_str().unwrap().to_string()
    }

    #[test]
    fn open_loads_and_detects_encoding() {
        let d = tempdir().unwrap();
        let p = write(d.path(), "a.txt", "hello\nworld\n");
        let t = TextEditTab::new(&p);
        assert!(t.error.is_none());
        assert_eq!(t.content, "hello\nworld\n");
        assert_eq!(t.encoding, crate::encoding::EncodingKind::Utf8);
        assert_eq!(t.line_count(), 2);
    }

    #[test]
    fn save_writes_back_and_backs_up() {
        let d = tempdir().unwrap();
        let p = write(d.path(), "a.txt", "old\n");
        let mut t = TextEditTab::new(&p);
        t.content = "new\ncontent\n".to_string();
        assert!(t.save());
        assert_eq!(fs::read_to_string(&p).unwrap(), "new\ncontent\n");
        assert!(fs::metadata(format!("{p}.bak")).is_ok(), "应有 .bak 备份");
    }

    #[test]
    fn undo_redo_roundtrip() {
        let d = tempdir().unwrap();
        let p = write(d.path(), "a.txt", "base\n");
        let mut t = TextEditTab::new(&p);
        t.push_snapshot();
        t.content = "edited\n".to_string();
        t.undo();
        assert_eq!(t.content, "base\n");
        t.redo();
        assert_eq!(t.content, "edited\n");
    }

    #[test]
    fn convert_trim_tabs_line_endings() {
        let d = tempdir().unwrap();
        let p = write(d.path(), "a.txt", "a  \nb\tc\r\n");
        let mut t = TextEditTab::new(&p);
        t.convert_trim();
        assert_eq!(t.content, "a\nb\tc\r\n");
        t.convert_tabs();
        assert_eq!(t.content, "a\nb    c\r\n");
        t.convert_line_ending(false); // → LF
        assert_eq!(t.content, "a\nb    c\n");
        t.convert_line_ending(true); // → CRLF
        assert_eq!(t.content, "a\r\nb    c\r\n");
    }

    #[test]
    fn replace_all_counts_and_replaces() {
        let d = tempdir().unwrap();
        let p = write(d.path(), "a.txt", "x y x y\n");
        let mut t = TextEditTab::new(&p);
        t.search = "x".to_string();
        t.replace = "z".to_string();
        assert_eq!(t.replace_all(), 2);
        assert_eq!(t.content, "z y z y\n");
    }

    #[test]
    fn find_next_locates_line() {
        let d = tempdir().unwrap();
        let p = write(d.path(), "a.txt", "l1\nneedle here\nl3\n");
        let mut t = TextEditTab::new(&p);
        t.search = "needle".to_string();
        assert_eq!(t.find_next(), Some(1));
        let mut t2 = TextEditTab::new(&p);
        t2.search = "missing".to_string();
        assert_eq!(t2.find_next(), None);
    }
}
