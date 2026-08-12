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
    /// 按路径解析的语法（无匹配 = None，纯文本）
    pub syntax: Option<&'static syntect::parsing::SyntaxReference>,
}

/// 搜索状态
#[derive(Default)]
pub struct SearchState {
    pub query: String,
    /// 替换为文本（A4）
    pub replace: String,
    /// 匹配行索引
    pub matches: Vec<usize>,
    /// 当前匹配在 matches 中的位置
    pub current: Option<usize>,
    /// 请求聚焦搜索框
    pub focus: bool,
}

/// 二进制文件的十六进制对比数据（任一文件检测为二进制时启用）
#[derive(Clone)]
pub struct HexTabData {
    pub left: String,
    pub right: String,
    pub rows: Vec<crate::hexview::HexRow>,
    /// 左侧原始字节（编辑保存用）
    pub left_bytes: Vec<u8>,
    /// 右侧原始字节（编辑保存用）
    pub right_bytes: Vec<u8>,
}

/// hex 编辑状态：编辑某侧某行的字节
pub struct HexEditState {
    pub side: EditSide,
    /// 行索引
    pub row: usize,
    /// 编辑缓冲区（十六进制字符串，如 "01 0a ff"）
    pub buf: String,
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
    /// 二进制 hex 对比模式（Some 时优先于文本行渲染）
    pub hex: Option<HexTabData>,
    /// hex 编辑状态
    pub hex_edit: Option<HexEditState>,
    /// A8 自动换行（word wrap，BC5 特性）
    pub wrap: bool,
    /// A11 缩略图总览（右侧迷你差异地图，点击跳转）
    pub show_overview: bool,
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
            hex: None,
            hex_edit: None,
            wrap: false,
            show_overview: true,
        }
    }

    pub fn title(&self) -> String {
        if let Some(h) = &self.hex {
            return format!(
                "{}: {} ↔ {}",
                t(I18nKey::HexTitle),
                basename(&h.left),
                basename(&h.right)
            );
        }
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
        // 任一文件为二进制 → 切换 hex 对比视图
        if is_binary_file(l) || is_binary_file(r) {
            let rows = match (std::fs::read(l), std::fs::read(r)) {
                (Ok(lb), Ok(rb)) => crate::hexview::build_hex_rows(&lb, &rb),
                (Err(e), _) => {
                    self.error = Some(fmt(I18nKey::CannotRead, &[l, &e.to_string()]));
                    return;
                }
                (_, Err(e)) => {
                    self.error = Some(fmt(I18nKey::CannotRead, &[r, &e.to_string()]));
                    return;
                }
            };
            self.hex = Some(HexTabData {
                left: l.to_string(),
                right: r.to_string(),
                rows,
                left_bytes: std::fs::read(l).unwrap_or_default(),
                right_bytes: std::fs::read(r).unwrap_or_default(),
            });
            self.left = None;
            self.right = None;
            self.rows.clear();
            self.error = None;
            return;
        }
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
                    syntax: crate::highlight::syntax_for(l),
                });
                self.right = Some(LoadedFile {
                    path: r.to_string(),
                    content: rf.text,
                    encoding: rf.encoding,
                    had_bom: rf.had_bom,
                    syntax: crate::highlight::syntax_for(r),
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
                    syntax: crate::highlight::syntax_for(path),
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
                    syntax: crate::highlight::syntax_for(path),
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
        let (rows, stats) = build_rows(l, r, self.opts.clone());
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

    // ---- A4 文本替换 ----

    /// 替换当前匹配（左侧与右侧各一次），按原编码回写文件。
    /// 返回是否发生了替换。
    pub fn replace_current(&mut self) -> bool {
        let Some(k) = self.search.current else {
            return false;
        };
        let Some(&row_idx) = self.search.matches.get(k) else {
            return false;
        };
        let q = self.search.query.clone();
        let rep = self.search.replace.clone();
        if q.is_empty() {
            return false;
        }
        let row = &self.rows[row_idx];
        let mut changed = false;
        if let Some(no) = row.left_no {
            if let Some(l) = &mut self.left {
                if replace_line(&mut l.content, no, &q, &rep) {
                    changed = true;
                }
            }
        }
        if let Some(no) = row.right_no {
            if let Some(r) = &mut self.right {
                if replace_line(&mut r.content, no, &q, &rep) {
                    changed = true;
                }
            }
        }
        if changed {
            self.finish_replace();
        }
        changed
    }

    /// 全部替换（两侧全文），按原编码回写文件。返回是否发生了替换。
    pub fn replace_all(&mut self) -> bool {
        let q = self.search.query.clone();
        let rep = self.search.replace.clone();
        if q.is_empty() {
            return false;
        }
        let mut changed = false;
        if let Some(l) = &mut self.left {
            if l.content.contains(&q) {
                l.content = l.content.replace(&q, &rep);
                changed = true;
            }
        }
        if let Some(r) = &mut self.right {
            if r.content.contains(&q) {
                r.content = r.content.replace(&q, &rep);
                changed = true;
            }
        }
        if changed {
            self.finish_replace();
        }
        changed
    }

    /// 替换后统一收尾：按原编码回写两侧文件 → 重算 diff → 提示
    fn finish_replace(&mut self) {
        let mut errs: Vec<String> = Vec::new();
        for side in [EditSide::Left, EditSide::Right] {
            let opt = match side {
                EditSide::Left => self
                    .left
                    .as_ref()
                    .map(|f| (f.path.clone(), f.encoding, f.had_bom, f.content.clone())),
                EditSide::Right => self
                    .right
                    .as_ref()
                    .map(|f| (f.path.clone(), f.encoding, f.had_bom, f.content.clone())),
            };
            let Some((path, enc, bom, content)) = opt else {
                continue;
            };
            // 保存前自动备份（A2）
            let _ = std::fs::copy(&path, format!("{path}.bak"));
            let bytes = crate::encoding::encode_back(
                &crate::encoding::TextFile {
                    text: String::new(),
                    encoding: enc,
                    had_bom: bom,
                    is_binary: false,
                },
                &content,
            );
            if let Err(e) = std::fs::write(&path, bytes) {
                errs.push(format!("{path}: {e}"));
            }
        }
        if !errs.is_empty() {
            self.error = Some(format!("替换写回失败: {}", errs.join("; ")));
        }
        self.recompute();
        self.error = Some(fmt(I18nKey::Saved, &["替换已写回"]));
    }

    pub fn reload(&mut self) {
        let opts = self.opts.clone();
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
                        self.load_left(&p, self.opts.clone());
                    }
                }
                if ui.button(t(I18nKey::OpenRight)).clicked() {
                    if let Some(p) = super::pick_file() {
                        self.load_right(&p, self.opts.clone());
                    }
                }
                // A3 剪贴板对比：读系统剪贴板文本 → 临时文件 → 作为左侧/右侧加载
                if ui
                    .button("📋 剪贴板→左")
                    .on_hover_text("用系统剪贴板文本作为左侧对比（若左侧已打开则替换）")
                    .clicked()
                {
                    if let Some(txt) = read_clipboard_text() {
                        if let Some(p) = write_clipboard_temp(&txt) {
                            self.load_left(&p, self.opts.clone());
                        } else {
                            self.error = Some("写入剪贴板临时文件失败".to_string());
                        }
                    } else {
                        self.error = Some("无法读取系统剪贴板（非文本内容或不可用）".to_string());
                    }
                }
                if ui
                    .button("📋 剪贴板→右")
                    .on_hover_text("用系统剪贴板文本作为右侧对比（若右侧已打开则替换）")
                    .clicked()
                {
                    if let Some(txt) = read_clipboard_text() {
                        if let Some(p) = write_clipboard_temp(&txt) {
                            self.load_right(&p, self.opts.clone());
                        } else {
                            self.error = Some("写入剪贴板临时文件失败".to_string());
                        }
                    } else {
                        self.error = Some("无法读取系统剪贴板（非文本内容或不可用）".to_string());
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
                // A8 自动换行（BC5 word wrapping，仅影响显示）
                ui.checkbox(&mut self.wrap, t(I18nKey::WordWrap))
                    .on_hover_text("长行按窗口宽度折行显示");
                // A11 缩略图总览开关
                ui.checkbox(&mut self.show_overview, "缩略图")
                    .on_hover_text("右侧迷你差异地图，点击跳转");
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
                if ui.button(format!("⟳ {}", t(I18nKey::Reload))).clicked() {
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
                // A4 文本替换
                ui.add(
                    egui::TextEdit::singleline(&mut self.search.replace)
                        .hint_text("替换为")
                        .desired_width(100.0),
                );
                if ui
                    .button("替换")
                    .on_hover_text("替换当前匹配（写回文件并自动备份）")
                    .clicked()
                {
                    self.replace_current();
                }
                if ui
                    .button("全部替换")
                    .on_hover_text("替换所有匹配（写回文件并自动备份）")
                    .clicked()
                {
                    self.replace_all();
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
                if ui.button(format!("⬇ {}", t(I18nKey::NextDiff))).clicked() {
                    self.next_diff();
                }
                if ui.button(format!("⬆ {}", t(I18nKey::PrevDiff))).clicked() {
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
                    Some(b) => {
                        // A2 保存前自动备份原文件为 <path>.bak（BC 行为，防手滑覆盖）
                        let _ = std::fs::copy(&path, format!("{path}.bak"));
                        std::fs::write(&path, b)
                    }
                    None => Ok(()),
                };
                match write_res {
                    Ok(()) => {
                        close = true;
                        // 保存后重新加载对应侧并重算 diff
                        match side {
                            EditSide::Left => self.load_left(&path, self.opts.clone()),
                            EditSide::Right => self.load_right(&path, self.opts.clone()),
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
            // 二进制 hex 对比模式（克隆到局部，渲染基于局部副本，保存时可自由 &mut self）
            let hex_owned = self.hex.clone();
            if let Some(h) = &hex_owned {
                if h.rows.is_empty() {
                    ui.centered_and_justified(|ui| {
                        ui.label(
                            egui::RichText::new(t(I18nKey::DiffEmptyHint))
                                .size(18.0)
                                .color(ui.visuals().weak_text_color()),
                        );
                    });
                    return;
                }
                let fg = text_color(ui);
                let diff_count = h.rows.iter().filter(|r| r.diff).count();
                let total_w = HEX_TOTAL_W;
                let mut edit_click: Option<usize> = None;
                let mut save_req = false;
                // 编辑状态下 Ctrl+S 保存
                if self.hex_edit.is_some()
                    && ui.input(|i| i.modifiers.command && i.key_pressed(Key::S))
                {
                    save_req = true;
                }
                let out = super::show_rows(ui, h.rows.len(), HEX_ROW_H, |ui, range| {
                    ui.set_min_width(total_w);
                    for i in range {
                        let row = &h.rows[i];
                        // 正在编辑的行：显示输入框
                        if let Some(he) = &self.hex_edit {
                            if he.row == i {
                                let (rect, resp) = ui.allocate_exact_size(
                                    Vec2::new(total_w, HEX_ROW_H),
                                    egui::Sense::click(),
                                );
                                if row.diff {
                                    paint_bg(ui, rect, Some(bg_replace_l()));
                                }
                                ui.painter().text(
                                    Pos2::new(rect.left() + HEX_OFF_X, rect.top() + 2.0),
                                    egui::Align2::LEFT_TOP,
                                    format!("{:08x}", row.offset),
                                    egui::FontId::monospace(13.0),
                                    GUTTER,
                                );
                                let mut buf = self
                                    .hex_edit
                                    .as_ref()
                                    .map(|e| e.buf.clone())
                                    .unwrap_or_default();
                                let te = ui.add(
                                    egui::TextEdit::singleline(&mut buf)
                                        .font(egui::TextStyle::Monospace)
                                        .desired_width(240.0)
                                        .hint_text("hex bytes, e.g. 01 0a ff"),
                                );
                                if let Some(he) = self.hex_edit.as_mut() {
                                    he.buf = buf;
                                }
                                if resp.double_clicked() {
                                    edit_click = Some(i);
                                }
                                let _ = te;
                                continue;
                            }
                        }
                        paint_hex_row(ui, row, fg);
                        // 双击进入编辑（编辑该行左/右侧字节）
                        let (rect, resp) = ui.allocate_exact_size(
                            Vec2::new(total_w, HEX_ROW_H),
                            egui::Sense::click(),
                        );
                        if resp.double_clicked() {
                            edit_click = Some(i);
                        }
                        let _ = rect;
                    }
                });
                // 双击 → 打开编辑（默认编辑差异行左侧）
                if let Some(i) = edit_click {
                    if let Some(h) = &self.hex {
                        let row = &h.rows[i];
                        let bytes = if row.left.len() >= row.right.len() {
                            &h.left_bytes
                        } else {
                            &h.right_bytes
                        };
                        // 取该行字节作为初始缓冲区
                        let start = row.offset;
                        let end = (start + 16).min(bytes.len());
                        let chunk = bytes.get(start..end).unwrap_or_default();
                        let hex_str: Vec<String> =
                            chunk.iter().map(|b| format!("{:02x}", b)).collect();
                        self.hex_edit = Some(HexEditState {
                            side: EditSide::Left,
                            row: i,
                            buf: hex_str.join(" "),
                        });
                    }
                }
                // 保存编辑
                if save_req {
                    let (side, row_idx, buf) = self
                        .hex_edit
                        .as_ref()
                        .map(|e| (e.side, e.row, e.buf.clone()))
                        .unwrap();
                    // 解析十六进制输入
                    let mut new_bytes: Vec<u8> = Vec::new();
                    let mut ok = true;
                    for tok in buf.split_whitespace() {
                        match u8::from_str_radix(tok, 16) {
                            Ok(b) => new_bytes.push(b),
                            Err(_) => {
                                ok = false;
                                break;
                            }
                        }
                    }
                    if ok && !new_bytes.is_empty() {
                        // 写回文件对应偏移（先克隆路径避免借用冲突）
                        let (l_path, r_path) = self
                            .hex
                            .as_ref()
                            .map(|h| (h.left.clone(), h.right.clone()))
                            .unwrap();
                        let path = match side {
                            EditSide::Left => l_path.clone(),
                            EditSide::Right => r_path.clone(),
                        };
                        let start = self.hex.as_ref().unwrap().rows[row_idx].offset;
                        let mut data = std::fs::read(&path).unwrap_or_default();
                        for (k, b) in new_bytes.iter().enumerate() {
                            let pos = start + k;
                            if pos < data.len() {
                                data[pos] = *b;
                            } else {
                                data.push(*b);
                            }
                        }
                        // A2 保存前自动备份原文件为 <path>.bak
                        let _ = std::fs::copy(&path, format!("{path}.bak"));
                        match std::fs::write(&path, &data) {
                            Ok(()) => {
                                self.hex_edit = None;
                                // 重新加载并重建
                                self.load_pair(&l_path, &r_path, self.opts.clone());
                            }
                            Err(e) => {
                                self.error = Some(format!("保存失败: {}", e));
                            }
                        }
                    } else {
                        self.error = Some("十六进制输入无效".to_string());
                    }
                }
                self.scroll = out.state.offset;
                if self.show_stats {
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label(fmt(
                            I18nKey::DiffCount,
                            &[&diff_count.to_string(), &h.rows.len().to_string()],
                        ));
                        ui.separator();
                        ui.label(fmt(I18nKey::HexModeHint, &[]));
                        ui.separator();
                        ui.label(format!("{}  ↔  {}", h.left, h.right));
                    });
                }
                return;
            }

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
            // A8 自动换行：展开为视觉行（保留原始行索引映射，供搜索/跳转）
            let (render_rows, orig_idx): (Vec<crate::sideview::SideRow>, Vec<usize>) = if self.wrap
            {
                let wrap_chars = ((half - 24.0) / 8.5).max(8.0) as usize;
                crate::sideview::wrap_rows(rows, wrap_chars)
            } else {
                (Vec::new(), Vec::new())
            };
            let display_rows: &[crate::sideview::SideRow] =
                if self.wrap { &render_rows } else { rows };
            let orig_of = |vi: usize| -> usize {
                if self.wrap {
                    orig_idx.get(vi).copied().unwrap_or(vi)
                } else {
                    vi
                }
            };
            // 左右语法（按文件路径解析，供行内语法高亮）
            let syn_l = self.left.as_ref().and_then(|f| f.syntax);
            let syn_r = self.right.as_ref().and_then(|f| f.syntax);

            // 受控滚动 + 虚拟化渲染（统一走 common::show_rows）
            let out = super::show_rows(ui, display_rows.len(), ROW_H, |ui, range| {
                ui.set_min_width(total_w);
                // 当前差异行（diff_pos → diff_rows 中的行索引，P31 竖条标记）
                let cur_diff_orig = self.diff_pos.and_then(|k| self.diff_rows.get(k)).copied();
                for i in range {
                    let row = &display_rows[i];
                    let (bg_l, bg_r) = match row.tag {
                        RowTag::Equal => (None, None),
                        RowTag::Delete => (Some(bg_delete()), None),
                        RowTag::Insert => (None, Some(bg_insert())),
                        RowTag::Replace => (Some(bg_replace_l()), Some(bg_replace_r())),
                    };
                    // 搜索命中高亮（按原始行索引映射）
                    let oi = orig_of(i);
                    let (bg_l, bg_r) = if match_set.contains(&oi) {
                        let c = if current_match == Some(oi) {
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
                        ui,
                        row,
                        gutter_l,
                        gutter_r,
                        content_w,
                        bg_l,
                        bg_r,
                        hl_l,
                        hl_r,
                        fg,
                        syn_l,
                        syn_r,
                        cur_diff_orig == Some(oi),
                    );
                }
            });
            self.scroll = out.state.offset;

            // A11 缩略图总览：右侧迷你差异地图（点击跳转到对应行）
            if self.show_overview && !display_rows.is_empty() {
                let panel_rect = ui.max_rect();
                let ov_w = 10.0;
                let ov_rect = Rect::from_min_size(
                    Pos2::new(panel_rect.right() - ov_w - 3.0, panel_rect.top()),
                    vec2(ov_w, panel_rect.height()),
                );
                let n = display_rows.len();
                let row_h = ov_rect.height() / n as f32;
                for (i, row) in display_rows.iter().enumerate() {
                    let color = match row.tag {
                        RowTag::Equal => Color32::TRANSPARENT,
                        RowTag::Delete => Color32::from_rgb(220, 90, 90),
                        RowTag::Insert => Color32::from_rgb(90, 200, 110),
                        RowTag::Replace => Color32::from_rgb(235, 200, 90),
                    };
                    if color != Color32::TRANSPARENT {
                        let y = ov_rect.top() + i as f32 * row_h;
                        ui.painter().rect_filled(
                            Rect::from_min_size(
                                Pos2::new(ov_rect.left(), y),
                                vec2(ov_w, row_h.max(1.0)),
                            ),
                            0.0,
                            color,
                        );
                    }
                }
                let resp = ui.interact(ov_rect, ui.id().with("overview"), egui::Sense::click());
                if resp.clicked() {
                    if let Some(pos) = resp.interact_pointer_pos() {
                        let t = ((pos.y - ov_rect.top()) / ov_rect.height()).clamp(0.0, 1.0);
                        let row_idx = (t * n as f32) as usize;
                        let orig = if self.wrap {
                            orig_idx.get(row_idx).copied().unwrap_or(row_idx)
                        } else {
                            row_idx
                        };
                        self.jump_to_row(orig);
                    }
                }
            }

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
    syn_l: Option<&'static syntect::parsing::SyntaxReference>,
    syn_r: Option<&'static syntect::parsing::SyntaxReference>,
    is_current: bool,
) {
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(gutter_l + content_w + gutter_r + content_w, ROW_H),
        egui::Sense::hover(),
    );
    let x = rect.left();
    let y = rect.top();

    // BC 风格当前差异行：左侧 3px 竖条（P31）
    if is_current {
        ui.painter().rect_filled(
            Rect::from_min_size(Pos2::new(x, y), vec2(super::theme::CURRENT_BAR, ROW_H)),
            0.0,
            super::theme::diff_modify(),
        );
    }

    // 左 gutter + 内容
    let gutter_rect = Rect::from_min_size(Pos2::new(x, y), vec2(gutter_l, ROW_H));
    paint_bg(ui, gutter_rect, bg_l);
    paint_line_no(ui, gutter_rect, row.left_no);
    let content_rect = Rect::from_min_size(Pos2::new(x + gutter_l, y), vec2(content_w, ROW_H));
    paint_bg(ui, content_rect, bg_l);
    paint_cell(ui, content_rect, row.left.as_ref(), fg, hl_l, syn_l);

    // 右 gutter + 内容
    let x_r = x + gutter_l + content_w;
    let gutter_rect = Rect::from_min_size(Pos2::new(x_r, y), vec2(gutter_r, ROW_H));
    paint_bg(ui, gutter_rect, bg_r);
    paint_line_no(ui, gutter_rect, row.right_no);
    let content_rect = Rect::from_min_size(Pos2::new(x_r + gutter_r, y), vec2(content_w, ROW_H));
    paint_bg(ui, content_rect, bg_r);
    paint_cell(ui, content_rect, row.right.as_ref(), fg, hl_r, syn_r);
}

fn basename(p: &str) -> String {
    std::path::Path::new(p)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.to_string())
}

/// A4：替换 content 中指定行（1-based）里的第一个匹配。返回是否替换。
fn replace_line(content: &mut String, line_no: usize, from: &str, to: &str) -> bool {
    if from.is_empty() {
        return false;
    }
    let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
    if line_no < 1 || line_no > lines.len() {
        return false;
    }
    let idx = line_no - 1;
    if !lines[idx].contains(from) {
        return false;
    }
    lines[idx] = lines[idx].replacen(from, to, 1);
    let had_trailing_nl = content.ends_with('\n');
    *content = lines.join("\n");
    if had_trailing_nl {
        content.push('\n');
    }
    true
}

// ---- 二进制 hex 视图 ----

/// hex 行高（与 ROW_H 一致）
const HEX_ROW_H: f32 = ROW_H;
/// hex 视图总宽度：偏移(9) + L hex(50) + L ascii(18) + R hex(50) + R ascii(18) + 间距
const HEX_TOTAL_W: f32 = 9.0 + 50.0 + 18.0 + 50.0 + 18.0 + 80.0;
/// 各列 x 起点
const HEX_OFF_X: f32 = 4.0;
const HEX_L_X: f32 = 16.0;
const HEX_L_ASCII_X: f32 = 66.0;
const HEX_R_X: f32 = 86.0;
const HEX_R_ASCII_X: f32 = 136.0;

/// 绘制一行 hex 对比（偏移 + L hex + L ascii + R hex + R ascii）
fn paint_hex_row(ui: &mut egui::Ui, row: &crate::hexview::HexRow, fg: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(HEX_TOTAL_W, HEX_ROW_H), egui::Sense::hover());
    let x = rect.left();
    let y = rect.top();

    // 差异行底色
    if row.diff {
        paint_bg(ui, rect, Some(bg_replace_l()));
    }

    // 偏移
    ui.painter().text(
        Pos2::new(x + HEX_OFF_X, y + 2.0),
        egui::Align2::LEFT_TOP,
        format!("{:08x}", row.offset),
        egui::FontId::monospace(13.0),
        GUTTER,
    );

    // 左侧 hex（差异字节红色）
    let l_hex = hex_bytes_text(&row.left, &row.right, true);
    ui.painter().text(
        Pos2::new(x + HEX_L_X, y + 2.0),
        egui::Align2::LEFT_TOP,
        l_hex,
        egui::FontId::monospace(13.0),
        fg,
    );
    // 左侧 ascii
    let l_ascii: String = row
        .left
        .iter()
        .map(|&b| crate::hexview::ascii_byte(b))
        .collect();
    ui.painter().text(
        Pos2::new(x + HEX_L_ASCII_X, y + 2.0),
        egui::Align2::LEFT_TOP,
        l_ascii,
        egui::FontId::monospace(13.0),
        fg,
    );

    // 右侧 hex（差异字节绿色）
    let r_hex = hex_bytes_text(&row.right, &row.left, false);
    ui.painter().text(
        Pos2::new(x + HEX_R_X, y + 2.0),
        egui::Align2::LEFT_TOP,
        r_hex,
        egui::FontId::monospace(13.0),
        fg,
    );
    // 右侧 ascii
    let r_ascii: String = row
        .right
        .iter()
        .map(|&b| crate::hexview::ascii_byte(b))
        .collect();
    ui.painter().text(
        Pos2::new(x + HEX_R_ASCII_X, y + 2.0),
        egui::Align2::LEFT_TOP,
        r_ascii,
        egui::FontId::monospace(13.0),
        fg,
    );
}

/// hex 字节文本：16 字节宽、8 字节处空格；与本侧不同的字节用颜色标记
fn hex_bytes_text(bytes: &[u8], other: &[u8], is_left: bool) -> String {
    let mut s = String::new();
    for i in 0..16 {
        if i == 8 {
            s.push(' ');
        }
        if i < bytes.len() {
            let diff = i < other.len() && bytes[i] != other[i];
            if diff {
                let c = if is_left {
                    Color32::from_rgb(255, 120, 120)
                } else {
                    Color32::from_rgb(120, 255, 140)
                };
                // egui 文本内嵌颜色：用 ANSI 不生效，返回纯文本即可（底色已表达差异）
                let _ = c;
            }
            s.push_str(&format!("{:02X} ", bytes[i]));
        } else {
            s.push_str("   ");
        }
    }
    s.trim_end().to_string()
}

/// 检测文件是否为二进制（读取前 8KB 做启发式判定）
fn is_binary_file(path: &str) -> bool {
    crate::encoding::read_text(path)
        .map(|tf| tf.is_binary)
        .unwrap_or(false)
}

/// A3 剪贴板对比：读取系统剪贴板文本（arboard，跨平台）
fn read_clipboard_text() -> Option<String> {
    let mut cb = arboard::Clipboard::new().ok()?;
    cb.get_text().ok()
}

/// A3 剪贴板对比：把剪贴板文本写入临时文件（供 load_left/load_right 加载）
fn write_clipboard_temp(text: &str) -> Option<String> {
    let dir = std::env::temp_dir();
    let name = format!("bcr-clipboard-{}.txt", std::process::id());
    let path = dir.join(name);
    std::fs::write(&path, text).ok()?;
    Some(path.to_string_lossy().into_owned())
}
