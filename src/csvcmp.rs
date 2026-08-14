//! CSV/表格对比（P7：结构化数据 diff）。
//!
//! `bcr csv LEFT RIGHT`：解析两个 CSV 文件（RFC 4180 子集），按主键对齐
//! 行后逐列对比，输出行级状态与列级差异。支持：
//! - `--key <列名|列号>`：按指定列对齐（类似数据库 join）；缺省按行号对齐
//! - `--delimiter <字符>`：自定义分隔符（默认逗号，支持 `\t` 制表符）
//! - `--no-header`：首行不是表头
//! - 输出 `[L] 仅左侧 / [R] 仅右侧 / [M] 修改（列出变化的列）/ [S] 相同`

use crate::i18n::{fmt, t, Key};
use clap::Args;
use std::collections::BTreeMap;
use std::io::{self, IsTerminal};

/// csv 子命令参数
#[derive(Args, Debug)]
pub struct CsvArgs {
    /// 左侧 CSV 文件
    pub left: String,

    /// 右侧 CSV 文件
    pub right: String,

    /// 对齐主键：列名（如 id）或列号（0 起）；缺省按行号对齐
    #[arg(long)]
    pub key: Option<String>,

    /// 字段分隔符（支持 \t 表示制表符）
    #[arg(long, default_value = ",")]
    pub delimiter: String,

    /// 首行不是表头（表头默认参与对比并作为列名）
    #[arg(long)]
    pub no_header: bool,

    /// 同时显示相同行
    #[arg(long)]
    pub show_same: bool,

    /// 输出统计信息
    #[arg(long)]
    pub summary: bool,

    /// 颜色输出：auto | always | never
    #[arg(long, default_value = "auto", value_parser = ["auto", "always", "never"])]
    pub color: String,

    /// 以 JSON 契约输出结果（schema: csv.v1）
    #[arg(long)]
    pub json: bool,
}

/// 解析后的表格：表头 + 数据行
#[derive(Clone)]
pub(crate) struct Table {
    /// 列名（无表头时用 "col0/col1/..." 代替）
    pub(crate) headers: Vec<String>,
    /// 数据行
    pub(crate) rows: Vec<Vec<String>>,
}

/// 解析 CSV 文本（RFC 4180 子集：逗号分隔、双引号引用、"" 转义、引号内可含分隔符与换行）
pub(crate) fn parse_csv(text: &str, delim: char) -> Vec<Vec<String>> {
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;
    let mut chars = text.chars().peekable();
    let mut row_started = false;

    while let Some(c) = chars.next() {
        if in_quotes {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    field.push('"');
                    chars.next();
                } else {
                    in_quotes = false;
                }
            } else {
                field.push(c);
            }
        } else {
            match c {
                '"' => {
                    // 字段开头的引号进入引用模式
                    if field.is_empty() {
                        in_quotes = true;
                        row_started = true;
                    } else {
                        field.push(c);
                    }
                }
                d if d == delim => {
                    row.push(std::mem::take(&mut field));
                    row_started = true;
                }
                '\n' => {
                    row.push(std::mem::take(&mut field));
                    rows.push(std::mem::take(&mut row));
                    row_started = false;
                }
                '\r' => {} // 忽略 CR（兼容 CRLF）
                c => {
                    field.push(c);
                    row_started = true;
                }
            }
        }
    }
    // 末尾未换行
    if row_started || !field.is_empty() || !row.is_empty() {
        row.push(std::mem::take(&mut field));
        rows.push(row);
    }
    // 去掉全空行
    rows.retain(|r| r.iter().any(|f| !f.is_empty()));
    rows
}

