//! HTML 对比报告导出（P4）。
//!
//! 把目录对比结果（compare）渲染为自包含的 HTML 文件：内嵌 CSS、
//! 统计摘要 + 差异条目表格，浏览器直接打开即可查看。无外部依赖。

use crate::compare::{CompareResult, FileStatus};

/// 把对比结果渲染为自包含 HTML 字符串
pub fn render_html(
    left_label: &str,
    right_label: &str,
    result: &CompareResult,
    generated_at: &str,
) -> String {
    let st = result.stats;
    let rows: String = result
        .entries
        .iter()
        .map(|e| {
            let letter = e.status.letter();
            let cls = status_class(letter);
            let rel = escape(&e.rel);
            let desc = match e.status {
                FileStatus::Moved => format!(
                    "{} → {}",
                    escape(&e.rel),
                    escape(e.moved_to.as_deref().unwrap_or(""))
                ),
                _ => rel.clone(),
            };
            let size = match (&e.left, &e.right) {
                (Some(l), Some(r)) => format!("{} → {}", fmt_size(l.size), fmt_size(r.size)),
                (Some(l), None) => format!("{} → -", fmt_size(l.size)),
                (None, Some(r)) => format!("- → {}", fmt_size(r.size)),
                (None, None) => String::new(),
            };
            format!(
                "<tr><td><span class=\"tag {cls}\">[{letter}]</span></td><td class=\"path\">{desc}</td><td class=\"size\">{size}</td></tr>"
            )
        })
        .collect();

    format!(
        r#"<!DOCTYPE html>
<html lang="zh">
<head>
<meta charset="utf-8">
<title>bcr 对比报告: {left} ↔ {right}</title>
<style>
body {{ font-family: -apple-system, "Segoe UI", "Microsoft YaHei", sans-serif; margin: 24px; color: #222; }}
h1 {{ font-size: 20px; margin-bottom: 4px; }}
h2 {{ font-size: 14px; font-weight: normal; color: #666; margin-top: 0; }}
.summary {{ display: flex; gap: 16px; flex-wrap: wrap; margin: 16px 0; padding: 12px 16px; background: #f5f5f5; border-radius: 8px; }}
.summary .item {{ display: flex; align-items: baseline; gap: 6px; }}
.summary .num {{ font-size: 22px; font-weight: 700; }}
table {{ border-collapse: collapse; width: 100%; max-width: 900px; }}
th, td {{ text-align: left; padding: 6px 12px; border-bottom: 1px solid #eee; }}
th {{ background: #fafafa; font-size: 13px; color: #666; }}
td.path {{ font-family: ui-monospace, Menlo, Consolas, monospace; font-size: 13px; }}
td.size {{ font-family: ui-monospace, Menlo, Consolas, monospace; font-size: 12px; color: #888; text-align: right; }}
.tag {{ font-family: ui-monospace, Menlo, Consolas, monospace; font-weight: 700; }}
.tag.L {{ color: #e05555; }}
.tag.R {{ color: #4a7ad0; }}
.tag.C {{ color: #c9a227; }}
.tag.S {{ color: #3aa35a; }}
.tag.M {{ color: #a060c9; }}
.footer {{ margin-top: 16px; color: #999; font-size: 12px; }}
</style>
</head>
<body>
<h1>📊 bcr 对比报告</h1>
<h2>{left} ↔ {right}</h2>
<div class="summary">
  <div class="item"><span class="num" style="color:#3aa35a">{same}</span><span>相同</span></div>
  <div class="item"><span class="num" style="color:#e05555">{lo}</span><span>仅左侧</span></div>
  <div class="item"><span class="num" style="color:#4a7ad0">{ro}</span><span>仅右侧</span></div>
  <div class="item"><span class="num" style="color:#c9a227">{diff}</span><span>内容不同</span></div>
  <div class="item"><span class="num" style="color:#a060c9">{moved}</span><span>移动/重命名</span></div>
</div>
<table>
<thead><tr><th>状态</th><th>路径</th><th>大小</th></tr></thead>
<tbody>{rows}</tbody>
</table>
<div class="footer">生成于 {generated_at} · bcr {version}</div>
</body>
</html>"#,
        left = escape(left_label),
        right = escape(right_label),
        same = st.same,
        lo = st.left_only,
        ro = st.right_only,
        diff = st.differ,
        moved = st.moved,
        rows = rows,
        generated_at = escape(generated_at),
        version = env!("CARGO_PKG_VERSION"),
    )
}

fn status_class(letter: char) -> &'static str {
    match letter {
        'L' => "L",
        'R' => "R",
        'C' => "C",
        'S' => "S",
        'M' => "M",
        _ => "",
    }
}

fn fmt_size(n: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = 1024.0 * 1024.0;
    const GB: f64 = 1024.0 * 1024.0 * 1024.0;
    if n >= GB as u64 {
        format!("{:.1} GB", n as f64 / GB)
    } else if n >= MB as u64 {
        format!("{:.1} MB", n as f64 / MB)
    } else if n >= KB as u64 {
        format!("{:.1} KB", n as f64 / KB)
    } else {
        format!("{n} B")
    }
}

/// HTML 转义
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compare::{CompareResult, CompareStats, FileEntry};
    use crate::fsscan::FileMeta;
    use std::time::UNIX_EPOCH;

    fn meta(size: u64) -> FileMeta {
        FileMeta {
            size,
            mtime: UNIX_EPOCH,
            mode: None,
            symlink: None,
        }
    }

    fn result() -> CompareResult {
        CompareResult {
            entries: vec![
                FileEntry {
                    rel: "only_l.txt".into(),
                    status: FileStatus::LeftOnly,
                    left: Some(meta(1234)),
                    right: None,
                    moved_to: None,
                    attrs_differ: false,
                },
                FileEntry {
                    rel: "old.txt".into(),
                    status: FileStatus::Moved,
                    left: Some(meta(100)),
                    right: Some(meta(100)),
                    moved_to: Some("new.txt".into()),
                    attrs_differ: false,
                },
            ],
            stats: CompareStats {
                same: 5,
                left_only: 1,
                right_only: 0,
                differ: 0,
                moved: 1,
            },
            warnings: vec![],
        }
    }

    #[test]
    fn html_contains_summary_and_rows() {
        let html = render_html("/a", "/b", &result(), "2026-01-01");
        assert!(html.contains("📊"));
        assert!(html.contains("/a ↔ /b"));
        assert!(html.contains("only_l.txt"));
        assert!(html.contains("[L]"));
        assert!(html.contains("old.txt → new.txt"));
        assert!(html.contains("[M]"));
        assert!(html.contains(">5<"));
        assert!(html.contains(">1<"));
    }

    #[test]
    fn html_escapes_special_chars() {
        let mut r = result();
        r.entries[0].rel = "a<b>&\"c".into();
        let html = render_html("/a", "/b", &r, "t");
        assert!(html.contains("a&lt;b&gt;&amp;&quot;c"));
        assert!(!html.contains("<b>"));
    }

    #[test]
    fn fmt_size_units() {
        assert_eq!(fmt_size(500), "500 B");
        assert_eq!(fmt_size(2048), "2.0 KB");
        assert_eq!(fmt_size(3 * 1024 * 1024), "3.0 MB");
        assert_eq!(fmt_size(2 * 1024 * 1024 * 1024), "2.0 GB");
    }
}
