//! CSV 表格对比标签页（P29）：并排渲染左右表格，行对齐 + 单元格级差异高亮。
//!
//! 对标 Beyond Compare 表格视图：
//! - 行按主键（或行号）对齐，行级状态着色（L/R/M/S）
//! - 修改行的变化列在左右两侧同时着色
//! - 工具栏：主键下拉 / 分隔符 / 显示相同 / 状态过滤
//! - 表头点击排序（纯显示排序，不改对齐数据）

use super::common::*;
use crate::csvcmp::{align_tables, serialize_csv, RowStats, RowStatus, Table};
use crate::i18n::{fmt, t, Key as I18nKey};
use eframe::egui::{self, Pos2, Rect, Vec2};

/// CSV 表格行过滤（对齐 BC 显示过滤器；CSV 无 Moved 状态）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CsvFilter {
    All,
    Diff,
    LeftOnly,
    RightOnly,
    Modified,
    Same,
}

impl CsvFilter {
    fn matches(self, st: RowStatus) -> bool {
        match self {
            CsvFilter::All => true,
            CsvFilter::Diff => st != RowStatus::Same,
            CsvFilter::LeftOnly => st == RowStatus::LeftOnly,
            CsvFilter::RightOnly => st == RowStatus::RightOnly,
            CsvFilter::Modified => st == RowStatus::Modified,
            CsvFilter::Same => st == RowStatus::Same,
        }
    }
}

/// 排序列（左侧或右侧的列索引）
#[derive(Debug, Clone, Copy)]
struct SortKey {
    side: bool, // true = 左侧
    col: usize,
    asc: bool,
}

/// CSV 表格标签页
pub(crate) struct CsvTab {
    pub(crate) left: String,
    pub(crate) right: String,
    table_a: Option<Table>,
    table_b: Option<Table>,
    aligned: Vec<crate::csvcmp::AlignedRow>,
    stats: RowStats,
    /// P56-7：最近一次表格对齐耗时（秒，状态栏显示）
    pub(crate) elapsed_secs: Option<f32>,
    /// 当前主键列名（空 = 行号对齐）
    key: String,
    /// 分隔符显示值（"," / "\\t"）
    delimiter: String,
    pub(crate) show_same: bool,
    pub(crate) filter: CsvFilter,
    sort: Option<SortKey>,
    error: Option<String>,
    /// P37-1c：当前选中单元格（对齐行下标, 列号）
    pub(crate) selected: Option<(usize, usize)>,
    /// P37-1c：隐藏所有行都相同的列（BC Hide Same Columns）
    pub(crate) hide_same_cols: bool,
    /// P37-1c：列宽自适应（BC Adjust Column Sizes to Fit）
    pub(crate) auto_fit: bool,
    /// P37-1l：修改单元格弹窗开关
    show_cell_edit: bool,
    /// P37-1l：修改单元格输入缓冲
    cell_edit_buf: String,
    /// P44-6：排序对话框开关（BC 编辑>排序...）
    show_sort_dialog: bool,
}

impl CsvTab {
    pub(crate) fn new(left: &str, right: &str) -> Self {
        let mut t = CsvTab {
            left: left.to_string(),
            right: right.to_string(),
            table_a: None,
            table_b: None,
            aligned: Vec::new(),
            stats: RowStats::default(),
            elapsed_secs: None,
            key: String::new(),
            delimiter: ",".to_string(),
            show_same: false,
            filter: CsvFilter::Diff,
            sort: None,
            error: None,
            selected: None,
            hide_same_cols: false,
            auto_fit: false,
            show_cell_edit: false,
            cell_edit_buf: String::new(),
            show_sort_dialog: false,
        };
        t.reload();
        t
    }

    pub(crate) fn title(&self) -> String {
        fmt(I18nKey::CsvTitle, &[&self.left, &self.right])
    }

    /// 统计信息（P31 状态栏用）
    pub(crate) fn stats(&self) -> RowStats {
        self.stats
    }

    fn delim_char(&self) -> char {
        match self.delimiter.as_str() {
            "\\t" | "tab" => '\t',
            s if s.chars().count() == 1 => s.chars().next().unwrap(),
            _ => ',',
        }
    }

    /// P34：打开左侧文件（空会话填充）
    pub(crate) fn open_left(&mut self) {
        if let Some(p) = super::pick_file() {
            self.left = p;
            self.reload();
        }
    }

    /// P34：打开右侧文件（空会话填充）
    pub(crate) fn open_right(&mut self) {
        if let Some(p) = super::pick_file() {
            self.right = p;
            self.reload();
        }
    }

    /// P34：直接填充两侧并重新加载（拖拽/程序化填充）
    pub(crate) fn load_pair(&mut self, l: &str, r: &str) {
        self.left = l.to_string();
        self.right = r.to_string();
        self.reload();
    }

    /// P34：是否为空会话（两侧均未选择文件）
    pub(crate) fn is_empty(&self) -> bool {
        self.left.is_empty() && self.right.is_empty()
    }

    /// 重新加载：读文件 → 解析 → 对齐
    pub(crate) fn reload(&mut self) {
        self.error = None;
        // P34：空路径守卫（空会话）
        if self.left.is_empty() && self.right.is_empty() {
            self.table_a = None;
            self.table_b = None;
            self.aligned.clear();
            return;
        }
        let delim = self.delim_char();
        let mut read = |p: &str| -> Option<String> {
            // 空路径（拖入单文件/空会话）不读文件，视为空内容
            if p.is_empty() {
                return Some(String::new());
            }
            match std::fs::read_to_string(p) {
                Ok(s) => Some(s),
                Err(e) => {
                    self.error = Some(format!("{}: {e}", p));
                    None
                }
            }
        };
        let (la, lb) = match (read(&self.left), read(&self.right)) {
            (Some(a), Some(b)) => (a, b),
            _ => {
                self.table_a = None;
                self.table_b = None;
                self.aligned.clear();
                return;
            }
        };
        let a = Table::new(&la, delim, false);
        let b = Table::new(&lb, delim, false);
        let key = if self.key.is_empty() {
            None
        } else {
            Some(self.key.as_str())
        };
        let _start = std::time::Instant::now();
        let (aligned, stats) = align_tables(&a, &b, key);
        self.table_a = Some(a);
        self.table_b = Some(b);
        self.aligned = aligned;
        self.stats = stats;
        // P56-7：记录表格对齐耗时
        self.elapsed_secs = Some(_start.elapsed().as_secs_f32());
    }

    /// 可选主键列名列表（去重，保持表头顺序）
    fn key_options(&self) -> Vec<String> {
        let mut v: Vec<String> = Vec::new();
        if let Some(a) = &self.table_a {
            for h in &a.headers {
                if !v.contains(h) {
                    v.push(h.clone());
                }
            }
        }
        v
    }

