//! 文本/CSV 对比报告导出（P11）。
//!
//! 在 HTML 报告（htmlreport.rs）之外补充两种轻量格式：
//! - 文本报告（.txt）：统计摘要 + 差异条目表（[L]/[R]/[C]/[M] 状态标记）
//! - CSV 报告（.csv）：机器可读，每行一个条目，可用 Excel/脚本处理
//!
//! 与 `--html` 平级：`bcr compare A B --txt r.txt --csv r.csv`

use crate::compare::{CompareResult, FileStatus};

/// 报告可选字段
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportField {
    /// 状态标记 [L]/[R]/[C]/[M]
    Status,
    /// 路径
    Path,
    /// 两侧大小
    Size,
    /// 修改时间
    Mtime,
    /// 移动/重命名目标
    Moved,
}

/// 解析报告字段列表（逗号分隔，如 "status,path,size"）；空 = 全部字段
pub fn parse_fields(s: &str) -> Result<Vec<ReportField>, String> {
    if s.trim().is_empty() {
        return Ok(vec![
            ReportField::Status,
            ReportField::Path,
            ReportField::Size,
            ReportField::Mtime,
            ReportField::Moved,
        ]);
    }
    let mut out = Vec::new();
    for tok in s.split(',') {
        let tok = tok.trim().to_ascii_lowercase();
        let f = match tok.as_str() {
            "status" => ReportField::Status,
            "path" => ReportField::Path,
            "size" => ReportField::Size,
            "mtime" => ReportField::Mtime,
            "moved" => ReportField::Moved,
            _ => return Err(format!("未知报告字段: {}", tok)),
        };
        if !out.contains(&f) {
            out.push(f);
        }
    }
    Ok(out)
}

/// CSV 转义：含逗号/引号/换行时包双引号并转义内部引号
fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// 报告布局选项
#[derive(Debug, Clone)]
pub struct ReportOptions {
    /// 自定义标题（None = 默认 "bcr compare: L ↔ R"）
    pub title: Option<String>,
    /// 是否输出统计行（默认 true）
    pub include_stats: bool,
    /// 排序方式：path（默认，按路径）| status（按状态分组）| size（按差异大小）
    pub sort: String,
    /// 按状态分组输出（仅文本报告；CSV 保持机器可读）
    pub group_by_status: bool,
}

impl Default for ReportOptions {
    fn default() -> Self {
        ReportOptions {
            title: None,
            include_stats: true,
            sort: "path".to_string(),
            group_by_status: false,
        }
    }
}

/// 按选项排序条目（path=字典序；status=状态字母序；size=差异大小降序）
fn sorted_entries<'a>(result: &'a CompareResult, sort: &str) -> Vec<&'a crate::compare::FileEntry> {
    let mut entries: Vec<&crate::compare::FileEntry> = result.entries.iter().collect();
    match sort {
        "status" => entries.sort_by_key(|e| (e.status.letter(), e.rel.clone())),
        "size" => entries.sort_by_key(|e| {
            let l = e.left.as_ref().map(|m| m.size).unwrap_or(0);
            let r = e.right.as_ref().map(|m| m.size).unwrap_or(0);
            std::cmp::Reverse(l.abs_diff(r))
        }),
        _ => entries.sort_by_key(|e| e.rel.clone()),
    }
    entries
}

/// 渲染文本报告：标题 + 统计 + 条目表（fields 控制每行展示的字段）
pub fn render_txt_fields(
    left: &str,
    right: &str,
    result: &CompareResult,
    fields: &[ReportField],
) -> String {
    render_txt_opts(left, right, result, fields, &ReportOptions::default())
}

/// 渲染文本报告（带布局选项：自定义标题/统计开关）
pub fn render_txt_opts(
    left: &str,
    right: &str,
    result: &CompareResult,
    fields: &[ReportField],
    opts: &ReportOptions,
) -> String {
    let mut out = String::new();
    let st = result.stats;
    match &opts.title {
        Some(t) => out.push_str(&format!("{}\n", t)),
        None => out.push_str(&format!("bcr compare: {}  ↔  {}\n", left, right)),
    }
    if opts.include_stats {
        out.push_str(&format!(
            "统计: {} 相同, {} 仅左侧, {} 仅右侧, {} 内容不同, {} 移动/重命名\n",
            st.same, st.left_only, st.right_only, st.differ, st.moved
        ));
        out.push_str(&format!("条目总数: {}\n", result.entries.len()));
    }
    out.push_str("----------------------------------------\n");
    let sorted = sorted_entries(result, &opts.sort);
    if opts.group_by_status {
        // 按状态分组输出：C / L / R / M
        for letter in ['C', 'L', 'R', 'M'] {
            let group: Vec<_> = sorted
                .iter()
                .filter(|e| e.status.letter() == letter)
                .collect();
            if group.is_empty() {
                continue;
            }
            out.push_str(&format!("--- {} ---\n", status_group_label(letter)));
            for e in group {
                out.push_str(&format_entry_line(e, fields));
                out.push('\n');
            }
        }
    } else {
        for e in &sorted {
            out.push_str(&format_entry_line(e, fields));
            out.push('\n');
        }
    }
    out
}

