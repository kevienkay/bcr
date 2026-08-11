//! 目录对比标签页：树形差异视图（可折叠）+ 键盘导航 + 双击打开并排 Diff。

use super::common::*;
use crate::compare::{compare_dirs, CompareResult, FileStatus};
use crate::fsscan::Filter;
use crate::i18n::{fmt, t, Key as I18nKey};
use crate::sync::{build_plan, execute_op, SyncOp};
use eframe::egui::{self, Color32, Key, Pos2, Vec2};
use std::collections::HashSet;

/// 目录视图状态过滤（B1 显示过滤，对齐 BC 显示过滤器）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewFilter {
    /// 全部（含相同，受 show_same 控制）
    All,
    /// 仅差异（differ/left_only/right_only/moved）
    Diff,
    /// 仅左侧存在
    LeftOnly,
    /// 仅右侧存在
    RightOnly,
    /// 仅移动/重命名
    Moved,
    /// 仅相同
    Same,
}

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
    /// 状态过滤（B1）
    pub view_filter: ViewFilter,
    pub result: Option<CompareResult>,
    pub error: Option<String>,
    pub scroll: Vec2,
    /// 请求打开并排 diff（rel 相对路径，由主应用拼完整路径）
    pub open_diff: Option<String>,
    /// 手动对齐：请求用指定左右相对路径打开并排 diff（不同文件名配对）
    pub open_pair: Option<(String, String)>,
    /// 折叠的目录路径集合（空字符串表示根）
    pub(crate) collapsed: HashSet<String>,
    /// 选中的展平行索引
    pub(crate) selected: Option<usize>,
    /// 展平后的行
    pub(crate) flat: Vec<FlatRow>,
    /// 需要滚动到选中行的标记
    scroll_to_selected: bool,
    /// 同步面板是否展开
    pub show_sync: bool,
    /// 同步模式：update | mirror | two-way
    pub sync_mode: String,
    /// 同步计划（生成后缓存，供勾选/执行）
    pub sync_plan: Option<Vec<SyncOp>>,
    /// 勾选的计划项索引
    pub sync_checked: HashSet<usize>,
    /// 同步执行结果消息
    pub sync_msg: Option<String>,
    /// 上次自动刷新时间（秒，egui time）
    last_auto_refresh: f64,
    /// 手动对齐弹窗：选中的左侧文件相对路径
    align_left: Option<String>,
    /// 手动对齐弹窗：选中的右侧文件相对路径
    align_right: Option<String>,
    /// 手动对齐弹窗开关
    show_align: bool,
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
            view_filter: ViewFilter::Diff,
            result: None,
            error: None,
            scroll: Vec2::ZERO,
            open_diff: None,
            open_pair: None,
            collapsed: HashSet::new(),
            selected: None,
            flat: Vec::new(),
            scroll_to_selected: false,
            show_sync: false,
            sync_mode: "update".to_string(),
            sync_plan: None,
            sync_checked: HashSet::new(),
            sync_msg: None,
            last_auto_refresh: 0.0,
            align_left: None,
            align_right: None,
            show_align: false,
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
            true,
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
        // 状态过滤（B1）：下拉选择覆盖 only_diff 复选框
        match self.view_filter {
            ViewFilter::All => {
                if self.only_diff {
                    visible.retain(|e| e.status != FileStatus::Same);
                }
            }
            ViewFilter::Diff => {
                visible.retain(|e| e.status != FileStatus::Same);
            }
            ViewFilter::LeftOnly => {
                visible.retain(|e| e.status == FileStatus::LeftOnly);
            }
            ViewFilter::RightOnly => {
                visible.retain(|e| e.status == FileStatus::RightOnly);
            }
            ViewFilter::Moved => {
                visible.retain(|e| e.status == FileStatus::Moved);
            }
            ViewFilter::Same => {
                visible.retain(|e| e.status == FileStatus::Same);
            }
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

    /// 当前选中文件的相对路径（选中目录或未选中时返回 None）
    pub(crate) fn selected_rel(&self) -> Option<String> {
        let idx = self.selected?;
        let row = self.flat.get(idx)?;
        let ei = row.entry?;
        let r = self.result.as_ref()?;
        r.entries.get(ei).map(|e| e.rel.clone())
    }

    /// 生成同步计划（基于当前 left/right/过滤/模式），勾选默认全部可执行项
    pub fn gen_sync_plan(&mut self) {
        let filter = match Filter::new(&split_globs(&self.includes), &split_globs(&self.excludes)) {
            Ok(f) => f,
            Err(e) => {
                self.sync_msg = Some(fmt(I18nKey::FilterError, &[&e.to_string()]));
                return;
            }
        };
        let (l, r) = match (crate::vfs::open(&self.left), crate::vfs::open(&self.right)) {
            (Ok(l), Ok(r)) => (l, r),
            (Err(e), _) => {
                self.sync_msg = Some(format!("打开 {} 失败: {}", self.left, e));
                return;
            }
            (_, Err(e)) => {
                self.sync_msg = Some(format!("打开 {} 失败: {}", self.right, e));
                return;
            }
        };
        match build_plan(
            &self.sync_mode,
            self.compare_content,
            l.as_ref(),
            r.as_ref(),
            &filter,
        ) {
            Ok(plan) => {
                self.sync_checked.clear();
                for (i, op) in plan.iter().enumerate() {
                    // 跳过/冲突不可执行，默认不勾选
                    if !matches!(op, SyncOp::Skip { .. } | SyncOp::Conflict { .. }) {
                        self.sync_checked.insert(i);
                    }
                }
                self.sync_plan = Some(plan);
                self.sync_msg = None;
            }
            Err(e) => {
                self.sync_msg = Some(e);
            }
        }
    }

    /// 执行勾选的同步操作，完成后重新对比
    pub fn run_sync_checked(&mut self) {
        let Some(plan) = self.sync_plan.clone() else {
            self.sync_msg = Some("请先生成计划".to_string());
            return;
        };
        let (l, r) = match (crate::vfs::open(&self.left), crate::vfs::open(&self.right)) {
            (Ok(l), Ok(r)) => (l, r),
            _ => return,
        };
        let mut n_ok = 0usize;
        let mut n_err = 0usize;
        for (i, op) in plan.iter().enumerate() {
            if !self.sync_checked.contains(&i) {
                continue;
            }
            match execute_op(op, l.as_ref(), r.as_ref()) {
                Some(_) => n_err += 1,
                None => n_ok += 1,
            }
        }
        self.sync_msg = Some(format!("同步完成: 成功 {} 项，失败 {} 项", n_ok, n_err));
        self.sync_plan = None;
        self.sync_checked.clear();
        self.refresh();
    }

    /// 对选中文件执行单项操作（复制/删除等），成功则重新对比
    pub fn run_single_op(&mut self, op: SyncOp) {
        let (l, r) = match (crate::vfs::open(&self.left), crate::vfs::open(&self.right)) {
            (Ok(l), Ok(r)) => (l, r),
            _ => return,
        };
        match execute_op(&op, l.as_ref(), r.as_ref()) {
            Some(e) => self.sync_msg = Some(format!("操作失败: {}", e)),
            None => {
                self.sync_msg = Some(format!("完成: {}", op.describe()));
                self.refresh();
            }
        }
    }

    /// 批量操作：把全部差异/仅左侧文件复制到右侧（跳过仅右侧，避免覆盖）
    pub fn run_batch_copy_to_right(&mut self) {
        let Some(r) = self.result.clone() else { return };
        let rels: Vec<String> = r
            .entries
            .iter()
            .filter(|e| matches!(e.status, FileStatus::Differ | FileStatus::LeftOnly))
            .map(|e| e.rel.clone())
            .collect();
        if rels.is_empty() {
            self.sync_msg = Some("没有可复制的差异文件".to_string());
            return;
        }
        let (l, r) = match (crate::vfs::open(&self.left), crate::vfs::open(&self.right)) {
            (Ok(l), Ok(r)) => (l, r),
            _ => return,
        };
        let mut ok = 0usize;
        let mut err = 0usize;
        for rel in &rels {
            let op = SyncOp::Copy {
                rel: rel.clone(),
                from_src: true,
            };
            if execute_op(&op, l.as_ref(), r.as_ref()).is_some() {
                err += 1;
            } else {
                ok += 1;
            }
        }
        self.sync_msg = Some(format!("批量复制: 成功 {}，失败 {}", ok, err));
        self.refresh();
    }

    /// 批量操作：删除右侧全部差异/仅右侧文件（镜像清理）
    pub fn run_batch_delete_right(&mut self) {
        let Some(res) = self.result.clone() else {
            return;
        };
        let rels: Vec<String> = res
            .entries
            .iter()
            .filter(|e| matches!(e.status, FileStatus::Differ | FileStatus::RightOnly))
            .map(|e| e.rel.clone())
            .collect();
        if rels.is_empty() {
            self.sync_msg = Some("没有可删除的差异文件".to_string());
            return;
        }
        let (l, r) = match (crate::vfs::open(&self.left), crate::vfs::open(&self.right)) {
            (Ok(l), Ok(r)) => (l, r),
            _ => return,
        };
        let mut ok = 0usize;
        let mut err = 0usize;
        for rel in &rels {
            let op = SyncOp::Delete { rel: rel.clone() };
            if execute_op(&op, l.as_ref(), r.as_ref()).is_some() {
                err += 1;
            } else {
                ok += 1;
            }
        }
        self.sync_msg = Some(format!("批量删除: 成功 {}，失败 {}", ok, err));
        self.refresh();
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
        // 自动刷新：每 2 秒重扫一次（仅在已加载过结果后生效）
        let now = ui.input(|i| i.time);
        if self.result.is_some() && now - self.last_auto_refresh > 2.0 {
            self.last_auto_refresh = now;
            self.refresh();
        }
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
                // B1 状态过滤下拉（BC 显示过滤器）
                let filter_labels = [
                    (ViewFilter::All, "全部"),
                    (ViewFilter::Diff, "仅差异"),
                    (ViewFilter::LeftOnly, "仅左侧"),
                    (ViewFilter::RightOnly, "仅右侧"),
                    (ViewFilter::Moved, "仅移动"),
                    (ViewFilter::Same, "仅相同"),
                ];
                let cur = self.view_filter;
                egui::ComboBox::from_id_salt("dir_view_filter")
                    .selected_text(
                        filter_labels
                            .iter()
                            .find(|(v, _)| *v == cur)
                            .map(|(_, l)| *l)
                            .unwrap_or("全部"),
                    )
                    .show_ui(ui, |ui| {
                        for (v, l) in filter_labels {
                            if ui.selectable_label(cur == v, l).clicked() {
                                self.view_filter = v;
                                self.rebuild_tree();
                            }
                        }
                    });
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
                ui.separator();
                if ui.button("⇄ 同步").clicked() {
                    self.show_sync = !self.show_sync;
                    if self.show_sync && self.sync_plan.is_none() {
                        self.gen_sync_plan();
                    }
                }
                if ui
                    .button("⇱ 手动对齐")
                    .on_hover_text("左右各选一个文件配对对比")
                    .clicked()
                {
                    self.show_align = !self.show_align;
                    if self.show_align {
                        // 默认选中第一个可对齐项
                        if self.align_left.is_none() {
                            self.align_left =
                                self.flat.iter().find(|r| !r.is_dir).map(|r| r.path.clone());
                        }
                    }
                }
                // 批量操作：作用于全部差异文件（only_diff 视图）
                if let Some(r) = &self.result {
                    let has_diff = r.entries.iter().any(|e| e.status != FileStatus::Same);
                    if has_diff {
                        ui.separator();
                        if ui
                            .button("批量复制→右")
                            .on_hover_text("把全部差异/仅左侧文件复制到右侧")
                            .clicked()
                        {
                            self.run_batch_copy_to_right();
                        }
                        if ui
                            .button("批量删除右侧")
                            .on_hover_text("删除右侧全部差异文件")
                            .clicked()
                        {
                            self.run_batch_delete_right();
                        }
                    }
                }
                // 选中文件单项操作
                if let Some(rel) = self.selected_rel() {
                    ui.separator();
                    ui.label(format!("选中: {}", rel));
                    if ui.button("→ 复制到右").clicked() {
                        let op = SyncOp::Copy {
                            rel: rel.clone(),
                            from_src: true,
                        };
                        self.run_single_op(op);
                    }
                    if ui.button("← 复制到左").clicked() {
                        let op = SyncOp::Copy {
                            rel: rel.clone(),
                            from_src: false,
                        };
                        self.run_single_op(op);
                    }
                    if ui.button("删除右侧").clicked() {
                        let op = SyncOp::Delete { rel: rel.clone() };
                        self.run_single_op(op);
                    }
                    if ui.button("删除左侧").clicked() {
                        // 删除左侧 = 把右侧当源、左侧当目标执行 Delete
                        let (l, r) =
                            match (crate::vfs::open(&self.right), crate::vfs::open(&self.left)) {
                                (Ok(r), Ok(l)) => (l, r),
                                _ => return,
                            };
                        match execute_op(
                            &SyncOp::Delete { rel: rel.clone() },
                            l.as_ref(),
                            r.as_ref(),
                        ) {
                            Some(e) => self.sync_msg = Some(format!("操作失败: {}", e)),
                            None => {
                                self.sync_msg = Some(format!("完成: 删除 {}", rel));
                                self.refresh();
                            }
                        }
                    }
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

        // 手动对齐弹窗：左右各选一个文件，配对打开并排 diff
        if self.show_align {
            let mut keep = true;
            let mut open_req: Option<(String, String)> = None;
            let mut close_req = false;
            egui::Window::new("手动对齐")
                .collapsible(false)
                .resizable(true)
                .default_size([520.0, 360.0])
                .open(&mut keep)
                .show(ui.ctx(), |ui| {
                    ui.label(
                        "左侧与右侧各选一个文件，点击「打开对比」配对比较（支持不同文件名）。",
                    );
                    ui.separator();
                    ui.horizontal(|ui| {
                        // 左侧文件列表（仅差异/仅左侧）
                        ui.group(|ui| {
                            ui.label("左侧");
                            egui::ScrollArea::vertical()
                                .max_height(240.0)
                                .show(ui, |ui| {
                                    let entries: Vec<String> = self
                                        .result
                                        .as_ref()
                                        .map(|r| {
                                            r.entries
                                                .iter()
                                                .filter(|e| {
                                                    !matches!(
                                                        e.status,
                                                        FileStatus::Same | FileStatus::RightOnly
                                                    )
                                                })
                                                .map(|e| e.rel.clone())
                                                .collect()
                                        })
                                        .unwrap_or_default();
                                    for rel in entries {
                                        let sel = self.align_left.as_deref() == Some(rel.as_str());
                                        if ui.selectable_label(sel, &rel).clicked() {
                                            self.align_left = Some(rel);
                                        }
                                    }
                                });
                        });
                        ui.group(|ui| {
                            ui.label("右侧");
                            egui::ScrollArea::vertical()
                                .max_height(240.0)
                                .show(ui, |ui| {
                                    let entries: Vec<String> = self
                                        .result
                                        .as_ref()
                                        .map(|r| {
                                            r.entries
                                                .iter()
                                                .filter(|e| {
                                                    !matches!(
                                                        e.status,
                                                        FileStatus::Same | FileStatus::LeftOnly
                                                    )
                                                })
                                                .map(|e| e.rel.clone())
                                                .collect()
                                        })
                                        .unwrap_or_default();
                                    for rel in entries {
                                        let sel = self.align_right.as_deref() == Some(rel.as_str());
                                        if ui.selectable_label(sel, &rel).clicked() {
                                            self.align_right = Some(rel);
                                        }
                                    }
                                });
                        });
                    });
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("打开对比").clicked() {
                            if let (Some(l), Some(r)) = (&self.align_left, &self.align_right) {
                                open_req = Some((l.clone(), r.clone()));
                            }
                        }
                        if ui.button(t(I18nKey::Close)).clicked() {
                            close_req = true;
                        }
                    });
                });
            if let Some((l, r)) = open_req {
                self.open_pair = Some((l, r));
                self.show_align = false;
            }
            if close_req || !keep {
                self.show_align = false;
            }
        }

        // 同步面板（右侧浮窗）
        if self.show_sync {
            let mut keep = true;
            egui::Window::new("同步")
                .collapsible(false)
                .resizable(true)
                .default_size([420.0, 420.0])
                .open(&mut keep)
                .show(ui.ctx(), |ui| {
                    ui.horizontal(|ui| {
                        ui.label("模式");
                        for m in ["update", "mirror", "two-way"] {
                            if ui.selectable_label(self.sync_mode == m, m).clicked() {
                                self.sync_mode = m.to_string();
                            }
                        }
                    });
                    ui.horizontal(|ui| {
                        if ui
                            .checkbox(&mut self.compare_content, t(I18nKey::ContentHash))
                            .changed()
                        {
                            // 内容哈希变化只影响下一次生成计划
                        }
                        if ui.button("生成计划").clicked() {
                            self.gen_sync_plan();
                        }
                        if ui.button("全选").clicked() {
                            if let Some(plan) = &self.sync_plan {
                                self.sync_checked.clear();
                                for (i, op) in plan.iter().enumerate() {
                                    if !matches!(op, SyncOp::Skip { .. } | SyncOp::Conflict { .. })
                                    {
                                        self.sync_checked.insert(i);
                                    }
                                }
                            }
                        }
                        if ui.button("执行勾选").clicked() {
                            self.run_sync_checked();
                        }
                    });
                    if let Some(msg) = &self.sync_msg {
                        ui.colored_label(Color32::from_rgb(230, 180, 80), msg);
                    }
                    ui.separator();
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        if let Some(plan) = &self.sync_plan {
                            if plan.is_empty() {
                                ui.label("两侧已一致，无需同步");
                            }
                            for (i, op) in plan.iter().enumerate() {
                                ui.horizontal(|ui| {
                                    let mut checked = self.sync_checked.contains(&i);
                                    if ui
                                        .checkbox(&mut checked, "")
                                        .on_disabled_hover_text("跳过/冲突项不可执行")
                                        .changed()
                                    {
                                        if checked {
                                            self.sync_checked.insert(i);
                                        } else {
                                            self.sync_checked.remove(&i);
                                        }
                                    }
                                    ui.label(op.tag());
                                    ui.label(op.describe());
                                });
                            }
                        } else {
                            ui.label("点击「生成计划」预览同步操作");
                        }
                    });
                });
            if !keep {
                self.show_sync = false;
            }
        }

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
                            // 右键菜单：复制路径 / 打开所在位置 / 系统应用打开
                            if resp.secondary_clicked() {
                                self.selected = Some(idx);
                            }
                            resp.context_menu(|ui| {
                                let full_l = std::path::Path::new(&self.left).join(&e.rel);
                                let full_r = std::path::Path::new(&self.right).join(&e.rel);
                                if ui.button("复制左侧路径").clicked() {
                                    ui.ctx().copy_text(full_l.to_string_lossy().into_owned());
                                    ui.close();
                                }
                                if ui.button("复制右侧路径").clicked() {
                                    ui.ctx().copy_text(full_r.to_string_lossy().into_owned());
                                    ui.close();
                                }
                                ui.separator();
                                if ui.button("打开左侧文件").clicked() {
                                    open_with_system_app(&full_l.to_string_lossy());
                                    ui.close();
                                }
                                if ui.button("打开右侧文件").clicked() {
                                    open_with_system_app(&full_r.to_string_lossy());
                                    ui.close();
                                }
                                ui.separator();
                                if ui.button("在对比中打开 (Enter)").clicked() {
                                    pending_open = Some(e.rel.clone());
                                    ui.close();
                                }
                            });
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

/// 用系统默认应用打开文件/目录（跨平台：macOS open / Windows explorer / Linux xdg-open）
fn open_with_system_app(path: &str) {
    #[cfg(target_os = "macos")]
    let cmd = "open";
    #[cfg(target_os = "windows")]
    let cmd = "explorer";
    #[cfg(all(unix, not(target_os = "macos")))]
    let cmd = "xdg-open";
    #[cfg(not(any(unix, windows)))]
    let cmd = "open";
    let _ = std::process::Command::new(cmd).arg(path).spawn();
}

fn basename(p: &str) -> String {
    std::path::Path::new(p)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.to_string())
}