/// P37-1c：RFC 4180 序列化表格（表头 + 数据行）。
///
/// 字段含分隔符 / 双引号 / 换行时加引号并转义 `"`。
pub(crate) fn serialize_csv(table: &Table, delim: char) -> String {
    fn quote(field: &str, delim: char) -> String {
        if field.contains(delim)
            || field.contains('"')
            || field.contains('\n')
            || field.contains('\r')
        {
            format!("\"{}\"", field.replace('"', "\"\""))
        } else {
            field.to_string()
        }
    }
    let mut out = String::new();
    let cols = table
        .headers
        .len()
        .max(table.rows.iter().map(|r| r.len()).max().unwrap_or(0));
    if !table.headers.is_empty() {
        out.push_str(
            &table
                .headers
                .iter()
                .map(|h| quote(h, delim))
                .collect::<Vec<_>>()
                .join(&delim.to_string()),
        );
        out.push('\n');
    }
    for row in &table.rows {
        let cells: Vec<String> = (0..cols)
            .map(|i| quote(row.get(i).map(|s| s.as_str()).unwrap_or(""), delim))
            .collect();
        out.push_str(&cells.join(&delim.to_string()));
        out.push('\n');
    }
    out
}

impl Table {
    pub(crate) fn new(text: &str, delim: char, no_header: bool) -> Self {
        let parsed = parse_csv(text, delim);
        let (headers, rows) = if no_header || parsed.is_empty() {
            let width = parsed.iter().map(|r| r.len()).max().unwrap_or(0);
            let headers: Vec<String> = (0..width).map(|i| format!("col{i}")).collect();
            (headers, parsed)
        } else {
            let headers = parsed[0].clone();
            (headers, parsed[1..].to_vec())
        };
        Table { headers, rows }
    }

    /// 克隆表格（行列操作用）
    pub(crate) fn clone_table(&self) -> Self {
        self.clone()
    }

    /// 取主键值：--key 指定列名或列号；返回 (key, 行内是否可用)
    pub(crate) fn key_of(&self, row: &[String], key: &str) -> Option<String> {
        if let Ok(idx) = key.parse::<usize>() {
            return row.get(idx).cloned();
        }
        self.headers
            .iter()
            .position(|h| h == key)
            .and_then(|i| row.get(i).cloned())
    }
}

/// 行级统计
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RowStats {
    pub same: usize,
    pub left_only: usize,
    pub right_only: usize,
    pub modified: usize,
}

/// 行对齐状态（供 GUI 渲染）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RowStatus {
    Same,
    LeftOnly,
    RightOnly,
    Modified,
}

/// 行对齐结果（结构化，供 GUI 表格视图渲染）
pub(crate) struct AlignedRow {
    /// 左侧行号（None = 仅右侧）
    pub a_no: Option<usize>,
    /// 右侧行号（None = 仅左侧）
    pub b_no: Option<usize>,
    /// 行状态
    pub status: RowStatus,
    /// 修改的列索引（Modified 时非空）
    pub changed_cols: Vec<usize>,
}

