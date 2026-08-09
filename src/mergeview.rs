//! M5 三路合并视图模型。
//!
//! 基于 [`crate::merge::compute_blocks`] 把三路归并结果展开为可渲染的行序列：
//! 每一行同时描述 base / left / right 三栏的单元格（含行内高亮），
//! 并保留块级信息（冲突块、解决选择）供保存合并结果。

use crate::merge::{compute_blocks, MergeBlock};
use crate::render::intra_line;
use crate::sideview::Cell;
use similar::Algorithm;

/// 块类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    /// 公共区：三侧一致
    Context,
    /// 只有左侧改动
    LeftOnly,
    /// 只有右侧改动
    RightOnly,
    /// 两侧做了相同改动
    Same,
    /// 两侧改动不同 → 冲突
    Conflict,
}

/// 冲突解决选择（仅 Conflict 块有效）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resolution {
    /// 未解决
    Auto,
    /// 取左侧
    Left,
    /// 取右侧
    Right,
    /// 取 base
    Base,
}

/// 三路合并视图的一行（三栏对齐）
#[derive(Debug, Clone)]
pub struct MergeRow {
    pub base: Option<Cell>,
    pub left: Option<Cell>,
    pub right: Option<Cell>,
    pub kind: BlockKind,
    /// base 行号（1-based）
    pub base_no: Option<usize>,
    /// 是否属于冲突块（整行高亮 + 导航）
    pub in_conflict: bool,
}

/// 块级信息（渲染 + 保存共用）
#[derive(Debug, Clone)]
pub struct BlockInfo {
    pub kind: BlockKind,
    pub base: Vec<String>,
    pub left: Vec<String>,
    pub right: Vec<String>,
    pub resolution: Resolution,
}

/// 三路合并视图
#[derive(Debug, Default)]
pub struct MergeView {
    pub rows: Vec<MergeRow>,
    pub blocks: Vec<BlockInfo>,
    /// 冲突块数
    pub conflicts: usize,
    /// 每个冲突块起始行索引（rows 内）
    pub conflict_rows: Vec<usize>,
    /// 每个冲突块在 blocks 中的索引（与 conflict_rows 一一对应）
    pub conflict_block_indices: Vec<usize>,
}

/// 构建三路合并视图
pub fn build_merge_view(base: &str, left: &str, right: &str) -> MergeView {
    let base_lines: Vec<&str> = base.lines().collect();
    let left_lines: Vec<&str> = left.lines().collect();
    let right_lines: Vec<&str> = right.lines().collect();
    let blocks = compute_blocks(&base_lines, &left_lines, &right_lines, Algorithm::Patience);

    let mut view = MergeView::default();
    for blk in &blocks {
        let kind = classify(blk, &base_lines);
        let info = BlockInfo {
            kind,
            base: base_lines[blk.base.clone()]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            left: blk.left.iter().map(|s| s.to_string()).collect(),
            right: blk.right.iter().map(|s| s.to_string()).collect(),
            resolution: Resolution::Auto,
        };
        if kind == BlockKind::Conflict {
            view.conflicts += 1;
            view.conflict_rows.push(view.rows.len());
            // blocks 尚未 push，当前长度即本块在 blocks 中的下标
            view.conflict_block_indices.push(view.blocks.len());
        }
        expand_block(blk, &base_lines, kind, &mut view);
        view.blocks.push(info);
    }
    view
}

/// 判定块类型
fn classify(blk: &MergeBlock<'_>, base: &[&str]) -> BlockKind {
    let base_slice: Vec<&str> = base[blk.base.clone()].to_vec();
    if blk.conflict {
        BlockKind::Conflict
    } else if blk.left == blk.right {
        if blk.left == base_slice {
            BlockKind::Context
        } else {
            BlockKind::Same
        }
    } else if blk.left == base_slice {
        BlockKind::RightOnly
    } else {
        BlockKind::LeftOnly
    }
}

