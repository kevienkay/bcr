// P58: 剩余少量标签专项/内部状态项待逐项迁入

// P58: 窗口内菜单仅Linux渲染, 其驱动的进阶方法在macOS/Windows未调用; 待逐项迁入原生菜单

//! 三路合并标签页：BASE/LEFT/RIGHT 三栏渲染、冲突导航与解决、保存。

use super::common::*;
use super::{icons, widgets};
use crate::i18n::{fmt, t, Key as I18nKey};
use crate::mergeview::{build_merge_view, render_merged, MergeView, Resolution};
use eframe::egui::{self, Color32, Key, Pos2, Rect, Vec2};

pub struct MergeTab {
    pub base_path: String,
    pub left_path: String,
    pub right_path: String,
    pub view: MergeView,
    /// P56-7：最近一次三路合并计算耗时（秒，状态栏显示）
    pub elapsed_secs: Option<f32>,
    pub error: Option<String>,
    pub scroll: Vec2,
    /// 当前冲突块下标（conflict_rows 索引）
    pub conflict_idx: Option<usize>,
    pub label_l: String,
    pub label_r: String,
    /// 预览窗格滚动偏移
    pub preview_scroll: Vec2,
    pub show_preview: bool,
    /// P45-1：当前行（渲染点击记录，行级采用定位用）
    pub cur_line: usize,
}

