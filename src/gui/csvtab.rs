//! CSV 表格对比标签页（P29）：并排渲染左右表格，行对齐 + 单元格级差异高亮。
//!
//! 对标 Beyond Compare 表格视图：
//! - 行按主键（或行号）对齐，行级状态着色（L/R/M/S）
//! - 修改行的变化列在左右两侧同时着色
//! - 工具栏：主键下拉 / 分隔符 / 显示相同 / 状态过滤
//! - 表头点击排序（纯显示排序，不改对齐数据）

use super::common::*;
use crate::csvcmp::{align_tables, RowStats, RowStatus, Table};
use crate::i18n::{fmt, t, Key as I18nKey};
use eframe::egui::{self, Color32, Pos2, Rect, Vec2};

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
    left: String,
    right: String,
    table_a: Option<Table>,
    table_b: Option<Table>,
    aligned: Vec<crate::csvcmp::AlignedRow>,
    stats: RowStats,
    /// 当前主键列名（空 = 行号对齐）
    key: String,
    /// 分隔符显示值（"," / "\\t"）
    delimiter: String,
    show_same: bool,
    filter: CsvFilter,
    sort: Option<SortKey>,
    error: Option<String>,
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
            key: String::new(),
            delimiter: ",".to_string(),
            show_same: false,
            filter: CsvFilter::Diff,
            sort: None,
            error: None,
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

    /// 重新加载：读文件 → 解析 → 对齐
    fn reload(&mut self) {
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
        let (aligned, stats) = align_tables(&a, &b, key);
        self.table_a = Some(a);
        self.table_b = Some(b);
        self.aligned = aligned;
        self.stats = stats;
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

    pub(crate) fn ui(&mut self, ui: &mut egui::Ui) {
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
                if ui.button(t(I18nKey::Reload)).clicked() {
                    self.reload();
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

        if let Some(err) = &self.error {
            ui.colored_label(Color32::from_rgb(235, 90, 90), err);
            return;
        }
        let (Some(a), Some(b)) = (&self.table_a, &self.table_b) else {
            return;
        };
        let ncols = self.col_count();
        let visible = self.visible_rows();
        let total = visible.len();

        // 表头行（固定，不随行滚动）：左侧列名 | 右侧列名（点击排序）
        egui::Panel::top("csvtab_header").show(ui, |ui| {
            // P31 表头底色：与内容区区分
            let header_bg = if ui.visuals().dark_mode {
                Color32::from_rgb(40, 44, 52)
            } else {
                Color32::from_rgb(236, 238, 242)
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
                        let mut text = h.clone();
                        if let Some(sk) = &self.sort {
                            if sk.side == side && sk.col == ci {
                                text.push_str(if sk.asc { " ▲" } else { " ▼" });
                            }
                        }
                        if ui.button(text).clicked() {
                            click = Some((side, ci));
                        }
                        ui.add_space(4.0);
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
                        // P32-A4：行右键菜单（复制路径/打开文件）
                        let (lp, rp) = (self.left.clone(), self.right.clone());
                        resp.context_menu(|ui| {
                            if ui.button("复制左侧路径").clicked() {
                                ui.ctx().copy_text(lp.clone());
                                ui.close();
                            }
                            if ui.button("复制右侧路径").clicked() {
                                ui.ctx().copy_text(rp.clone());
                                ui.close();
                            }
                            ui.separator();
                            if ui.button("打开左侧文件").clicked() {
                                super::common::open_with_system_app(&lp);
                                ui.close();
                            }
                            if ui.button("打开右侧文件").clicked() {
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
                        // 单元格
                        let col_w = 110.0;
                        for (col_idx, cell) in
                            row.map(|r| r.iter().enumerate()).into_iter().flatten()
                        {
                            let x0 =
                                rect.left() + gutter_width(total) + 24.0 + col_idx as f32 * col_w;
                            let crect =
                                Rect::from_min_size(Pos2::new(x0, rect.top()), vec2(col_w, ROW_H));
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
}
