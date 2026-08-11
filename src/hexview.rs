//! 十六进制对比视图（P1：二进制文件友好对比）。
//!
//! 将两个文件的原始字节按 16 字节对齐分块，生成并排对比行：
//! 每行含偏移、左侧字节、右侧字节与差异标记。CLI（`bcr hex`）
//! 与 GUI（DiffTab 二进制自动切换）共用该模型，纯逻辑可单测。

/// 一行 hex 对比（最多 16 字节对齐）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HexRow {
    /// 行起始偏移（字节）
    pub offset: usize,
    /// 左侧该块的字节（长度 0..=16）
    pub left: Vec<u8>,
    /// 右侧该块的字节（长度 0..=16）
    pub right: Vec<u8>,
    /// 该行是否存在字节差异（长度不等或字节不等）
    pub diff: bool,
}

/// 按 16 字节分块构建对比行；两侧长度不同时按各自实际字节截取。
pub fn build_hex_rows(left: &[u8], right: &[u8]) -> Vec<HexRow> {
    let n = left.len().max(right.len());
    let mut rows = Vec::new();
    let mut offset = 0usize;
    while offset < n {
        let end = (offset + 16).min(n);
        let l_end = left.len().min(end);
        let r_end = right.len().min(end);
        let l = left[offset..l_end].to_vec();
        let r = right[offset..r_end].to_vec();
        let diff = l != r;
        rows.push(HexRow {
            offset,
            left: l,
            right: r,
            diff,
        });
        offset = end;
    }
    rows
}

/// 字节的 ASCII 表示：可打印 ASCII → 原字符，否则 '.'
pub fn ascii_byte(b: u8) -> char {
    if (0x20..=0x7E).contains(&b) {
        b as char
    } else {
        '.'
    }
}

/// 一行字节的 xxd 风格 hex 文本（8 字节处加空格分隔）
#[cfg(test)]
fn hex_text(bytes: &[u8]) -> String {
    let mut s = String::new();
    for (i, b) in bytes.iter().enumerate() {
        if i == 8 {
            s.push(' ');
        }
        s.push_str(&format!("{:02X} ", b));
    }
    // 不足 16 字节补齐到 16 字节宽度（保证两侧对齐）
    let width = bytes.len().min(16);
    for _ in width..16 {
        if width == 8 {
            s.push(' ');
        }
        s.push_str("   ");
    }
    if !s.is_empty() && bytes.len() >= 8 {
        s.push(' ');
    }
    s.trim_end().to_string()
}

/// 一行字节的 ASCII 文本（16 字节宽，不足补空格）
fn ascii_text(bytes: &[u8]) -> String {
    let mut s: String = bytes.iter().map(|&b| ascii_byte(b)).collect();
    let width = bytes.len().min(16);
    for _ in width..16 {
        s.push(' ');
    }
    s
}

/// 差异字节索引（两侧相同长度时才有意义；用于 CLI 高亮）
fn diff_indices(row: &HexRow) -> Vec<usize> {
    let n = row.left.len().min(row.right.len());
    (0..n).filter(|&i| row.left[i] != row.right[i]).collect()
}

const RED_BG: &str = "\x1b[41m";
const GREEN_BG: &str = "\x1b[42m";
const RESET: &str = "\x1b[0m";

/// CLI 渲染：默认只显示差异行，--show-same 显示全部。
/// 每对输出两行：L 行（偏移 + 左侧 hex + ASCII）与 R 行（对齐右侧）。
pub fn render_hex(rows: &[HexRow], color: bool, show_same: bool) {
    for row in rows {
        if !row.diff && !show_same {
            continue;
        }
        let mark = if row.diff { '!' } else { '=' };
        let diffs = diff_indices(row);
        println!(
            "{}  {:08x}  L {}  |{}|",
            mark,
            row.offset,
            colored_hex(&row.left, &diffs, color, true),
            ascii_text(&row.left)
        );
        println!(
            "   {:8}  R {}  |{}|",
            "",
            colored_hex(&row.right, &diffs, color, false),
            ascii_text(&row.right)
        );
    }
}

/// 差异字节着色：左侧红色背景、右侧绿色背景（color=true 时）
fn colored_hex(bytes: &[u8], diffs: &[usize], color: bool, is_left: bool) -> String {
    let mut s = String::new();
    for (i, b) in bytes.iter().enumerate() {
        if i == 8 {
            s.push(' ');
        }
        let highlighted = color && diffs.contains(&i);
        if highlighted {
            s.push_str(if is_left { RED_BG } else { GREEN_BG });
            s.push_str(&format!("{:02X}", b));
            s.push_str(RESET);
        } else {
            s.push_str(&format!("{:02X}", b));
        }
        s.push(' ');
    }
    // 补齐宽度（16 字节）
    for i in bytes.len()..16 {
        if i == 8 {
            s.push(' ');
        }
        s.push_str("   ");
    }
    s.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_files_no_diff_rows() {
        let rows = build_hex_rows(b"hello world", b"hello world");
        assert_eq!(rows.len(), 1);
        assert!(!rows[0].diff);
        assert_eq!(rows[0].offset, 0);
    }

    #[test]
    fn single_byte_diff_marked() {
        let rows = build_hex_rows(b"aaaa", b"aaab");
        assert_eq!(rows.len(), 1);
        assert!(rows[0].diff);
        assert_eq!(diff_indices(&rows[0]), vec![3]);
    }

    #[test]
    fn multiple_blocks() {
        let l: Vec<u8> = (0..20).collect();
        let r: Vec<u8> = (0..20).collect();
        let rows = build_hex_rows(&l, &r);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].offset, 0);
        assert_eq!(rows[1].offset, 16);
        assert_eq!(rows[0].left.len(), 16);
        assert_eq!(rows[1].left.len(), 4);
        assert!(!rows[0].diff);
        assert!(!rows[1].diff);
    }

    #[test]
    fn different_lengths_diff() {
        let rows = build_hex_rows(b"0123456789abcdef", b"0123456789abcdefg");
        assert_eq!(rows.len(), 2);
        // 前 16 字节相同 → 第一块无差异；多出的第 17 字节在第二块
        assert!(!rows[0].diff);
        assert!(rows[1].diff);
        assert_eq!(rows[1].left, Vec::<u8>::new());
        assert_eq!(rows[1].right, vec![b'g']);
    }

    #[test]
    fn empty_sides() {
        assert!(build_hex_rows(b"", b"").is_empty());
        let rows = build_hex_rows(b"", b"x");
        assert_eq!(rows.len(), 1);
        assert!(rows[0].diff);
    }

    #[test]
    fn ascii_byte_mapping() {
        assert_eq!(ascii_byte(b'A'), 'A');
        assert_eq!(ascii_byte(0), '.');
        assert_eq!(ascii_byte(0x7F), '.');
        assert_eq!(ascii_byte(0x20), ' ');
    }

    #[test]
    fn hex_text_formatting() {
        let t = hex_text(b"\x89PNG\r\n\x1a\n");
        assert!(t.contains("89"));
        assert!(t.contains("50"));
        // 8 字节分隔：前 8 字节后有额外空格
        let t16 = hex_text(&(0..16).collect::<Vec<u8>>());
        assert!(t16.contains("07  08"));
    }

    #[test]
    fn ascii_text_padding() {
        let s = ascii_text(b"AB");
        assert_eq!(s.len(), 16);
        assert!(s.starts_with("AB"));
    }
}
