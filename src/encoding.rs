//! P0：编码检测与二进制检测。
//!
//! 统一的文本读取入口：任何路径（本地文件）先读字节，再做确定性检测
//! （BOM → 严格 UTF-8 → UTF-16 无 BOM 嗅探 → 二进制判定 → chardetng 多字节
//! 编码 → Latin-1 保底），保证 CLI/GUI 面对 GBK、UTF-16、二进制文件时
//! 行为一致、永不 panic。
//!
//! 可用 `BCR_ENCODING` 环境变量（或 CLI `--encoding`，会写入该变量）强制
//! 指定编码，跳过自动检测。

use encoding_rs::Encoding;
use std::io::{self, Read};
use std::path::Path;

/// 文本读取默认大小上限（字节）。超过时拒绝按文本加载，防止 OOM。
pub const DEFAULT_MAX_TEXT_BYTES: u64 = 256 * 1024 * 1024;

/// 读取文件前检查大小上限（`BCR_MAX_SIZE` 环境变量可覆盖，单位 MB；0 表示不限制）
pub fn check_size(path: &str) -> io::Result<()> {
    let max = env_max_bytes();
    if max == 0 {
        return Ok(());
    }
    let meta = std::fs::metadata(Path::new(path))?;
    if meta.len() > max {
        return Err(io::Error::new(
            io::ErrorKind::FileTooLarge,
            format!(
                "文件过大: {} ({} bytes > {} bytes 上限)",
                path,
                meta.len(),
                max
            ),
        ));
    }
    Ok(())
}

/// 从 `BCR_MAX_SIZE`（MB）解析上限字节数；未设置返回默认值。
fn env_max_bytes() -> u64 {
    if let Ok(v) = std::env::var("BCR_MAX_SIZE") {
        if let Ok(mb) = v.trim().parse::<u64>() {
            return mb * 1024 * 1024;
        }
    }
    DEFAULT_MAX_TEXT_BYTES
}

/// 读取本地文件并解码（`-` 表示 stdin）
pub fn read_input(path: &str) -> io::Result<TextFile> {
    if path == "-" {
        let mut buf = Vec::new();
        io::stdin().read_to_end(&mut buf)?;
        Ok(decode(&buf))
    } else {
        check_size(path)?;
        let data = read_mmap(path)?;
        Ok(decode(&data))
    }
}

/// 读取本地文件并解码（不处理 stdin）
pub fn read_text(path: &str) -> io::Result<TextFile> {
    check_size(path)?;
    let data = read_mmap(path)?;
    Ok(decode(&data))
}

/// C1：memmap2 只读映射读取（避免整文件拷贝进堆，峰值内存更低）。
/// 映射后立即复制为 Vec（decode 需要稳定缓冲区，且不长期持有映射避免
/// 外部修改触发 SIGBUS）；对超大文件比 std::fs::read 少一次内核→用户拷贝。
fn read_mmap(path: &str) -> io::Result<Vec<u8>> {
    let f = std::fs::File::open(Path::new(path))?;
    unsafe { Ok(memmap2::Mmap::map(&f)?.to_vec()) }
}

/// 检测/指定的编码种类
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodingKind {
    Utf8,
    Utf16Le,
    Utf16Be,
    Utf32Le,
    Utf32Be,
    /// 其他 encoding_rs 编码（GBK/Big5/Shift_JIS/Windows-1252 等）
    Other(&'static Encoding),
}

impl EncodingKind {
    /// 人类可读名称（状态栏/提示用）
    #[allow(dead_code)] // 测试与后续状态栏展示使用
    pub fn name(&self) -> &'static str {
        match self {
            EncodingKind::Utf8 => "UTF-8",
            EncodingKind::Utf16Le => "UTF-16LE",
            EncodingKind::Utf16Be => "UTF-16BE",
            EncodingKind::Utf32Le => "UTF-32LE",
            EncodingKind::Utf32Be => "UTF-32BE",
            EncodingKind::Other(e) => e.name(),
        }
    }
}

/// 解码后的文本文件
#[derive(Debug, Clone)]
pub struct TextFile {
    /// 解码后的文本（二进制文件此字段为空串）
    pub text: String,
    /// 检测出的编码
    pub encoding: EncodingKind,
    /// 原文件是否带 BOM
    pub had_bom: bool,
    /// 判定为二进制文件（不应按文本处理）
    pub is_binary: bool,
}

