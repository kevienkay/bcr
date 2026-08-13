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
    /// P33：文件大小（详情行显示）
    pub size: u64,
    /// P33：修改时间可读串（详情行显示）
    pub mtime: String,
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
#[derive(Clone, Copy, PartialEq, Eq)]
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
    /// P32-A5：差异块（连续差异行的起止），供折叠
    pub diff_blocks: Vec<(usize, usize)>,
    /// P32-A5：已折叠的差异块（按块索引）
    pub collapsed_blocks: std::collections::HashSet<usize>,
    /// P32-B5：已忽略的行（原始行索引），导航/统计排除
    pub ignored_rows: std::collections::HashSet<usize>,
    pub search: SearchState,
    /// 待跳转行号（1-based）
    pub goto_line: Option<usize>,
    pub goto_focus: bool,
    /// 编辑状态（编辑左侧/右侧内容）
    pub editing: Option<EditState>,
    /// P32-A2：行内直接编辑状态（双击行进入）
    pub inline_edit: Option<InlineEditState>,
    /// P32-A6：撤销栈（编辑/替换前的文件内容）
    pub undo_stack: Vec<EditSnapshot>,
    /// P32-A6：重做栈
    pub redo_stack: Vec<EditSnapshot>,
    /// 二进制 hex 对比模式（Some 时优先于文本行渲染）
    pub hex: Option<HexTabData>,
    /// hex 编辑状态
    pub hex_edit: Option<HexEditState>,
    /// B4：hex 差异导航位置（hex 差异行索引）
    pub hex_diff_pos: Option<usize>,
    /// A8 自动换行（word wrap，BC5 特性）
    pub wrap: bool,
    /// A11 缩略图总览（右侧迷你差异地图，点击跳转）
    pub show_overview: bool,
    /// P33：长行横向滚动偏移（两栏固定半屏，超长行栏内左右滑动查看）
    pub h_scroll: f32,
}

/// 编辑窗口状态
pub struct EditState {
    pub side: EditSide,
    pub path: String,
    pub content: String,
}

/// P32-A2：行内直接编辑状态（双击差异行/内容行进入）
#[derive(Clone)]
pub struct InlineEditState {
    pub side: EditSide,
    /// 行索引（对齐后的显示行索引）
    pub row: usize,
    /// 编辑缓冲区
    pub buf: String,
}