    /// 可见行索引（过滤 + 排序）
    fn visible_rows(&self) -> Vec<usize> {
        let mut idx: Vec<usize> = (0..self.aligned.len())
            .filter(|&i| {
                let ar = &self.aligned[i];
                (self.show_same || ar.status != RowStatus::Same) && self.filter.matches(ar.status)
            })
            .collect();
        if let Some(sk) = &self.sort {
            idx.sort_by(|&i, &j| {
                let vi = self.sort_val(i, sk);
                let vj = self.sort_val(j, sk);
                let ord = vi.cmp(&vj);
                if sk.asc {
                    ord
                } else {
                    ord.reverse()
                }
            });
        }
        idx
    }

    /// 排序列取值：优先排序列所在侧，缺失行取另一侧同列（保持可比较）
    fn sort_val(&self, aligned_idx: usize, sk: &SortKey) -> String {
        let ar = &self.aligned[aligned_idx];
        let (a_row, b_row) = (
            ar.a_no
                .and_then(|n| self.table_a.as_ref().and_then(|t| t.rows.get(n))),
            ar.b_no
                .and_then(|n| self.table_b.as_ref().and_then(|t| t.rows.get(n))),
        );
        let val = |row: Option<&Vec<String>>| -> String {
            row.and_then(|r| r.get(sk.col)).cloned().unwrap_or_default()
        };
        let primary = if sk.side { a_row } else { b_row };
        if primary.is_some() {
            val(primary)
        } else if sk.side {
            val(b_row)
        } else {
            val(a_row)
        }
    }

    pub(crate) fn col_count(&self) -> usize {
        let n = |t: &Option<Table>| t.as_ref().map(|t| t.headers.len()).unwrap_or(0);
        n(&self.table_a).max(n(&self.table_b))
    }

    /// B3：行数（对齐后行数，与渲染一致）
    pub(crate) fn row_count(&self) -> usize {
        self.aligned.len()
    }

    /// P37-1c：隐藏相同列时，返回「需要显示的列索引列表」（None = 全部显示）
    pub(crate) fn visible_cols(&self) -> Option<Vec<usize>> {
        if !self.hide_same_cols {
            return None;
        }
        let (Some(a), Some(b)) = (&self.table_a, &self.table_b) else {
            return None;
        };
        let ncols = a.headers.len().max(b.headers.len());
        if ncols == 0 {
            return None;
        }
        // 每列：所有对齐行（两侧都有）该列值都相等 → 相同列
        let mut same_col = vec![true; ncols];
        for ar in &self.aligned {
            let (Some(ai), Some(bi)) = (ar.a_no, ar.b_no) else {
                // 仅一侧存在的行：该行所有列都视为不同（避免隐藏差异）
                for c in same_col.iter_mut() {
                    *c = false;
                }
                break;
            };
            let (Some(a_row), Some(b_row)) = (a.rows.get(ai), b.rows.get(bi)) else {
                continue;
            };
            for (ci, same) in same_col.iter_mut().enumerate() {
                let av = a_row.get(ci).map(|s| s.as_str()).unwrap_or("");
                let bv = b_row.get(ci).map(|s| s.as_str()).unwrap_or("");
                if av != bv {
                    *same = false;
                }
            }
        }
        let vis: Vec<usize> = (0..ncols).filter(|&c| !same_col[c]).collect();
        if vis.is_empty() {
            None
        } else {
            Some(vis)
        }
    }

    /// P37-1c：列宽自适应（按表头 + 可见单元格最大宽度；上限 320px，下限 60px）
    fn col_widths(&self, vis_cols: Option<&Vec<usize>>) -> Vec<f32> {
        let (Some(a), Some(b)) = (&self.table_a, &self.table_b) else {
            return vec![110.0];
        };
        let ncols = a.headers.len().max(b.headers.len());
        let mut widths: Vec<f32> = vec![60.0; ncols];
        let mut update = |ci: usize, s: &str| {
            if let Some(v) = vis_cols {
                if !v.contains(&ci) {
                    return;
                }
            }
            let w = (s.chars().count() as f32) * 8.0 + 16.0;
            if w > widths[ci] {
                widths[ci] = w.min(320.0);
            }
        };
        for (ci, h) in a.headers.iter().enumerate() {
            update(ci, h);
        }
        for (ci, h) in b.headers.iter().enumerate() {
            update(ci, h);
        }
        for ar in &self.aligned {
            for (side, no) in [(true, ar.a_no), (false, ar.b_no)] {
                let t = if side { a } else { b };
                if let Some(n) = no {
                    if let Some(row) = t.rows.get(n) {
                        for (ci, cell) in row.iter().enumerate() {
                            update(ci, cell);
                        }
                    }
                }
            }
        }
        widths
    }

    /// P37-1c：复制左侧单元格到右侧（BC Copy Cell to Right Side）
    ///
    /// 从对齐行取左侧值 → 写入右侧表对应行同列 → 序列化写回右侧文件（备份 .bak）→ 重新加载。
    /// 返回是否成功。
    pub(crate) fn copy_cell_right(&mut self) -> bool {
        let Some((aligned_idx, col)) = self.selected else {
            return false;
        };
        let (Some(a), Some(b)) = (&self.table_a, &self.table_b) else {
            return false;
        };
        let Some(ar) = self.aligned.get(aligned_idx) else {
            return false;
        };
        // 需要左侧有值、右侧有对应行
        let (Some(ai), Some(bi)) = (ar.a_no, ar.b_no) else {
            return false;
        };
        let Some(a_row) = a.rows.get(ai) else {
            return false;
        };
        let Some(v) = a_row.get(col) else {
            return false;
        };
        // b 是 & 引用不能 get_mut：克隆右侧行做修改
        let mut b_rows = b.rows.clone();
        let Some(b_row) = b_rows.get_mut(bi) else {
            return false;
        };
        if b_row.len() <= col {
            b_row.resize(col + 1, String::new());
        }
        b_row[col] = v.clone();
        let delim = self.delim_char();
        let out = serialize_csv(
            &Table {
                headers: b.headers.clone(),
                rows: b_rows,
            },
            delim,
        );
        // A2 模式：写回前备份 .bak
        let _ = std::fs::copy(&self.right, format!("{}.bak", self.right));
        if std::fs::write(&self.right, out).is_err() {
            return false;
        }
        self.reload();
        true
    }

    // ---- P37-1l：行列操作（BC 编辑菜单 删除/插入行列、修改单元格） ----

