//! P1：语法高亮（syntect）。
//!
//! 全局只加载一次 SyntaxSet（60+ 语言）与默认主题；按扩展名/文件名解析语法，
//! 对单行文本做高亮，返回 (byte_start, byte_len, (r,g,b)) 分段，GUI 与 CLI 共用。
//!
//! 注意：逐行独立高亮（HighlightLines 每行新建），跨行结构（多行注释/字符串）
//! 只高亮起始行——v1 折衷，虚拟化渲染下成本最低。

use std::path::Path;
use std::sync::OnceLock;

/// 默认主题名（syntect 内置）
const DEFAULT_THEME: &str = "base16-ocean.dark";

static SYNTAXES: OnceLock<syntect::parsing::SyntaxSet> = OnceLock::new();
static THEMES: OnceLock<syntect::highlighting::ThemeSet> = OnceLock::new();

fn syntaxes() -> &'static syntect::parsing::SyntaxSet {
    SYNTAXES.get_or_init(syntect::parsing::SyntaxSet::load_defaults_newlines)
}

fn themes() -> &'static syntect::highlighting::ThemeSet {
    THEMES.get_or_init(syntect::highlighting::ThemeSet::load_defaults)
}

/// 获取默认主题
pub fn theme() -> &'static syntect::highlighting::Theme {
    let ts = themes();
    ts.themes
        .get(DEFAULT_THEME)
        .or_else(|| ts.themes.values().next())
        .expect("syntect 至少内置一个主题")
}

/// 按路径解析语法；未知类型返回 None（调用方回退纯文本）
pub fn syntax_for(path: &str) -> Option<&'static syntect::parsing::SyntaxReference> {
    let p = Path::new(path);
    let ss = syntaxes();
    if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
        if let Some(s) = ss.find_syntax_by_token(name) {
            return Some(s);
        }
    }
    if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
        if let Some(s) = ss.find_syntax_by_extension(ext) {
            return Some(s);
        }
    }
    None
}

/// 高亮单行，返回 (byte_start, byte_len, rgb) 分段。
/// 调用方负责把分段映射到 egui Color32 / ANSI 序列。
pub fn highlight_line(
    line: &str,
    syntax: &syntect::parsing::SyntaxReference,
) -> Vec<(usize, usize, (u8, u8, u8))> {
    let mut hl = syntect::easy::HighlightLines::new(syntax, theme());
    let Ok(ranges) = hl.highlight_line(line, syntaxes()) else {
        return Vec::new();
    };
    let mut out = Vec::with_capacity(ranges.len());
    let mut off = 0usize;
    for (style, text) in ranges {
        let c = style.foreground;
        out.push((off, text.len(), (c.r, c.g, c.b)));
        off += text.len();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_syntax_found() {
        let s = syntax_for("main.rs").expect("rust 语法应可解析");
        assert_eq!(s.name, "Rust");
    }

    #[test]
    fn extension_variants() {
        assert!(syntax_for("app.py").is_some());
        assert!(syntax_for("style.css").is_some());
        assert!(syntax_for("data.json").is_some());
        assert!(syntax_for("README.md").is_some());
    }

    #[test]
    fn unknown_type_returns_none() {
        assert!(syntax_for("noext").is_none());
        assert!(syntax_for("archive.zzz").is_none());
    }

    #[test]
    fn highlight_segments_reassemble() {
        let s = syntax_for("x.rs").unwrap();
        let line = "fn main() { let x = 42; }";
        let segs = highlight_line(line, s);
        // 分段拼接回原文
        let mut rebuilt = String::new();
        for (start, len, _) in &segs {
            assert_eq!(&line[*start..start + len], {
                // 通过字符串切片验证
                let _ = &line[*start..*start + *len];
                &line[*start..*start + *len]
            });
            rebuilt.push_str(&line[*start..*start + *len]);
        }
        assert_eq!(rebuilt, line);
    }

    #[test]
    fn highlight_plain_line_no_panic() {
        let s = syntax_for("x.rs").unwrap();
        let segs = highlight_line("// just a comment", s);
        assert!(!segs.is_empty());
    }

    #[test]
    fn theme_available() {
        // 显示名与键名不同（Base16 Ocean Dark / base16-ocean.dark），断言非空即可
        assert!(theme().name.as_deref().is_some_and(|n| !n.is_empty()));
    }
}