/// 结构化对比：按 key（或行号）对齐两表，返回逐行状态 + 变化列
pub(crate) fn align_tables(a: &Table, b: &Table, key: Option<&str>) -> (Vec<AlignedRow>, RowStats) {
    let mut out: Vec<AlignedRow> = Vec::new();
    let mut stats = RowStats::default();

    match key {
        Some(k) => {
            // 按主键对齐：右侧建立 key -> 行索引
            let mut b_idx: BTreeMap<String, usize> = BTreeMap::new();
            for (i, row) in b.rows.iter().enumerate() {
                if let Some(_kv) = b.key_of(row, k) {
                    b_idx.insert(_kv, i);
                }
            }
            let mut used_b: std::collections::BTreeSet<usize> = Default::default();
            for (i, row) in a.rows.iter().enumerate() {
                let Some(kv) = a.key_of(row, k) else {
                    continue;
                };
                if let Some(&j) = b_idx.get(&kv) {
                    used_b.insert(j);
                    let (status, changed) = compare_row_cells(
                        a.headers.as_slice(),
                        row,
                        b.headers.as_slice(),
                        &b.rows[j],
                    );
                    match status {
                        RowStatus::Same => stats.same += 1,
                        RowStatus::Modified => stats.modified += 1,
                        _ => {}
                    }
                    out.push(AlignedRow {
                        a_no: Some(i),
                        b_no: Some(j),
                        status,
                        changed_cols: changed,
                    });
                } else {
                    stats.left_only += 1;
                    out.push(AlignedRow {
                        a_no: Some(i),
                        b_no: None,
                        status: RowStatus::LeftOnly,
                        changed_cols: Vec::new(),
                    });
                }
            }
            for (j, row) in b.rows.iter().enumerate() {
                if used_b.contains(&j) {
                    continue;
                }
                if let Some(_kv) = b.key_of(row, k) {
                    stats.right_only += 1;
                    out.push(AlignedRow {
                        a_no: None,
                        b_no: Some(j),
                        status: RowStatus::RightOnly,
                        changed_cols: Vec::new(),
                    });
                }
            }
        }
        None => {
            // 按行号对齐
            let n = a.rows.len().max(b.rows.len());
            for i in 0..n {
                match (a.rows.get(i), b.rows.get(i)) {
                    (Some(ra), Some(rb)) => {
                        let (status, changed) =
                            compare_row_cells(a.headers.as_slice(), ra, b.headers.as_slice(), rb);
                        match status {
                            RowStatus::Same => stats.same += 1,
                            RowStatus::Modified => stats.modified += 1,
                            _ => {}
                        }
                        out.push(AlignedRow {
                            a_no: Some(i),
                            b_no: Some(i),
                            status,
                            changed_cols: changed,
                        });
                    }
                    (Some(_), None) => {
                        stats.left_only += 1;
                        out.push(AlignedRow {
                            a_no: Some(i),
                            b_no: None,
                            status: RowStatus::LeftOnly,
                            changed_cols: Vec::new(),
                        });
                    }
                    (None, Some(_)) => {
                        stats.right_only += 1;
                        out.push(AlignedRow {
                            a_no: None,
                            b_no: Some(i),
                            status: RowStatus::RightOnly,
                            changed_cols: Vec::new(),
                        });
                    }
                    (None, None) => {}
                }
            }
        }
    }
    (out, stats)
}

/// 对比一对已对齐的行：返回状态与变化列索引
fn compare_row_cells(
    _a_headers: &[String],
    a_row: &[String],
    _b_headers: &[String],
    b_row: &[String],
) -> (RowStatus, Vec<usize>) {
    if a_row == b_row {
        return (RowStatus::Same, Vec::new());
    }
    let mut changed: Vec<usize> = Vec::new();
    let n = a_row.len().max(b_row.len());
    for i in 0..n {
        let av = a_row.get(i).cloned().unwrap_or_default();
        let bv = b_row.get(i).cloned().unwrap_or_default();
        if av != bv {
            changed.push(i);
        }
    }
    (RowStatus::Modified, changed)
}

/// 对比两个 CSV，返回渲染行（字符串，便于测试）
pub(crate) fn compare_csv(a: &Table, b: &Table, key: Option<&str>) -> (Vec<String>, RowStats) {
    let mut out: Vec<String> = Vec::new();

    // 表头差异
    if a.headers != b.headers {
        out.push(format!(
            "{}  表头: {} -> {}",
            t(Key::CsvHeaderDiff),
            a.headers.join(","),
            b.headers.join(",")
        ));
    }

    let (aligned, stats) = align_tables(a, b, key);
    for ar in &aligned {
        match ar.status {
            RowStatus::Same => {}
            RowStatus::LeftOnly => {
                let i = ar.a_no.unwrap_or(0);
                let Some(row) = a.rows.get(i) else { continue };
                if row.is_empty() {
                    continue;
                }
                if let Some(k) = key {
                    if let Some(kv) = a.key_of(row, k) {
                        out.push(format!("[L] 行{}  {}={}", i + 1, k, kv));
                        continue;
                    }
                }
                out.push(format!("[L] 行{}  {}", i + 1, row.join(",")));
            }
            RowStatus::RightOnly => {
                let j = ar.b_no.unwrap_or(0);
                let Some(row) = b.rows.get(j) else { continue };
                if row.is_empty() {
                    continue;
                }
                if let Some(k) = key {
                    if let Some(kv) = b.key_of(row, k) {
                        out.push(format!("[R] 行{}  {}={}", j + 1, k, kv));
                        continue;
                    }
                }
                out.push(format!("[R] 行{}  {}", j + 1, row.join(",")));
            }
            RowStatus::Modified => {
                let (i, j) = (ar.a_no.unwrap_or(0), ar.b_no.unwrap_or(0));
                let a_row = &a.rows[i];
                let b_row = &b.rows[j];
                let mut changes: Vec<String> = Vec::new();
                for &ci in &ar.changed_cols {
                    let av = a_row.get(ci).cloned().unwrap_or_default();
                    let bv = b_row.get(ci).cloned().unwrap_or_default();
                    let h = a
                        .headers
                        .get(ci)
                        .or(b.headers.get(ci))
                        .cloned()
                        .unwrap_or_else(|| format!("col{ci}"));
                    changes.push(format!("{h}: {av} -> {bv}"));
                }
                out.push(format!(
                    "[M] 行{} ↔ {}  {}",
                    i + 1,
                    j + 1,
                    changes.join("; ")
                ));
            }
        }
    }

    if out.is_empty() {
        out.push(t(Key::CsvIdentical).to_string());
    }
    (out, stats)
}

