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
    /// B1 F2 重命名：目标文件的相对路径
    pub(crate) rename_target: Option<String>,
    /// B1 F2 重命名：输入缓冲区
    pub(crate) rename_buf: String,
    /// B2 过滤/显示面板：是否展开（左侧 SidePanel）
    pub(crate) show_filter_panel: bool,
    /// B2 扩展名过滤（逗号分隔，如 "txt,rs"；空 = 全部）
    pub(crate) ext_filter: String,
    /// B2 大小下限（字节，空 = 不限）
    pub(crate) min_size: String,
    /// B2 大小上限（字节，空 = 不限）
    pub(crate) max_size: String,
    /// B2 修改时间下限（YYYY-MM-DD，空 = 不限）
    pub(crate) mtime_from: String,
    /// B2 修改时间上限（YYYY-MM-DD，空 = 不限）
    pub(crate) mtime_to: String,
    /// B2 后台任务（对比/同步在独立线程执行，UI 不卡顿）
    pub bg: Option<BgTask>,
    /// P36-D2：右键「排除」的文件相对路径集合（会话级）
    pub(crate) hidden: HashSet<String>,
}

/// B2 后台任务：线程 + 结果通道 + 暂停/取消标志 + 进度
pub struct BgTask {
    /// 任务标题（如「目录对比」「同步」）
    pub label: String,
    /// 结果通道（UI 线程每帧 poll）
    pub rx: std::sync::mpsc::Receiver<BgResult>,
    /// 暂停标志（true = 暂停，线程忙等）
    pub pause: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// 取消标志（true = 终止）
    pub cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// 已完成项数（进度显示）
    pub done: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    /// 总项数（0 = 不确定进度）
    pub total: usize,
}

impl BgTask {
    /// 暂停切换：返回新的暂停状态
    pub fn toggle_pause(&self) -> bool {
        let cur = self.pause.load(std::sync::atomic::Ordering::SeqCst);
        self.pause.store(!cur, std::sync::atomic::Ordering::SeqCst);
        !cur
    }

    pub fn request_cancel(&self) {
        self.cancel.store(true, std::sync::atomic::Ordering::SeqCst);
    }
}

/// B2 后台任务回传结果
pub enum BgResult {
    /// 目录对比完成
    Compare(Result<CompareResult, String>),
    /// 同步完成
    SyncDone { ok: usize, err: usize },
}

