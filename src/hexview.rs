#![allow(dead_code)] // P58: 字节显示模式仅菜单可达, 待迁入
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

/// P37-1d：hex 视图字节值显示模式（BC 视图菜单）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HexValueMode {
    /// 逐字节显示（默认，xxd 风格）
    #[default]
    Raw,
    /// 小端序：每 4 字节按 u32 LE 解释
    LittleEndian,
    /// 大端序：每 4 字节按 u32 BE 解释
    BigEndian,
}

/// P37-1d：偏移列格式化（BC Byte Addresses：hex / dec）
pub fn format_offset(offset: usize, hex: bool) -> String {
    if hex {
        format!("{:08x}", offset)
    } else {
        format!("{:08}", offset)
    }
}

/// P37-1d：按显示模式生成一行字节的 hex 文本。
///
/// Raw：逐字节 `{:02X}`（8 字节处空格）；LE/BE：每 4 字节一组解释为 u32。
/// 不足 4 字节的剩余部分按逐字节显示；不足 16 字节补齐宽度（与 Raw 对齐）。
pub fn hex_values_text(bytes: &[u8], mode: HexValueMode) -> String {
    match mode {
        HexValueMode::Raw => hex_text_pub(bytes),
        HexValueMode::LittleEndian | HexValueMode::BigEndian => {
            let mut s = String::new();
            let mut i = 0;
            let mut group = 0;
            while i < bytes.len() {
                if group == 4 {
                    s.push(' ');
                }
                let end = (i + 4).min(bytes.len());
                if end - i == 4 {
                    let mut buf = [0u8; 4];
                    buf.copy_from_slice(&bytes[i..end]);
                    let v = match mode {
                        HexValueMode::LittleEndian => u32::from_le_bytes(buf),
                        HexValueMode::BigEndian => u32::from_be_bytes(buf),
                        _ => unreachable!(),
                    };
                    s.push_str(&format!("{:08X} ", v));
                    i = end;
                } else {
                    // 剩余不足 4 字节：逐字节
                    for b in &bytes[i..end] {
                        s.push_str(&format!("{:02X} ", b));
                    }
                    i = end;
                }
                group += 1;
            }
            // 补齐到 4 组（16 字节）宽度：每组 8 字符 + 1 空格 = 9
            for _ in group..4 {
                s.push_str("         ");
            }
            s.trim_end().to_string()
        }
    }
}

/// 逐字节 xxd 风格（8 字节处加空格，不足 16 字节补齐宽度）
fn hex_text_pub(bytes: &[u8]) -> String {
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

    // ---- P37-1d：偏移格式 / 值显示模式 ----------------

    #[test]
    fn format_offset_hex_and_dec() {
        assert_eq!(format_offset(0, true), "00000000");
        assert_eq!(format_offset(255, true), "000000ff");
        assert_eq!(format_offset(255, false), "00000255");
        assert_eq!(format_offset(0x1234, false), "00004660");
    }

    #[test]
    fn hex_values_raw_is_bytewise() {
        let b = [0x01, 0x02, 0x03, 0x04, 0x05];
        let s = hex_values_text(&b, HexValueMode::Raw);
        assert!(s.contains("01 02 03 04"));
        assert!(s.contains("05"));
        // 8 字节分隔：5 字节不足 8，无额外分隔
        assert!(!s.starts_with("01 02 03 04 05  0"));
    }

    #[test]
    fn hex_values_little_endian_groups() {
        // 0x01020304 的小端字节序 = 04 03 02 01 → u32 LE = 0x01020304
        let b = [0x04, 0x03, 0x02, 0x01, 0x08, 0x07, 0x06, 0x05];
        let s = hex_values_text(&b, HexValueMode::LittleEndian);
        assert!(
            s.contains("01020304"),
            "LE 第一组应解释为 0x01020304: {}",
            s
        );
        assert!(
            s.contains("05060708"),
            "LE 第二组应解释为 0x05060708: {}",
            s
        );
        // 与 Raw 不同
        let raw = hex_values_text(&b, HexValueMode::Raw);
        assert_ne!(s, raw);
    }

    #[test]
    fn hex_values_big_endian_groups() {
        // 0x01020304 的大端字节序 = 01 02 03 04 → u32 BE = 0x01020304
        let b = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
        let s = hex_values_text(&b, HexValueMode::BigEndian);
        assert!(
            s.contains("01020304"),
            "BE 第一组应解释为 0x01020304: {}",
            s
        );
        assert!(
            s.contains("05060708"),
            "BE 第二组应解释为 0x05060708: {}",
            s
        );
        // LE 与 BE 结果不同（字节序反转）
        let le = hex_values_text(&b, HexValueMode::LittleEndian);
        assert_ne!(le, s);
    }

    #[test]
    fn hex_values_partial_group_falls_back_to_bytes() {
        // 不足 4 字节的尾部逐字节显示
        let b = [0xAA, 0xBB, 0xCC, 0xDD, 0xEE];
        let s = hex_values_text(&b, HexValueMode::BigEndian);
        assert!(s.contains("AABBCCDD"), "前 4 字节成组: {}", s);
        assert!(s.contains("EE"), "剩余字节逐字节: {}", s);
    }
}
