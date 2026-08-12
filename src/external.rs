//! A14：第三方对比工具接入。
//!
//! 未支持的文件格式（二进制/未知扩展名）可通过外部工具对比，配置在
//! `~/.bcr-external.toml`：
//!
//! ```toml
//! [tools]
//! "docx" = "soffice --diff {left} {right}"
//! "pdf"  = "diffpdf {left} {right}"
//! "xlsx" = "xlsdiff {left} {right}"
//! ```
//!
//! 占位符：`{left}` `{right}`（两侧文件路径，自动加引号转义）。
//! CLI：`bcr diff --external a.docx b.docx`；GUI：双击未知格式文件自动查表。

use std::collections::BTreeMap;
use std::path::Path;

/// 外部工具配置（~/.bcr-external.toml）
#[derive(Debug, Default)]
pub struct ExternalTools {
    /// 扩展名（小写，无点）→ 命令模板
    pub tools: BTreeMap<String, String>,
}

impl ExternalTools {
    /// 配置文件路径
    pub fn config_path() -> std::path::PathBuf {
        let home = std::env::var("HOME")
            .map(std::path::PathBuf::from)
            .unwrap_or_default();
        home.join(".bcr-external.toml")
    }

    /// 加载配置（文件不存在返回空表，不算错误）
    pub fn load() -> Self {
        let mut tools = BTreeMap::new();
        let path = Self::config_path();
        if let Ok(text) = std::fs::read_to_string(&path) {
            if let Ok(tbl) = text.parse::<toml::Table>() {
                if let Some(t) = tbl.get("tools").and_then(|v| v.as_table()) {
                    for (k, v) in t {
                        if let Some(cmd) = v.as_str() {
                            tools.insert(k.to_lowercase(), cmd.to_string());
                        }
                    }
                }
            }
        }
        ExternalTools { tools }
    }

    /// 按文件路径查找命令模板（扩展名匹配，小写）
    pub fn command_for(&self, path: &str) -> Option<&str> {
        let ext = Path::new(path)
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        self.tools.get(&ext).map(|s| s.as_str())
    }

    /// 渲染命令：替换 {left}/{right} 占位符并做 shell 转义
    pub fn render(template: &str, left: &str, right: &str) -> String {
        template
            .replace("{left}", &shell_quote(left))
            .replace("{right}", &shell_quote(right))
    }

    /// 执行外部对比命令，返回退出码（None = 命令不存在）
    pub fn run(template: &str, left: &str, right: &str) -> Option<i32> {
        let cmd = Self::render(template, left, right);
        // sh -c 执行（跨平台：Windows 用 cmd /C）
        #[cfg(windows)]
        let code = std::process::Command::new("cmd")
            .args(["/C", &cmd])
            .status()
            .ok()
            .map(|s| s.code().unwrap_or(2));
        #[cfg(not(windows))]
        let code = std::process::Command::new("sh")
            .args(["-c", &cmd])
            .status()
            .ok()
            .map(|s| s.code().unwrap_or(2));
        code
    }
}

/// POSIX shell 单引号转义（Windows cmd 下退化为原样 + 双引号）
fn shell_quote(s: &str) -> String {
    #[cfg(windows)]
    {
        format!("\"{}\"", s.replace('"', "\"\""))
    }
    #[cfg(not(windows))]
    {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_replaces_placeholders() {
        let cmd = ExternalTools::render("soffice --diff {left} {right}", "/a/b.docx", "/c/d.docx");
        #[cfg(windows)]
        {
            assert!(cmd.contains("\"/a/b.docx\""));
            assert!(cmd.contains("\"/c/d.docx\""));
        }
        #[cfg(not(windows))]
        {
            assert!(cmd.contains("'/a/b.docx'"));
            assert!(cmd.contains("'/c/d.docx'"));
        }
    }

    #[test]
    fn shell_quote_escapes_single_quote() {
        let q = shell_quote("it's here");
        #[cfg(windows)]
        {
            assert!(q.contains("\"it's here\""));
        }
        #[cfg(not(windows))]
        {
            assert!(q.contains("it'\\''s here"));
        }
    }

    #[test]
    fn command_for_matches_extension() {
        let mut t = ExternalTools::default();
        t.tools
            .insert("docx".into(), "soffice --diff {left} {right}".into());
        assert!(t.command_for("/x/a.DOCX").is_some());
        assert!(t.command_for("/x/a.docx").is_some());
        assert!(t.command_for("/x/a.txt").is_none());
        assert!(t.command_for("/x/noext").is_none());
    }
}