/// 字节 → TextFile 的完整检测链
pub fn decode(data: &[u8]) -> TextFile {
    // 0. 用户强制指定编码（BCR_ENCODING / --encoding）
    if let Ok(name) = std::env::var("BCR_ENCODING") {
        if !name.is_empty() {
            if let Some(kind) = kind_for_label(&name) {
                return decode_with(kind, data);
            }
        }
    }

    // 1. BOM 嗅探（注意 UTF-32LE 的 BOM 是 FF FE 00 00，须先于 UTF-16LE 判断）
    if data.starts_with(&[0xEF, 0xBB, 0xBF]) {
        let mut tf = decode_with(EncodingKind::Utf8, &data[3..]);
        tf.had_bom = true;
        return tf;
    }
    if data.starts_with(&[0xFF, 0xFE, 0x00, 0x00]) {
        return decode_with(EncodingKind::Utf32Le, &data[4..]);
    }
    if data.starts_with(&[0x00, 0x00, 0xFE, 0xFF]) {
        return decode_with(EncodingKind::Utf32Be, &data[4..]);
    }
    if data.starts_with(&[0xFF, 0xFE]) {
        return decode_with(EncodingKind::Utf16Le, &data[2..]);
    }
    if data.starts_with(&[0xFE, 0xFF]) {
        return decode_with(EncodingKind::Utf16Be, &data[2..]);
    }

    // 2. UTF-16 无 BOM 嗅探（必须在严格 UTF-8 验证之前：UTF-16 的 ASCII 内容
    //    含 NUL 字节，而 NUL 是合法 UTF-8 码点，先验 UTF-8 会把它误吞）
    if let Some(kind) = sniff_utf16(data) {
        return decode_with(kind, data);
    }
    // 2.5. UTF-16 无 BOM 试解码兜底（NUL 分布嗅探对中文等高字节非零内容失效：
    //    汉字 UTF-16 高字节多在 0x4E-0x9F，NUL 稀疏；此时 chardetng 兜底会把
    //    UTF-16 字节流中的 0x0A 当换行 → 一行文本显示多行+乱码）
    //    必须在二进制判定之前：低字节为 0 的汉字（如「一」U+4E00 → 00 4E）
    //    会触发 looks_binary 的 NUL 阈值误判
    if let Some(tf) = try_decode_utf16(data) {
        return tf;
    }

    // 3. 二进制判定（NUL 密度/控制字符；也在 UTF-8 验证之前：全 NUL 数据
    //    是合法 UTF-8，需先拦截）
    if looks_binary(data) {
        return TextFile {
            text: String::new(),
            encoding: EncodingKind::Utf8,
            had_bom: false,
            is_binary: true,
        };
    }

    // 4. 严格 UTF-8
    if std::str::from_utf8(data).is_ok() {
        return decode_with(EncodingKind::Utf8, data);
    }

    // 5. chardetng 多字节编码检测（GBK/Big5/Shift_JIS 等）
    let mut det = chardetng::EncodingDetector::new();
    det.feed(data, true);
    let enc = det.guess(None, true);
    let tf = decode_with(EncodingKind::Other(enc), data);
    // 替换字符过多说明猜错，退回 Latin-1 保底
    let repl = tf.text.chars().filter(|&c| c == '\u{FFFD}').count();
    let total = tf.text.chars().count().max(1);
    if repl * 100 <= total * 10 {
        return tf;
    }

    // 6. Latin-1 保底（永不失败）
    decode_with(EncodingKind::Other(encoding_rs::WINDOWS_1252), data)
}

/// 用指定编码解码（无 BOM 处理）
fn decode_with(kind: EncodingKind, data: &[u8]) -> TextFile {
    let text = match kind {
        EncodingKind::Utf32Le => decode_utf32(data, true),
        EncodingKind::Utf32Be => decode_utf32(data, false),
        _ => {
            let enc = encoding_for(kind);
            match enc.decode_without_bom_handling_and_without_replacement(data) {
                Some(cow) => cow.into_owned(),
                // 含无效字节：退回带替换的解码，避免返回空内容
                None => enc.decode_without_bom_handling(data).0.into_owned(),
            }
        }
    };
    let had_bom = matches!(
        kind,
        EncodingKind::Utf16Le
            | EncodingKind::Utf16Be
            | EncodingKind::Utf32Le
            | EncodingKind::Utf32Be
    );
    TextFile {
        text,
        encoding: kind,
        had_bom,
        is_binary: false,
    }
}