    /// 写回一侧文件（A2 模式 .bak 备份），side=true 左侧
    fn write_side(&self, side: bool, table: &Table) -> bool {
        let path = if side { &self.left } else { &self.right };
        let delim = self.delim_char();
        let out = serialize_csv(table, delim);
        let _ = std::fs::copy(path, format!("{path}.bak"));
        std::fs::write(path, out).is_ok()
    }

    /// 删除选中行（两侧原始行同步删除，写回存在侧的文件）
    pub(crate) fn delete_row(&mut self) -> bool {
        let Some((aligned_idx, _)) = self.selected else {
            return false;
        };
        let (Some(a), Some(b)) = (&self.table_a, &self.table_b) else {
            return false;
        };
        let Some(ar) = self.aligned.get(aligned_idx) else {
            return false;
        };
        let mut a_t = a.clone_table();
        let mut b_t = b.clone_table();
        let mut ok = true;
        if let Some(ai) = ar.a_no {
            if ai < a_t.rows.len() {
                a_t.rows.remove(ai);
                ok &= self.write_side(true, &a_t);
            }
        }
        if let Some(bi) = ar.b_no {
            if bi < b_t.rows.len() {
                b_t.rows.remove(bi);
                ok &= self.write_side(false, &b_t);
            }
        }
        if ok {
            self.reload();
        }
        ok
    }

    /// 在选中行前插入空行（两侧同步插入）
    pub(crate) fn insert_row(&mut self) -> bool {
        let Some((aligned_idx, _)) = self.selected else {
            return false;
        };
        let (Some(a), Some(b)) = (&self.table_a, &self.table_b) else {
            return false;
        };
        let Some(ar) = self.aligned.get(aligned_idx) else {
            return false;
        };
        let ncols = self.col_count();
        let mut a_t = a.clone_table();
        let mut b_t = b.clone_table();
        if let Some(ai) = ar.a_no {
            if ai <= a_t.rows.len() {
                a_t.rows.insert(ai, vec![String::new(); ncols]);
            }
        }
        if let Some(bi) = ar.b_no {
            if bi <= b_t.rows.len() {
                b_t.rows.insert(bi, vec![String::new(); ncols]);
            }
        }
        let ok = self.write_side(true, &a_t) & self.write_side(false, &b_t);
        if ok {
            self.reload();
        }
        ok
    }

    /// P44-6：在选中行后插入空行（两侧同步插入；BC 在后面插入行 ⌥⌃↩）
    pub(crate) fn insert_row_after(&mut self) -> bool {
        let Some((aligned_idx, _)) = self.selected else {
            return false;
        };
        let (Some(a), Some(b)) = (&self.table_a, &self.table_b) else {
            return false;
        };
        let Some(ar) = self.aligned.get(aligned_idx) else {
            return false;
        };
        let ncols = self.col_count();
        let mut a_t = a.clone_table();
        let mut b_t = b.clone_table();
        if let Some(ai) = ar.a_no {
            let at = (ai + 1).min(a_t.rows.len());
            a_t.rows.insert(at, vec![String::new(); ncols]);
        }
        if let Some(bi) = ar.b_no {
            let bt = (bi + 1).min(b_t.rows.len());
            b_t.rows.insert(bt, vec![String::new(); ncols]);
        }
        let ok = self.write_side(true, &a_t) & self.write_side(false, &b_t);
        if ok {
            self.reload();
        }
        ok
    }

    /// 删除选中列（两侧表头与数据同步删除）
    pub(crate) fn delete_col(&mut self) -> bool {
        let Some((_, col)) = self.selected else {
            return false;
        };
        let (Some(a), Some(b)) = (&self.table_a, &self.table_b) else {
            return false;
        };
        let mut a_t = a.clone_table();
        let mut b_t = b.clone_table();
        let mut ok = true;
        if col < a_t.headers.len() {
            a_t.headers.remove(col);
            for r in &mut a_t.rows {
                if col < r.len() {
                    r.remove(col);
                }
            }
            ok &= self.write_side(true, &a_t);
        }
        if col < b_t.headers.len() {
            b_t.headers.remove(col);
            for r in &mut b_t.rows {
                if col < r.len() {
                    r.remove(col);
                }
            }
            ok &= self.write_side(false, &b_t);
        }
        if ok {
            self.reload();
        }
        ok
    }

    /// 在选中列前插入空列（两侧同步插入）
    pub(crate) fn insert_col(&mut self) -> bool {
        let Some((_, col)) = self.selected else {
            return false;
        };
        let (Some(a), Some(b)) = (&self.table_a, &self.table_b) else {
            return false;
        };
        let mut a_t = a.clone_table();
        let mut b_t = b.clone_table();
        if col <= a_t.headers.len() {
            a_t.headers.insert(col, format!("col{col}"));
            for r in &mut a_t.rows {
                r.insert(col.min(r.len()), String::new());
            }
        }
        if col <= b_t.headers.len() {
            b_t.headers.insert(col, format!("col{col}"));
            for r in &mut b_t.rows {
                r.insert(col.min(r.len()), String::new());
            }
        }
        let ok = self.write_side(true, &a_t) & self.write_side(false, &b_t);
        if ok {
            self.reload();
        }
        ok
    }

    /// P45-5：在后面插入列（两侧同步插入；BC 在后面插入列）
    pub(crate) fn insert_col_after(&mut self) -> bool {
        let Some((_, col)) = self.selected else {
            return false;
        };
        let (Some(a), Some(b)) = (&self.table_a, &self.table_b) else {
            return false;
        };
        let mut a_t = a.clone_table();
        let mut b_t = b.clone_table();
        let at = (col + 1).min(a_t.headers.len());
        if at <= a_t.headers.len() {
            a_t.headers.insert(at, format!("col{at}"));
            for r in &mut a_t.rows {
                r.insert(at.min(r.len()), String::new());
            }
        }
        let bt = (col + 1).min(b_t.headers.len());
        if bt <= b_t.headers.len() {
            b_t.headers.insert(bt, format!("col{bt}"));
            for r in &mut b_t.rows {
                r.insert(bt.min(r.len()), String::new());
            }
        }
        let ok = self.write_side(true, &a_t) & self.write_side(false, &b_t);
        if ok {
            self.reload();
        }
        ok
    }

    /// P45-5：选中单元格（行对齐索引, 列索引；供菜单/测试）
    #[cfg(test)]
    pub(crate) fn select_row_col(&mut self, row: usize, col: usize) {
        self.selected = Some((row, col));
    }

