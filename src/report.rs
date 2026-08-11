//! 文本/CSV 对比报告导出（P11）。
//!
//! 在 HTML 报告（htmlreport.rs）之外补充两种轻量格式：
//! - 文本报告（.txt）：统计摘要 + 差异条目表（[L]/[R]/[C]/[M] 状态标记）
//! - CSV 报告（.csv）：机器可读，每行一个条目，可用 Excel/脚本处理
//!
//! 与 `--html` 平级：`bcr compare A B --txt r.txt --csv r.csv`

use crate::compare::{CompareResult, FileStatus};

/// CSV 转义：含逗号/引号/换行时包双引号并转义内部引号
fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// 渲染文本报告：标题 + 统计 + 条目表
pub fn render_txt(left: &str, right: &str, result: &CompareResult) -> String {
    let mut out = String::new();
    let st = result.stats;
    out.push_str(&format!("bcr compare: {}  ↔  {}\n", left, right));
    out.push_str(&format!(
        "统计: {} 相同, {} 仅左侧, {} 仅右侧, {} 内容不同, {} 移动/重命名\n",
        st.same, st.left_only, st.right_only, st.differ, st.moved
    ));
    out.push_str(&format!("条目总数: {}\n", result.entries.len()));
    out.push_str("----------------------------------------\n");
    for e in &result.entries {
        let letter = e.status.letter();
        let desc = match e.status {
            FileStatus::Moved => format!("{} → {}", e.rel, e.moved_to.as_deref().unwrap_or("")),
            _ => e.rel.clone(),
        };
        // 两侧大小
        let sizes = match (&e.left, &e.right) {
            (Some(l), Some(r)) => format!("{}B → {}B", l.size, r.size),
            (Some(l), None) => format!("{}B → -", l.size),
            (None, Some(r)) => format!("- → {}B", r.size),
            (None, None) => String::new(),
        };
        if sizes.is_empty() {
            out.push_str(&format!("[{letter}] {desc}\n"));
        } else {
            out.push_str(&format!("[{letter}] {desc}  ({sizes})\n"));
        }
    }
    out
}

/// 渲染 CSV 报告：表头 + 每行一个条目
pub fn render_csv(left: &str, right: &str, result: &CompareResult) -> String {
    let mut out = String::new();
    out.push_str("status,path,left_size,right_size,moved_to\n");
    for e in &result.entries {
        let letter = e.status.letter();
        let lsize = e
            .left
            .as_ref()
            .map(|m| m.size.to_string())
            .unwrap_or_default();
        let rsize = e
            .right
            .as_ref()
            .map(|m| m.size.to_string())
            .unwrap_or_default();
        let moved = e.moved_to.clone().unwrap_or_default();
        out.push_str(&format!(
            "{},{},{},{},{}\n",
            letter,
            csv_escape(&e.rel),
            lsize,
            rsize,
            csv_escape(&moved)
        ));
    }
    // 统计追加为注释行（不破坏机器可读性，Excel 忽略 # 开头的行）
    let st = result.stats;
    out.push_str(&format!(
        "# left={}, right={}, same={}, left_only={}, right_only={}, differ={}, moved={}\n",
        left, right, st.same, st.left_only, st.right_only, st.differ, st.moved
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compare::{CompareResult, CompareStats, FileEntry};
    use crate::fsscan::FileMeta;
    use std::time::UNIX_EPOCH;

    fn meta(size: u64) -> Option<FileMeta> {
        Some(FileMeta {
            size,
            mtime: UNIX_EPOCH,
        })
    }

    fn sample_result() -> CompareResult {
        CompareResult {
            entries: vec![
                FileEntry {
                    rel: "a.txt".into(),
                    status: FileStatus::Differ,
                    left: meta(10),
                    right: meta(12),
                    moved_to: None,
                },
                FileEntry {
                    rel: "old.rs".into(),
                    status: FileStatus::Moved,
                    left: meta(5),
                    right: meta(5),
                    moved_to: Some("new.rs".into()),
                },
                FileEntry {
                    rel: "only_l.log".into(),
                    status: FileStatus::LeftOnly,
                    left: meta(3),
                    right: None,
                    moved_to: None,
                },
            ],
            stats: CompareStats {
                same: 1,
                left_only: 1,
                right_only: 0,
                differ: 1,
                moved: 1,
            },
            warnings: vec![],
        }
    }

    #[test]
    fn txt_report_includes_entries_and_stats() {
        let r = render_txt("/l", "/r", &sample_result());
        assert!(r.contains("[C] a.txt"));
        assert!(r.contains("[M] old.rs → new.rs"));
        assert!(r.contains("[L] only_l.log"));
        assert!(r.contains("1 移动/重命名"));
    }

    #[test]
    fn csv_report_machine_readable() {
        let r = render_csv("/l", "/r", &sample_result());
        let lines: Vec<&str> = r.lines().collect();
        assert_eq!(lines[0], "status,path,left_size,right_size,moved_to");
        assert_eq!(lines[1], "C,a.txt,10,12,");
        assert_eq!(lines[2], "M,old.rs,5,5,new.rs");
        assert_eq!(lines[3], "L,only_l.log,3,,");
        assert!(lines[4].starts_with("# left="));
    }

    #[test]
    fn csv_escape_quotes_comma() {
        assert_eq!(csv_escape("plain"), "plain");
        assert_eq!(csv_escape("a,b"), "\"a,b\"");
        assert_eq!(csv_escape("say \"hi\""), "\"say \"\"hi\"\"\"");
    }
}