/// 简单 UTF-32 解码（encoding_rs 不覆盖 UTF-32，自实现；无效码点 → U+FFFD）
fn decode_utf32(data: &[u8], little: bool) -> String {
    let mut s = String::new();
    let mut i = 0;
    while i + 4 <= data.len() {
        let b: [u8; 4] = [data[i], data[i + 1], data[i + 2], data[i + 3]];
        let u = if little {
            u32::from_le_bytes(b)
        } else {
            u32::from_be_bytes(b)
        };
        if let Some(c) = char::from_u32(u) {
            s.push(c);
        } else {
            s.push('\u{FFFD}');
        }
        i += 4;
    }
    s
}

/// UTF-16 无 BOM 嗅探：采样前 4096 字节（截断为偶数），
/// 奇数地址 NUL 占比高 → LE；偶数地址 NUL 占比高 → BE。
fn sniff_utf16(data: &[u8]) -> Option<EncodingKind> {
    let n = (data.len() / 2) * 2;
    if n < 16 {
        return None;
    }
    let pairs = n / 2;
    let odd_nul = (0..pairs).filter(|&i| data[2 * i + 1] == 0).count();
    let even_nul = (0..pairs).filter(|&i| data[2 * i] == 0).count();
    if odd_nul * 100 / pairs > 60 && even_nul * 100 / pairs < 30 {
        Some(EncodingKind::Utf16Le)
    } else if even_nul * 100 / pairs > 60 && odd_nul * 100 / pairs < 30 {
        Some(EncodingKind::Utf16Be)
    } else {
        None
    }
}

/// UTF-16 无 BOM 试解码兜底：NUL 分布嗅探对中文等高字节非零内容失效时，
/// 直接按 LE/BE 解码采样并验证结果合理性（替换字符少 + 可读文本占比高）。
/// 合理性判定：
/// - 无替换字符（had_errors=false）或替换占比 < 5%；
/// - 可读字符（非控制、非私有区、常见 CJK/ASCII）占比 ≥ 80%；
/// - 字节流含 NUL 且非纯 ASCII（排除 UTF-8 文本按 UTF-16 误读）；
///
/// 返回完整解码结果。
fn try_decode_utf16(data: &[u8]) -> Option<TextFile> {
    let n = (data.len() / 2) * 2;
    if n < 8 {
        return None;
    }
    // 关键约束 1：纯 ASCII 文本（如 hello\nworld\n）无 NUL，按 UTF-16BE
    // 误读后每个码元落在 CJK 基本区（0x65-0x7A 高字节全在 4E00-9FFF 内），
    // 可读性仍 100%——直接交给严格 UTF-8 验证，不进 UTF-16 试解码
    let sample_ascii = data[..n]
        .iter()
        .filter(|&&b| (0x20..=0x7E).contains(&b))
        .count();
    if sample_ascii * 100 > n * 90 {
        return None;
    }
    // 关键约束 2：真实 UTF-16 文本字节流必含 NUL（ASCII 字符/换行的高字节
    // 或低字节为 0 的汉字）；纯中文无换行无 NUL 的 UTF-16LE 属信息论上
    // 不可区分场景，退回 chardetng（不会产生多行乱码，因无 0A 字节）
    if !data[..n].contains(&0) {
        return None;
    }
    let sample = &data[..n.min(4096)];
    for (kind, enc) in [
        (EncodingKind::Utf16Le, encoding_rs::UTF_16LE),
        (EncodingKind::Utf16Be, encoding_rs::UTF_16BE),
    ] {
        let Some(text) = enc.decode_without_bom_handling_and_without_replacement(sample) else {
            // 有替换字符：不是该字节序的 UTF-16 文本
            continue;
        };
        let text = text.into_owned();
        let total = text.chars().count();
        if total == 0 {
            continue;
        }
        // 可读性：控制字符（含 NUL/私有区）少，且常见文本字符占比高
        let readable = text.chars().filter(|&c| is_readable_text_char(c)).count();
        let ctrl = text
            .chars()
            .filter(|&c| c.is_control() && !matches!(c, '\t' | '\n' | '\r'))
            .count();
        // 私有区/未分配码元占比硬性上限：真实 UTF-16 文本不含私有区字符；
        // UTF-8 文本被按错误字节序误读时（如 hello 世界 → 0xE4B8）私有区占比高
        let private_use = text
            .chars()
            .filter(|&c| ('\u{E000}'..='\u{F8FF}').contains(&c))
            .count();
        if ctrl * 100 > total * 5 {
            continue;
        }
        if private_use * 100 > total * 5 {
            continue;
        }
        // 可读性阈值 80%：真实 UTF-16 中文/ASCII 文本解码后几乎全可读；
        // UTF-8 文本按 UTF-16LE 误读会产生私有区/未分配码元（如 hello 世界 → 50%）
        if readable * 100 < total * 80 {
            continue;
        }
        return Some(decode_with(kind, data));
    }
    None
}