/// 执行勾选的同步操作（纯逻辑，后台线程与同步版共用）。
/// 返回 (成功数, 失败数)；支持取消/暂停（AtomicBool 标志）。
fn execute_sync_ops(
    plan: &[SyncOp],
    checked: &[usize],
    l: &dyn crate::vfs::Vfs,
    r: &dyn crate::vfs::Vfs,
    cancel: &std::sync::atomic::AtomicBool,
    pause: &std::sync::atomic::AtomicBool,
    done: &std::sync::atomic::AtomicUsize,
) -> (usize, usize) {
    let mut n_ok = 0usize;
    let mut n_err = 0usize;
    for (k, i) in checked.iter().enumerate() {
        if cancel.load(std::sync::atomic::Ordering::SeqCst) {
            break;
        }
        // 暂停：忙等直到继续或取消
        while pause.load(std::sync::atomic::Ordering::SeqCst)
            && !cancel.load(std::sync::atomic::Ordering::SeqCst)
        {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        if cancel.load(std::sync::atomic::Ordering::SeqCst) {
            break;
        }
        match execute_op(&plan[*i], l, r) {
            Some(_) => n_err += 1,
            None => n_ok += 1,
        }
        done.store(k + 1, std::sync::atomic::Ordering::SeqCst);
    }
    (n_ok, n_err)
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
            rename_target: None,
            rename_buf: String::new(),
            hidden: HashSet::new(),
            show_filter_panel: false,
            ext_filter: String::new(),
            min_size: String::new(),
            max_size: String::new(),
            mtime_from: String::new(),
            mtime_to: String::new(),
            bg: None,
        }
    }

    pub fn title(&self) -> String {
        fmt(
            I18nKey::DirTitle,
            &[&basename(&self.left), &basename(&self.right)],
        )
    }

    pub fn refresh(&mut self) {
        // P34：空路径守卫（空会话）——两侧均为空时不扫描，交由空状态 UI 处理
        if self.left.is_empty() && self.right.is_empty() {
            self.result = None;
            self.error = None;
            self.flat.clear();
            return;
        }
        // B2：后台线程执行对比，UI 不卡顿（大目录）
        if self.bg.is_some() {
            return;
        }
        let filter = match Filter::new(&split_globs(&self.includes), &split_globs(&self.excludes)) {
            Ok(f) => f,
            Err(e) => {
                self.error = Some(fmt(I18nKey::FilterError, &[&e.to_string()]));
                self.result = None;
                self.flat.clear();
                return;
            }
        };
        let left = self.left.clone();
        let right = self.right.clone();
        let compare_content = self.compare_content;
        let (tx, rx) = std::sync::mpsc::channel();
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let pause = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let done = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let cancel2 = cancel.clone();
        let pause2 = pause.clone();
        let _handle = std::thread::spawn(move || {
            let result = compare_dirs(
                std::path::Path::new(&left),
                std::path::Path::new(&right),
                &filter,
                compare_content,
                true,
            );
            // 取消后丢弃结果
            if cancel2.load(std::sync::atomic::Ordering::SeqCst) {
                return;
            }
            let _ = pause2;
            let _ = tx.send(BgResult::Compare(result.map_err(|e| e.to_string())));
        });
        self.bg = Some(BgTask {
            label: t(I18nKey::Refresh).to_string(),
            rx,
            pause,
            cancel,
            done,
            total: 0,
        });
    }

    /// P36-D1：交换左右两侧目录（BC 会话菜单「交换两边」）
    pub fn swap_sides(&mut self) {
        std::mem::swap(&mut self.left, &mut self.right);
        self.refresh();
    }

    // ---- P36-D2：逐文件操作（BC 操作菜单「复制到边/删除/排除」）----

    /// 复制单个文件到另一侧（to_right=true：左→右；false：右→左）
    pub fn copy_single(&mut self, rel: &str, to_right: bool) {
        let (l, r) = match (crate::vfs::open(&self.left), crate::vfs::open(&self.right)) {
            (Ok(l), Ok(r)) => (l, r),
            _ => return,
        };
        let op = SyncOp::Copy {
            rel: rel.to_string(),
            src_rel: None,
            from_src: to_right,
        };
        let err = execute_op(&op, l.as_ref(), r.as_ref());
        self.sync_msg = Some(match err {
            Some(e) => format!("复制失败: {}", e),
            None => format!("已复制: {}", basename(rel)),
        });
        self.refresh_sync();
    }

    /// 删除单个文件（delete_right=true：删右侧；false：删左侧）
    pub fn delete_single(&mut self, rel: &str, delete_right: bool) {
        let (l, r) = match (crate::vfs::open(&self.left), crate::vfs::open(&self.right)) {
            (Ok(l), Ok(r)) => (l, r),
            _ => return,
        };
        let op = SyncOp::Delete {
            rel: rel.to_string(),
        };
        let err = if delete_right {
            execute_op(&op, l.as_ref(), r.as_ref())
        } else {
            execute_op(&op, r.as_ref(), l.as_ref())
        };
        self.sync_msg = Some(match err {
            Some(e) => format!("删除失败: {}", e),
            None => format!("已删除: {}", basename(rel)),
        });
        self.refresh_sync();
    }

    /// 从视图排除该文件（会话级，rebuild_tree 时过滤）
    pub fn exclude(&mut self, rel: &str) {
        self.hidden.insert(rel.to_string());
        self.rebuild_tree();
    }

    /// P34：打开左侧目录（空会话填充）
    pub fn open_left_dir(&mut self) {
        if let Some(p) = super::pick_dir() {
            self.left = p;
            self.refresh();
        }
    }

    /// P34：打开右侧目录（空会话填充）
    pub fn open_right_dir(&mut self) {
        if let Some(p) = super::pick_dir() {
            self.right = p;
            self.refresh();
        }
    }

    /// B2：每帧轮询后台任务结果，完成后应用
    pub fn poll_bg(&mut self) {
        let Some(bg) = &self.bg else { return };
        match bg.rx.try_recv() {
            Ok(BgResult::Compare(Ok(r))) => {
                for w in &r.warnings {
                    self.error = Some(w.clone());
                }
                self.result = Some(r);
                self.bg = None;
                self.rebuild_tree();
            }
            Ok(BgResult::Compare(Err(e))) => {
                self.error = Some(fmt(I18nKey::ScanFailed, &[&e]));
                self.result = None;
                self.bg = None;
                self.rebuild_tree();
            }
            Ok(BgResult::SyncDone { ok, err }) => {
                self.sync_msg = Some(format!("同步完成: 成功 {} 项，失败 {} 项", ok, err));
                self.sync_plan = None;
                self.sync_checked.clear();
                self.bg = None;
                self.refresh();
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                // 线程已退出但未发送结果（如被取消）
                self.bg = None;
            }
        }
    }

    /// 同步刷新（测试/无头场景用）：直接执行对比并立即应用结果
    pub fn refresh_sync(&mut self) {
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
    pub(crate) fn rebuild_tree(&mut self) {
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
        // P36-D2：右键「排除」的文件（会话级，重建时过滤）
        if !self.hidden.is_empty() {
            visible.retain(|e| !self.hidden.contains(&e.rel));
        }
        // B2：扩展名过滤（逗号分隔，如 "txt,rs"）
        let exts: Vec<String> = self
            .ext_filter
            .split(',')
            .map(|s| s.trim().trim_start_matches('.').to_lowercase())
            .filter(|s| !s.is_empty())
            .collect();
        if !exts.is_empty() {
            visible.retain(|e| {
                let ext = std::path::Path::new(&e.rel)
                    .extension()
                    .map(|s| s.to_string_lossy().to_lowercase())
                    .unwrap_or_default();
                exts.iter().any(|x| x == &ext)
            });
        }
        // B2：大小范围（字节；取存在侧的最大 size）
        let min_sz = self.min_size.trim().parse::<u64>().ok();
        let max_sz = self.max_size.trim().parse::<u64>().ok();
        if min_sz.is_some() || max_sz.is_some() {
            visible.retain(|e| {
                let sz = e
                    .left
                    .as_ref()
                    .map(|m| m.size)
                    .into_iter()
                    .chain(e.right.as_ref().map(|m| m.size))
                    .max()
                    .unwrap_or(0);
                min_sz.map(|v| sz >= v).unwrap_or(true) && max_sz.map(|v| sz <= v).unwrap_or(true)
            });
        }
        // B2：修改时间范围（YYYY-MM-DD → 当日零点 Unix 秒，取存在侧最新 mtime）
        let t_from = parse_date_secs(&self.mtime_from);
        let t_to = parse_date_secs(&self.mtime_to);
        if t_from.is_some() || t_to.is_some() {
            visible.retain(|e| {
                let mt = e
                    .left
                    .as_ref()
                    .map(|m| m.mtime)
                    .into_iter()
                    .chain(e.right.as_ref().map(|m| m.mtime))
                    .map(|t| {
                        t.duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_secs())
                            .unwrap_or(0)
                    })
                    .max()
                    .unwrap_or(0);
                t_from.map(|v| mt >= v).unwrap_or(true) && t_to.map(|v| mt <= v).unwrap_or(true)
            });
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
            false, // GUI 目录同步暂不支持忽略结构（跨目录配对在 CLI 层）
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

    /// 执行勾选的同步操作（B2 后台线程，支持暂停/取消），完成后重新对比
    pub fn run_sync_checked(&mut self) {
        if self.bg.is_some() {
            return;
        }
        let Some(plan) = self.sync_plan.clone() else {
            self.sync_msg = Some("请先生成计划".to_string());
            return;
        };
        let checked: Vec<usize> = self.sync_checked.iter().copied().collect();
        let left = self.left.clone();
        let right = self.right.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let pause = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let done = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let cancel2 = cancel.clone();
        let pause2 = pause.clone();
        let done2 = done.clone();
        let total = checked.len();
        // dyn Vfs 非 Send，线程内重新打开（每次调用独立建连）
        let _handle = std::thread::spawn(move || {
            let (l, r) = match (crate::vfs::open(&left), crate::vfs::open(&right)) {
                (Ok(l), Ok(r)) => (l, r),
                _ => {
                    let _ = tx.send(BgResult::SyncDone { ok: 0, err: 1 });
                    return;
                }
            };
            let (n_ok, n_err) = execute_sync_ops(
                &plan,
                &checked,
                l.as_ref(),
                r.as_ref(),
                &cancel2,
                &pause2,
                &done2,
            );
            let _ = tx.send(BgResult::SyncDone {
                ok: n_ok,
                err: n_err,
            });
        });
        self.bg = Some(BgTask {
            label: "同步".to_string(),
            rx,
            pause,
            cancel,
            done,
            total,
        });
    }

    /// 同步执行勾选的同步操作（测试/无头场景用），完成后立即刷新
    #[cfg(test)]
    pub fn run_sync_checked_sync(&mut self) {
        let Some(plan) = self.sync_plan.clone() else {
            self.sync_msg = Some("请先生成计划".to_string());
            return;
        };
        let checked: Vec<usize> = self.sync_checked.iter().copied().collect();
        let (l, r) = match (crate::vfs::open(&self.left), crate::vfs::open(&self.right)) {
            (Ok(l), Ok(r)) => (l, r),
            _ => return,
        };
        let noop_cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let noop_pause = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let noop_done = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let (n_ok, n_err) = execute_sync_ops(
            &plan,
            &checked,
            l.as_ref(),
            r.as_ref(),
            &noop_cancel,
            &noop_pause,
            &noop_done,
        );
        self.sync_msg = Some(format!("同步完成: 成功 {} 项，失败 {} 项", n_ok, n_err));
        self.sync_plan = None;
        self.sync_checked.clear();
        self.refresh_sync();
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
                self.refresh_sync();
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
                src_rel: None,
                from_src: true,
            };
            if execute_op(&op, l.as_ref(), r.as_ref()).is_some() {
                err += 1;
            } else {
                ok += 1;
            }
        }
        self.sync_msg = Some(format!("批量复制: 成功 {}，失败 {}", ok, err));
        self.refresh_sync();
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
        self.refresh_sync();
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
        // P36-D3：视图过滤快捷键（BC 显示全部/差异/相同 = 1/2/3；输入框聚焦时不触发）
        // 放在 flat 空检查之前：过滤可能让列表为空，用户需能切回 All
        if !ui.ctx().egui_wants_keyboard_input() {
            let num = if ui.input(|i| i.key_pressed(Key::Num1)) {
                Some(ViewFilter::All)
            } else if ui.input(|i| i.key_pressed(Key::Num2)) {
                Some(ViewFilter::Diff)
            } else if ui.input(|i| i.key_pressed(Key::Num3)) {
                Some(ViewFilter::Same)
            } else {
                None
            };
            if let Some(vf) = num {
                if self.view_filter != vf {
                    self.view_filter = vf;
                    self.rebuild_tree();
                }
            }
        }
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
        // B1：F2 重命名选中文件
        if ui.input(|i| i.key_pressed(Key::F2)) {
            if let Some(rel) = self.selected_rel() {
                self.rename_target = Some(rel.clone());
                self.rename_buf = basename(&rel);
            }
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        // B2：轮询后台任务结果（对比/同步完成时应用）
        self.poll_bg();
        // 自动刷新：每 2 秒重扫一次（仅在已加载过结果且无后台任务时生效）
        let now = ui.input(|i| i.time);
        if self.result.is_some() && self.bg.is_none() && now - self.last_auto_refresh > 2.0 {
            self.last_auto_refresh = now;
            self.refresh();
        }
        egui::Panel::top("dirtab_tools").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                // P31 路径栏：左路径 ⇄ 右路径（弱色只读，BC 观感）
                ui.label(
                    egui::RichText::new(&self.left)
                        .color(ui.visuals().weak_text_color())
                        .monospace(),
                );
                ui.label(egui::RichText::new("⇄").color(ui.visuals().weak_text_color()));
                ui.label(
                    egui::RichText::new(&self.right)
                        .color(ui.visuals().weak_text_color())
                        .monospace(),
                );
                ui.separator();
                // B2 后台任务指示与暂停/取消控制
                if let Some(bg) = &self.bg {
                    let done = bg.done.load(std::sync::atomic::Ordering::SeqCst);
                    let paused = bg.pause.load(std::sync::atomic::Ordering::SeqCst);
                    ui.label(format!(
                        "⏳ {} {}",
                        bg.label,
                        if bg.total > 0 {
                            format!("{done}/{}", bg.total)
                        } else {
                            "…".to_string()
                        }
                    ));
                    if bg.total > 0 {
                        let frac = (done as f32 / bg.total as f32).clamp(0.0, 1.0);
                        ui.add(egui::ProgressBar::new(frac).desired_width(120.0));
                    } else {
                        ui.spinner();
                    }
                    if ui
                        .button(if paused { "▶ 继续" } else { "⏸ 暂停" })
                        .clicked()
                    {
                        bg.toggle_pause();
                    }
                    if ui.button("✕ 取消").clicked() {
                        bg.request_cancel();
                    }
                    ui.separator();
                }
                if ui
                    .button(format!("⟳ {}", t(I18nKey::Refresh)))
                    .on_hover_text("刷新 (F5)")
                    .clicked()
                {
                    self.refresh();
                }
                // P36-D1：交换左右两侧（BC 会话菜单「交换两边」）
                if ui
                    .button(format!("⇄ {}", t(I18nKey::SwapSides)))
                    .on_hover_text("交换左右两侧目录")
                    .clicked()
                {
                    self.swap_sides();
                }
                // B2：过滤/显示面板开关（左侧 SidePanel）
                if ui
                    .selectable_label(self.show_filter_panel, "⛭ 过滤")
                    .on_hover_text("扩展名/大小/时间过滤面板")
                    .clicked()
                {
                    self.show_filter_panel = !self.show_filter_panel;
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
                if ui
                    .button("⇄ 同步")
                    .on_hover_text("生成同步计划（update/mirror/two-way）")
                    .clicked()
                {
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
                            .button("⧉ 批量复制→右")
                            .on_hover_text("把全部差异/仅左侧文件复制到右侧")
                            .clicked()
                        {
                            self.run_batch_copy_to_right();
                        }
                        if ui
                            .button("🗑 批量删除右侧")
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
                            src_rel: None,
                            from_src: true,
                        };
                        self.run_single_op(op);
                    }
                    if ui.button("← 复制到左").clicked() {
                        let op = SyncOp::Copy {
                            rel: rel.clone(),
                            src_rel: None,
                            from_src: false,
                        };
                        self.run_single_op(op);
                    }
                    if ui.button("🗑 删除右侧").clicked() {
                        let op = SyncOp::Delete { rel: rel.clone() };
                        self.run_single_op(op);
                    }
                    if ui.button("🗑 删除左侧").clicked() {
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

        // B1：F2 重命名弹窗（选中文件 → 输入新名 → 重命名左侧或右侧实际文件）
        if let Some(rel) = self.rename_target.clone() {
            let mut keep = true;
            let mut do_rename = false;
            let mut cancel_req = false;
            egui::Window::new("重命名 (F2)")
                .collapsible(false)
                .resizable(false)
                .default_size([320.0, 100.0])
                .open(&mut keep)
                .show(ui.ctx(), |ui| {
                    ui.label(format!("重命名: {}", rel));
                    ui.text_edit_singleline(&mut self.rename_buf);
                    ui.horizontal(|ui| {
                        if ui.button("确定").clicked() || ui.input(|i| i.key_pressed(Key::Enter))
                        {
                            do_rename = true;
                        }
                        if ui.button("取消").clicked() {
                            cancel_req = true;
                        }
                    });
                });
            let new_name = self.rename_buf.trim().to_string();
            if do_rename && !new_name.is_empty() && new_name != basename(&rel) {
                let new_rel = {
                    let parent = std::path::Path::new(&rel)
                        .parent()
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    if parent.is_empty() {
                        new_name.clone()
                    } else {
                        format!("{}/{}", parent, new_name)
                    }
                };
                let (l, r) = match (crate::vfs::open(&self.left), crate::vfs::open(&self.right)) {
                    (Ok(l), Ok(r)) => (l, r),
                    _ => {
                        self.sync_msg = Some("无法打开目录进行重命名".to_string());
                        self.rename_target = None;
                        return;
                    }
                };
                let err = l
                    .rename(&rel, &new_rel)
                    .or_else(|_| r.rename(&rel, &new_rel))
                    .err()
                    .map(|e| e.to_string());
                self.sync_msg = Some(match err {
                    None => format!("重命名: {} → {}", rel, new_rel),
                    Some(e) => format!("重命名失败: {}", e),
                });
                self.refresh_sync();
            }
            // 仅当确认/取消/关闭时关闭弹窗；否则保持打开等待输入
            if do_rename || cancel_req || !keep {
                self.rename_target = None;
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

        // B2：左侧过滤/显示面板（可折叠）——扩展名/大小/时间范围，与工具栏过滤联动
        if self.show_filter_panel {
            egui::Panel::left("dir_filter_panel")
                .resizable(true)
                .default_size(230.0)
                .show(ui, |ui| {
                    ui.heading("过滤/显示");
                    ui.separator();
                    ui.label("扩展名（逗号分隔）");
                    let mut ext = self.ext_filter.clone();
                    let r = ui.add(
                        egui::TextEdit::singleline(&mut ext)
                            .hint_text("txt,rs,md")
                            .desired_width(200.0),
                    );
                    if r.changed() {
                        self.ext_filter = ext;
                        self.rebuild_tree();
                    }
                    ui.add_space(6.0);
                    ui.label("大小范围（字节）");
                    ui.horizontal(|ui| {
                        ui.label("最小");
                        let mut mn = self.min_size.clone();
                        let r1 = ui.add(
                            egui::TextEdit::singleline(&mut mn)
                                .hint_text("0")
                                .desired_width(70.0),
                        );
                        if r1.changed() {
                            self.min_size = mn;
                            self.rebuild_tree();
                        }
                        ui.label("最大");
                        let mut mx = self.max_size.clone();
                        let r2 = ui.add(
                            egui::TextEdit::singleline(&mut mx)
                                .hint_text("不限")
                                .desired_width(70.0),
                        );
                        if r2.changed() {
                            self.max_size = mx;
                            self.rebuild_tree();
                        }
                    });
                    ui.add_space(6.0);
                    ui.label("修改时间（YYYY-MM-DD）");
                    ui.horizontal(|ui| {
                        ui.label("从");
                        let mut f = self.mtime_from.clone();
                        let r3 = ui.add(
                            egui::TextEdit::singleline(&mut f)
                                .hint_text("2026-01-01")
                                .desired_width(90.0),
                        );
                        if r3.changed() {
                            self.mtime_from = f;
                            self.rebuild_tree();
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("到");
                        let mut to = self.mtime_to.clone();
                        let r4 = ui.add(
                            egui::TextEdit::singleline(&mut to)
                                .hint_text("2026-12-31")
                                .desired_width(90.0),
                        );
                        if r4.changed() {
                            self.mtime_to = to;
                            self.rebuild_tree();
                        }
                    });
                    ui.add_space(8.0);
                    if ui.button("清除全部过滤").clicked() {
                        self.ext_filter.clear();
                        self.min_size.clear();
                        self.max_size.clear();
                        self.mtime_from.clear();
                        self.mtime_to.clear();
                        self.rebuild_tree();
                    }
                    ui.separator();
                    if let Some(r) = &self.result {
                        ui.label(format!(
                            "共 {} 项 / 当前显示 {} 项",
                            r.entries.len(),
                            self.flat.len()
                        ));
                    }
                });
        }

        egui::CentralPanel::default().show(ui, |ui| {
            if self.result.is_none() && self.error.is_none() {
                self.refresh();
            }
            // P34：空会话（两侧均未选择目录）→ 显示打开入口 + 拖拽提示
            if self.left.is_empty() && self.right.is_empty() {
                ui.centered_and_justified(|ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new(t(I18nKey::DirEmpty))
                                .size(16.0)
                                .color(ui.visuals().weak_text_color()),
                        );
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            if ui.button(t(I18nKey::OpenLeftDir)).clicked() {
                                self.open_left_dir();
                            }
                            if ui.button(t(I18nKey::OpenRightDir)).clicked() {
                                self.open_right_dir();
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
            // P36-D2：逐文件操作请求（右键菜单收集，闭包外执行）
            let mut copy_req: Option<(String, bool)> = None; // (rel, to_right)
            let mut delete_req: Option<(String, bool)> = None; // (rel, delete_right)
            let mut exclude_req: Option<String> = None;
            let mut scroll_to_sel = self.scroll_to_selected;
            self.scroll_to_selected = false;
            let selected = self.selected;

            // P33：BC 式列头（名称 | 大小 | 修改时间，两侧对齐），固定不随行滚动
            {
                let head_h = 22.0;
                let head_bg = if ui.visuals().dark_mode {
                    Color32::from_gray(42)
                } else {
                    Color32::from_rgb(251, 252, 252)
                };
                let (h_rect, _) = ui.allocate_exact_size(
                    Vec2::new(ui.available_width(), head_h),
                    egui::Sense::hover(),
                );
                paint_bg(ui, h_rect, Some(head_bg));
                let head_fg = ui.visuals().weak_text_color();
                let font = egui::FontId::proportional(12.0);
                // 左列：名称（BC: Name）
                ui.painter().text(
                    Pos2::new(h_rect.left() + 8.0, h_rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    "名称",
                    font.clone(),
                    head_fg,
                );
                // 右列：大小 / 修改时间（BC: Size / Modified）
                ui.painter().text(
                    Pos2::new(h_rect.right() - 150.0, h_rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    "大小",
                    font.clone(),
                    head_fg,
                );
                ui.painter().text(
                    Pos2::new(h_rect.right() - 8.0, h_rect.center().y),
                    egui::Align2::RIGHT_CENTER,
                    "修改时间",
                    font,
                    head_fg,
                );
                ui.separator();
            }

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
                        // BC 风格：目录名用文件夹色（浅蓝），与文件区分
                        let dir_color = if ui.visuals().dark_mode {
                            Color32::from_rgb(140, 180, 235)
                        } else {
                            Color32::from_rgb(60, 110, 190)
                        };
                        ui.painter().text(
                            Pos2::new(x0, rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            arrow,
                            egui::FontId::proportional(12.0),
                            dir_color,
                        );
                        ui.painter().text(
                            Pos2::new(x0 + 16.0, rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            &row.name,
                            egui::FontId::proportional(14.0),
                            dir_color,
                        );
                        if resp.clicked() {
                            pending_toggle = Some(row.path.clone());
                        }
                    } else if let Some(ei) = row.entry {
                        if let Some(e) = self.result.as_ref().and_then(|r| r.entries.get(ei)) {
                            let letter = e.status.letter();
                            let color = status_color(ui, letter);
                            // P31 状态徽标：圆形底色 + 字母（替代纯文本 [L]）
                            let badge_r = 9.0;
                            let badge_c = Pos2::new(x0 + badge_r, rect.center().y);
                            ui.painter().circle_filled(
                                badge_c,
                                badge_r,
                                color.gamma_multiply(0.25),
                            );
                            ui.painter().text(
                                badge_c,
                                egui::Align2::CENTER_CENTER,
                                letter.to_string(),
                                egui::FontId::monospace(12.0),
                                color,
                            );
                            ui.painter().text(
                                Pos2::new(x0 + 24.0, rect.center().y),
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
                                if ui.button("打开所在位置（左）").clicked() {
                                    super::common::reveal_in_file_manager(
                                        &full_l.to_string_lossy(),
                                    );
                                    ui.close();
                                }
                                if ui.button("打开所在位置（右）").clicked() {
                                    super::common::reveal_in_file_manager(
                                        &full_r.to_string_lossy(),
                                    );
                                    ui.close();
                                }
                                ui.separator();
                                if ui.button("在对比中打开 (Enter)").clicked() {
                                    pending_open = Some(e.rel.clone());
                                    ui.close();
                                }
                                // P36-D2：逐文件操作（BC 操作菜单「复制到边/删除/排除」）
                                ui.separator();
                                let rel = e.rel.clone();
                                let st = e.status;
                                if matches!(st, FileStatus::LeftOnly | FileStatus::Differ)
                                    && ui.button("→ 复制到右侧").clicked()
                                {
                                    copy_req = Some((rel.clone(), true));
                                    ui.close();
                                }
                                if matches!(st, FileStatus::RightOnly | FileStatus::Differ)
                                    && ui.button("← 复制到左侧").clicked()
                                {
                                    copy_req = Some((rel.clone(), false));
                                    ui.close();
                                }
                                if e.right.is_some() && ui.button("🗑 删除右侧").clicked() {
                                    delete_req = Some((rel.clone(), true));
                                    ui.close();
                                }
                                if e.left.is_some() && ui.button("🗑 删除左侧").clicked() {
                                    delete_req = Some((rel.clone(), false));
                                    ui.close();
                                }
                                if ui.button("🙈 排除").clicked() {
                                    exclude_req = Some(rel.clone());
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
            // P36-D2：逐文件操作（复制到边/删除/排除）
            if let Some((rel, to_right)) = copy_req {
                self.copy_single(&rel, to_right);
            }
            if let Some((rel, dr)) = delete_req {
                self.delete_single(&rel, dr);
            }
            if let Some(rel) = exclude_req {
                self.exclude(&rel);
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
fn basename(p: &str) -> String {
    std::path::Path::new(p)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.to_string())
}

/// B2：解析 YYYY-MM-DD → 当日零点 Unix 秒；格式非法返回 None
pub(crate) fn parse_date_secs(s: &str) -> Option<u64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 3 {
        return None;
    }
    let y: u64 = parts[0].parse().ok()?;
    let m: u64 = parts[1].parse().ok()?;
    let d: u64 = parts[2].parse().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    // 公历日序号 → Unix 秒（1970-01-01 起）
    let days = days_from_civil(y as i64, m as i64, d as i64)?;
    Some((days as u64) * 86400)
}

/// 公历日期 → 1970-01-01 起的天数（Howard Hinnant 算法）
fn days_from_civil(y: i64, m: i64, d: i64) -> Option<i64> {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146097 + doe - 719468)
}