/// 判断路径是否为 CSV/TSV 文件（GUI 表格视图路由用）
pub fn is_csv_file(path: &str) -> bool {
    let p = path.to_ascii_lowercase();
    p.ends_with(".csv") || p.ends_with(".tsv") || p.ends_with(".tab")
}

/// 运行 csv 子命令，返回进程退出码（0=无差异，1=有差异，2=错误）
pub fn run(args: &CsvArgs) -> i32 {
    let delim = match args.delimiter.as_str() {
        "\\t" | "tab" => '\t',
        s if s.chars().count() == 1 => s.chars().next().unwrap(),
        _ => {
            eprintln!("bcr: {}", fmt(Key::CsvBadDelimiter, &[&args.delimiter]));
            return 2;
        }
    };

    let read = |p: &str| match std::fs::read_to_string(p) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("bcr: {}", fmt(Key::CannotRead, &[p, &e.to_string()]));
            std::process::exit(2);
        }
    };
    let a = Table::new(&read(&args.left), delim, args.no_header);
    let b = Table::new(&read(&args.right), delim, args.no_header);

    let (lines, stats) = compare_csv(&a, &b, args.key.as_deref());

    // JSON 契约输出(csv.v1)
    if args.json {
        let v = crate::jsonout::envelope_csv(
            &args.left,
            &args.right,
            stats.same,
            stats.left_only,
            stats.right_only,
            stats.modified,
        );
        println!("{}", serde_json::to_string(&v).unwrap_or_default());
        return if stats.left_only + stats.right_only + stats.modified > 0 {
            1
        } else {
            0
        };
    }

    let color = match args.color.as_str() {
        "always" => true,
        "never" => false,
        _ => io::stdout().is_terminal(),
    };
    for l in &lines {
        // 简单着色：行首 [L]/[R]/[M] 标红蓝黄
        if color {
            if l.starts_with("[L]") {
                println!("\x1b[31m{l}\x1b[0m");
            } else if l.starts_with("[R]") {
                println!("\x1b[34m{l}\x1b[0m");
            } else if l.starts_with("[M]") {
                println!("\x1b[33m{l}\x1b[0m");
            } else {
                println!("{l}");
            }
        } else {
            println!("{l}");
        }
    }

    if args.summary {
        println!(
            "{}",
            fmt(
                Key::SummaryCsv,
                &[
                    &stats.same.to_string(),
                    &stats.left_only.to_string(),
                    &stats.right_only.to_string(),
                    &stats.modified.to_string(),
                ]
            )
        );
    }

    if stats.left_only + stats.right_only + stats.modified > 0 {
        1
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tbl(text: &str) -> Table {
        Table::new(text, ',', false)
    }

    #[test]
    fn parse_basic_and_quoted() {
        let t = tbl("a,b,c\n1,\"x,y\",3\n");
        assert_eq!(t.headers, vec!["a", "b", "c"]);
        assert_eq!(t.rows.len(), 1);
        assert_eq!(t.rows[0], vec!["1", "x,y", "3"]);
    }

    #[test]
    fn parse_escaped_quotes_and_newline() {
        let t = tbl("k,v\n1,\"line1\nline2\"\n");
        assert_eq!(t.rows[0][1], "line1\nline2");
        let t2 = tbl("k,v\n1,\"say \"\"hi\"\"\"\n");
        assert_eq!(t2.rows[0][1], "say \"hi\"");
    }

    #[test]
    fn no_header_generates_col_names() {
        let t = Table::new("1,2\n3,4\n", ',', true);
        assert_eq!(t.headers, vec!["col0", "col1"]);
        assert_eq!(t.rows.len(), 2);
    }

    #[test]
    fn identical_files_no_diff() {
        let a = tbl("id,name\n1,alice\n2,bob\n");
        let b = tbl("id,name\n1,alice\n2,bob\n");
        let (lines, stats) = compare_csv(&a, &b, None);
        assert!(lines.contains(&t(Key::CsvIdentical).to_string()));
        assert_eq!(stats.same, 2);
        assert_eq!(stats.modified, 0);
    }

    #[test]
    fn row_alignment_detects_add_remove() {
        let a = tbl("id,name\n1,alice\n");
        let b = tbl("id,name\n1,alice\n2,bob\n");
        let (lines, stats) = compare_csv(&a, &b, None);
        assert!(lines.iter().any(|l| l.starts_with("[R]")));
        assert_eq!(stats.right_only, 1);
        assert_eq!(stats.same, 1);
    }

    #[test]
    fn key_alignment_matches_by_key() {
        let a = tbl("id,name\n1,alice\n2,bob\n");
        let b = tbl("id,name\n2,BOB\n1,alice\n");
        let (lines, stats) = compare_csv(&a, &b, Some("id"));
        // 按 id 对齐：行2 的 bob -> BOB 是修改，不是删除+新增
        assert_eq!(stats.modified, 1);
        assert_eq!(stats.same, 1);
        assert_eq!(stats.left_only, 0);
        assert_eq!(stats.right_only, 0);
        assert!(lines.iter().any(|l| l.contains("name: bob -> BOB")));
    }

    #[test]
    fn key_alignment_detects_orphans() {
        let a = tbl("id,v\n1,x\n3,z\n");
        let b = tbl("id,v\n1,x\n2,y\n");
        let (_, stats) = compare_csv(&a, &b, Some("id"));
        assert_eq!(stats.left_only, 1);
        assert_eq!(stats.right_only, 1);
        assert_eq!(stats.same, 1);
    }

    #[test]
    fn header_diff_reported() {
        let a = tbl("id,name\n1,a\n");
        let b = tbl("id,fullname\n1,a\n");
        let (lines, _) = compare_csv(&a, &b, None);
        assert!(lines.iter().any(|l| l.contains("表头")));
    }

    #[test]
    fn tab_delimiter() {
        let t = Table::new("a\tb\n1\t2\n", '\t', false);
        assert_eq!(t.rows[0], vec!["1", "2"]);
    }

    // ---- P29 结构化对齐 API ----

    #[test]
    fn align_by_row_number() {
        let a = tbl("id,name\n1,alice\n");
        let b = tbl("id,name\n1,alice\n2,bob\n");
        let (rows, stats) = align_tables(&a, &b, None);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].status, RowStatus::Same);
        assert_eq!(rows[0].a_no, Some(0));
        assert_eq!(rows[0].b_no, Some(0));
        assert_eq!(rows[1].status, RowStatus::RightOnly);
        assert_eq!(rows[1].a_no, None);
        assert_eq!(rows[1].b_no, Some(1));
        assert_eq!(stats.same, 1);
        assert_eq!(stats.right_only, 1);
    }

    #[test]
    fn align_reports_changed_cols() {
        let a = tbl("id,name,age\n1,alice,30\n");
        let b = tbl("id,name,age\n1,ALICE,31\n");
        let (rows, stats) = align_tables(&a, &b, None);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, RowStatus::Modified);
        assert_eq!(rows[0].changed_cols, vec![1, 2]);
        assert_eq!(stats.modified, 1);
    }

    #[test]
    fn align_by_key_matches_order_independent() {
        let a = tbl("id,name\n1,alice\n2,bob\n");
        let b = tbl("id,name\n2,BOB\n1,alice\n");
        let (rows, stats) = align_tables(&a, &b, Some("id"));
        // 按 id 对齐：行2 的 bob -> BOB 是修改，不是删除+新增
        assert_eq!(rows.len(), 2);
        assert_eq!(stats.modified, 1);
        assert_eq!(stats.same, 1);
        let mod_row = rows
            .iter()
            .find(|r| r.status == RowStatus::Modified)
            .unwrap();
        assert_eq!(mod_row.a_no, Some(1));
        assert_eq!(mod_row.b_no, Some(0));
        assert_eq!(mod_row.changed_cols, vec![1]);
    }

    #[test]
    fn align_by_key_orphans() {
        let a = tbl("id,v\n1,x\n3,z\n");
        let b = tbl("id,v\n1,x\n2,y\n");
        let (rows, stats) = align_tables(&a, &b, Some("id"));
        assert_eq!(stats.left_only, 1);
        assert_eq!(stats.right_only, 1);
        assert_eq!(stats.same, 1);
        let lo = rows
            .iter()
            .find(|r| r.status == RowStatus::LeftOnly)
            .unwrap();
        assert_eq!(lo.a_no, Some(1));
        assert_eq!(lo.b_no, None);
        let ro = rows
            .iter()
            .find(|r| r.status == RowStatus::RightOnly)
            .unwrap();
        assert_eq!(ro.a_no, None);
        assert_eq!(ro.b_no, Some(1));
    }

    #[test]
    fn align_headers_and_rows_public() {
        // GUI 需要访问 Table 的 headers/rows（pub(crate)）
        let t = Table::new("a,b\n1,2\n", ',', false);
        assert_eq!(t.headers.len(), 2);
        assert_eq!(t.rows.len(), 1);
        // parse_csv 也需 pub(crate) 供 GUI 复用
        let parsed = parse_csv("x,y\n1,2\n", ',');
        assert_eq!(parsed.len(), 2);
    }

    #[test]
    fn serialize_roundtrip_basic() {
        // P37-1c：普通字段往返一致
        let t = Table::new("a,b\n1,2\n3,4\n", ',', false);
        let s = serialize_csv(&t, ',');
        assert_eq!(s, "a,b\n1,2\n3,4\n");
        let back = Table::new(&s, ',', false);
        assert_eq!(back.headers, t.headers);
        assert_eq!(back.rows, t.rows);
    }

    #[test]
    fn serialize_quotes_special_fields() {
        // P37-1c：含分隔符/引号/换行的字段需加引号转义并往返一致
        let t = Table::new(
            "k,v\n1,\"x,y\"\n2,\"say \"\"hi\"\"\"\n3,\"line1\nline2\"\n",
            ',',
            false,
        );
        let s = serialize_csv(&t, ',');
        // 字段含逗号/引号/换行时必须被引号包裹
        assert!(s.contains("\"x,y\""));
        assert!(s.contains("\"say \"\"hi\"\"\""));
        assert!(s.contains("\"line1\nline2\""));
        // 往返一致
        let back = Table::new(&s, ',', false);
        assert_eq!(back.headers, t.headers);
        assert_eq!(back.rows, t.rows);
    }

    #[test]
    fn serialize_handles_uneven_rows() {
        // P37-1c：行长度不足时补空字段（补齐到最大列宽）
        let t = Table::new("a,b\n1\n2,3,4\n", ',', false);
        let s = serialize_csv(&t, ',');
        assert_eq!(s, "a,b\n1,,\n2,3,4\n");
    }
}