/// 可读文本字符：常见 CJK 汉字/标点、ASCII 可打印、常见空白；
/// 排除控制字符、私有区（E000-F8FF）等乱码区。
fn is_readable_text_char(c: char) -> bool {
    if c.is_ascii_graphic() || matches!(c, ' ' | '\t' | '\n' | '\r') {
        return true;
    }
    if ('\u{4E00}'..='\u{9FFF}').contains(&c) {
        return true; // CJK 统一表意文字
    }
    if ('\u{3000}'..='\u{303F}').contains(&c) {
        return true; // CJK 标点
    }
    if ('\u{FF00}'..='\u{FFEF}').contains(&c) {
        return true; // 全角形式
    }
    false
}

/// 二进制判定：前 8192 字节中 NUL 占比 ≥ 1%，或非文本控制字符占比 ≥ 5%。
/// （C0 中允许 \t \n \r \x0C \x08）
fn looks_binary(data: &[u8]) -> bool {
    let sample = &data[..data.len().min(8192)];
    if sample.is_empty() {
        return false;
    }
    let nuls = sample.iter().filter(|&&b| b == 0).count();
    if nuls * 100 >= sample.len() {
        return true;
    }
    let ctrl = sample
        .iter()
        .filter(|&&b| b < 0x20 && !matches!(b, b'\t' | b'\n' | b'\r' | 0x0C | 0x08))
        .count();
    ctrl * 100 >= sample.len() * 5
}

/// EncodingKind → encoding_rs 编码（UTF-32 无对应，仅在 decode_with 前过滤）
fn encoding_for(kind: EncodingKind) -> &'static Encoding {
    match kind {
        EncodingKind::Utf8 => encoding_rs::UTF_8,
        EncodingKind::Utf16Le => encoding_rs::UTF_16LE,
        EncodingKind::Utf16Be => encoding_rs::UTF_16BE,
        EncodingKind::Other(e) => e,
        EncodingKind::Utf32Le | EncodingKind::Utf32Be => encoding_rs::UTF_8, // 不会被调用
    }
}

/// 标签 → EncodingKind（BCR_ENCODING / --encoding 用）
fn kind_for_label(label: &str) -> Option<EncodingKind> {
    match label.trim().to_ascii_lowercase().as_str() {
        "utf-8" | "utf8" => Some(EncodingKind::Utf8),
        "utf-16le" | "utf16le" | "utf-16" => Some(EncodingKind::Utf16Le),
        "utf-16be" | "utf16be" => Some(EncodingKind::Utf16Be),
        "utf-32le" | "utf32le" => Some(EncodingKind::Utf32Le),
        "utf-32be" | "utf32be" => Some(EncodingKind::Utf32Be),
        _ => Encoding::for_label(label.as_bytes()).map(EncodingKind::Other),
    }
}

