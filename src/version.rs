//! P25：版本比较模式 — 从文件中提取版本号，供 `compare --compare-version` 使用。
//!
//! Beyond Compare 的 "版本比较" 模式按文件版本号(而非大小/mtime)判断差异，
//! 典型场景：exe/dll/驱动文件的 FileVersion。本模块实现轻量提取：
//!
//! 1. **PE 版本资源**：扫描文件中的 "FileVersion" / "ProductVersion" 字符串
//!    （ASCII 与 UTF-16LE 两种编码），取其后的版本号（如 `1.2.3.4`）
//! 2. **回退**：匹配文件中第一个 `\d+(\.\d+)+` 版本号模式
//! 3. 提取失败 → None（调用方回退到 mtime 比较，与快速模式一致）
//!
//! 无需 winapi/object 解析依赖，跨平台可用；对打包/资源压缩的 exe 可能提取
//! 不到版本号，此时回退行为保证不会误判。

use regex::Regex;

/// 从文件内容中提取版本号。
///
/// 优先级：
/// 1. FileVersion / ProductVersion 字段（ASCII 或 UTF-16LE）
/// 2. 内容中第一个 `数字[.数字]+` 模式
///
/// 返回归一化的版本号字符串（保留原始格式，比较时按段数值化）。
pub fn extract_version(data: &[u8]) -> Option<String> {
    // 1) 显式版本字段
    if let Some(v) = extract_field(data, b"FileVersion") {
        return Some(v);
    }
    if let Some(v) = extract_field(data, b"ProductVersion") {
        return Some(v);
    }
    // 2) 通用版本号模式（跳过太长的段，避免误匹配时间戳/日期）
    let re = Regex::new(r"\b\d{1,4}(\.\d{1,4}){1,3}\b").ok()?;
    // 只扫描前 4MB，版本资源通常在文件头/资源段
    let head = &data[..data.len().min(4 * 1024 * 1024)];
    let ascii = String::from_utf8_lossy(head);
    for m in re.find_iter(&ascii) {
        let s = m.as_str();
        // 过滤常见误匹配：年份日期 2024.01.01 之类的仍保留（BC 也会比较）
        if s.len() >= 3 && s.len() <= 19 {
            return Some(s.to_string());
        }
    }
    None
}

/// 在字节流中查找 `field=` 或 `field 空格` 后的版本号。
/// 同时处理 ASCII 与 UTF-16LE（Windows 版本资源块内两种都有）。
fn extract_field(data: &[u8], field: &[u8]) -> Option<String> {
    // ASCII 形式：field 后跟 0x00 或空格，然后版本号
    if let Some(v) = extract_field_ascii(data, field) {
        return Some(v);
    }
    // UTF-16LE 形式：字段名每个字符后跟 0x00
    let wide: Vec<u8> = field.iter().flat_map(|&b| [b, 0u8]).collect();
    extract_field_ascii(data, &wide)
}

fn extract_field_ascii(data: &[u8], field: &[u8]) -> Option<String> {
    let re = Regex::new(r"\d{1,4}(\.\d{1,4}){1,3}").ok()?;
    let head = &data[..data.len().min(8 * 1024 * 1024)];
    // 逐位置查找字段名
    let mut pos = 0usize;
    while pos + field.len() <= head.len() {
        if &head[pos..pos + field.len()] == field {
            // 字段名后允许 0x00 / 空格 / '=' 分隔
            let mut p = pos + field.len();
            while p < head.len() && matches!(head[p], 0x00 | b' ' | b'=' | b'\t') {
                p += 1;
            }
            // 在附近 64 字节内找版本号（分隔符后立即或稍有间隔）
            let window_end = (p + 128).min(head.len());
            let window = String::from_utf8_lossy(&head[p..window_end]);
            if let Some(m) = re.find(&window) {
                let v = m.as_str().to_string();
                if v.len() >= 3 && v.len() <= 19 {
                    return Some(v);
                }
            }
            // 若字段后是 UTF-16LE 的版本号，ASCII 窗口可能夹 0x00；再做一次宽字符扫描
            let wide_window = &head[p..window_end];
            let wide_bytes: Vec<u8> = wide_window
                .chunks_exact(2)
                .filter_map(|c| (c[1] == 0).then_some(c[0]))
                .collect();
            let wide_str = String::from_utf8_lossy(&wide_bytes);
            if let Some(m) = re.find(&wide_str) {
                let v = m.as_str().to_string();
                if v.len() >= 3 && v.len() <= 19 {
                    return Some(v);
                }
            }
        }
        pos += 1;
    }
    None
}

