//! M5 并排 Diff 视图的数据模型。
//!
//! 将行级 diff ops 展开为"并排行"序列：每一行同时描述左侧与右侧的
//! 单元格内容（含行内高亮分段）。GUI 只负责渲染该模型，逻辑可单元测试。

use crate::diff::normalize_line;
use crate::render::intra_line;
use similar::{capture_diff_slices, Algorithm, DiffTag};

/// 并排视图的一行
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SideRow {
    /// 左侧单元格（无内容则为 None，表示此行右侧是插入）
    pub left: Option<Cell>,
    /// 右侧单元格（无内容则为 None，表示此行左侧是删除）
    pub right: Option<Cell>,
    /// 行级状态：决定整行底色
    pub tag: RowTag,
    /// 左侧行号（1-based，仅左侧存在的行）
    pub left_no: Option<usize>,
    /// 右侧行号（1-based，仅右侧存在的行）
    pub right_no: Option<usize>,
}

/// 单元格内容：文本 + 行内高亮分段（bool=是否变更段）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    pub text: String,
    pub segments: Vec<(String, bool)>,
}

impl Cell {
    fn plain(text: &str) -> Self {
        Cell {
            text: text.to_string(),
            segments: vec![(text.to_string(), false)],
        }
    }
}

/// 行级状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowTag {
    Equal,
    Delete,
    Insert,
    Replace,
}

/// 差异统计
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Stats {
    pub equal: usize,
    pub delete: usize,
    pub insert: usize,
    pub replace: usize,
}

/// 忽略选项（与 CLI diff 对齐）
#[derive(Debug, Clone, Copy, Default)]
pub struct ViewOptions {
    pub ignore_whitespace: bool,
    pub ignore_trailing: bool,
    pub ignore_case: bool,
}