    /// 修改选中单元格（写回选中侧——有右侧行改右侧，否则改左侧）
    pub(crate) fn set_cell(&mut self, value: String) -> bool {
        let Some((aligned_idx, col)) = self.selected else {
            return false;
        };
        let (Some(a), Some(b)) = (&self.table_a, &self.table_b) else {
            return false;
        };
        let Some(ar) = self.aligned.get(aligned_idx) else {
            return false;
        };
        let mut ok = true;
        // 优先修改右侧（BC 编辑当前单元格；右侧为常用编辑目标）
        if let Some(bi) = ar.b_no {
            let mut b_t = b.clone_table();
            if let Some(r) = b_t.rows.get_mut(bi) {
                if r.len() <= col {
                    r.resize(col + 1, String::new());
                }
                r[col] = value.clone();
                ok &= self.write_side(false, &b_t);
            }
        } else if let Some(ai) = ar.a_no {
            let mut a_t = a.clone_table();
            if let Some(r) = a_t.rows.get_mut(ai) {
                if r.len() <= col {
                    r.resize(col + 1, String::new());
                }
                r[col] = value.clone();
                ok &= self.write_side(true, &a_t);
            }
        }
        if ok {
            self.reload();
        }
        ok
    }

    /// P44-6：打开排序对话框（BC 编辑>排序...）
    pub(crate) fn open_sort_dialog(&mut self) {
        self.show_sort_dialog = true;
    }

    /// P44-6：排序对话框是否打开（供测试/状态栏）
    #[cfg(test)]
    pub(crate) fn sort_dialog_open(&self) -> bool {
        self.show_sort_dialog
    }

    /// P44-6：当前排序列名（供菜单显示；"左 · 列N" / "右 · 列N"）
    pub(crate) fn sort_label(&self) -> String {
        match &self.sort {
            Some(sk) => format!("{} · 列{}", if sk.side { "左" } else { "右" }, sk.col),
            None => "列0".to_string(),
        }
    }

    /// P44-6：打开修改单元格弹窗（BC 编辑>修改...，⇧⌃↩）
    pub(crate) fn open_cell_edit(&mut self) {
        self.cell_edit_buf = self.selected_cell_text();
        self.show_cell_edit = true;
    }

    /// P44-6：当前选中单元格文本（弹窗预填）
    fn selected_cell_text(&self) -> String {
        let Some((aligned_idx, col)) = self.selected else {
            return String::new();
        };
        let Some(ar) = self.aligned.get(aligned_idx) else {
            return String::new();
        };
        if let Some(bi) = ar.b_no {
            if let Some(t) = &self.table_b {
                if let Some(r) = t.rows.get(bi) {
                    return r.get(col).cloned().unwrap_or_default();
                }
            }
        } else if let Some(ai) = ar.a_no {
            if let Some(t) = &self.table_a {
                if let Some(r) = t.rows.get(ai) {
                    return r.get(col).cloned().unwrap_or_default();
                }
            }
        }
        String::new()
    }