/// 把块展开为对齐行（行数 = 三侧最大值，不足留空）
fn expand_block(blk: &MergeBlock<'_>, base: &[&str], kind: BlockKind, view: &mut MergeView) {
    let base_len = blk.base.end - blk.base.start;
    let n = base_len.max(blk.left.len()).max(blk.right.len());
    for i in 0..n {
        // 该行是否有 base 行（i 落在 base 区间内）
        let base_line = (i < base_len).then(|| base[blk.base.start + i]);
        let base_cell = base_line.map(|s| highlight_base(s, blk, i, kind));
        let left_cell = blk
            .left
            .get(i)
            .map(|s| highlight_left(s, base, blk, i, kind));
        let right_cell = blk
            .right
            .get(i)
            .map(|s| highlight_right(s, base, blk, i, kind));
        let base_no = base_line.map(|_| blk.base.start + i + 1);
        view.rows.push(MergeRow {
            base: base_cell,
            left: left_cell,
            right: right_cell,
            kind,
            base_no,
            in_conflict: kind == BlockKind::Conflict,
        });
    }
}

fn cell_with_segments(text: &str, segs: Option<Vec<(String, bool)>>) -> Cell {
    match segs {
        Some(s) => Cell {
            text: text.to_string(),
            segments: s,
        },
        None => Cell {
            text: text.to_string(),
            segments: vec![(text.to_string(), false)],
        },
    }
}

/// base 栏：上下文/单侧改动时无高亮；冲突时对 base 与两侧的交集不做标记
fn highlight_base(s: &str, blk: &MergeBlock<'_>, i: usize, kind: BlockKind) -> Cell {
    match kind {
        BlockKind::LeftOnly => cell_with_segments(s, None),
        BlockKind::RightOnly => cell_with_segments(s, None),
        BlockKind::Same => cell_with_segments(s, None),
        BlockKind::Context => cell_with_segments(s, None),
        BlockKind::Conflict => {
            // base 与 left、right 分别做字符级 diff，标记共同变更段
            let segs = match (blk.left.get(i), blk.right.get(i)) {
                (Some(l), Some(r)) => {
                    let (sl, _) = intra_line(s, l);
                    let (sr, _) = intra_line(s, r);
                    // 合并两侧标记：base 中任一侧变更即标记
                    let mut merged: Vec<(String, bool)> = Vec::new();
                    for (a, b) in sl.iter().zip(sr.iter()) {
                        merged.push((a.0.clone(), a.1 || b.1));
                    }
                    Some(merged)
                }
                _ => None,
            };
            cell_with_segments(s, segs)
        }
    }
}

fn highlight_left(s: &str, base: &[&str], blk: &MergeBlock<'_>, i: usize, kind: BlockKind) -> Cell {
    match kind {
        BlockKind::LeftOnly | BlockKind::Conflict => {
            let base_len = blk.base.end - blk.base.start;
            let segs = (i < base_len).then(|| intra_line(base[blk.base.start + i], s).1);
            cell_with_segments(s, segs)
        }
        _ => cell_with_segments(s, None),
    }
}

fn highlight_right(
    s: &str,
    base: &[&str],
    blk: &MergeBlock<'_>,
    i: usize,
    kind: BlockKind,
) -> Cell {
    match kind {
        BlockKind::RightOnly | BlockKind::Conflict => {
            let base_len = blk.base.end - blk.base.start;
            let segs = (i < base_len).then(|| intra_line(base[blk.base.start + i], s).1);
            cell_with_segments(s, segs)
        }
        _ => cell_with_segments(s, None),
    }
}