/// P32-A6：编辑快照（撤销/重做用）
#[derive(Clone)]
pub struct EditSnapshot {
    pub side: EditSide,
    pub path: String,
    pub before: String,
    pub after: String,
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
            diff_blocks: Vec::new(),
            collapsed_blocks: std::collections::HashSet::new(),
            ignored_rows: std::collections::HashSet::new(),
            search: SearchState::default(),
            goto_line: None,
            goto_focus: false,
            editing: None,
            inline_edit: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            hex: None,
            hex_edit: None,
            hex_diff_pos: None,
            wrap: false,
            show_overview: true,
            h_scroll: 0.0,
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
                    size: file_size(l),
                    mtime: file_mtime_str(l),
                });
                self.right = Some(LoadedFile {
                    path: r.to_string(),
                    content: rf.text,
                    encoding: rf.encoding,
                    had_bom: rf.had_bom,
                    syntax: crate::highlight::syntax_for(r),
                    size: file_size(r),
                    mtime: file_mtime_str(r),
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
                    size: file_size(path),
                    mtime: file_mtime_str(path),
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
                    size: file_size(path),
                    mtime: file_mtime_str(path),
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
        // P32-B5：忽略行从差异行/统计中排除（会话级）
        let mut diff_rows: Vec<usize> = Vec::new();
        let mut ignored_delete = 0usize;
        let mut ignored_insert = 0usize;
        let mut ignored_replace = 0usize;
        for (i, row) in rows.iter().enumerate() {
            if row.tag == RowTag::Equal {
                continue;
            }
            if self.ignored_rows.contains(&i) {
                match row.tag {
                    RowTag::Delete => ignored_delete += 1,
                    RowTag::Insert => ignored_insert += 1,
                    RowTag::Replace => ignored_replace += 1,
                    RowTag::Equal => {}
                }
                continue;
            }
            diff_rows.push(i);
        }
        // P32-A5：差异块 = 连续未忽略差异行分组（起止索引）
        let mut diff_blocks: Vec<(usize, usize)> = Vec::new();
        for &i in &diff_rows {
            match diff_blocks.last_mut() {
                Some((_, end)) if *end + 1 == i => *end = i,
                _ => diff_blocks.push((i, i)),
            }
        }
        // 清理失效的折叠块索引
        self.collapsed_blocks.retain(|&b| b < diff_blocks.len());
        self.diff_rows = diff_rows;
        self.diff_blocks = diff_blocks;
        self.diff_pos = None;
        self.rows = rows;
        // 统计：扣除忽略的差异行
        self.stats = Stats {
            delete: stats.delete - ignored_delete,
            insert: stats.insert - ignored_insert,
            replace: stats.replace - ignored_replace,
            equal: stats.equal,
        };
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

    // ---- P33 菜单栏转发：打开左/右对话框、剪贴板加载、聚焦搜索 ----
    /// 打开左侧文件（文件对话框）
    pub fn open_left_dialog(&mut self) {
        if let Some(p) = super::pick_file() {
            self.load_left(&p, self.opts.clone());
        }
    }

    /// 打开右侧文件（文件对话框）
    pub fn open_right_dialog(&mut self) {
        if let Some(p) = super::pick_file() {
            self.load_right(&p, self.opts.clone());
        }
    }

    /// 剪贴板 → 左侧（读系统剪贴板文本 → 临时文件 → 加载）
    pub fn load_clipboard_left(&mut self) {
        match read_clipboard_text() {
            Some(txt) => {
                if let Some(p) = write_clipboard_temp(&txt) {
                    self.load_left(&p, self.opts.clone());
                } else {
                    self.error = Some("写入剪贴板临时文件失败".to_string());
                }
            }
            None => self.error = Some("无法读取系统剪贴板（非文本内容或不可用）".to_string()),
        }
    }

    /// 剪贴板 → 右侧
    pub fn load_clipboard_right(&mut self) {
        match read_clipboard_text() {
            Some(txt) => {
                if let Some(p) = write_clipboard_temp(&txt) {
                    self.load_right(&p, self.opts.clone());
                } else {
                    self.error = Some("写入剪贴板临时文件失败".to_string());
                }
            }
            None => self.error = Some("无法读取系统剪贴板（非文本内容或不可用）".to_string()),
        }
    }

    /// 聚焦搜索框（菜单 Search>Find）
    pub fn focus_search(&mut self) {
        self.search.focus = true;
    }

    // ---- P32-A2/A6：行内编辑提交 + 撤销/重做 ----

    /// 提交行内编辑：修改对应侧文件对应行 → 按原编码写回 → 重新加载 → 入撤销栈
    pub fn commit_inline_edit(&mut self) {
        let Some(ie) = self.inline_edit.take() else {
            return;
        };
        let Some(row) = self.rows.get(ie.row) else {
            return;
        };
        // 取该侧文件信息与行号
        let (path, line_no, enc, bom) = match ie.side {
            EditSide::Left => match (&self.left, row.left_no) {
                (Some(f), Some(no)) => (f.path.clone(), no, f.encoding, f.had_bom),
                _ => return,
            },
            EditSide::Right => match (&self.right, row.right_no) {
                (Some(f), Some(no)) => (f.path.clone(), no, f.encoding, f.had_bom),
                _ => return,
            },
        };
        // 读取当前文件原文，替换目标行
        let Ok(orig) = std::fs::read_to_string(&path) else {
            return;
        };
        let mut lines: Vec<&str> = orig.split('\n').collect();
        let idx = line_no.saturating_sub(1);
        if idx >= lines.len() {
            return;
        }
        let before_line = lines[idx].to_string();
        let after_line = ie.buf.clone();
        if before_line == after_line {
            return;
        }
        lines[idx] = &after_line;
        let new_content = lines.join("\n");
        // 按原编码写回（保留 BOM，自动备份）
        let _ = std::fs::copy(&path, format!("{path}.bak"));
        let bytes = crate::encoding::encode_back(
            &crate::encoding::TextFile {
                text: String::new(),
                encoding: enc,
                had_bom: bom,
                is_binary: false,
            },
            &new_content,
        );
        if let Err(e) = std::fs::write(&path, bytes) {
            self.error = Some(fmt(I18nKey::SaveFailed, &[&e.to_string()]));
            return;
        }
        // 入撤销栈（重做栈清空）
        self.undo_stack.push(EditSnapshot {
            side: ie.side,
            path: path.clone(),
            before: orig,
            after: new_content,
        });
        self.redo_stack.clear();
        // 重新加载对应侧并重算
        match ie.side {
            EditSide::Left => self.load_left(&path, self.opts.clone()),
            EditSide::Right => self.load_right(&path, self.opts.clone()),
        }
        self.error = Some(fmt(I18nKey::Saved, &[&path]));
    }

    /// 撤销：恢复快照 before 内容到文件并重新加载
    pub fn undo(&mut self) {
        let Some(snap) = self.undo_stack.pop() else {
            return;
        };
        let before = snap.before.clone();
        let path = snap.path.clone();
        let (enc, bom) = match snap.side {
            EditSide::Left => self
                .left
                .as_ref()
                .map(|f| (f.encoding, f.had_bom))
                .unwrap_or((crate::encoding::EncodingKind::Utf8, false)),
            EditSide::Right => self
                .right
                .as_ref()
                .map(|f| (f.encoding, f.had_bom))
                .unwrap_or((crate::encoding::EncodingKind::Utf8, false)),
        };
        let _ = std::fs::copy(&path, format!("{path}.bak"));
        let bytes = crate::encoding::encode_back(
            &crate::encoding::TextFile {
                text: String::new(),
                encoding: enc,
                had_bom: bom,
                is_binary: false,
            },
            &before,
        );
        if let Err(e) = std::fs::write(&path, bytes) {
            self.error = Some(fmt(I18nKey::SaveFailed, &[&e.to_string()]));
            self.undo_stack.push(snap);
            return;
        }
        match snap.side {
            EditSide::Left => self.load_left(&path, self.opts.clone()),
            EditSide::Right => self.load_right(&path, self.opts.clone()),
        }
        self.redo_stack.push(snap);
        self.error = Some(fmt(I18nKey::Saved, &["已撤销"]));
    }

    /// 重做：恢复快照 after 内容到文件并重新加载
    pub fn redo(&mut self) {
        let Some(snap) = self.redo_stack.pop() else {
            return;
        };
        let after = snap.after.clone();
        let path = snap.path.clone();
        let (enc, bom) = match snap.side {
            EditSide::Left => self
                .left
                .as_ref()
                .map(|f| (f.encoding, f.had_bom))
                .unwrap_or((crate::encoding::EncodingKind::Utf8, false)),
            EditSide::Right => self
                .right
                .as_ref()
                .map(|f| (f.encoding, f.had_bom))
                .unwrap_or((crate::encoding::EncodingKind::Utf8, false)),
        };
        let _ = std::fs::copy(&path, format!("{path}.bak"));
        let bytes = crate::encoding::encode_back(
            &crate::encoding::TextFile {
                text: String::new(),
                encoding: enc,
                had_bom: bom,
                is_binary: false,
            },
            &after,
        );
        if let Err(e) = std::fs::write(&path, bytes) {
            self.error = Some(fmt(I18nKey::SaveFailed, &[&e.to_string()]));
            self.redo_stack.push(snap);
            return;
        }
        match snap.side {
            EditSide::Left => self.load_left(&path, self.opts.clone()),
            EditSide::Right => self.load_right(&path, self.opts.clone()),
        }
        self.undo_stack.push(snap);
        self.error = Some(fmt(I18nKey::Saved, &["已重做"]));
    }

    // ---- P35-A1：复制差异块到另一侧（BC Copy to Other Side）----

    /// 把当前差异块的内容复制到目标侧（覆盖目标侧该块），入撤销栈。
    /// `target` 是被覆盖的一侧：Right = 左侧→右侧，Left = 右侧→左侧。
    pub fn copy_block_to(&mut self, target: EditSide) -> bool {
        // 定位当前差异块（diff_pos → diff_rows → 所在块）
        let Some(pos) = self.diff_pos else {
            return false;
        };
        let Some(&cur_row) = self.diff_rows.get(pos) else {
            return false;
        };
        self.copy_block_at(cur_row, target)
    }

    /// 复制指定行所在的差异块到目标侧（`row` 为 self.rows 的行索引）。
    /// `target` 是被覆盖的一侧：Right = 左侧→右侧，Left = 右侧→左侧。
    pub fn copy_block_at(&mut self, row: usize, target: EditSide) -> bool {
        let Some(&(s, e)) = self
            .diff_blocks
            .iter()
            .find(|&&(s, e)| s <= row && row <= e)
        else {
            return false;
        };
        let src_is_left = target == EditSide::Right;
        // 目标侧文件信息（路径 + 编码 + BOM + 原文）
        let (dst_path, dst_enc, dst_bom, dst_orig) = match target {
            EditSide::Right => match &self.right {
                Some(f) => (f.path.clone(), f.encoding, f.had_bom, f.content.clone()),
                None => return false,
            },
            EditSide::Left => match &self.left {
                Some(f) => (f.path.clone(), f.encoding, f.had_bom, f.content.clone()),
                None => return false,
            },
        };
        // 重建目标侧全文：块内取源侧，块外保持目标侧原样
        let mut new_lines: Vec<String> = Vec::new();
        for (i, row) in self.rows.iter().enumerate() {
            let in_block = i >= s && i <= e;
            let (src_cell, dst_cell) = if src_is_left {
                (&row.left, &row.right)
            } else {
                (&row.right, &row.left)
            };
            let keep = if in_block {
                src_cell.as_ref().map(|c| c.text.clone())
            } else {
                dst_cell.as_ref().map(|c| c.text.clone())
            };
            if let Some(text) = keep {
                new_lines.push(text);
            }
        }
        let mut new_content = new_lines.join("\n");
        // 保留原末尾换行特征
        if dst_orig.ends_with('\n') && !new_content.is_empty() {
            new_content.push('\n');
        }
        if dst_orig == new_content {
            return false;
        }
        // 备份 + 按原编码写回
        let _ = std::fs::copy(&dst_path, format!("{dst_path}.bak"));
        let bytes = crate::encoding::encode_back(
            &crate::encoding::TextFile {
                text: String::new(),
                encoding: dst_enc,
                had_bom: dst_bom,
                is_binary: false,
            },
            &new_content,
        );
        if let Err(e) = std::fs::write(&dst_path, bytes) {
            self.error = Some(fmt(I18nKey::SaveFailed, &[&e.to_string()]));
            return false;
        }
        // 入撤销栈（重做栈清空）
        self.undo_stack.push(EditSnapshot {
            side: target,
            path: dst_path.clone(),
            before: dst_orig,
            after: new_content,
        });
        self.redo_stack.clear();
        // 重新加载目标侧并重算
        match target {
            EditSide::Right => self.load_right(&dst_path, self.opts.clone()),
            EditSide::Left => self.load_left(&dst_path, self.opts.clone()),
        }
        self.error = Some(fmt(I18nKey::Saved, &["已复制到另一侧"]));
        true
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
        let n = self.diff_rows.len();
        // diff_pos 是 diff_rows 的索引（与 P31 竖条标记一致），循环前进
        let next = match self.diff_pos {
            Some(p) => (p + 1) % n,
            None => 0,
        };
        self.diff_pos = Some(next);
        self.jump_to_row(self.diff_rows[next]);
    }

    pub fn prev_diff(&mut self) {
        if self.diff_rows.is_empty() {
            return;
        }
        let n = self.diff_rows.len();
        let prev = match self.diff_pos {
            Some(p) => (p + n - 1) % n,
            None => n - 1,
        };
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

    /// B4：hex 模式下一差异（按行循环跳转）
    pub fn hex_next_diff(&mut self) {
        let Some(h) = &self.hex else { return };
        let n = h.rows.len();
        if n == 0 {
            return;
        }
        let start = self.hex_diff_pos.map(|p| p + 1).unwrap_or(0);
        let mut target = None;
        for i in start..n {
            if h.rows[i].diff {
                target = Some(i);
                break;
            }
        }
        if target.is_none() {
            for i in 0..start.min(n) {
                if h.rows[i].diff {
                    target = Some(i);
                    break;
                }
            }
        }
        if let Some(i) = target {
            self.hex_diff_pos = Some(i);
            self.scroll.y = (i as f32 * HEX_ROW_H - 4.0 * HEX_ROW_H).max(0.0);
            self.scroll.x = 0.0;
        }
    }

    /// B4：hex 模式上一差异（按行循环跳转）
    pub fn hex_prev_diff(&mut self) {
        let Some(h) = &self.hex else { return };
        let n = h.rows.len();
        if n == 0 {
            return;
        }
        let start = self.hex_diff_pos.map(|p| p.saturating_sub(1)).unwrap_or(n);
        let mut target = None;
        for i in (0..=start).rev() {
            if i < n && h.rows[i].diff {
                target = Some(i);
                break;
            }
        }
        if target.is_none() {
            for i in (0..n).rev() {
                if h.rows[i].diff {
                    target = Some(i);
                    break;
                }
            }
        }
        if let Some(i) = target {
            self.hex_diff_pos = Some(i);
            self.scroll.y = (i as f32 * HEX_ROW_H - 4.0 * HEX_ROW_H).max(0.0);
            self.scroll.x = 0.0;
        }
    }

    pub fn handle_keys(&mut self, ui: &egui::Ui) {
        let ctrl = ui.input(|i| i.modifiers.command);
        // P32-A2：内联编辑中 Enter 提交 / ESC 取消（优先于搜索 Enter）
        if self.inline_edit.is_some() {
            if ui.input(|i| i.key_pressed(Key::Enter)) {
                self.commit_inline_edit();
            }
            if ui.input(|i| i.key_pressed(Key::Escape)) {
                self.inline_edit = None;
            }
            return;
        }
        // P32-A6：撤销/重做（Ctrl+Z / Ctrl+Y 或 Ctrl+Shift+Z）
        if ui.input(|i| i.key_pressed(Key::Z) && ctrl) {
            if ui.input(|i| i.modifiers.shift) {
                self.redo();
            } else {
                self.undo();
            }
            return;
        }
        if ui.input(|i| i.key_pressed(Key::Y) && ctrl) {
            self.redo();
            return;
        }
        if ui.input(|i| i.key_pressed(Key::F) && ctrl) {
            self.search.focus = true;
            return;
        }
        if ui.input(|i| i.key_pressed(Key::G) && ctrl) {
            self.goto_focus = true;
            return;
        }
        // B1：F6 下一差异 / F7 上一差异（hex 模式下走 hex 差异导航）
        if ui.input(|i| i.key_pressed(Key::F6)) {
            if self.hex.is_some() {
                self.hex_next_diff();
            } else {
                self.next_diff();
            }
        }
        if ui.input(|i| i.key_pressed(Key::F7)) {
            if self.hex.is_some() {
                self.hex_prev_diff();
            } else {
                self.prev_diff();
            }
        }
        // B7：F5 重新加载
        if ui.input(|i| i.key_pressed(Key::F5)) {
            self.reload();
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
                // ---- 打开（BC: Open 按钮组）----
                if ui.button(t(I18nKey::OpenLeft)).clicked() {
                    self.open_left_dialog();
                }
                if ui.button(t(I18nKey::OpenRight)).clicked() {
                    self.open_right_dialog();
                }
                // A3 剪贴板对比（复用 P33 菜单栏转发方法）
                if ui
                    .button("📋 剪贴板→左")
                    .on_hover_text("用系统剪贴板文本作为左侧对比（若左侧已打开则替换）")
                    .clicked()
                {
                    self.load_clipboard_left();
                }
                if ui
                    .button("📋 剪贴板→右")
                    .on_hover_text("用系统剪贴板文本作为右侧对比（若右侧已打开则替换）")
                    .clicked()
                {
                    self.load_clipboard_right();
                }
                ui.separator();
                // ---- 显示选项（BC: 显示过滤/规则组）----
                ui.checkbox(&mut self.show_stats, t(I18nKey::StatsPanel))
                    .changed();
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
                // A8 自动换行（BC5 word wrapping，仅影响显示）
                ui.checkbox(&mut self.wrap, t(I18nKey::WordWrap))
                    .on_hover_text("长行按窗口宽度折行显示");
                // A11 缩略图总览开关
                ui.checkbox(&mut self.show_overview, "缩略图")
                    .on_hover_text("右侧迷你差异地图，点击跳转");
                ui.separator();
                // ---- 编辑（BC: Copy/编辑组）----
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
                // P32-A6：撤销/重做按钮
                let can_undo = !self.undo_stack.is_empty();
                let can_redo = !self.redo_stack.is_empty();
                if ui
                    .add_enabled(can_undo, egui::Button::new("↩ 撤销"))
                    .on_hover_text("Ctrl+Z")
                    .clicked()
                {
                    self.undo();
                }
                if ui
                    .add_enabled(can_redo, egui::Button::new("↪ 重做"))
                    .on_hover_text("Ctrl+Y")
                    .clicked()
                {
                    self.redo();
                }
                ui.separator();
                ui.separator();
                // ---- 操作（BC: Copy/Swap/Reload 组）----
                // P35-A1：复制差异块到另一侧（BC Copy to Other Side）
                let has_diff = self.diff_pos.is_some();
                if ui
                    .add_enabled(
                        has_diff,
                        egui::Button::new(format!("→ {}", t(I18nKey::CopyToRight))),
                    )
                    .on_hover_text("复制当前差异块左侧内容到右侧")
                    .clicked()
                {
                    self.copy_block_to(EditSide::Right);
                }
                if ui
                    .add_enabled(
                        has_diff,
                        egui::Button::new(format!("← {}", t(I18nKey::CopyToLeft))),
                    )
                    .on_hover_text("复制当前差异块右侧内容到左侧")
                    .clicked()
                {
                    self.copy_block_to(EditSide::Left);
                }
                if ui
                    .button(format!("⟳ {}", t(I18nKey::Reload)))
                    .on_hover_text("重新加载 (F5)")
                    .clicked()
                {
                    self.reload();
                }
                ui.separator();
                // ---- 差异导航（BC: Next Section/Prev Section 组）----
                ui.label(fmt(
                    I18nKey::DiffCount,
                    &[
                        &self.diff_rows.len().to_string(),
                        &self.rows.len().to_string(),
                    ],
                ));
                if ui
                    .button(format!("⬇ {}", t(I18nKey::NextDiff)))
                    .on_hover_text("下一差异 (F6)")
                    .clicked()
                {
                    self.next_diff();
                }
                if ui
                    .button(format!("⬆ {}", t(I18nKey::PrevDiff)))
                    .on_hover_text("上一差异 (F7)")
                    .clicked()
                {
                    self.prev_diff();
                }
                // 跳转行
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
                    || ui.button(format!("🎯 {}", t(I18nKey::Goto))).clicked()
                {
                    if let Some(line) = self.goto_line {
                        if line >= 1 {
                            self.jump_to_row(line - 1);
                        }
                    }
                }
                ui.separator();
                // ---- 搜索/替换（BC: 搜索组）----
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut self.search.query)
                        .id(egui::Id::new("diff_search"))
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
                    .button("🔁 替换")
                    .on_hover_text("替换当前匹配（写回文件并自动备份）")
                    .clicked()
                {
                    self.replace_current();
                }
                if ui
                    .button("🔁 全部替换")
                    .on_hover_text("替换所有匹配（写回文件并自动备份）")
                    .clicked()
                {
                    self.replace_all();
                }
                // P33：长行横向滚动条（两栏固定各半屏，超长行栏内左右滑动查看）
                if !self.rows.is_empty() && self.hex.is_none() {
                    let max_chars = self
                        .rows
                        .iter()
                        .flat_map(|r| [r.left.as_ref(), r.right.as_ref()])
                        .flatten()
                        .map(|c| c.text.chars().count())
                        .max()
                        .unwrap_or(0);
                    let max_line_w = max_chars as f32 * 9.0 + 24.0;
                    let avail = ui.available_width();
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
                    let gutter_l = crate::gui::common::gutter_width(max_no_l);
                    let gutter_r = crate::gui::common::gutter_width(max_no_r);
                    let mid_gap = super::theme::MID_GAP;
                    let half = ((avail - gutter_l - gutter_r - mid_gap) / 2.0).max(200.0);
                    let h_max = (max_line_w - half).max(0.0);
                    if h_max > 0.0 {
                        ui.separator();
                        ui.label("↔")
                            .on_hover_text("长行横向滚动：拖动查看超宽内容");
                        ui.add(
                            egui::Slider::new(&mut self.h_scroll, 0.0..=h_max)
                                .show_value(false)
                                .custom_formatter(|v, _| format!("{:.0}px", v)),
                        );
                    } else {
                        self.h_scroll = 0.0;
                    }
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
                let out = super::show_rows_offset(
                    ui,
                    h.rows.len(),
                    HEX_ROW_H,
                    self.scroll,
                    |ui, range| {
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
                    },
                );
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
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new(t(I18nKey::DiffEmptyHint))
                                .size(16.0)
                                .color(ui.visuals().weak_text_color()),
                        );
                        ui.add_space(12.0);
                        // P34：分别打开左右两侧（BC 式：不强求一次选满两个）
                        ui.horizontal(|ui| {
                            if ui.button(t(I18nKey::OpenLeft)).clicked() {
                                self.open_left_dialog();
                            }
                            if ui.button(t(I18nKey::OpenRight)).clicked() {
                                self.open_right_dialog();
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
            // P33：两栏固定各占半屏（BC 式等分），长行栏内横向滚动查看；随窗口缩放自适应
            let avail = ui.available_width();
            let mid_gap = super::theme::MID_GAP;
            let half = ((avail - gutter_l - gutter_r - mid_gap) / 2.0).max(200.0);
            let content_w = half;
            // 最长行所需宽度（供横向滚动条范围计算）
            let max_chars = self
                .rows
                .iter()
                .flat_map(|r| [r.left.as_ref(), r.right.as_ref()])
                .flatten()
                .map(|c| c.text.chars().count())
                .max()
                .unwrap_or(0);
            let max_line_w = max_chars as f32 * 9.0 + 24.0;
            // P32-A1：左右面板之间留空隙画差异连接线（BC 观感）
            let total_w = gutter_l + content_w + mid_gap + gutter_r + content_w;
            let _ = max_line_w; // P33：横向滚动条范围在工具栏计算（栏宽固定半屏）
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
            // P32-A2：取出行内编辑状态，渲染循环内传入；双击请求在下循环结束后处理
            let mut inline = self.inline_edit.take();
            let mut dbl: Option<(usize, EditSide)> = None;
            // P32-A5：构建折叠显示映射 (显示行索引 vi, 折叠占位块)
            // 折叠块只保留首行 + 一个“N 行已折叠”占位行；块内其余行隐藏
            let mut view: Vec<(usize, Option<usize>)> = Vec::new();
            let mut placed_block: std::collections::HashSet<usize> =
                std::collections::HashSet::new();
            let mut seen_first: std::collections::HashSet<usize> = std::collections::HashSet::new();
            for vi in 0..display_rows.len() {
                let oi = orig_of(vi);
                // oi 属于哪个差异块？
                let blk = self
                    .diff_blocks
                    .iter()
                    .position(|&(s, e)| oi >= s && oi <= e);
                match blk {
                    Some(bi) if self.collapsed_blocks.contains(&bi) => {
                        let (s, _e) = self.diff_blocks[bi];
                        if oi == s && !seen_first.contains(&s) {
                            // 块首行保留，随后追加占位行
                            view.push((vi, None));
                            if !placed_block.contains(&bi) {
                                view.push((vi, Some(bi)));
                                placed_block.insert(bi);
                            }
                            seen_first.insert(s);
                        }
                        // 其余行（含首行的后续视觉行）隐藏
                    }
                    _ => view.push((vi, None)),
                }
            }
            let mut fold_toggle: Option<usize> = None;
            let mut ignore_req: Option<usize> = None;
            // P35-A1：右键复制差异块到另一侧请求 (行索引, 目标侧)
            let mut copy_req: Option<(usize, EditSide)> = None;

            // BC 式左右两页：顶部文件名头部（固定视口宽度，不随内容横向滚动移动）
            // P33：两行结构 — 第一行文件名，第二行详情（时间 | 大小 | 编码），对标 BC 5
            {
                let head_h = 42.0;
                let head_bg = if ui.visuals().dark_mode {
                    Some(Color32::from_gray(38))
                } else {
                    Some(Color32::from_gray(230))
                };
                let head_fg = if ui.visuals().dark_mode {
                    Color32::from_rgb(150, 190, 240)
                } else {
                    Color32::from_rgb(60, 110, 190)
                };
                let detail_fg = ui.visuals().weak_text_color();
                // 每侧头部信息：文件名 + 详情行
                let l_info = self
                    .left
                    .as_ref()
                    .map(|f| (basename(&f.path), file_detail_line(f)));
                let r_info = self
                    .right
                    .as_ref()
                    .map(|f| (basename(&f.path), file_detail_line(f)));
                let l_name = l_info
                    .as_ref()
                    .map(|(n, _)| n.clone())
                    .unwrap_or_else(|| t(I18nKey::OpenLeft).to_string());
                let r_name = r_info
                    .as_ref()
                    .map(|(n, _)| n.clone())
                    .unwrap_or_else(|| t(I18nKey::OpenRight).to_string());
                let l_detail = l_info.as_ref().map(|(_, d)| d.clone()).unwrap_or_default();
                let r_detail = r_info.as_ref().map(|(_, d)| d.clone()).unwrap_or_default();
                // 头部两栏各占视口半宽（gutter + half），长行内容超宽时头部不跟随滚动
                let head_l_w = gutter_l + half;
                let head_r_w = gutter_r + half;
                ui.horizontal(|ui| {
                    // 左头部：两行（文件名 13px + 详情 11px）
                    let (l_rect, _) =
                        ui.allocate_exact_size(Vec2::new(head_l_w, head_h), egui::Sense::hover());
                    paint_bg(ui, l_rect, head_bg);
                    ui.painter().text(
                        Pos2::new(l_rect.left() + 10.0, l_rect.top() + 11.0),
                        egui::Align2::LEFT_CENTER,
                        l_name,
                        egui::FontId::proportional(13.0),
                        head_fg,
                    );
                    ui.painter().text(
                        Pos2::new(l_rect.left() + 10.0, l_rect.top() + 30.0),
                        egui::Align2::LEFT_CENTER,
                        l_detail,
                        egui::FontId::proportional(11.0),
                        detail_fg,
                    );
                    // 中间空隙
                    ui.allocate_exact_size(Vec2::new(mid_gap, head_h), egui::Sense::hover());
                    // 右头部
                    let (r_rect, _) =
                        ui.allocate_exact_size(Vec2::new(head_r_w, head_h), egui::Sense::hover());
                    paint_bg(ui, r_rect, head_bg);
                    ui.painter().text(
                        Pos2::new(r_rect.left() + 10.0, r_rect.top() + 11.0),
                        egui::Align2::LEFT_CENTER,
                        r_name,
                        egui::FontId::proportional(13.0),
                        head_fg,
                    );
                    ui.painter().text(
                        Pos2::new(r_rect.left() + 10.0, r_rect.top() + 30.0),
                        egui::Align2::LEFT_CENTER,
                        r_detail,
                        egui::FontId::proportional(11.0),
                        detail_fg,
                    );
                });
                ui.separator();
            }

            let out = super::show_rows(ui, view.len(), ROW_H, |ui, range| {
                ui.set_min_width(total_w);
                // 当前差异行（diff_pos → diff_rows 中的行索引，P31 竖条标记）
                let cur_diff_orig = self.diff_pos.and_then(|k| self.diff_rows.get(k)).copied();
                for i in range {
                    let (vi, placeholder) = view[i];
                    let oi = orig_of(vi);
                    if let Some(bi) = placeholder {
                        // 折叠占位行：点击展开
                        let (s, e) = self.diff_blocks[bi];
                        let n = e - s + 1;
                        let (rect, resp) =
                            ui.allocate_exact_size(Vec2::new(total_w, ROW_H), egui::Sense::click());
                        paint_bg(
                            ui,
                            rect,
                            if ui.visuals().dark_mode {
                                Some(Color32::from_gray(26))
                            } else {
                                Some(Color32::from_gray(240))
                            },
                        );
                        let fg = ui.visuals().weak_text_color();
                        ui.painter().text(
                            Pos2::new(rect.left() + 10.0, rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            format!("⏵ {n} 行已折叠（点击展开）"),
                            egui::FontId::proportional(12.0),
                            fg,
                        );
                        if resp.clicked() {
                            fold_toggle = Some(bi);
                        }
                        continue;
                    }
                    let row = &display_rows[vi];
                    let (bg_l, bg_r) = match row.tag {
                        RowTag::Equal => (None, None),
                        RowTag::Delete => (Some(bg_delete()), None),
                        RowTag::Insert => (None, Some(bg_insert())),
                        RowTag::Replace => (Some(bg_replace_l()), Some(bg_replace_r())),
                    };
                    // 搜索命中高亮（按原始行索引映射）
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
                    // P32-B5：忽略行弱化显示（半透明灰）
                    let ignored = self.ignored_rows.contains(&oi);
                    let (bg_l, bg_r) = if ignored {
                        let dim = if ui.visuals().dark_mode {
                            Color32::from_gray(42)
                        } else {
                            Color32::from_gray(226)
                        };
                        (Some(dim), Some(dim))
                    } else {
                        (bg_l, bg_r)
                    };
                    // P32-A5：块首行左侧画折叠箭头 ▾（点击折叠）
                    let block_start = self.diff_blocks.iter().position(|&(s, _)| s == oi);
                    let (hit, resp) = paint_diff_row(
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
                        inline.as_mut().filter(|ie| ie.row == oi),
                        block_start,
                        ignored,
                        self.h_scroll,
                    );
                    match hit {
                        Some(RowHit::Edit(side)) => dbl = Some((oi, side)),
                        Some(RowHit::FoldToggle(bi)) => fold_toggle = Some(bi),
                        None => {}
                    }
                    // P32-A4：行右键菜单（复制路径/打开文件/忽略）——闭包内只收集请求
                    let row_idx = oi;
                    // P35-A1：该行是否属于某个差异块（决定是否显示“复制到另一侧”）
                    let row_in_diff = self
                        .diff_blocks
                        .iter()
                        .any(|&(s, e)| s <= row_idx && row_idx <= e);
                    let (lp, rp) = (
                        self.left.as_ref().map(|f| f.path.clone()),
                        self.right.as_ref().map(|f| f.path.clone()),
                    );
                    resp.context_menu(|ui| {
                        // P35-A1：复制差异块到另一侧（最核心操作，置顶）
                        if row_in_diff {
                            if ui.button(t(I18nKey::CopyToRight)).clicked() {
                                copy_req = Some((row_idx, EditSide::Right));
                                ui.close();
                            }
                            if ui.button(t(I18nKey::CopyToLeft)).clicked() {
                                copy_req = Some((row_idx, EditSide::Left));
                                ui.close();
                            }
                            ui.separator();
                        }
                        if let Some(p) = &lp {
                            if ui.button("复制左侧路径").clicked() {
                                ui.ctx().copy_text(p.clone());
                                ui.close();
                            }
                        }
                        if let Some(p) = &rp {
                            if ui.button("复制右侧路径").clicked() {
                                ui.ctx().copy_text(p.clone());
                                ui.close();
                            }
                        }
                        ui.separator();
                        if let Some(p) = &lp {
                            if ui.button("打开左侧文件").clicked() {
                                super::common::open_with_system_app(p);
                                ui.close();
                            }
                        }
                        if let Some(p) = &rp {
                            if ui.button("打开右侧文件").clicked() {
                                super::common::open_with_system_app(p);
                                ui.close();
                            }
                        }
                        ui.separator();
                        if ignored {
                            if ui.button("取消忽略此行").clicked() {
                                ignore_req = Some(row_idx);
                                ui.close();
                            }
                        } else if ui.button("忽略此行").clicked() {
                            ignore_req = Some(row_idx);
                            ui.close();
                        }
                    });
                }
            });
            // P32-A5：处理折叠切换
            if let Some(bi) = fold_toggle {
                if !self.collapsed_blocks.remove(&bi) {
                    self.collapsed_blocks.insert(bi);
                }
                self.inline_edit = inline;
                return;
            }
            // P32-B5：右键忽略/取消忽略该行（会话级）
            if let Some(row) = ignore_req {
                if !self.ignored_rows.remove(&row) {
                    self.ignored_rows.insert(row);
                }
                self.inline_edit = inline;
                self.recompute();
                return;
            }
            // P35-A1：右键复制差异块到另一侧（改变文件内容，清空行内编辑）
            if let Some((row, side)) = copy_req {
                self.copy_block_at(row, side);
                self.inline_edit = None;
                return;
            }
            // P32-A2：双击行内容 → 进入行内编辑（buf 取该侧当前行文本）
            if let Some((i, side)) = dbl {
                let text = match side {
                    EditSide::Left => display_rows[i].left.as_ref().map(|c| c.text.clone()),
                    EditSide::Right => display_rows[i].right.as_ref().map(|c| c.text.clone()),
                };
                if let Some(text) = text {
                    inline = Some(InlineEditState {
                        side,
                        row: i,
                        buf: text,
                    });
                    self.inline_edit = inline;
                }
            } else {
                self.inline_edit = inline;
            }
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

/// P32-A2/A5/B5：行交互命中结果
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RowHit {
    /// 双击行内容（进入行内编辑）
    Edit(EditSide),
    /// 点击折叠箭头（切换差异块折叠）
    FoldToggle(usize),
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
    mut inline: Option<&mut InlineEditState>,
    block_start: Option<usize>,
    ignored: bool,
    // P33：横向滚动偏移（长行栏内左右滑动查看）
    h_scroll: f32,
) -> (Option<RowHit>, egui::Response) {
    let mid_gap = super::theme::MID_GAP;
    let (rect, resp) = ui.allocate_exact_size(
        Vec2::new(gutter_l + content_w + mid_gap + gutter_r + content_w, ROW_H),
        egui::Sense::click(),
    );
    let x = rect.left();
    let y = rect.top();

    // P32-A5：块首行左侧画折叠箭头 ▾（点击折叠）
    if let Some(bi) = block_start {
        let arrow_x = x + super::theme::CURRENT_BAR + 3.0;
        let arrow_rect = Rect::from_min_size(Pos2::new(arrow_x - 2.0, y), vec2(14.0, ROW_H));
        let arrow_color = if ignored {
            ui.visuals().weak_text_color()
        } else {
            super::theme::diff_modify()
        };
        ui.painter().text(
            arrow_rect.center(),
            egui::Align2::CENTER_CENTER,
            "▾",
            egui::FontId::proportional(11.0),
            arrow_color,
        );
        if resp.clicked() && arrow_rect.contains(resp.interact_pointer_pos().unwrap_or(Pos2::ZERO))
        {
            return (Some(RowHit::FoldToggle(bi)), resp);
        }
    }

    // BC 风格当前差异行：左侧 3px 竖条（P31）
    if is_current {
        ui.painter().rect_filled(
            Rect::from_min_size(Pos2::new(x, y), vec2(super::theme::CURRENT_BAR, ROW_H)),
            0.0,
            super::theme::diff_modify(),
        );
    }

    // 左 gutter + 内容（P31：gutter 用微灰底色与内容区分，BC 观感）
    let gutter_bg = if ui.visuals().dark_mode {
        Some(Color32::from_gray(30))
    } else {
        Some(Color32::from_gray(238))
    };
    let gutter_rect = Rect::from_min_size(Pos2::new(x, y), vec2(gutter_l, ROW_H));
    paint_bg(ui, gutter_rect, gutter_bg);
    paint_line_no(ui, gutter_rect, row.left_no);
    let content_rect = Rect::from_min_size(Pos2::new(x + gutter_l, y), vec2(content_w, ROW_H));
    paint_bg(ui, content_rect, bg_l);
    // P32-A2：行内编辑命中左侧 → 就地 TextEdit
    let editing_side = inline.as_ref().map(|ie| ie.side);
    match editing_side {
        Some(EditSide::Left) => {
            if let Some(ie) = inline.as_mut() {
                ui.add(
                    egui::TextEdit::singleline(&mut ie.buf)
                        .font(egui::TextStyle::Monospace)
                        .desired_width(content_w - 8.0),
                );
            }
        }
        _ => {
            paint_cell(
                ui,
                content_rect,
                row.left.as_ref(),
                fg,
                hl_l,
                syn_l,
                h_scroll,
            );
        }
    }

    // P32-A1：左右面板空隙画差异连接线（有差异的行画线连接两侧，BC 观感）
    let mid_x = x + gutter_l + content_w;
    let mid_rect = Rect::from_min_size(Pos2::new(mid_x, y), vec2(mid_gap, ROW_H));
    let mid_color = diff_mid_line_color(row.tag);
    if let Some(c) = mid_color {
        // 空隙底色（比 gutter 略深一档，突出连接线）
        paint_bg(
            ui,
            mid_rect,
            if ui.visuals().dark_mode {
                Some(Color32::from_gray(24))
            } else {
                Some(Color32::from_gray(244))
            },
        );
        // 水平连接线（行垂直居中，左右各留 2px）
        let cy = y + ROW_H / 2.0;
        ui.painter().line_segment(
            [
                Pos2::new(mid_x + 2.0, cy),
                Pos2::new(mid_x + mid_gap - 2.0, cy),
            ],
            egui::Stroke::new(1.5, c),
        );
    } else {
        // 无差异行：弱色垂直分隔线（延续面板分隔感）
        let sep = if ui.visuals().dark_mode {
            Color32::from_gray(48)
        } else {
            Color32::from_gray(210)
        };
        ui.painter().line_segment(
            [
                Pos2::new(mid_x + mid_gap / 2.0, y),
                Pos2::new(mid_x + mid_gap / 2.0, y + ROW_H),
            ],
            egui::Stroke::new(1.0, sep),
        );
    }

    // 右 gutter + 内容
    let x_r = mid_x + mid_gap;
    let gutter_rect = Rect::from_min_size(Pos2::new(x_r, y), vec2(gutter_r, ROW_H));
    paint_bg(ui, gutter_rect, gutter_bg);
    paint_line_no(ui, gutter_rect, row.right_no);
    let content_rect = Rect::from_min_size(Pos2::new(x_r + gutter_r, y), vec2(content_w, ROW_H));
    paint_bg(ui, content_rect, bg_r);
    // P32-A2：行内编辑命中右侧 → 就地 TextEdit
    let editing_side = inline.as_ref().map(|ie| ie.side);
    match editing_side {
        Some(EditSide::Right) => {
            if let Some(ie) = inline.as_mut() {
                ui.add(
                    egui::TextEdit::singleline(&mut ie.buf)
                        .font(egui::TextStyle::Monospace)
                        .desired_width(content_w - 8.0),
                );
            }
        }
        _ => {
            paint_cell(
                ui,
                content_rect,
                row.right.as_ref(),
                fg,
                hl_r,
                syn_r,
                h_scroll,
            );
        }
    }

    // P32-A2：双击行内容 → 进入行内编辑（左/右内容区命中）
    if resp.double_clicked() {
        if let Some(pos) = resp.interact_pointer_pos() {
            let left_zone = Rect::from_min_size(Pos2::new(x + gutter_l, y), vec2(content_w, ROW_H));
            let right_zone =
                Rect::from_min_size(Pos2::new(x_r + gutter_r, y), vec2(content_w, ROW_H));
            if left_zone.contains(pos) {
                return (Some(RowHit::Edit(EditSide::Left)), resp);
            }
            if right_zone.contains(pos) {
                return (Some(RowHit::Edit(EditSide::Right)), resp);
            }
        }
    }
    (None, resp)
}

/// P32-A1：差异连接线颜色（有差异的行返回对应颜色，无差异返回 None）
pub(crate) fn diff_mid_line_color(tag: RowTag) -> Option<Color32> {
    match tag {
        RowTag::Equal => None,
        RowTag::Delete => Some(super::theme::diff_delete()),
        RowTag::Insert => Some(super::theme::diff_insert()),
        RowTag::Replace => Some(super::theme::diff_modify()),
    }
}

fn basename(p: &str) -> String {
    std::path::Path::new(p)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.to_string())
}

/// P33：文件大小（详情行显示，BC 式 "12,345 bytes"）
fn file_size(p: &str) -> u64 {
    std::fs::metadata(p).map(|m| m.len()).unwrap_or(0)
}

/// P33：修改时间可读串（详情行显示，BC 式 "2026-08-13 08:09:20"）
fn file_mtime_str(p: &str) -> String {
    let meta = match std::fs::metadata(p) {
        Ok(m) => m,
        Err(_) => return String::new(),
    };
    let t = match meta.modified() {
        Ok(t) => t,
        Err(_) => return String::new(),
    };
    crate::report::fmt_mtime_pub(t)
}

/// P33：文件详情行（BC 式 "时间 | 大小 bytes | 编码"）
fn file_detail_line(f: &LoadedFile) -> String {
    let mut parts: Vec<String> = Vec::new();
    if !f.mtime.is_empty() {
        parts.push(f.mtime.clone());
    }
    if f.size > 0 {
        parts.push(format!("{} bytes", f.size));
    }
    // 编码 + BOM 标记
    let mut enc = f.encoding.name().to_string();
    if f.had_bom {
        enc.push_str(" BOM");
    }
    parts.push(enc);
    // 语法名（如 Python/JavaScript），BC 的 "Delphi Source" 语义
    if let Some(syn) = f.syntax {
        if !syn.name.is_empty() {
            parts.push(syn.name.clone());
        }
    }
    parts.join("  |  ")
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