/// 将两个文件内容展开为并排行序列。
///
/// 行级 diff 基于归一化后的比较键（与 CLI 一致），但单元格内容始终是原始行。
/// Replace 行对做字符级 diff 得到行内高亮分段。
pub fn build_rows(left: &str, right: &str, opts: ViewOptions) -> (Vec<SideRow>, Stats) {
    let algo = Algorithm::Patience;
    let lines_l: Vec<&str> = left.lines().collect();
    let lines_r: Vec<&str> = right.lines().collect();
    let keys_l: Vec<String> = lines_l
        .iter()
        .map(|l| {
            normalize_line(
                l,
                opts.ignore_whitespace,
                opts.ignore_trailing,
                opts.ignore_case,
            )
        })
        .collect();
    let keys_r: Vec<String> = lines_r
        .iter()
        .map(|l| {
            normalize_line(
                l,
                opts.ignore_whitespace,
                opts.ignore_trailing,
                opts.ignore_case,
            )
        })
        .collect();
    let ops = capture_diff_slices(algo, &keys_l, &keys_r);

    let mut rows = Vec::new();
    let mut stats = Stats::default();
    let mut li = 0usize;
    let mut ri = 0usize;

    for op in &ops {
        match op.tag() {
            DiffTag::Equal => {
                let n = op.old_range().len();
                for k in 0..n {
                    let text = lines_l[li + k];
                    rows.push(SideRow {
                        left: Some(Cell::plain(text)),
                        right: Some(Cell::plain(text)),
                        tag: RowTag::Equal,
                        left_no: Some(li + k + 1),
                        right_no: Some(ri + k + 1),
                    });
                }
                stats.equal += n;
                li += n;
                ri += n;
            }
            DiffTag::Delete => {
                let n = op.old_range().len();
                for k in 0..n {
                    rows.push(SideRow {
                        left: Some(Cell::plain(lines_l[li + k])),
                        right: None,
                        tag: RowTag::Delete,
                        left_no: Some(li + k + 1),
                        right_no: None,
                    });
                }
                stats.delete += n;
                li += n;
            }
            DiffTag::Insert => {
                let n = op.new_range().len();
                for k in 0..n {
                    rows.push(SideRow {
                        left: None,
                        right: Some(Cell::plain(lines_r[ri + k])),
                        tag: RowTag::Insert,
                        left_no: None,
                        right_no: Some(ri + k + 1),
                    });
                }
                stats.insert += n;
                ri += n;
            }
            DiffTag::Replace => {
                let d = op.old_range().len();
                let i = op.new_range().len();
                let paired = d.min(i);
                for k in 0..paired {
                    let (segs_l, segs_r) = intra_line(lines_l[li + k], lines_r[ri + k]);
                    rows.push(SideRow {
                        left: Some(Cell {
                            text: lines_l[li + k].to_string(),
                            segments: segs_l,
                        }),
                        right: Some(Cell {
                            text: lines_r[ri + k].to_string(),
                            segments: segs_r,
                        }),
                        tag: RowTag::Replace,
                        left_no: Some(li + k + 1),
                        right_no: Some(ri + k + 1),
                    });
                }
                for k in paired..d {
                    rows.push(SideRow {
                        left: Some(Cell::plain(lines_l[li + k])),
                        right: None,
                        tag: RowTag::Delete,
                        left_no: Some(li + k + 1),
                        right_no: None,
                    });
                }
                for k in paired..i {
                    rows.push(SideRow {
                        left: None,
                        right: Some(Cell::plain(lines_r[ri + k])),
                        tag: RowTag::Insert,
                        left_no: None,
                        right_no: Some(ri + k + 1),
                    });
                }
                stats.replace += paired;
                stats.delete += d - paired;
                stats.insert += i - paired;
                li += d;
                ri += i;
            }
        }
    }

    (rows, stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_files_all_equal_rows() {
        let (rows, stats) = build_rows("a\nb\nc\n", "a\nb\nc\n", ViewOptions::default());
        assert_eq!(rows.len(), 3);
        assert!(rows.iter().all(|r| r.tag == RowTag::Equal));
        assert_eq!(rows[0].left_no, Some(1));
        assert_eq!(rows[0].right_no, Some(1));
        assert_eq!(rows[2].left_no, Some(3));
        assert_eq!(
            stats,
            Stats {
                equal: 3,
                delete: 0,
                insert: 0,
                replace: 0
            }
        );
    }

    #[test]
    fn empty_files_no_rows() {
        let (rows, stats) = build_rows("", "", ViewOptions::default());
        assert!(rows.is_empty());
        assert_eq!(stats, Stats::default());
    }

    #[test]
    fn pure_insert_rows() {
        let (rows, stats) = build_rows("a\n", "a\nX\nY\n", ViewOptions::default());
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[1].tag, RowTag::Insert);
        assert_eq!(rows[1].left, None);
        assert_eq!(rows[1].right.as_ref().unwrap().text, "X");
        assert_eq!(rows[1].left_no, None);
        assert_eq!(rows[1].right_no, Some(2));
        assert_eq!(stats.insert, 2);
        assert_eq!(stats.equal, 1);
    }

    #[test]
    fn pure_delete_rows() {
        let (rows, stats) = build_rows("a\nb\nc\n", "a\n", ViewOptions::default());
        assert_eq!(rows[1].tag, RowTag::Delete);
        assert_eq!(rows[1].right, None);
        assert_eq!(rows[1].left.as_ref().unwrap().text, "b");
        assert_eq!(rows[1].left_no, Some(2));
        assert_eq!(rows[1].right_no, None);
        assert_eq!(stats.delete, 2);
    }

    #[test]
    fn replace_rows_carry_intra_segments() {
        let (rows, stats) = build_rows("foo bar\n", "foo baz\n", ViewOptions::default());
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].tag, RowTag::Replace);
        let l = rows[0].left.as_ref().unwrap();
        let r = rows[0].right.as_ref().unwrap();
        // 分段拼接还原原文
        let lj: String = l.segments.iter().map(|(s, _)| s.as_str()).collect();
        let rj: String = r.segments.iter().map(|(s, _)| s.as_str()).collect();
        assert_eq!(lj, "foo bar");
        assert_eq!(rj, "foo baz");
        // 至少有变更段
        assert!(l.segments.iter().any(|(_, c)| *c));
        assert!(r.segments.iter().any(|(_, c)| *c));
        assert_eq!(stats.replace, 1);
    }

    #[test]
    fn ignore_whitespace_merges_rows() {
        let opts = ViewOptions {
            ignore_whitespace: true,
            ..Default::default()
        };
        let (rows, stats) = build_rows("a b\n", "ab\n", opts);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].tag, RowTag::Equal);
        assert_eq!(
            stats,
            Stats {
                equal: 1,
                delete: 0,
                insert: 0,
                replace: 0
            }
        );
        // 不忽略时是 Replace
        let (rows2, _) = build_rows("a b\n", "ab\n", ViewOptions::default());
        assert_eq!(rows2[0].tag, RowTag::Replace);
    }

    #[test]
    fn ignore_case_merges_rows() {
        let opts = ViewOptions {
            ignore_case: true,
            ..Default::default()
        };
        let (rows, _) = build_rows("Hello\n", "hello\n", opts);
        assert_eq!(rows[0].tag, RowTag::Equal);
    }

    #[test]
    fn ignore_trailing_merges_rows() {
        let opts = ViewOptions {
            ignore_trailing: true,
            ..Default::default()
        };
        let (rows, _) = build_rows("hello  \n", "hello\n", opts);
        assert_eq!(rows[0].tag, RowTag::Equal);
    }

    #[test]
    fn unicode_lines_work() {
        let (rows, stats) = build_rows(
            "中文第一行\n中文第二行\n",
            "中文第一行\n中文第二行改\n",
            ViewOptions::default(),
        );
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1].tag, RowTag::Replace);
        assert_eq!(stats.replace, 1);
    }

    #[test]
    fn unpaired_replace_split_into_delete_and_insert() {
        // 替换 1 行为 2 行：1 对 Replace + 1 个 Insert
        let (rows, stats) = build_rows("x\ny\n", "x\na\nb\n", ViewOptions::default());
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[1].tag, RowTag::Replace);
        assert_eq!(rows[2].tag, RowTag::Insert);
        assert_eq!(stats.replace, 1);
        assert_eq!(stats.insert, 1);
    }

    #[test]
    fn line_numbers_track_per_side() {
        let (rows, _) = build_rows("a\nb\nc\nd\n", "a\nc\nd\n", ViewOptions::default());
        // a(1,1) b删除(2,-) c(3,2) d(4,3)
        assert_eq!(rows[0].left_no, Some(1));
        assert_eq!(rows[0].right_no, Some(1));
        assert_eq!(rows[1].left_no, Some(2));
        assert_eq!(rows[1].right_no, None);
        assert_eq!(rows[2].left_no, Some(3));
        assert_eq!(rows[2].right_no, Some(2));
        assert_eq!(rows[3].left_no, Some(4));
        assert_eq!(rows[3].right_no, Some(3));
    }
}