/// 按当前解决选择生成合并输出文本。
///
/// 未解决的冲突块输出 git 风格冲突标记（与 CLI merge 语义一致），
/// 标签由调用方提供（GUI 中为 BASE/LEFT/RIGHT 文件名）。
pub fn render_merged(view: &MergeView, label_l: &str, label_r: &str) -> (Vec<String>, usize) {
    let mut out = Vec::new();
    let mut unresolved = 0usize;
    for blk in &view.blocks {
        match blk.kind {
            BlockKind::Conflict => match blk.resolution {
                Resolution::Left => out.extend(blk.left.iter().cloned()),
                Resolution::Right => out.extend(blk.right.iter().cloned()),
                Resolution::Base => out.extend(blk.base.iter().cloned()),
                Resolution::Auto => {
                    unresolved += 1;
                    out.push(format!("<<<<<<< {label_l}"));
                    out.extend(blk.left.iter().cloned());
                    out.push("=======".to_string());
                    out.extend(blk.right.iter().cloned());
                    out.push(format!(">>>>>>> {label_r}"));
                }
            },
            BlockKind::LeftOnly | BlockKind::Same => out.extend(blk.left.iter().cloned()),
            BlockKind::RightOnly => out.extend(blk.right.iter().cloned()),
            BlockKind::Context => out.extend(blk.base.iter().cloned()),
        }
    }
    (out, unresolved)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_changes_all_context() {
        let v = build_merge_view("a\nb\nc\n", "a\nb\nc\n", "a\nb\nc\n");
        assert_eq!(v.conflicts, 0);
        assert_eq!(v.rows.len(), 3);
        assert!(v.rows.iter().all(|r| r.kind == BlockKind::Context));
        assert!(v.blocks.iter().all(|b| b.kind == BlockKind::Context));
    }

    #[test]
    fn single_side_changes_classified() {
        // 左侧改第 1 行，右侧改第 3 行 → LeftOnly + RightOnly
        let v = build_merge_view(
            "l1\nl2\nl3\nl4\nl5\n",
            "L1\nl2\nl3\nl4\nl5\n",
            "l1\nl2\nl3\nl4\nR5\n",
        );
        assert_eq!(v.conflicts, 0);
        let kinds: Vec<BlockKind> = v.blocks.iter().map(|b| b.kind).collect();
        assert!(kinds.contains(&BlockKind::LeftOnly));
        assert!(kinds.contains(&BlockKind::RightOnly));
        assert!(kinds.contains(&BlockKind::Context));
    }

    #[test]
    fn same_change_no_conflict() {
        let v = build_merge_view("l1\nX\nl3\n", "l1\nZ\nl3\n", "l1\nZ\nl3\n");
        assert_eq!(v.conflicts, 0);
        assert!(v.blocks.iter().any(|b| b.kind == BlockKind::Same));
    }

    #[test]
    fn conflicting_changes_detected() {
        let v = build_merge_view("l1\nX\nl3\n", "l1\nL\nl3\n", "l1\nR\nl3\n");
        assert_eq!(v.conflicts, 1);
        assert_eq!(v.conflict_rows, vec![1]); // 冲突块从第 2 行（索引 1）开始
        assert!(v.rows[1].in_conflict);
        assert_eq!(v.rows[1].base_no, Some(2));
        assert_eq!(v.rows[1].left.as_ref().unwrap().text, "L");
        assert_eq!(v.rows[1].right.as_ref().unwrap().text, "R");
        // base 栏无行号错位
        assert_eq!(v.rows[2].base_no, Some(3));
    }

    #[test]
    fn insert_only_block() {
        let v = build_merge_view("a\nb\n", "a\nIA\nb\n", "a\nb\n");
        assert_eq!(v.conflicts, 0);
        let kinds: Vec<BlockKind> = v.blocks.iter().map(|b| b.kind).collect();
        assert!(kinds.contains(&BlockKind::LeftOnly));
        // 插入行：base 为空、left 有内容
        let insert_row = v
            .rows
            .iter()
            .find(|r| r.left.is_some() && r.base.is_none())
            .unwrap();
        assert_eq!(insert_row.left.as_ref().unwrap().text, "IA");
        assert_eq!(insert_row.kind, BlockKind::LeftOnly);
    }

    #[test]
    fn line_numbers_track_base() {
        let v = build_merge_view(
            "l1\nl2\nl3\nl4\nl5\n",
            "l1\nX\nl3\nl4\nl5\n",
            "l1\nl2\nl3\nY\nl5\n",
        );
        let numbers: Vec<Option<usize>> = v.rows.iter().map(|r| r.base_no).collect();
        assert_eq!(numbers, vec![Some(1), Some(2), Some(3), Some(4), Some(5)]);
    }

    #[test]
    fn render_merged_unresolved_emits_markers() {
        let v = build_merge_view("l1\nX\nl3\n", "l1\nL\nl3\n", "l1\nR\nl3\n");
        let (out, unresolved) = render_merged(&v, "LEFT", "RIGHT");
        assert_eq!(unresolved, 1);
        assert_eq!(
            out,
            vec![
                "l1",
                "<<<<<<< LEFT",
                "L",
                "=======",
                "R",
                ">>>>>>> RIGHT",
                "l3"
            ]
        );
    }

    #[test]
    fn render_merged_resolved_take_left() {
        let mut v = build_merge_view("l1\nX\nl3\n", "l1\nL\nl3\n", "l1\nR\nl3\n");
        v.blocks[1].resolution = Resolution::Left;
        let (out, unresolved) = render_merged(&v, "LEFT", "RIGHT");
        assert_eq!(unresolved, 0);
        assert_eq!(out, vec!["l1", "L", "l3"]);
    }

    #[test]
    fn render_merged_resolved_take_right() {
        let mut v = build_merge_view("l1\nX\nl3\n", "l1\nL\nl3\n", "l1\nR\nl3\n");
        v.blocks[1].resolution = Resolution::Right;
        let (out, unresolved) = render_merged(&v, "LEFT", "RIGHT");
        assert_eq!(unresolved, 0);
        assert_eq!(out, vec!["l1", "R", "l3"]);
    }

    #[test]
    fn render_merged_resolved_take_base() {
        let mut v = build_merge_view("l1\nX\nl3\n", "l1\nL\nl3\n", "l1\nR\nl3\n");
        v.blocks[1].resolution = Resolution::Base;
        let (out, _) = render_merged(&v, "LEFT", "RIGHT");
        assert_eq!(out, vec!["l1", "X", "l3"]);
    }

    #[test]
    fn render_merged_mixed_blocks() {
        let v = build_merge_view(
            "l1\nl2\nl3\nl4\nl5\n",
            "L1\nl2\nl3\nl4\nl5\n",
            "l1\nl2\nl3\nl4\nR5\n",
        );
        let (out, unresolved) = render_merged(&v, "LEFT", "RIGHT");
        assert_eq!(unresolved, 0);
        assert_eq!(out, vec!["L1", "l2", "l3", "l4", "R5"]);
    }

    #[test]
    fn conflict_rows_highlighted_cells_have_segments() {
        let v = build_merge_view("foo bar\n", "foo baz\n", "foo qux\n");
        assert_eq!(v.conflicts, 1);
        let row = &v.rows[0];
        // left/right 均有行内高亮分段
        let lj: String = row
            .left
            .as_ref()
            .unwrap()
            .segments
            .iter()
            .map(|(s, _)| s.as_str())
            .collect();
        let rj: String = row
            .right
            .as_ref()
            .unwrap()
            .segments
            .iter()
            .map(|(s, _)| s.as_str())
            .collect();
        assert_eq!(lj, "foo baz");
        assert_eq!(rj, "foo qux");
        assert!(row.left.as_ref().unwrap().segments.iter().any(|(_, c)| *c));
    }

    #[test]
    fn empty_files_no_rows() {
        let v = build_merge_view("", "", "");
        assert_eq!(v.conflicts, 0);
        assert!(v.rows.is_empty());
    }

    #[test]
    fn conflict_block_indices_match_rows() {
        // 构造两个冲突块 + 中间的公共区，验证 conflict_rows 与 blocks 下标一一对应
        let base = "l1\nX2\nl3\nl4\nX5\nl6\n";
        let left = "l1\nL2\nl3\nl4\nL5\nl6\n";
        let right = "l1\nR2\nl3\nl4\nR5\nl6\n";
        let v = build_merge_view(base, left, right);
        assert_eq!(v.conflicts, 2);
        assert_eq!(v.conflict_rows.len(), 2);
        assert_eq!(v.conflict_block_indices.len(), 2);
        // 每个 conflict_block_indices 指向 blocks 中 kind==Conflict 的块
        for &bi in &v.conflict_block_indices {
            assert_eq!(v.blocks[bi].kind, BlockKind::Conflict);
        }
        // 不同冲突块索引不重复
        assert_ne!(v.conflict_block_indices[0], v.conflict_block_indices[1]);
    }

    #[test]
    fn conflict_block_indices_resolve_blocks() {
        // 用 conflict_rows 找到的起始行必须落在对应冲突块的 rows 区间内
        let base = "a\nX\nb\n";
        let left = "a\nL\nb\n";
        let right = "a\nR\nb\n";
        let v = build_merge_view(base, left, right);
        assert_eq!(v.conflicts, 1);
        let row = v.conflict_rows[0];
        let bi = v.conflict_block_indices[0];
        // 该行属于冲突块（in_conflict 标记一致）
        assert!(v.rows[row].in_conflict);
        assert_eq!(v.blocks[bi].kind, BlockKind::Conflict);
    }
}
