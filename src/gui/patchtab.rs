//! P37-1h：补丁视图（对标 BC Text Patch，`.patch`/`.diff` 自动进入）。
//!
//! 解析补丁文件 → 左侧（旧）vs 右侧（新）双栏对比，
//! 支持应用补丁（把右侧内容写回 b 侧路径，A2 模式 .bak 备份）。

use super::common::*;
use crate::i18n::{fmt, t, Key as I18nKey};
use crate::patchview::{parse_patch, ParsedPatch};
use crate::sideview::{build_rows, RowTag, SideRow, ViewOptions};
use eframe::egui::{self, Pos2, Rect, Vec2};

/// 补丁标签页
pub struct PatchTab {
    pub(crate) path: String,
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
    /// P37-1k：书签（编号 0-9 → 渲染行索引）
    bookmarks: std::collections::HashMap<u8, usize>,
    /// P37-1k：书签编号输入框
    bookmark_no: String,
    /// P45-5：选区（rows 行索引范围，选择选择内容用）
    pub(crate) selection: Option<(usize, usize)>,
    /// P46-2：当前差异行索引（差异导航定位）
    diff_pos: Option<usize>,
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
            bookmarks: std::collections::HashMap::new(),
            bookmark_no: String::new(),
            selection: None,
            diff_pos: None,
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

    /// P46-2：差异行索引集合（RowTag ≠ Equal 的行）
    fn diff_rows(&self) -> Vec<usize> {
        self.rows
            .iter()
            .enumerate()
            .filter(|(_, r)| r.tag != RowTag::Equal)
            .map(|(i, _)| i)
            .collect()
    }

    /// P46-2：下一差异（BC 搜索>下一个差异，⇧⌥⌃↓）
    pub fn next_diff(&mut self) {
        let rows = self.diff_rows();
        if rows.is_empty() {
            self.diff_pos = None;
            return;
        }
        let cur = self
            .diff_pos
            .and_then(|p| rows.iter().position(|&r| r >= p));
        let next = match cur {
            Some(c) if rows[c] > self.diff_pos.unwrap_or(0) => c,
            Some(c) => (c + 1) % rows.len(),
            None => 0,
        };
        self.diff_pos = Some(rows[next]);
        self.jump_to(rows[next]);
    }

    /// P46-2：上一差异（BC 搜索>上一个差异，⇧⌥⌃↑）
    pub fn prev_diff(&mut self) {
        let rows = self.diff_rows();
        if rows.is_empty() {
            self.diff_pos = None;
            return;
        }
        let cur = self
            .diff_pos
            .and_then(|p| rows.iter().rposition(|&r| r <= p));
        let prev = match cur {
            Some(c) if rows[c] < self.diff_pos.unwrap_or(usize::MAX) => c,
            Some(c) => (c + rows.len() - 1) % rows.len(),
            None => rows.len() - 1,
        };
        self.diff_pos = Some(rows[prev]);
        self.jump_to(rows[prev]);
    }

    /// P46-2：下一差异部分（BC 搜索>下一个差异部分，⇧⌃↓；跳到下一连续差异块首行）
    pub fn next_diff_section(&mut self) {
        self.section_nav(true);
    }

    /// P46-2：上一差异部分（BC 搜索>上一个差异部分，⇧⌃↑）
    pub fn prev_diff_section(&mut self) {
        self.section_nav(false);
    }

    /// P46-2：区块导航通用实现（连续差异行合并为一个区块，取区块首行）
    fn section_nav(&mut self, forward: bool) {
        let rows = self.diff_rows();
        if rows.is_empty() {
            self.diff_pos = None;
            return;
        }
        // 连续差异行合并为区块（相邻行差距 >1 视为新块）
        let mut blocks: Vec<usize> = Vec::new();
        let mut prev: Option<usize> = None;
        for r in &rows {
            if prev.map(|p| r - p > 1).unwrap_or(true) {
                blocks.push(*r);
            }
            prev = Some(*r);
        }
        let cur_row = self.diff_pos.unwrap_or(0);
        if forward {
            let next = blocks
                .iter()
                .find(|&&b| b > cur_row)
                .or_else(|| blocks.first())
                .copied()
                .unwrap_or(blocks[0]);
            self.diff_pos = Some(next);
            self.jump_to(next);
        } else {
            let prev_b = blocks
                .iter()
                .rev()
                .find(|&&b| b < cur_row)
                .or_else(|| blocks.last())
                .copied()
                .unwrap_or(blocks[0]);
            self.diff_pos = Some(prev_b);
            self.jump_to(prev_b);
        }
    }