/// 状态分组标题
fn status_group_label(letter: char) -> &'static str {
    match letter {
        'C' => "内容不同",
        'L' => "仅左侧",
        'R' => "仅右侧",
        'M' => "移动/重命名",
        _ => "相同",
    }
}

/// 渲染单个条目的文本行（按 fields 选择）
fn format_entry_line(e: &crate::compare::FileEntry, fields: &[ReportField]) -> String {
    let mut line = String::new();
    if fields.contains(&ReportField::Status) {
        line.push_str(&format!("[{}] ", e.status.letter()));
    }
    if fields.contains(&ReportField::Path) {
        let desc = match e.status {
            FileStatus::Moved => format!("{} → {}", e.rel, e.moved_to.as_deref().unwrap_or("")),
            _ => e.rel.clone(),
        };
        line.push_str(&desc);
    }
    if fields.contains(&ReportField::Size) {
        let sizes = match (&e.left, &e.right) {
            (Some(l), Some(r)) => format!("  ({}B → {}B)", l.size, r.size),
            (Some(l), None) => format!("  ({}B → -)", l.size),
            (None, Some(r)) => format!("  (- → {}B)", r.size),
            (None, None) => String::new(),
        };
        line.push_str(&sizes);
    }
    if fields.contains(&ReportField::Mtime) {
        let m = match (&e.left, &e.right) {
            (Some(l), Some(r)) => {
                format!(
                    "  [{} ↔ {}]",
                    fmt_mtime_pub(l.mtime),
                    fmt_mtime_pub(r.mtime)
                )
            }
            (Some(l), None) => format!("  [{}]", fmt_mtime_pub(l.mtime)),
            (None, Some(r)) => format!("  [{}]", fmt_mtime_pub(r.mtime)),
            (None, None) => String::new(),
        };
        line.push_str(&m);
    }
    if fields.contains(&ReportField::Moved) {
        if let Some(to) = &e.moved_to {
            line.push_str(&format!("  (moved: {})", to));
        }
    }
    line.trim_end().to_string()
}

/// 渲染文本报告（全部字段，向后兼容）
#[allow(dead_code)] // 公共 API，保留向后兼容
pub fn render_txt(left: &str, right: &str, result: &CompareResult) -> String {
    render_txt_fields(left, right, result, &parse_fields("").unwrap())
}

/// 渲染 CSV 报告：表头 + 每行一个条目（fields 控制列）
/// 渲染 CSV 报告（带布局选项：自定义标题/统计开关）
pub fn render_csv_opts(
    left: &str,
    right: &str,
    result: &CompareResult,
    fields: &[ReportField],
    opts: &ReportOptions,
) -> String {
    let mut header: Vec<String> = Vec::new();
    for f in fields {
        let name = match f {
            ReportField::Status => "status",
            ReportField::Path => "path",
            ReportField::Size => "left_size,right_size",
            ReportField::Mtime => "left_mtime,right_mtime",
            ReportField::Moved => "moved_to",
        };
        for part in name.split(',') {
            header.push(part.to_string());
        }
    }
    let mut out = String::new();
    // 自定义标题作为 # 注释行（保持机器可读）
    if let Some(t) = &opts.title {
        out.push_str(&format!("# {}\n", t));
    }
    out.push_str(&header.join(","));
    out.push('\n');
    // 排序（CSV 不分组，保持单表结构）
    let sorted = sorted_entries(result, &opts.sort);
    for e in sorted {
        let mut cols: Vec<String> = Vec::new();
        for f in fields {
            match f {
                ReportField::Status => cols.push(e.status.letter().to_string()),
                ReportField::Path => cols.push(csv_escape(&e.rel)),
                ReportField::Size => {
                    cols.push(
                        e.left
                            .as_ref()
                            .map(|m| m.size.to_string())
                            .unwrap_or_default(),
                    );
                    cols.push(
                        e.right
                            .as_ref()
                            .map(|m| m.size.to_string())
                            .unwrap_or_default(),
                    );
                }
                ReportField::Mtime => {
                    cols.push(
                        e.left
                            .as_ref()
                            .map(|m| fmt_mtime_pub(m.mtime))
                            .unwrap_or_default(),
                    );
                    cols.push(
                        e.right
                            .as_ref()
                            .map(|m| fmt_mtime_pub(m.mtime))
                            .unwrap_or_default(),
                    );
                }
                ReportField::Moved => cols.push(csv_escape(e.moved_to.as_deref().unwrap_or(""))),
            }
        }
        out.push_str(&cols.join(","));
        out.push('\n');
    }
    // 统计追加为注释行（不破坏机器可读性，Excel 忽略 # 开头的行）
    if opts.include_stats {
        let st = result.stats;
        out.push_str(&format!(
            "# left={}, right={}, same={}, left_only={}, right_only={}, differ={}, moved={}\n",
            left, right, st.same, st.left_only, st.right_only, st.differ, st.moved
        ));
    }
    out
}