/// 按原编码回写（GUI 编辑保存用）：保留 BOM 与原编码。
pub fn encode_back(tf: &TextFile, text: &str) -> Vec<u8> {
    let mut out = Vec::new();
    // 原文件带 BOM 时补回对应 BOM
    if tf.had_bom {
        match tf.encoding {
            EncodingKind::Utf8 => out.extend_from_slice(&[0xEF, 0xBB, 0xBF]),
            EncodingKind::Utf16Le => out.extend_from_slice(&[0xFF, 0xFE]),
            EncodingKind::Utf16Be => out.extend_from_slice(&[0xFE, 0xFF]),
            EncodingKind::Utf32Le => out.extend_from_slice(&[0xFF, 0xFE, 0x00, 0x00]),
            EncodingKind::Utf32Be => out.extend_from_slice(&[0x00, 0x00, 0xFE, 0xFF]),
            EncodingKind::Other(_) => {}
        }
    }
    match tf.encoding {
        EncodingKind::Utf32Le => {
            for c in text.chars() {
                out.extend_from_slice(&(c as u32).to_le_bytes());
            }
        }
        EncodingKind::Utf32Be => {
            for c in text.chars() {
                out.extend_from_slice(&(c as u32).to_be_bytes());
            }
        }
        // encoding_rs 的 UTF-16 encode() 实际返回 UTF-8 字节（decode-only），须手写
        EncodingKind::Utf16Le => {
            for u in text.chars().map(|c| c as u32) {
                if u <= 0xFFFF {
                    out.extend_from_slice(&(u as u16).to_le_bytes());
                } else {
                    let v = u - 0x10000;
                    out.extend_from_slice(&((0xD800 + (v >> 10)) as u16).to_le_bytes());
                    out.extend_from_slice(&((0xDC00 + (v & 0x3FF)) as u16).to_le_bytes());
                }
            }
        }
        EncodingKind::Utf16Be => {
            for u in text.chars().map(|c| c as u32) {
                if u <= 0xFFFF {
                    out.extend_from_slice(&(u as u16).to_be_bytes());
                } else {
                    let v = u - 0x10000;
                    out.extend_from_slice(&((0xD800 + (v >> 10)) as u16).to_be_bytes());
                    out.extend_from_slice(&((0xDC00 + (v & 0x3FF)) as u16).to_be_bytes());
                }
            }
        }
        _ => {
            let (cow, _, _) = encoding_for(tf.encoding).encode(text);
            out.extend_from_slice(&cow);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf8_plain() {
        let tf = decode("hello 世界\n".as_bytes());
        assert!(!tf.is_binary);
        assert_eq!(tf.text, "hello 世界\n");
        assert_eq!(tf.encoding.name(), "UTF-8");
    }

    #[test]
    fn utf8_with_bom() {
        let mut data = vec![0xEF, 0xBB, 0xBF];
        data.extend_from_slice("hi".as_bytes());
        let tf = decode(&data);
        assert!(!tf.is_binary);
        assert_eq!(tf.text, "hi");
        assert!(tf.had_bom);
    }

    #[test]
    fn utf16le_with_bom() {
        let mut data = vec![0xFF, 0xFE];
        data.extend_from_slice(&[b'h', 0x00, b'i', 0x00]);
        let tf = decode(&data);
        assert!(!tf.is_binary);
        assert_eq!(tf.text, "hi");
        assert_eq!(tf.encoding.name(), "UTF-16LE");
        assert!(tf.had_bom);
    }

    #[test]
    fn utf16be_with_bom() {
        let mut data = vec![0xFE, 0xFF];
        data.extend_from_slice(&[0x00, b'h', 0x00, b'i']);
        let tf = decode(&data);
        assert_eq!(tf.text, "hi");
        assert_eq!(tf.encoding.name(), "UTF-16BE");
    }

    #[test]
    fn utf16le_no_bom() {
        let data: Vec<u8> = "hello world".bytes().flat_map(|b| [b, 0x00]).collect();
        let tf = decode(&data);
        assert!(!tf.is_binary);
        assert_eq!(tf.text, "hello world");
        assert_eq!(tf.encoding.name(), "UTF-16LE");
    }

    #[test]
    fn utf32le_with_bom() {
        let mut data = vec![0xFF, 0xFE, 0x00, 0x00];
        for c in "hi".chars() {
            data.extend_from_slice(&(c as u32).to_le_bytes());
        }
        let tf = decode(&data);
        assert!(!tf.is_binary);
        assert_eq!(tf.text, "hi");
        assert_eq!(tf.encoding.name(), "UTF-32LE");
    }

    #[test]
    fn gbk_chinese() {
        // "中文" 的 GBK 编码：0xD6D0 0xCEC4
        let data = [0xD6, 0xD0, 0xCE, 0xC4, b'\n'];
        let tf = decode(&data);
        assert!(!tf.is_binary);
        assert_eq!(tf.text, "中文\n");
        assert_eq!(tf.encoding.name(), "GBK");
    }

    #[test]
    fn binary_png_detected() {
        // PNG 头 + NUL 密集数据
        let mut data = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        data.extend(std::iter::repeat_n(0u8, 512));
        let tf = decode(&data);
        assert!(tf.is_binary);
    }

    #[test]
    fn binary_nul_dense() {
        let data = vec![0u8; 1024];
        let tf = decode(&data);
        assert!(tf.is_binary);
    }

    #[test]
    fn latin1_fallback() {
        // é è (0xE9 0xE8) 非 UTF-8 单字节，落入保底编码
        let data = [0xE9, 0xE8, b'\n'];
        let tf = decode(&data);
        assert!(!tf.is_binary);
        assert_eq!(tf.text, "éè\n");
    }

    #[test]
    fn encode_back_roundtrip_gbk() {
        let data = [0xD6, 0xD0, 0xCE, 0xC4];
        let tf = decode(&data);
        let out = encode_back(&tf, "中文");
        assert_eq!(out, data);
    }

    #[test]
    fn encode_back_roundtrip_utf16le_bom() {
        let mut data = vec![0xFF, 0xFE];
        data.extend_from_slice(&[b'h', 0x00, b'i', 0x00]);
        let tf = decode(&data);
        let out = encode_back(&tf, "hi");
        assert_eq!(out, data);
    }

    #[test]
    fn encode_back_utf8_bom() {
        let mut data = vec![0xEF, 0xBB, 0xBF];
        data.extend_from_slice(b"hi");
        let tf = decode(&data);
        let out = encode_back(&tf, "hi");
        assert_eq!(out, data);
    }

    #[test]
    fn env_override_forces_encoding() {
        // 直接验证 kind_for_label + decode_with 组合，避免 set_var 污染并行测试
        let kind = kind_for_label("gbk").unwrap();
        let data = [0xD6, 0xD0, 0xCE, 0xC4];
        let tf = decode_with(kind, &data);
        assert_eq!(tf.text, "中文");
    }

    #[test]
    fn kind_labels_parse() {
        assert_eq!(kind_for_label("utf-8"), Some(EncodingKind::Utf8));
        assert_eq!(kind_for_label("UTF-16LE"), Some(EncodingKind::Utf16Le));
        assert_eq!(kind_for_label("utf-16be"), Some(EncodingKind::Utf16Be));
        assert_eq!(
            kind_for_label("gbk"),
            Some(EncodingKind::Other(encoding_rs::GBK))
        );
        assert_eq!(kind_for_label("nonsense-xyz"), None);
    }

    #[test]
    fn gbk_roundtrip_via_label() {
        let kind = kind_for_label("gbk").unwrap();
        let tf = decode_with(kind, &[0xD6, 0xD0, 0xCE, 0xC4]);
        let out = encode_back(&tf, "中文");
        assert_eq!(out, [0xD6, 0xD0, 0xCE, 0xC4]);
    }
}

#[cfg(test)]
mod repro {
    use super::*;

    #[test]
    fn utf16le_cn_no_bom_detected() {
        // 中文 UTF-16LE 无 BOM（用户场景：一行文本显示多行+乱码）
        let mut data = Vec::new();
        for u in "这是一行测试文本\n".encode_utf16() {
            data.push((u & 0xFF) as u8);
            data.push((u >> 8) as u8);
        }
        let tf = decode(&data);
        assert!(!tf.is_binary, "中文 UTF-16LE 不应判为二进制");
        assert_eq!(tf.text, "这是一行测试文本\n");
        assert_eq!(tf.encoding.name(), "UTF-16LE");
    }

    #[test]
    fn utf16le_cn_short_detected() {
        // 短中文（<16 字节，NUL 嗅探失效）；含换行 → 必含 NUL，可识别
        let mut data = Vec::new();
        for u in "中文测试\n".encode_utf16() {
            data.push((u & 0xFF) as u8);
            data.push((u >> 8) as u8);
        }
        let tf = decode(&data);
        assert!(!tf.is_binary);
        assert_eq!(tf.text, "中文测试\n");
        assert_eq!(tf.encoding.name(), "UTF-16LE");
    }

    #[test]
    fn gbk_not_misdetected_as_utf16() {
        // GBK 中文不应被 UTF-16 试解码误判
        let bytes = [
            0xD5, 0xE2, 0xCA, 0xC7, 0xD2, 0xBB, 0xD0, 0xD0, 0xB2, 0xE2, 0xCA, 0xD4, 0xCE, 0xC4,
            0xB1, 0xBE, 0x0A,
        ];
        let tf = decode(&bytes);
        assert!(!tf.is_binary);
        assert_eq!(tf.text, "这是一行测试文本\n");
        assert_eq!(tf.encoding.name(), "GBK");
    }
}