    pub(crate) fn ui(&mut self, ui: &mut egui::Ui) {
        // P44-6：表格快捷键（BC 编辑菜单：⇧⌃↩ 修改、⌘⌥⌃↩ 前面插入行、⌥⌃↩ 后面插入行）
        if self.selected.is_some() && !ui.ctx().egui_wants_keyboard_input() {
            if ui
                .input(|i| i.modifiers.ctrl && i.modifiers.shift && i.key_pressed(egui::Key::Enter))
            {
                self.show_cell_edit = true;
            }
            if ui.input(|i| {
                i.modifiers.command
                    && i.modifiers.alt
                    && i.modifiers.ctrl
                    && i.key_pressed(egui::Key::Enter)
            }) {
                self.insert_row();
            }
            if ui.input(|i| i.modifiers.alt && i.modifiers.ctrl && i.key_pressed(egui::Key::Enter))
            {
                self.insert_row_after();
            }
        }
        if crate::gui::common::SHOW_TOOLBAR.load(std::sync::atomic::Ordering::Relaxed) {
            egui::Panel::top("csvtab_tools").show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    // 主键下拉
                    let opts = self.key_options();
                    ui.label(t(I18nKey::CsvKeyCol));
                    let mut key = self.key.clone();
                    let selected = if key.is_empty() {
                        t(I18nKey::CsvRowAlign).to_string()
                    } else {
                        key.clone()
                    };
                    egui::ComboBox::from_id_salt("csv_key")
                        .selected_text(selected)
                        .show_ui(ui, |ui| {
                            if ui
                                .selectable_label(key.is_empty(), t(I18nKey::CsvRowAlign))
                                .clicked()
                            {
                                key.clear();
                            }
                            for h in &opts {
                                if ui.selectable_label(self.key == *h, h).clicked() {
                                    key = h.clone();
                                }
                            }
                        });
                    if key != self.key {
                        self.key = key;
                        self.reload();
                    }
                    // 分隔符
                    ui.separator();
                    ui.label(t(I18nKey::CsvDelimiter));
                    let mut delim = self.delimiter.clone();
                    egui::ComboBox::from_id_salt("csv_delim")
                        .selected_text(delim.clone())
                        .show_ui(ui, |ui| {
                            for d in [",", "\\t"] {
                                if ui.selectable_label(delim == d, d).clicked() {
                                    delim = d.to_string();
                                }
                            }
                        });
                    if delim != self.delimiter {
                        self.delimiter = delim;
                        self.reload();
                    }
                    // 显示相同 + 过滤
                    ui.separator();
                    if ui
                        .checkbox(&mut self.show_same, t(I18nKey::ShowSame))
                        .changed()
                    {
                        // 仅刷新显示，不需重排
                    }
                    let filter_labels = [
                        (CsvFilter::All, t(I18nKey::CsvFilterAll)),
                        (CsvFilter::Diff, t(I18nKey::CsvFilterDiff)),
                        (CsvFilter::LeftOnly, t(I18nKey::CsvFilterLeft)),
                        (CsvFilter::RightOnly, t(I18nKey::CsvFilterRight)),
                        (CsvFilter::Modified, t(I18nKey::CsvFilterModified)),
                        (CsvFilter::Same, t(I18nKey::CsvFilterSame)),
                    ];
                    let cur = self.filter;
                    egui::ComboBox::from_id_salt("csv_filter")
                        .selected_text(
                            filter_labels
                                .iter()
                                .find(|(v, _)| *v == cur)
                                .map(|(_, l)| *l)
                                .unwrap_or(""),
                        )
                        .show_ui(ui, |ui| {
                            for (v, l) in &filter_labels {
                                if ui.selectable_label(cur == *v, *l).clicked() {
                                    self.filter = *v;
                                }
                            }
                        });
                    // 重新加载
                    if ui
                        .button(format!("⟳ {}", t(I18nKey::Reload)))
                        .on_hover_text(t(I18nKey::ReloadHint))
                        .clicked()
                    {
                        self.reload();
                    }
                    // P37-1c：隐藏相同列 / 列宽自适应
                    ui.separator();
                    ui.checkbox(&mut self.hide_same_cols, t(I18nKey::HideSameCols));
                    ui.checkbox(&mut self.auto_fit, t(I18nKey::FitColumns));
                    // P37-1c：复制单元格至右侧（需先选中单元格）
                    ui.separator();
                    if ui
                        .button(format!("→ {}", t(I18nKey::CopyCellRight)))
                        .on_hover_text("把左侧单元格复制到右侧对应位置（需先点击选中单元格）")
                        .clicked()
                    {
                        self.copy_cell_right();
                    }
                    // 统计
                    let s = self.stats;
                    ui.separator();
                    ui.label(format!(
                        "{} / {} / {} / {}",
                        fmt(I18nKey::StatSame, &[&s.same.to_string()]),
                        fmt(I18nKey::StatDelete, &[&s.left_only.to_string()]),
                        fmt(I18nKey::StatInsert, &[&s.right_only.to_string()]),
                        fmt(I18nKey::StatReplace, &[&s.modified.to_string()]),
                    ));
                });
            });
        } // csvtab_tools 门控闭合

        if let Some(err) = &self.error {
            ui.colored_label(super::theme::error_color(), err);
            return;
        }
        let (Some(a), Some(b)) = (&self.table_a, &self.table_b) else {
            // P34：空会话（两侧均未选择文件）→ 显示打开入口 + 拖拽提示
            egui::CentralPanel::default().show(ui, |ui| {
                // P52-2：统一空状态（表格用绿色系）
                super::common::empty_state(
                    ui,
                    "📊",
                    super::theme::card_icon_colors()[4],
                    t(I18nKey::DiffEmptyHint),
                    t(I18nKey::DragHint),
                    |ui| {
                        ui.horizontal(|ui| {
                            if ui.button(t(I18nKey::OpenLeft)).clicked() {
                                self.open_left();
                            }
                            if ui.button(t(I18nKey::OpenRight)).clicked() {
                                self.open_right();
                            }
                        });
                    },
                );
            });
            return;
        };
        let ncols = self.col_count();
        let visible = self.visible_rows();
        let total = visible.len();
        // P37-1c：隐藏相同列 + 列宽（auto_fit 时按内容计算）
        let vis_cols = self.visible_cols();
        let widths: Vec<f32> = if self.auto_fit {
            self.col_widths(vis_cols.as_ref())
        } else {
            vec![110.0; ncols]
        };

        // 表头行（固定，不随行滚动）：左侧列名 | 右侧列名（点击排序）
        egui::Panel::top("csvtab_header").show(ui, |ui| {
            // P31 表头底色：与内容区区分
            let header_bg = if ui.visuals().dark_mode {
                super::theme::column_head_bg(true)
            } else {
                super::theme::column_head_bg(false)
            };
            let hrect = ui.max_rect();
            ui.painter().rect_filled(hrect, 0.0, header_bg);
            ui.horizontal(|ui| {
                ui.add_space(4.0);
                // 行号 + 状态 占位（与数据行同宽）
                let gutter = gutter_width(a.rows.len().max(b.rows.len())) + 26.0;
                ui.add_space(gutter);
                let fg = text_color(ui);
                let mut click: Option<(bool, usize)> = None;
                for (side, t) in [(true, a), (false, b)] {
                    if side {
                        ui.separator();
                    }
                    for (ci, h) in t.headers.iter().enumerate() {
                        // P37-1c：跳过隐藏的相同列
                        if let Some(vc) = &vis_cols {
                            if !vc.contains(&ci) {
                                continue;
                            }
                        }
                        let w = widths.get(ci).copied().unwrap_or(110.0);
                        let mut text = h.clone();
                        if let Some(sk) = &self.sort {
                            if sk.side == side && sk.col == ci {
                                text.push_str(if sk.asc { " ▲" } else { " ▼" });
                            }
                        }
                        if ui.add_sized([w, 20.0], egui::Button::new(text)).clicked() {
                            click = Some((side, ci));
                        }
                        ui.add_space(2.0);
                    }
                    let _ = fg;
                }
                if let Some((side, col)) = click {
                    let asc = match &self.sort {
                        Some(sk) if sk.side == side && sk.col == col => !sk.asc,
                        _ => true,
                    };
                    self.sort = Some(SortKey { side, col, asc });
                }
            });
        });

        // 数据行：左右并排虚拟化
        let show_same = self.show_same;
        let filter = self.filter;
        let sort = self.sort;
        // P37-1c：请求收集（借用安全：闭包内只设标志，闭包外执行）
        let mut click_cell: Option<(usize, usize)> = None;
        let mut copy_cell_req = false;
        // P37-1l：行列操作请求
        let mut delete_row_req = false;
        let mut insert_row_req = false;
        let mut delete_col_req = false;
        let mut insert_col_req = false;
        let mut edit_cell_req = false;
        egui::CentralPanel::default().show(ui, |ui| {
            ui.columns(2, |cols| {
                for (ci, side) in [(0usize, true), (1, false)].into_iter() {
                    let ui = &mut cols[ci];
                    let table = if side { a } else { b };
                    let out = super::show_rows(ui, total, ROW_H, |ui, range| {
                        let fg = text_color(ui);
                        for vi in range {
                            let aligned_idx = visible[vi];
                            let ar = &self.aligned[aligned_idx];
                            let (row, row_no) = if side {
                                (
                                    ar.a_no.and_then(|n| table.rows.get(n)),
                                    ar.a_no.map(|n| n + 1),
                                )
                            } else {
                                (
                                    ar.b_no.and_then(|n| table.rows.get(n)),
                                    ar.b_no.map(|n| n + 1),
                                )
                            };
                            let (rect, resp) = ui.allocate_exact_size(
                                Vec2::new(ui.available_width().max(200.0), ROW_H),
                                egui::Sense::click(),
                            );
                            // P32-A4：行右键菜单（复制路径/打开文件）+ P37-1c 复制单元格至右侧
                            let (lp, rp) = (self.left.clone(), self.right.clone());
                            resp.context_menu(|ui| {
                                if ui.button(t(I18nKey::CopyLeftPath)).clicked() {
                                    ui.ctx().copy_text(lp.clone());
                                    ui.close();
                                }
                                if ui.button(t(I18nKey::CopyRightPath)).clicked() {
                                    ui.ctx().copy_text(rp.clone());
                                    ui.close();
                                }
                                ui.separator();
                                if ui.button(t(I18nKey::CopyCellRight)).clicked() {
                                    copy_cell_req = true;
                                    ui.close();
                                }
                                // P37-1l：行列操作（BC 编辑菜单 删除/插入行列、修改单元格）
                                ui.separator();
                                if ui.button(t(I18nKey::CsvDeleteRow)).clicked() {
                                    delete_row_req = true;
                                    ui.close();
                                }
                                if ui.button(t(I18nKey::CsvInsertRow)).clicked() {
                                    insert_row_req = true;
                                    ui.close();
                                }
                                if ui.button(t(I18nKey::CsvDeleteCol)).clicked() {
                                    delete_col_req = true;
                                    ui.close();
                                }
                                if ui.button(t(I18nKey::CsvInsertCol)).clicked() {
                                    insert_col_req = true;
                                    ui.close();
                                }
                                if ui.button(t(I18nKey::CsvEditCell)).clicked() {
                                    edit_cell_req = true;
                                    ui.close();
                                }
                                ui.separator();
                                if ui.button(t(I18nKey::OpenLeftFile)).clicked() {
                                    super::common::open_with_system_app(&lp);
                                    ui.close();
                                }
                                if ui.button(t(I18nKey::OpenRightFile)).clicked() {
                                    super::common::open_with_system_app(&rp);
                                    ui.close();
                                }
                            });
                            // 行级底色（P31：hover 浅色 + 状态色）
                            let bg = match (ar.status, side) {
                                (RowStatus::LeftOnly, true) | (RowStatus::RightOnly, false) => {
                                    Some(bg_replace_l())
                                }
                                (RowStatus::RightOnly, true) | (RowStatus::LeftOnly, false) => {
                                    Some(bg_match())
                                }
                                (RowStatus::Modified, _) => Some(bg_match()),
                                _ => None,
                            };
                            let bg = if resp.hovered() {
                                Some(bg.unwrap_or(bg_match()))
                            } else {
                                bg
                            };
                            paint_bg(ui, rect, bg);
                            // 行号
                            paint_line_no(
                                ui,
                                Rect::from_min_size(rect.min, vec2(gutter_width(total), ROW_H)),
                                row_no,
                            );
                            // 状态字母
                            let letter = match ar.status {
                                RowStatus::Same => 'S',
                                RowStatus::LeftOnly => 'L',
                                RowStatus::RightOnly => 'R',
                                RowStatus::Modified => 'M',
                            };
                            let sc = status_color(ui, letter);
                            ui.painter().text(
                                Pos2::new(rect.left() + gutter_width(total) + 8.0, rect.center().y),
                                egui::Align2::LEFT_CENTER,
                                letter.to_string(),
                                egui::FontId::monospace(12.0),
                                sc,
                            );
                            // 单元格（P37-1c：隐藏相同列过滤 + 自适应宽度 + 点击选中）
                            let mut x0 = rect.left() + gutter_width(total) + 24.0;
                            for (col_idx, cell) in
                                row.map(|r| r.iter().enumerate()).into_iter().flatten()
                            {
                                // 隐藏相同列时跳过
                                if let Some(vc) = &vis_cols {
                                    if !vc.contains(&col_idx) {
                                        continue;
                                    }
                                }
                                let col_w = widths.get(col_idx).copied().unwrap_or(110.0);
                                let crect = Rect::from_min_size(
                                    Pos2::new(x0, rect.top()),
                                    vec2(col_w, ROW_H),
                                );
                                // 单元格点击选中（P37-1c）
                                let cell_resp = ui.interact(
                                    crect,
                                    ui.id().with(("csvcell", aligned_idx, col_idx)),
                                    egui::Sense::click(),
                                );
                                if cell_resp.clicked() {
                                    click_cell = Some((aligned_idx, col_idx));
                                }
                                // 选中高亮（浅蓝）
                                if self.selected == Some((aligned_idx, col_idx)) {
                                    paint_bg(ui, crect, Some(bg_select()));
                                }
                                // 修改列高亮：左侧红、右侧黄
                                let hl = if ar.status == RowStatus::Modified
                                    && ar.changed_cols.contains(&col_idx)
                                {
                                    Some(if side { hl_replace_l() } else { hl_replace_r() })
                                } else {
                                    None
                                };
                                if let Some(c) = hl {
                                    paint_bg(ui, crect, Some(c));
                                }
                                ui.painter().text(
                                    Pos2::new(x0 + 4.0, rect.center().y),
                                    egui::Align2::LEFT_CENTER,
                                    cell,
                                    egui::FontId::monospace(FONT_SIZE),
                                    fg,
                                );
                                x0 += col_w;
                            }
                            let _ = ncols;
                            let _ = show_same;
                            let _ = filter;
                            let _ = sort;
                        }
                    });
                    let _ = out;
                }
            });
        });
        // P37-1c：闭包外处理单元格点击选中 / 复制请求（借用安全）
        if let Some(cell) = click_cell {
            self.selected = Some(cell);
        }
        if copy_cell_req {
            self.copy_cell_right();
        }
        // P37-1l：行列操作（闭包外执行）
        if delete_row_req {
            self.delete_row();
        }
        if insert_row_req {
            self.insert_row();
        }
        if delete_col_req {
            self.delete_col();
        }
        if insert_col_req {
            self.insert_col();
        }
        if edit_cell_req {
            self.show_cell_edit = true;
        }
        // P37-1l：修改单元格弹窗
        if self.show_cell_edit {
            let mut keep = true;
            let mut apply = false;
            let mut close_req = false;
            crate::gui::common::dialog_window(ui.ctx(), t(I18nKey::CsvEditCell))
                .collapsible(false)
                .resizable(false)
                .open(&mut keep)
                .show(ui.ctx(), |ui| {
                    ui.label(format!(
                        "行 {} / 列 {}：",
                        self.selected.map(|(r, _)| r).unwrap_or(0),
                        self.selected.map(|(_, c)| c).unwrap_or(0)
                    ));
                    ui.add(
                        egui::TextEdit::singleline(&mut self.cell_edit_buf).desired_width(200.0),
                    );
                    ui.horizontal(|ui| {
                        if ui.button(t(I18nKey::Save)).clicked() {
                            apply = true;
                        }
                        if ui.button(t(I18nKey::Cancel)).clicked() {
                            close_req = true;
                        }
                    });
                });
            if apply {
                let v = std::mem::take(&mut self.cell_edit_buf);
                self.set_cell(v);
                self.show_cell_edit = false;
            } else if close_req || !keep {
                self.show_cell_edit = false;
            }
        }
        // P44-6：排序对话框（BC 编辑>排序...；选列 + 升/降序）
        if self.show_sort_dialog {
            let mut keep = true;
            let mut apply = false;
            let mut close_req = false;
            let headers = self.key_options();
            // 当前排序列（默认第一列升序）
            let mut col_name = self.sort_label();
            let mut asc = self.sort.map(|s| s.asc).unwrap_or(true);
            crate::gui::common::dialog_window(ui.ctx(), t(I18nKey::MenuSort))
                .collapsible(false)
                .resizable(false)
                .open(&mut keep)
                .show(ui.ctx(), |ui| {
                    ui.label(format!("当前排序列：{}", col_name));
                    if !headers.is_empty() {
                        egui::ComboBox::from_id_salt("csv_sort_col")
                            .selected_text(col_name.clone())
                            .show_ui(ui, |ui| {
                                for (side, label) in [(true, "左"), (false, "右")] {
                                    let n = if side {
                                        self.table_a.as_ref().map(|t| t.headers.len()).unwrap_or(0)
                                    } else {
                                        self.table_b.as_ref().map(|t| t.headers.len()).unwrap_or(0)
                                    };
                                    for c in 0..n {
                                        let name = format!("{} · 列{}", label, c);
                                        if ui
                                            .selectable_label(col_name == name, name.clone())
                                            .clicked()
                                        {
                                            col_name = name;
                                        }
                                    }
                                }
                            });
                    }
                    ui.horizontal(|ui| {
                        ui.radio_value(&mut asc, true, t(I18nKey::SortAscending));
                        ui.radio_value(&mut asc, false, t(I18nKey::SortDescending));
                    });
                    ui.horizontal(|ui| {
                        if ui.button(t(I18nKey::Save)).clicked() {
                            apply = true;
                        }
                        if ui.button(t(I18nKey::Cancel)).clicked() {
                            close_req = true;
                        }
                    });
                });
            if apply {
                // 解析 col_name → (side, col)
                let (side, col) = parse_sort_col(&col_name);
                self.sort = Some(SortKey { side, col, asc });
                self.show_sort_dialog = false;
            } else if close_req || !keep {
                self.show_sort_dialog = false;
            }
        }
    }
}