    /// P46-2：滚动到指定行（行顶部对齐）
    fn jump_to(&mut self, row: usize) {
        self.scroll.y = (row as f32 * super::theme::ROW_H - 4.0 * super::theme::ROW_H).max(0.0);
    }

    /// P46-2：当前差异行（测试/状态用）
    #[cfg(test)]
    pub(crate) fn current_diff_pos(&self) -> Option<usize> {
        self.diff_pos
    }

    /// P45-5：选择选择内容——把第一个差异块（Delete/Insert/Replace 行连续段）选为选区
    pub(crate) fn select_selection(&mut self) {
        if self.rows.is_empty() {
            self.selection = None;
            return;
        }
        let mut start = None;
        let mut end = None;
        for (i, r) in self.rows.iter().enumerate() {
            if matches!(r.tag, RowTag::Delete | RowTag::Insert | RowTag::Replace) {
                if start.is_none() {
                    start = Some(i);
                }
                end = Some(i);
            } else if start.is_some() {
                break;
            }
        }
        self.selection = match (start, end) {
            (Some(s), Some(e)) => Some((s, e)),
            _ => None,
        };
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

    // ---- P37-1k：书签（BC 切换/转到/清除书签） ----

    /// 切换书签：当前可见顶部行绑定编号（0-9），已存在则取消
    pub fn toggle_bookmark(&mut self, no: u8) {
        if no > 9 {
            return;
        }
        let top = (self.scroll.y / super::theme::ROW_H) as usize;
        if self.bookmarks.get(&no) == Some(&top) {
            self.bookmarks.remove(&no);
        } else {
            self.bookmarks.insert(no, top);
        }
    }

    /// 转到书签（0-9）：滚动到对应行
    pub fn goto_bookmark(&mut self, no: u8) {
        if let Some(&row) = self.bookmarks.get(&no) {
            self.scroll.y = row as f32 * super::theme::ROW_H;
        }
    }

    /// 清除全部书签
    pub fn clear_bookmarks(&mut self) {
        self.bookmarks.clear();
    }

    /// 当前书签（测试用）
    #[cfg(test)]
    pub(crate) fn bookmarks(&self) -> &std::collections::HashMap<u8, usize> {
        &self.bookmarks
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
        // P46-2：差异导航快捷键（BC 搜索菜单 ⇧⌥⌃↓/↑ 差异、⇧⌃↓/↑ 差异部分；输入框聚焦时不触发）
        if !ui.ctx().egui_wants_keyboard_input() {
            if ui.input(|i| {
                i.modifiers.ctrl
                    && i.modifiers.shift
                    && i.modifiers.alt
                    && i.key_pressed(egui::Key::ArrowDown)
            }) {
                self.next_diff();
            }
            if ui.input(|i| {
                i.modifiers.ctrl
                    && i.modifiers.shift
                    && i.modifiers.alt
                    && i.key_pressed(egui::Key::ArrowUp)
            }) {
                self.prev_diff();
            }
            if ui.input(|i| {
                i.modifiers.ctrl
                    && i.modifiers.shift
                    && !i.modifiers.alt
                    && i.key_pressed(egui::Key::ArrowDown)
            }) {
                self.next_diff_section();
            }
            if ui.input(|i| {
                i.modifiers.ctrl
                    && i.modifiers.shift
                    && !i.modifiers.alt
                    && i.key_pressed(egui::Key::ArrowUp)
            }) {
                self.prev_diff_section();
            }
        }
        if crate::gui::common::SHOW_TOOLBAR.load(std::sync::atomic::Ordering::Relaxed) {
            egui::Panel::top("patch_tools").show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .button(format!("📂 {}", t(I18nKey::OpenFile)))
                        .on_hover_text("打开补丁文件")
                        .clicked()
                    {
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
                    // P37-1k：书签（BC 搜索菜单 切换/转到/清除书签）
                    if !self.path.is_empty() {
                        ui.separator();
                        let mut no = self.bookmark_no.clone();
                        ui.label("#");
                        ui.add(
                            egui::TextEdit::singleline(&mut no)
                                .desired_width(30.0)
                                .hint_text("0-9"),
                        );
                        if no != self.bookmark_no {
                            self.bookmark_no = no;
                        }
                        let parsed_no = self.bookmark_no.trim().parse::<u8>().ok();
                        if ui
                            .add_enabled(
                                parsed_no.is_some(),
                                egui::Button::new(t(I18nKey::ToggleBookmark)),
                            )
                            .on_hover_text("当前顶部行绑定/取消书签编号")
                            .clicked()
                        {
                            if let Some(n) = parsed_no {
                                self.toggle_bookmark(n);
                            }
                        }
                        if ui
                            .add_enabled(
                                parsed_no.is_some(),
                                egui::Button::new(t(I18nKey::GoToBookmark)),
                            )
                            .clicked()
                        {
                            if let Some(n) = parsed_no {
                                self.goto_bookmark(n);
                            }
                        }
                        if ui
                            .add_enabled(
                                !self.bookmarks.is_empty(),
                                egui::Button::new(t(I18nKey::ClearBookmarks)),
                            )
                            .clicked()
                        {
                            self.clear_bookmarks();
                        }
                    }
                });
            });
        } // patch_tools 门控闭合

        if let Some(err) = self.error.clone() {
            crate::gui::common::dialog_window(ui.ctx(), t(I18nKey::Hint))
                .collapsible(false)
                .resizable(false)
                .show(ui.ctx(), |ui| {
                    ui.colored_label(super::theme::error_color(), err);
                    if ui.button(t(I18nKey::Close)).clicked() {
                        self.error = None;
                    }
                });
        }
        if let Some(m) = self.msg.clone() {
            crate::gui::common::dialog_window(ui.ctx(), t(I18nKey::Hint))
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
            // P37-1k：书签标记（行索引 → 编号集合，渲染用）
            let bookmark_marks: std::collections::HashMap<usize, u8> =
                self.bookmarks.iter().map(|(&no, &row)| (row, no)).collect();
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
                    // P45-5：选区行叠加蓝色高亮（选择选择内容）
                    let sel_bg = self.selection.is_some_and(|(s, e)| i >= s && i <= e);
                    // P37-1k：书签标记（🔖 + 编号）
                    if let Some(&no) = bookmark_marks.get(&i) {
                        ui.painter().text(
                            Pos2::new(rect.left() + 2.0, rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            format!("🔖{}", no),
                            egui::FontId::monospace(11.0),
                            super::theme::plan_color(ui.visuals().dark_mode),
                        );
                    }
                    // 左列
                    let lr = Rect::from_min_size(rect.min, vec2(gutter + half, ROW_H));
                    let bg_use = if sel_bg {
                        Some(super::theme::selection_overlay())
                    } else {
                        bg
                    };
                    paint_bg(ui, lr, bg_use);
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

    // ---- P37-1k：书签（切换/转到/清除） ----

    #[test]
    fn bookmarks_toggle_goto_clear() {
        let d = tempdir().unwrap();
        let p = write(
            d.path(),
            "a.patch",
            "--- a/a.txt\n+++ b/a.txt\n@@ -1,3 +1,3 @@\n line1\n-old line\n+new line\n line3\n",
        );
        let mut t = PatchTab::new(&p);
        // 初始无书签
        assert!(t.bookmarks().is_empty());
        // 滚动到第 2 行后切换书签 0
        t.scroll.y = 2.0 * crate::gui::theme::ROW_H;
        t.toggle_bookmark(0);
        assert_eq!(t.bookmarks().get(&0), Some(&2), "书签 0 应绑定第 2 行");
        // 再切换同一编号同一行 → 取消
        t.toggle_bookmark(0);
        assert!(t.bookmarks().get(&0).is_none(), "再次切换应取消书签");
        // 重新绑定：当前顶部行（scroll.y 仍为第 2 行）绑定编号 3
        t.toggle_bookmark(3);
        assert_eq!(
            t.bookmarks().get(&3),
            Some(&2),
            "书签 3 应绑定当前顶部第 2 行"
        );
        // 跳走后再回到书签位置
        t.scroll.y = 100.0;
        t.goto_bookmark(3);
        assert_eq!(
            t.scroll.y,
            2.0 * crate::gui::theme::ROW_H,
            "应回到书签绑定的第 2 行"
        );
        // 清除
        t.clear_bookmarks();
        assert!(t.bookmarks().is_empty());
    }

    #[test]
    fn bookmark_no_out_of_range_ignored() {
        let d = tempdir().unwrap();
        let p = write(
            d.path(),
            "a.patch",
            "--- a/a\n+++ b/a\n@@ -1,1 +1,1 @@\n-x\n+y\n",
        );
        let mut t = PatchTab::new(&p);
        t.toggle_bookmark(10); // 超出 0-9 → 忽略
        assert!(t.bookmarks().is_empty());
    }
}
