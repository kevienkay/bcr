//! P37-1h：补丁文件解析（unified diff）。
//!
//! 解析 `.patch`/`.diff` 文件为左右两侧可对比的文本：
//! - 左侧 = 上下文行 + 删除行（`-`）
//! - 右侧 = 上下文行 + 新增行（`+`）
//! - 支持 `--- a/xxx`、`+++ b/xxx` 头与多个 `@@ -l,c +l,c @@` hunk

/// 解析结果：左右两侧还原文本 + 统计
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedPatch {
    /// a 侧路径（--- 头，去 a/ 前缀）
    pub a_path: String,
    /// b 侧路径（+++ 头，去 b/ 前缀）
    pub b_path: String,
    /// 左侧（旧）全文
    pub left: String,
    /// 右侧（新）全文
    pub right: String,
    /// 新增行数（+）
    pub added: usize,
    /// 删除行数（-）
    pub removed: usize,
}

/// 判断文件是否为补丁文件（扩展名 .patch/.diff）
pub fn is_patch_file(path: &str) -> bool {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    ext.eq_ignore_ascii_case("patch") || ext.eq_ignore_ascii_case("diff")
}

/// 解析 unified diff 文本；非补丁格式返回 None
pub fn parse_patch(text: &str) -> Option<ParsedPatch> {
    let mut a_path = String::new();
    let mut b_path = String::new();
    let mut left: Vec<String> = Vec::new();
    let mut right: Vec<String> = Vec::new();
    let mut added = 0usize;
    let mut removed = 0usize;
    let mut in_hunk = false;

    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("--- ") {
            // --- a/xxx 或 --- xxx
            a_path = strip_prefix_path(rest);
            in_hunk = false;
            continue;
        }
        if let Some(rest) = line.strip_prefix("+++ ") {
            b_path = strip_prefix_path(rest);
            in_hunk = false;
            continue;
        }
        if line.starts_with("@@ ") {
            in_hunk = true;
            continue;
        }
        if in_hunk {
            if let Some(body) = line.strip_prefix("+") {
                // 排除 "+++ " 头（已在上面处理）
                if !body.starts_with("+") {
                    right.push(body.to_string());
                    added += 1;
                    continue;
                }
            }
            if let Some(body) = line.strip_prefix('-') {
                if !body.starts_with('-') {
                    left.push(body.to_string());
                    removed += 1;
                    continue;
                }
            }
            if let Some(body) = line.strip_prefix(' ') {
                left.push(body.to_string());
                right.push(body.to_string());
                continue;
            }
            // hunk 内的非 +/-/空格 行（如 \ No newline at end of file）：忽略
        }
    }

    if left.is_empty() && right.is_empty() && a_path.is_empty() && b_path.is_empty() {
        return None;
    }
    Some(ParsedPatch {
        a_path,
        b_path,
        left: left.join("\n"),
        right: right.join("\n"),
        added,
        removed,
    })
}

/// 去掉 a/ b/ 前缀（--- a/src/main.rs → src/main.rs）
fn strip_prefix_path(s: &str) -> String {
    let s = s.trim();
    if let Some(rest) = s.strip_prefix("a/").or_else(|| s.strip_prefix("b/")) {
        rest.to_string()
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_patch_file_detects_extensions() {
        assert!(is_patch_file("x.patch"));
        assert!(is_patch_file("x.diff"));
        assert!(is_patch_file("x.PATCH"));
        assert!(!is_patch_file("x.txt"));
        assert!(!is_patch_file("x.rs"));
    }

    #[test]
    fn parses_standard_unified_diff() {
        let patch = "--- a/src/a.txt\n+++ b/src/a.txt\n@@ -1,3 +1,3 @@\n line1\n-old line\n+new line\n line3\n";
        let p = parse_patch(patch).expect("应解析成功");
        assert_eq!(p.a_path, "src/a.txt");
        assert_eq!(p.b_path, "src/a.txt");
        assert_eq!(p.left, "line1\nold line\nline3");
        assert_eq!(p.right, "line1\nnew line\nline3");
        assert_eq!(p.added, 1);
        assert_eq!(p.removed, 1);
    }

    #[test]
    fn parses_multiple_hunks() {
        let patch = "--- a/x.txt\n+++ b/x.txt\n@@ -1,2 +1,2 @@\n-a\n+b\n@@ -5,2 +5,2 @@\n-c\n+d\n";
        let p = parse_patch(patch).unwrap();
        assert_eq!(p.left, "a\nc");
        assert_eq!(p.right, "b\nd");
        assert_eq!(p.added, 2);
        assert_eq!(p.removed, 2);
    }

    #[test]
    fn rejects_non_patch_text() {
        assert!(parse_patch("just some text\nno diff markers\n").is_none());
    }

    #[test]
    fn handles_no_newline_marker() {
        let patch = "--- a/a\n+++ b/a\n@@ -1,1 +1,1 @@\n-old\n+new\n\\ No newline at end of file\n";
        let p = parse_patch(patch).unwrap();
        assert_eq!(p.left, "old");
        assert_eq!(p.right, "new");
    }

    #[test]
    fn strips_a_b_prefixes() {
        let patch = "--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1,1 +1,1 @@\n-x\n+y\n";
        let p = parse_patch(patch).unwrap();
        assert_eq!(p.a_path, "src/main.rs");
        assert_eq!(p.b_path, "src/main.rs");
    }
}