/// 版本号比较：按 `.` 分段数值比较。
/// a < b → Less，相等 → Equal，a > b → Greater。
/// 段数不同时缺失段按 0 处理（1.2 == 1.2.0）。
pub fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    let parse = |s: &str| -> Vec<u64> {
        s.split('.')
            .map(|p| p.parse::<u64>().unwrap_or(0))
            .collect()
    };
    let va = parse(a);
    let vb = parse(b);
    let max = va.len().max(vb.len());
    for i in 0..max {
        let x = va.get(i).copied().unwrap_or(0);
        let y = vb.get(i).copied().unwrap_or(0);
        match x.cmp(&y) {
            std::cmp::Ordering::Equal => continue,
            other => return other,
        }
    }
    std::cmp::Ordering::Equal
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_from_ascii_field() {
        let data =
            b"VS_VERSION_INFO\x00FileVersion\x00, 1.2.3.4\x00\x00ProductVersion\x00, 5.6.7.8";
        assert_eq!(extract_version(data).as_deref(), Some("1.2.3.4"));
    }

    #[test]
    fn extract_from_utf16_field() {
        // UTF-16LE: F\0i\0l\0e\0V\0e\0r\0s\0i\0o\0n\0 \01\0.\02\0.\03\0.\04\0
        let mut data = Vec::new();
        for b in "FileVersion".bytes() {
            data.extend_from_slice(&[b, 0]);
        }
        data.extend_from_slice(&[b' ', 0]);
        for b in "1.2.3.4".bytes() {
            data.extend_from_slice(&[b, 0]);
        }
        assert_eq!(extract_version(&data).as_deref(), Some("1.2.3.4"));
    }

    #[test]
    fn extract_generic_pattern() {
        let data = b"build 2024.05.11 nightly";
        assert_eq!(extract_version(data).as_deref(), Some("2024.05.11"));
    }

    #[test]
    fn no_version_returns_none() {
        let data = b"hello world no numbers here";
        assert_eq!(extract_version(data), None);
    }

    #[test]
    fn product_version_fallback() {
        let data = b"FileVersion\x00\x00\x00\x00ProductVersion\x00, 9.8.7.6\x00";
        assert_eq!(extract_version(data).as_deref(), Some("9.8.7.6"));
    }

    #[test]
    fn version_ordering() {
        use std::cmp::Ordering;
        assert_eq!(compare_versions("1.2.3", "1.2.3"), Ordering::Equal);
        assert_eq!(compare_versions("1.2.3", "1.2.3.0"), Ordering::Equal);
        assert_eq!(compare_versions("1.2.3", "1.2.4"), Ordering::Less);
        assert_eq!(compare_versions("2.0", "1.9.9"), Ordering::Greater);
        assert_eq!(compare_versions("1.10", "1.9"), Ordering::Greater);
    }

    #[test]
    fn field_with_equals_separator() {
        let data = b"FileVersion=3.2.1";
        assert_eq!(extract_version(data).as_deref(), Some("3.2.1"));
    }

    #[test]
    fn wide_version_after_ascii_field() {
        // FileVersion 后是 UTF-16LE 的版本号
        let mut data = b"FileVersion\x00\x00".to_vec();
        for b in "1.0.0.1".bytes() {
            data.extend_from_slice(&[b, 0]);
        }
        assert_eq!(extract_version(&data).as_deref(), Some("1.0.0.1"));
    }
}