/// P44-6：解析排序对话框选择的列名（"左 · 列N" / "右 · 列N"）为 (side, col)
fn parse_sort_col(name: &str) -> (bool, usize) {
    let mut side = true;
    let mut col = 0usize;
    if let Some(rest) = name.strip_prefix("右") {
        side = false;
        if let Some(n) = rest.trim_start_matches([' ', '·']).strip_prefix("列") {
            col = n.parse().unwrap_or(0);
        }
    } else if let Some(rest) = name.strip_prefix("左") {
        if let Some(n) = rest.trim_start_matches([' ', '·']).strip_prefix("列") {
            col = n.parse().unwrap_or(0);
        }
    }
    (side, col)
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
    fn new_loads_and_aligns() {
        let d = tempdir().unwrap();
        let l = write(d.path(), "l.csv", "id,name\n1,alice\n2,bob\n");
        let r = write(d.path(), "r.csv", "id,name\n1,alice\n2,BOB\n");
        let t = CsvTab::new(&l, &r);
        assert!(t.error.is_none());
        assert_eq!(t.stats.same, 1);
        assert_eq!(t.stats.modified, 1);
        // 默认过滤=仅差异：可见行 = 修改行
        assert_eq!(t.visible_rows().len(), 1);
        assert_eq!(t.aligned[t.visible_rows()[0]].status, RowStatus::Modified);
    }

    #[test]
    fn key_switch_realigns() {
        let d = tempdir().unwrap();
        let l = write(d.path(), "l.csv", "id,v\n1,x\n3,z\n");
        let r = write(d.path(), "r.csv", "id,v\n1,x\n2,y\n");
        let mut t = CsvTab::new(&l, &r);
        // 行号对齐：行2 是 3,z vs 2,y → modified
        assert_eq!(t.stats.modified, 1);
        t.key = "id".to_string();
        t.reload();
        // 主键对齐：1 匹配；3 仅左；2 仅右
        assert_eq!(t.stats.left_only, 1);
        assert_eq!(t.stats.right_only, 1);
        assert_eq!(t.stats.same, 1);
    }

    #[test]
    fn filter_and_show_same() {
        let d = tempdir().unwrap();
        let l = write(d.path(), "l.csv", "id\n1\n2\n");
        let r = write(d.path(), "r.csv", "id\n1\n3\n4\n");
        let mut t = CsvTab::new(&l, &r);
        // 行号对齐：行1 same；行2 modified；行3 仅右
        t.show_same = true;
        t.filter = CsvFilter::All;
        assert_eq!(t.visible_rows().len(), 3);
        t.show_same = false;
        assert_eq!(t.visible_rows().len(), 2);
        t.filter = CsvFilter::LeftOnly;
        assert_eq!(t.visible_rows().len(), 0);
        t.filter = CsvFilter::RightOnly;
        assert_eq!(t.visible_rows().len(), 1);
    }

    #[test]
    fn sort_by_column() {
        let d = tempdir().unwrap();
        let l = write(d.path(), "l.csv", "id,name\n2,b\n1,a\n");
        let r = write(d.path(), "r.csv", "id,name\n2,b\n1,a\n");
        let mut t = CsvTab::new(&l, &r);
        t.show_same = true;
        t.filter = CsvFilter::All;
        t.sort = Some(SortKey {
            side: true,
            col: 0,
            asc: true,
        });
        let vis = t.visible_rows();
        // 按左侧 id 升序：1 行在前
        let vals: Vec<String> = vis
            .iter()
            .map(|&i| {
                let ar = &t.aligned[i];
                t.table_a
                    .as_ref()
                    .and_then(|t| ar.a_no.and_then(|n| t.rows.get(n)))
                    .and_then(|r| r.first())
                    .cloned()
                    .unwrap_or_default()
            })
            .collect();
        assert_eq!(vals, vec!["1", "2"]);
        // 降序
        t.sort = Some(SortKey {
            side: true,
            col: 0,
            asc: false,
        });
        let vals: Vec<String> = t
            .visible_rows()
            .iter()
            .map(|&i| {
                let ar = &t.aligned[i];
                t.table_a
                    .as_ref()
                    .and_then(|t| ar.a_no.and_then(|n| t.rows.get(n)))
                    .and_then(|r| r.first())
                    .cloned()
                    .unwrap_or_default()
            })
            .collect();
        assert_eq!(vals, vec!["2", "1"]);
    }

    // ---- P37-1c：隐藏相同列 / 复制单元格至右侧 ----------------

    #[test]
    fn hide_same_cols_filters_identical_columns() {
        let d = tempdir().unwrap();
        // id 两列相同、name 不同 → 隐藏 id 列，保留 name 列
        let l = write(d.path(), "l.csv", "id,name\n1,alice\n2,bob\n");
        let r = write(d.path(), "r.csv", "id,name\n1,ALICE\n2,BOB\n");
        let mut t = CsvTab::new(&l, &r);
        assert!(t.error.is_none());
        t.show_same = true;
        t.filter = CsvFilter::All;
        // 默认不隐藏：两列都可见
        assert!(
            t.visible_cols().is_none(),
            "未开隐藏时返回 None（全部显示）"
        );
        t.hide_same_cols = true;
        let vc = t.visible_cols().unwrap();
        // name 列（索引 1）有差异，必须保留；id 列（索引 0）全部相同被隐藏
        assert!(vc.contains(&1), "有差异的列必须保留: {:?}", vc);
        assert!(!vc.contains(&0), "相同列应被隐藏: {:?}", vc);
    }

    #[test]
    fn copy_cell_right_writes_file_and_reloads() {
        let d = tempdir().unwrap();
        let l = write(d.path(), "l.csv", "id,name\n1,alice\n2,bob\n");
        let r = write(d.path(), "r.csv", "id,name\n1,ALICE\n2,BOB\n");
        let mut t = CsvTab::new(&l, &r);
        t.show_same = true;
        t.filter = CsvFilter::All;
        // 选中第 0 行（aligned 下标）的 name 列（索引 1）
        t.selected = Some((0, 1));
        assert!(t.copy_cell_right(), "复制应成功");
        // 右侧文件已被更新为左侧值
        let content = fs::read_to_string(&r).unwrap();
        assert!(content.contains("alice"), "右侧第 1 行 name 应更新为 alice");
        // 备份 .bak 存在
        assert!(fs::metadata(format!("{r}.bak")).is_ok(), "应有 .bak 备份");
        // reload 后 stats 更新：第 1 行不再是修改
        let t2 = CsvTab::new(&l, &r);
        assert_eq!(t2.stats.modified, 1, "剩余 1 处修改（第 2 行）");
    }

    #[test]
    fn copy_cell_right_requires_selection() {
        let d = tempdir().unwrap();
        let l = write(d.path(), "l.csv", "id,name\n1,alice\n");
        let r = write(d.path(), "r.csv", "id,name\n1,ALICE\n");
        let mut t = CsvTab::new(&l, &r);
        // 未选中 → 返回 false
        assert!(!t.copy_cell_right());
    }

    // ---- P37-1l：行列操作（删除/插入行列、修改单元格） ----

    #[test]
    fn delete_row_removes_both_sides() {
        let d = tempdir().unwrap();
        let l = write(d.path(), "l.csv", "id,name\n1,alice\n2,bob\n");
        let r = write(d.path(), "r.csv", "id,name\n1,alice\n2,BOB\n");
        let mut t = CsvTab::new(&l, &r);
        t.show_same = true;
        t.filter = CsvFilter::All;
        // 选中第 0 行（对齐下标 0）
        t.selected = Some((0, 0));
        assert!(t.delete_row(), "删除行应成功");
        // 两侧文件都只剩第 2 行
        let lc = fs::read_to_string(&l).unwrap();
        let rc = fs::read_to_string(&r).unwrap();
        assert!(!lc.contains("1,alice"), "左侧第 1 行应删除: {lc}");
        assert!(lc.contains("2,bob"), "左侧第 2 行应保留: {lc}");
        assert!(!rc.contains("1,alice"), "右侧第 1 行应删除: {rc}");
        assert!(rc.contains("2,BOB"), "右侧第 2 行应保留: {rc}");
        // 备份存在
        assert!(fs::metadata(format!("{l}.bak")).is_ok());
        assert!(fs::metadata(format!("{r}.bak")).is_ok());
    }

    #[test]
    fn insert_row_adds_empty_both_sides() {
        let d = tempdir().unwrap();
        let l = write(d.path(), "l.csv", "id,name\n1,alice\n");
        let r = write(d.path(), "r.csv", "id,name\n1,ALICE\n");
        let mut t = CsvTab::new(&l, &r);
        t.show_same = true;
        t.filter = CsvFilter::All;
        t.selected = Some((0, 0));
        assert!(t.insert_row(), "插入行应成功");
        let lc = fs::read_to_string(&l).unwrap();
        let rows: Vec<&str> = lc.lines().collect();
        assert_eq!(rows.len(), 3, "表头 + 空行 + 原行: {lc}");
        // 第 2 行为空行（2 列空字段序列化为单个逗号）
        assert_eq!(rows[1], ",", "插入行应为空: {lc}");
    }

    #[test]
    fn delete_col_removes_from_both_sides() {
        let d = tempdir().unwrap();
        let l = write(d.path(), "l.csv", "id,name\n1,alice\n");
        let r = write(d.path(), "r.csv", "id,name\n1,ALICE\n");
        let mut t = CsvTab::new(&l, &r);
        t.show_same = true;
        t.filter = CsvFilter::All;
        // 选中 name 列（列 1）
        t.selected = Some((0, 1));
        assert!(t.delete_col(), "删除列应成功");
        let lc = fs::read_to_string(&l).unwrap();
        assert!(!lc.contains("name"), "表头应删除 name 列: {lc}");
        assert!(!lc.contains("alice"), "数据应删除 name 列: {lc}");
        assert!(lc.contains("id"), "id 列应保留: {lc}");
    }

    #[test]
    fn insert_col_adds_empty_both_sides() {
        let d = tempdir().unwrap();
        let l = write(d.path(), "l.csv", "id,name\n1,alice\n");
        let r = write(d.path(), "r.csv", "id,name\n1,ALICE\n");
        let mut t = CsvTab::new(&l, &r);
        t.show_same = true;
        t.filter = CsvFilter::All;
        t.selected = Some((0, 0));
        assert!(t.insert_col(), "插入列应成功");
        let lc = fs::read_to_string(&l).unwrap();
        assert!(lc.contains("col0"), "新列头应为 col0: {lc}");
    }

    #[test]
    fn set_cell_modifies_right_side() {
        let d = tempdir().unwrap();
        let l = write(d.path(), "l.csv", "id,name\n1,alice\n");
        let r = write(d.path(), "r.csv", "id,name\n1,ALICE\n");
        let mut t = CsvTab::new(&l, &r);
        t.show_same = true;
        t.filter = CsvFilter::All;
        // 选中第 0 行 name 列（列 1）→ 修改右侧
        t.selected = Some((0, 1));
        assert!(t.set_cell("Alice2".to_string()), "修改单元格应成功");
        let rc = fs::read_to_string(&r).unwrap();
        assert!(rc.contains("Alice2"), "右侧 name 应更新: {rc}");
        // 左侧不变
        let lc = fs::read_to_string(&l).unwrap();
        assert!(lc.contains("alice"), "左侧不应变: {lc}");
    }
}