/// 渲染 CSV 报告：表头 + 每行一个条目（fields 控制列）
pub fn render_csv_fields(
    left: &str,
    right: &str,
    result: &CompareResult,
    fields: &[ReportField],
) -> String {
    render_csv_opts(left, right, result, fields, &ReportOptions::default())
}

/// 渲染 CSV 报告（全部字段，向后兼容）
#[allow(dead_code)] // 公共 API，保留向后兼容
pub fn render_csv(left: &str, right: &str, result: &CompareResult) -> String {
    render_csv_fields(left, right, result, &parse_fields("").unwrap())
}

/// SystemTime → 可读时间串（%Y-%m-%d %H:%M:%S UTC）
pub fn fmt_mtime_pub(t: std::time::SystemTime) -> String {
    let secs = t
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // 简单转换：1970 起的天数 + 时分秒（避免引入 chrono 依赖）
    let days = secs / 86400;
    let rem = secs % 86400;
    let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    // days-from-civil（Howard Hinnant）
    let z = days as i64 + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let yy = if mo <= 2 { y + 1 } else { y };
    format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", yy, mo, d, h, m, s)
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
            mode: None,
            symlink: None,
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
                    attrs_differ: false,
                },
                FileEntry {
                    rel: "old.rs".into(),
                    status: FileStatus::Moved,
                    left: meta(5),
                    right: meta(5),
                    moved_to: Some("new.rs".into()),
                    attrs_differ: false,
                },
                FileEntry {
                    rel: "only_l.log".into(),
                    status: FileStatus::LeftOnly,
                    left: meta(3),
                    right: None,
                    moved_to: None,
                    attrs_differ: false,
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
        assert_eq!(
            lines[0],
            "status,path,left_size,right_size,left_mtime,right_mtime,moved_to"
        );
        assert_eq!(
            lines[1],
            "C,a.txt,10,12,1970-01-01 00:00:00,1970-01-01 00:00:00,"
        );
        assert_eq!(
            lines[2],
            "M,old.rs,5,5,1970-01-01 00:00:00,1970-01-01 00:00:00,new.rs"
        );
        assert_eq!(lines[3], "L,only_l.log,3,,1970-01-01 00:00:00,,");
        assert!(lines[4].starts_with("# left="));
    }

    #[test]
    fn csv_report_field_selection() {
        let fields = parse_fields("status,path").unwrap();
        let r = render_csv_fields("/l", "/r", &sample_result(), &fields);
        let lines: Vec<&str> = r.lines().collect();
        assert_eq!(lines[0], "status,path");
        assert_eq!(lines[1], "C,a.txt");
        assert_eq!(lines[2], "M,old.rs");
    }

    #[test]
    fn txt_report_field_selection() {
        let fields = parse_fields("path,size").unwrap();
        let r = render_txt_fields("/l", "/r", &sample_result(), &fields);
        // 无状态标记，有大小
        assert!(r.contains("a.txt  (10B → 12B)"));
        assert!(!r.contains("[C]"));
    }

    #[test]
    fn parse_fields_unknown_errors() {
        assert!(parse_fields("status,foo").is_err());
        assert!(parse_fields("").unwrap().len() == 5);
    }

    #[test]
    fn csv_escape_quotes_comma() {
        assert_eq!(csv_escape("plain"), "plain");
        assert_eq!(csv_escape("a,b"), "\"a,b\"");
        assert_eq!(csv_escape("say \"hi\""), "\"say \"\"hi\"\"\"");
    }
}