impl MergeTab {
    pub fn new(base: &str, left: &str, right: &str) -> Self {
        let mut t = MergeTab {
            base_path: base.to_string(),
            left_path: left.to_string(),
            right_path: right.to_string(),
            view: MergeView::default(),
            elapsed_secs: None,
            error: None,
            scroll: Vec2::ZERO,
            conflict_idx: None,
            label_l: "LEFT".to_string(),
            label_r: "RIGHT".to_string(),
            preview_scroll: Vec2::ZERO,
            show_preview: true,
            cur_line: 0,
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
        // 空路径（拖入单文件/空会话）不读文件，视为空内容
        let read = |p: &str| -> std::io::Result<String> {
            if p.is_empty() {
                Ok(String::new())
            } else {
                std::fs::read_to_string(p)
            }
        };
        let (b, l, r) = match (
            read(&self.base_path),
            read(&self.left_path),
            read(&self.right_path),
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
        let _start = std::time::Instant::now();
        self.view = build_merge_view(&b, &l, &r);
        self.conflict_idx = None;
        self.error = None;
        // P56-7：记录三路合并计算耗时
        self.elapsed_secs = Some(_start.elapsed().as_secs_f32());
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

    /// P37-1b：下一差异（非 Context 块循环跳转）
    pub fn next_diff(&mut self) {
        let n = self.view.diff_rows.len();
        if n == 0 {
            return;
        }
        // 从当前冲突位置向后找最近的差异块；无冲突时从 0 开始
        let cur = self
            .conflict_idx
            .and_then(|k| self.view.conflict_block_indices.get(k).copied());
        let start = cur
            .and_then(|bi| self.view.diff_block_indices.iter().position(|&b| b == bi))
            .unwrap_or(usize::MAX);
        let next = if start == usize::MAX {
            0
        } else {
            (start + 1) % n
        };
        if let Some(&row) = self.view.diff_rows.get(next) {
            self.jump_to_row(row);
        }
    }

    /// P37-1b：上一差异（非 Context 块循环跳转）
    pub fn prev_diff(&mut self) {
        let n = self.view.diff_rows.len();
        if n == 0 {
            return;
        }
        let cur = self
            .conflict_idx
            .and_then(|k| self.view.conflict_block_indices.get(k).copied());
        let start = cur
            .and_then(|bi| self.view.diff_block_indices.iter().position(|&b| b == bi))
            .unwrap_or(usize::MAX);
        let prev = if start == usize::MAX {
            n - 1
        } else {
            (start + n - 1) % n
        };
        if let Some(&row) = self.view.diff_rows.get(prev) {
            self.jump_to_row(row);
        }
    }

    /// P37-1b：跳到下一处「采用了左侧」的块（Left / LeftThenRight）
    pub fn next_taken_left(&mut self) {
        self.taken_nav(true, true, true);
    }

    /// P37-1b：跳到上一处「采用了左侧」的块
    pub fn prev_taken_left(&mut self) {
        self.taken_nav(false, true, true);
    }

    /// P37-1b：跳到下一处「采用了右侧」的块（Right / RightThenLeft）
    pub fn next_taken_right(&mut self) {
        self.taken_nav(true, false, false);
    }

    /// P37-1b：跳到上一处「采用了右侧」的块
    pub fn prev_taken_right(&mut self) {
        self.taken_nav(false, false, false);
    }

    /// 采用导航通用实现：循环遍历已解决的块
    fn taken_nav(&mut self, forward: bool, left: bool, _unused: bool) {
        use crate::mergeview::Resolution;
        // 收集匹配 resolution 的块在 diff_block_indices 中的下标
        let matched: Vec<usize> = self
            .view
            .diff_block_indices
            .iter()
            .enumerate()
            .filter(|(_, &bi)| {
                let r = self.view.blocks[bi].resolution;
                if left {
                    matches!(r, Resolution::Left | Resolution::LeftThenRight)
                } else {
                    matches!(r, Resolution::Right | Resolution::RightThenLeft)
                }
            })
            .map(|(i, _)| i)
            .collect();
        if matched.is_empty() {
            return;
        }
        // 当前位置：从当前冲突块往后找
        let cur = self
            .conflict_idx
            .and_then(|k| self.view.conflict_block_indices.get(k).copied())
            .and_then(|bi| self.view.diff_block_indices.iter().position(|&b| b == bi))
            .unwrap_or(usize::MAX);
        let next = if forward {
            match cur {
                usize::MAX => matched[0],
                c => {
                    let p = matched.iter().position(|&m| m > c);
                    match p {
                        Some(p) => matched[p],
                        None => matched[0],
                    }
                }
            }
        } else {
            match cur {
                usize::MAX => *matched.last().unwrap(),
                c => {
                    let p = matched.iter().rposition(|&m| m < c);
                    match p {
                        Some(p) => matched[p],
                        None => *matched.last().unwrap(),
                    }
                }
            }
        };
        if let Some(&row) = self.view.diff_rows.get(next) {
            self.conflict_idx = self.view.conflict_rows.iter().position(|&r| r == row);
            self.jump_to_row(row);
        }
    }

    /// P37-1b：清除当前冲突（未解决时默认取左）并跳到下一冲突区段（BC Clear Conflict Section, Next）
    pub fn clear_conflict_next(&mut self) {
        // 未定位到冲突时，先定位到第一个冲突（BC 语义：当前区段=当前位置）
        if self.conflict_idx.is_none() {
            self.next_conflict();
        }
        if let Some(bi) = self.current_conflict_block() {
            if let Some(blk) = self.view.blocks.get_mut(bi) {
                if blk.resolution == Resolution::Auto {
                    blk.resolution = Resolution::Left;
                }
            }
        }
        self.next_conflict();
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

    /// P57-10：解决当前冲突并前进到下一个冲突（BC 行为，不循环回第一个）。
    /// 用于"取左/取右/取BASE"等主要解决按钮，解决完自动跳到下一处。
    pub fn resolve_and_advance(&mut self, res: Resolution) {
        self.resolve_current(res);
        let n = self.view.conflict_rows.len();
        if let Some(k) = self.conflict_idx {
            if k + 1 < n {
                self.conflict_idx = Some(k + 1);
                if let Some(&row) = self.view.conflict_rows.get(k + 1) {
                    self.jump_to_row(row);
                }
            }
            // 已到最后一个冲突：保留当前位置（不循环）
        }
    }

    /// P45-1：行级采用（BC 采用左边的行/中心行/右边行）——当前行所在冲突块的对应行
    pub fn take_line(&mut self, res: Resolution) {
        // 找 cur_line 所在冲突块（view.rows 索引 → blocks 索引）
        let Some(&row) = self
            .view
            .conflict_rows
            .iter()
            .find(|&&s| s <= self.cur_line)
        else {
            return;
        };
        let Some(ci) = self.view.conflict_rows.iter().position(|&s| s == row) else {
            return;
        };
        let Some(&bi) = self.view.conflict_block_indices.get(ci) else {
            return;
        };
        let Some(blk) = self.view.blocks.get_mut(bi) else {
            return;
        };
        // 行偏移 = cur_line - 冲突块起始行；line_res 长度不足时补齐
        let off = self.cur_line - row;
        let len = blk.left.len().max(blk.right.len()).max(blk.base.len());
        if blk.line_res.len() < len {
            blk.line_res.resize(len, None);
        }
        if off < blk.line_res.len() {
            blk.line_res[off] = Some(res);
        }
    }

    /// P45-1：行级采用已解析行数（测试/状态用）
    #[cfg(test)]
    pub fn line_takes(&self) -> usize {
        self.view
            .blocks
            .iter()
            .map(|b| b.line_res.iter().filter(|r| r.is_some()).count())
            .sum()
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
        if crate::gui::common::SHOW_TOOLBAR.load(std::sync::atomic::Ordering::Relaxed) {
            egui::Panel::top("mergetab_tools").show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    if widgets::tool_button(ui, Some(icons::Icon::Refresh), t(I18nKey::Reload), "")
                        .clicked()
                    {
                        self.reload();
                    }
                    if widgets::tool_button(
                        ui,
                        Some(icons::Icon::Save),
                        t(I18nKey::SaveMerged),
                        "",
                    )
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
                    if widgets::tool_button(
                        ui,
                        Some(icons::Icon::Next),
                        t(I18nKey::NextConflict),
                        "下一冲突 (F7)",
                    )
                    .clicked()
                    {
                        self.next_conflict();
                    }
                    if widgets::tool_button(
                        ui,
                        Some(icons::Icon::Prev),
                        t(I18nKey::PrevConflict),
                        "上一冲突 (Shift+F7)",
                    )
                    .clicked()
                    {
                        self.prev_conflict();
                    }
                    // P37-1b：清除冲突区段并跳下一（BC Clear Conflict Section, Next）
                    if ui
                        .button(format!("✖ {}", t(I18nKey::ClearConflictNext)))
                        .on_hover_text("清除当前冲突（未解决默认取左）并跳到下一冲突区段")
                        .clicked()
                    {
                        self.clear_conflict_next();
                    }
                    ui.separator();
                    // P37-1b：差异导航（非 Context 块，BC Next/Previous Difference）
                    if ui
                        .button(format!("↡ {}", t(I18nKey::MergeNextDiff)))
                        .on_hover_text("下一差异")
                        .clicked()
                    {
                        self.next_diff();
                    }
                    if ui
                        .button(format!("↟ {}", t(I18nKey::MergePrevDiff)))
                        .on_hover_text("上一差异")
                        .clicked()
                    {
                        self.prev_diff();
                    }
                    ui.separator();
                    // P37-1b：左/右采用导航（BC Next/Previous Left/Right Taken）
                    if ui
                        .button(format!("◀ {}", t(I18nKey::NextLeftTaken)))
                        .on_hover_text("下一处采用左侧的区段")
                        .clicked()
                    {
                        self.next_taken_left();
                    }
                    if ui
                        .button(format!("▶ {}", t(I18nKey::NextRightTaken)))
                        .on_hover_text("下一处采用右侧的区段")
                        .clicked()
                    {
                        self.next_taken_right();
                    }
                    ui.separator();
                    if ui.button(format!("← {}", t(I18nKey::TakeLeft))).clicked() {
                        self.resolve_and_advance(Resolution::Left);
                    }
                    if ui.button(format!("→ {}", t(I18nKey::TakeRight))).clicked() {
                        self.resolve_and_advance(Resolution::Right);
                    }
                    if ui.button(format!("B {}", t(I18nKey::TakeBase))).clicked() {
                        self.resolve_and_advance(Resolution::Base);
                    }
                    // P37-1：顺序合并（BC 采用左边然后右边/采用右边然后左边）
                    if ui
                        .button(format!("⇉ {}", t(I18nKey::TakeLeftThenRight)))
                        .on_hover_text("先采用左边内容，再追加右边内容")
                        .clicked()
                    {
                        self.resolve_current(Resolution::LeftThenRight);
                    }
                    if ui
                        .button(format!("⇇ {}", t(I18nKey::TakeRightThenLeft)))
                        .on_hover_text("先采用右边内容，再追加左边内容")
                        .clicked()
                    {
                        self.resolve_current(Resolution::RightThenLeft);
                    }
                    // 显示当前冲突块的解决状态（P31：已解决 ✓ 绿标）
                    if let Some(bi) = self.current_conflict_block() {
                        if let Some(blk) = self.view.blocks.get(bi) {
                            ui.separator();
                            let (mark, color) = match blk.resolution {
                                Resolution::Auto => (
                                    t(I18nKey::ResAuto).to_string(),
                                    super::theme::conflict_color(ui.visuals().dark_mode),
                                ),
                                Resolution::Left
                                | Resolution::Right
                                | Resolution::Base
                                | Resolution::LeftThenRight
                                | Resolution::RightThenLeft => (
                                    format!("✓ {}", t(I18nKey::Resolved)),
                                    super::theme::resolved_color(ui.visuals().dark_mode),
                                ),
                            };
                            ui.colored_label(color, mark);
                        }
                    }
                });
            });
        } // mergetab_tools 门控闭合

        if let Some(err) = self.error.clone() {
            crate::gui::common::dialog_window(ui.ctx(), t(I18nKey::Hint))
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
        // P44-3：BC 冲突采用快捷键（⇧← 采用左边 / ⇧→ 采用右边；⌘B 左后右 / ⇧⌘B 右后左）
        let cmd = ui.input(|i| i.modifiers.command);
        let shift = ui.input(|i| i.modifiers.shift);
        if !cmd && shift && ui.input(|i| i.key_pressed(Key::ArrowLeft)) {
            self.resolve_current(Resolution::Left);
        }
        if !cmd && shift && ui.input(|i| i.key_pressed(Key::ArrowRight)) {
            self.resolve_current(Resolution::Right);
        }
        if cmd && !shift && ui.input(|i| i.key_pressed(Key::B)) {
            self.resolve_current(Resolution::LeftThenRight);
        }
        if cmd && shift && ui.input(|i| i.key_pressed(Key::B)) {
            self.resolve_current(Resolution::RightThenLeft);
        }
        // P44-3：⌘⇧⌃↓/↑ 下一/上一冲突部分（BC 搜索菜单，与 F7/⇧F7 等效）
        if ui.input(|i| {
            i.modifiers.command
                && i.modifiers.shift
                && i.modifiers.ctrl
                && i.key_pressed(Key::ArrowDown)
        }) {
            self.next_conflict();
        }
        if ui.input(|i| {
            i.modifiers.command
                && i.modifiers.shift
                && i.modifiers.ctrl
                && i.key_pressed(Key::ArrowUp)
        }) {
            self.prev_conflict();
        }
        // P45-1：⌥⇧←/→ 采用左/右行（行级采用，BC 编辑菜单）
        let alt = ui.input(|i| i.modifiers.alt);
        if alt && shift && ui.input(|i| i.key_pressed(Key::ArrowLeft)) {
            self.take_line(Resolution::Left);
        }
        if alt && shift && ui.input(|i| i.key_pressed(Key::ArrowRight)) {
            self.take_line(Resolution::Right);
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
                            super::theme::conflict_color(ui.visuals().dark_mode),
                            fmt(I18nKey::MergeUnresolved, &[&unresolved.to_string()]),
                        );
                    } else {
                        ui.colored_label(
                            super::theme::resolved_color(ui.visuals().dark_mode),
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
                // P52-2：统一空状态（三路合并用橙色系）。
                // P58：打开左/右文件入口已由原生菜单提供（dispatch 到 open_left/right），
                // 空状态仅保留 BASE（原生菜单无 BASE 入口）。
                super::common::empty_state(
                    ui,
                    "🔀",
                    super::theme::card_icon_colors()[2],
                    t(I18nKey::MergeEmpty),
                    t(I18nKey::DragHint),
                    |ui| {
                        ui.horizontal(|ui| {
                            if ui
                                .button(format!("📂 {}", t(I18nKey::OpenBase)))
                                .on_hover_text("打开 BASE 文件")
                                .clicked()
                            {
                                self.open_base();
                            }
                        });
                    },
                );
                return;
            }

            let rows = &self.view.rows;
            let max_no = rows.iter().filter_map(|r| r.base_no).max().unwrap_or(0);
            let gutter = gutter_width(max_no);
            let avail = ui.available_width();
            let col_w = ((avail - gutter * 3.0) / 3.0).max(150.0);
            let total_w = gutter * 3.0 + col_w * 3.0;
            let fg = text_color(ui);

            let mut click_line: Option<usize> = None;
            let out = {
                let mut sa = egui::ScrollArea::both().auto_shrink([false, false]);
                sa = sa.vertical_scroll_offset(self.scroll.y);
                sa = sa.horizontal_scroll_offset(self.scroll.x);
                let syn_b = crate::highlight::syntax_for(&self.base_path);
                let syn_l = crate::highlight::syntax_for(&self.left_path);
                let syn_r = crate::highlight::syntax_for(&self.right_path);
                sa.show_rows(ui, ROW_H, rows.len(), |ui, range| {
                    ui.set_min_width(total_w);
                    // P57-9：当前冲突块行范围（左侧边条，BC 当前区段视觉）
                    let cur_span = self.conflict_idx.and_then(|ci| {
                        let start = *self.view.conflict_rows.get(ci)?;
                        let bi = *self.view.conflict_block_indices.get(ci)?;
                        let blk = self.view.blocks.get(bi)?;
                        let n = blk.base.len().max(blk.left.len()).max(blk.right.len());
                        Some((start, start + n))
                    });
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
                        // P57-9：当前冲突块左侧 3px 蓝色侧边条
                        if let Some((s, e)) = cur_span {
                            if i >= s && i < e {
                                ui.painter().rect_filled(
                                    Rect::from_min_size(
                                        Pos2::new(resp.rect.left(), resp.rect.top()),
                                        vec2(3.0, ROW_H),
                                    ),
                                    0.0,
                                    super::theme::current_bar(ui.visuals().dark_mode),
                                );
                            }
                        }
                        // P45-1：点击行记录 cur_line（行级采用定位）
                        if resp.clicked() {
                            click_line = Some(i);
                        }
                        // P32-A4：行右键菜单（复制路径/打开所在位置/系统打开）
                        let (bp2, lp2, rp2) = (bp.clone(), lp.clone(), rp.clone());
                        resp.context_menu(|ui| {
                            if ui.button("复制 BASE 路径").clicked() {
                                ui.ctx().copy_text(bp2.clone());
                                ui.close();
                            }
                            if ui.button(t(I18nKey::CopyLeftPath)).clicked() {
                                ui.ctx().copy_text(lp2.clone());
                                ui.close();
                            }
                            if ui.button(t(I18nKey::CopyRightPath)).clicked() {
                                ui.ctx().copy_text(rp2.clone());
                                ui.close();
                            }
                            ui.separator();
                            if ui.button("打开所在位置（BASE）").clicked() {
                                super::common::reveal_in_file_manager(&bp2);
                                ui.close();
                            }
                            if ui.button(t(I18nKey::RevealLeft)).clicked() {
                                super::common::reveal_in_file_manager(&lp2);
                                ui.close();
                            }
                            if ui.button(t(I18nKey::RevealRight)).clicked() {
                                super::common::reveal_in_file_manager(&rp2);
                                ui.close();
                            }
                            ui.separator();
                            if ui.button("系统打开（BASE）").clicked() {
                                super::common::open_with_system_app(&bp2);
                                ui.close();
                            }
                            if ui.button(t(I18nKey::SystemOpenLeft)).clicked() {
                                super::common::open_with_system_app(&lp2);
                                ui.close();
                            }
                            if ui.button(t(I18nKey::SystemOpenRight)).clicked() {
                                super::common::open_with_system_app(&rp2);
                                ui.close();
                            }
                        });
                    }
                })
            };
            // P45-1：渲染后应用点击行（借用安全：闭包外写 self）
            if let Some(i) = click_line {
                self.cur_line = i;
            }
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
            Some(super::theme::merge_conflict_bg()),
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
            super::theme::diff_modify(ui.visuals().dark_mode),
        );
    }

    // BASE 列
    let g = Rect::from_min_size(Pos2::new(x, y), vec2(gutter, ROW_H));
    paint_bg(ui, g, bg_b);
    paint_line_no(ui, g, row.base_no);
    let c = Rect::from_min_size(Pos2::new(x + gutter, y), vec2(col_w, ROW_H));
    paint_bg(ui, c, bg_b);
    paint_cell(ui, c, row.base.as_ref(), fg, None, syn_b, 0.0, false);

    // LEFT 列
    let xl = x + gutter + col_w;
    let g = Rect::from_min_size(Pos2::new(xl, y), vec2(gutter, ROW_H));
    paint_bg(ui, g, bg_l);
    paint_line_no(ui, g, None);
    let c = Rect::from_min_size(Pos2::new(xl + gutter, y), vec2(col_w, ROW_H));
    paint_bg(ui, c, bg_l);
    paint_cell(ui, c, row.left.as_ref(), fg, hl_l, syn_l, 0.0, false);

    // RIGHT 列
    let xr = xl + gutter + col_w;
    let g = Rect::from_min_size(Pos2::new(xr, y), vec2(gutter, ROW_H));
    paint_bg(ui, g, bg_r);
    paint_line_no(ui, g, None);
    let c = Rect::from_min_size(Pos2::new(xr + gutter, y), vec2(col_w, ROW_H));
    paint_bg(ui, c, bg_r);
    paint_cell(ui, c, row.right.as_ref(), fg, hl_r, syn_r, 0.0, false);
    resp
}

fn basename(p: &str) -> String {
    std::path::Path::new(p)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.to_string())
}
