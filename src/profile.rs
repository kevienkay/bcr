//! 比较规则 Profile（P10）：把一次比较的完整"规则集"（过滤/忽略/编码等）打包成
//! 可命名复用的 Profile，存于 `~/.bcr-profiles.toml`。
//!
//! 类似 Beyond Compare 的规则（Rules）面板：保存后可用一条命令复用于任意比较。
//! `compare` / `diff` / `csv` 等命令支持 `--profile <name>` 合并规则。

use crate::i18n::{fmt, Key};
use clap::Args;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// 一条可复用规则集
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Profile {
    /// 包含过滤（glob，可重复）
    #[serde(default)]
    pub includes: Vec<String>,
    /// 排除过滤（glob，可重复）
    #[serde(default)]
    pub excludes: Vec<String>,
    /// 忽略所有空白差异
    #[serde(default)]
    pub ignore_whitespace: bool,
    /// 忽略行尾空白差异
    #[serde(default)]
    pub ignore_trailing: bool,
    /// 忽略大小写差异
    #[serde(default)]
    pub ignore_case: bool,
    /// 强制编码（utf-8/utf-16le/gbk/...，空 = 自动检测）
    #[serde(default)]
    pub encoding: Option<String>,
    /// 深度比较（内容哈希）
    #[serde(default)]
    pub compare_content: bool,
    /// 移动/重命名检测
    #[serde(default = "default_true")]
    pub detect_moves: bool,
}

fn default_true() -> bool {
    true
}

/// 全部 Profile（BTreeMap 保证 list 顺序稳定）
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Profiles {
    #[serde(default)]
    pub profiles: BTreeMap<String, Profile>,
}

/// profile 子命令参数
#[derive(Args, Debug)]
pub struct ProfileArgs {
    #[command(subcommand)]
    pub cmd: ProfileCmd,
}

#[derive(clap::Subcommand, Debug)]
pub enum ProfileCmd {
    /// 保存规则为 Profile：bcr profile save <name> [选项]
    Save(SaveProfileArgs),
    /// 列出全部 Profile
    List,
    /// 导出 Profile 为独立文件（迁移/分享）：bcr profile export <name> <file>
    Export(ExportProfileArgs),
    /// 从文件导入 Profile：bcr profile import <file> [--name <name>]
    Import(ImportProfileArgs),
    /// 删除 Profile
    Delete(DeleteProfileArgs),
}

#[derive(Args, Debug)]
pub struct SaveProfileArgs {
    /// Profile 名
    pub name: String,
    /// 包含过滤（glob，可重复）
    #[arg(long = "include")]
    pub includes: Vec<String>,
    /// 排除过滤（glob，可重复）
    #[arg(long = "exclude")]
    pub excludes: Vec<String>,
    /// 忽略所有空白差异
    #[arg(long)]
    pub ignore_whitespace: bool,
    /// 忽略行尾空白差异
    #[arg(long)]
    pub ignore_trailing: bool,
    /// 忽略大小写差异
    #[arg(long)]
    pub ignore_case: bool,
    /// 强制编码（utf-8/utf-16le/gbk/...）
    #[arg(long)]
    pub encoding: Option<String>,
    /// 深度比较（内容哈希）
    #[arg(long)]
    pub compare_content: bool,
    /// 关闭移动检测（默认开启）
    #[arg(long)]
    pub no_detect_moves: bool,
}

#[derive(Args, Debug)]
pub struct DeleteProfileArgs {
    /// Profile 名
    pub name: String,
}

#[derive(Args, Debug)]
pub struct ExportProfileArgs {
    /// Profile 名
    pub name: String,
    /// 输出文件路径
    pub file: String,
}

#[derive(Args, Debug)]
pub struct ImportProfileArgs {
    /// 输入文件路径（bcr profile export 生成的 .toml）
    pub file: String,
    /// 导入后的名字（缺省用文件内的 name 字段；无则用文件名）
    #[arg(long)]
    pub name: Option<String>,
}

/// Profile 文件路径：`~/.bcr-profiles.toml`
pub fn profiles_path() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".bcr-profiles.toml")
}

pub fn load() -> Profiles {
    let path = profiles_path();
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_all(p: &Profiles) -> Result<(), String> {
    let path = profiles_path();
    let toml_str = toml::to_string_pretty(p).map_err(|e| e.to_string())?;
    std::fs::write(&path, toml_str).map_err(|e| e.to_string())
}

/// 按名加载 Profile（不存在返回 Err）
pub fn get(name: &str) -> Result<Profile, String> {
    load()
        .profiles
        .get(name)
        .cloned()
        .ok_or_else(|| fmt(Key::ProfileNotFound, &[name]))
}

/// 运行 profile 子命令，返回进程退出码
pub fn run(args: &ProfileArgs) -> i32 {
    match &args.cmd {
        ProfileCmd::Save(sa) => run_save(sa),
        ProfileCmd::List => run_list(),
        ProfileCmd::Export(ea) => run_export(ea),
        ProfileCmd::Import(ia) => run_import(ia),
        ProfileCmd::Delete(da) => run_delete(da),
    }
}

fn run_save(a: &SaveProfileArgs) -> i32 {
    let mut all = load();
    if all.profiles.contains_key(&a.name) {
        eprintln!("bcr: {}", fmt(Key::ProfileExists, &[&a.name]));
        return 2;
    }
    all.profiles.insert(
        a.name.clone(),
        Profile {
            includes: a.includes.clone(),
            excludes: a.excludes.clone(),
            ignore_whitespace: a.ignore_whitespace,
            ignore_trailing: a.ignore_trailing,
            ignore_case: a.ignore_case,
            encoding: a.encoding.clone(),
            compare_content: a.compare_content,
            detect_moves: !a.no_detect_moves,
        },
    );
    match save_all(&all) {
        Ok(()) => {
            println!("{}", fmt(Key::ProfileSaved, &[&a.name]));
            0
        }
        Err(e) => {
            eprintln!("bcr: {}", fmt(Key::ProfileWriteFailed, &[&e]));
            2
        }
    }
}

fn run_list() -> i32 {
    let all = load();
    if all.profiles.is_empty() {
        println!("{}", fmt(Key::ProfileEmpty, &[]));
        return 0;
    }
    for (name, p) in &all.profiles {
        let mut parts: Vec<&str> = Vec::new();
        if !p.includes.is_empty() {
            parts.push("inc");
        }
        if !p.excludes.is_empty() {
            parts.push("exc");
        }
        if p.ignore_whitespace {
            parts.push("ws");
        }
        if p.ignore_trailing {
            parts.push("trail");
        }
        if p.ignore_case {
            parts.push("case");
        }
        if p.encoding.is_some() {
            parts.push("enc");
        }
        if p.compare_content {
            parts.push("hash");
        }
        if !p.detect_moves {
            parts.push("nomove");
        }
        let flags = if parts.is_empty() {
            "-".to_string()
        } else {
            parts.join(",")
        };
        println!("{name}\t[{flags}]");
    }
    0
}

fn run_delete(a: &DeleteProfileArgs) -> i32 {
    let mut all = load();
    if all.profiles.remove(&a.name).is_none() {
        eprintln!("bcr: {}", fmt(Key::ProfileNotFound, &[&a.name]));
        return 2;
    }
    match save_all(&all) {
        Ok(()) => {
            println!("{}", fmt(Key::ProfileDeleted, &[&a.name]));
            0
        }
        Err(e) => {
            eprintln!("bcr: {}", fmt(Key::ProfileWriteFailed, &[&e]));
            2
        }
    }
}

/// 导出 Profile 为独立 TOML 文件（含 name 字段，便于迁移/分享）
fn run_export(a: &ExportProfileArgs) -> i32 {
    let all = load();
    let Some(p) = all.profiles.get(&a.name) else {
        eprintln!("bcr: {}", fmt(Key::ProfileNotFound, &[&a.name]));
        return 2;
    };
    let doc = format!(
        "name = \"{}\"\n\n{}",
        a.name,
        toml::to_string_pretty(p).unwrap_or_default()
    );
    match std::fs::write(&a.file, &doc) {
        Ok(()) => {
            println!("已导出 Profile '{}' → {}", a.name, a.file);
            0
        }
        Err(e) => {
            eprintln!("bcr: 导出失败: {}", e);
            2
        }
    }
}

/// 从导出文件导入 Profile（--name 可改名；重名报错）
fn run_import(a: &ImportProfileArgs) -> i32 {
    let content = match std::fs::read_to_string(&a.file) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("bcr: 读取 {} 失败: {}", a.file, e);
            return 2;
        }
    };
    // 兼容两种格式：带 name 字段的导出文件 / 纯 Profile 表
    let file_name = std::path::Path::new(&a.file)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "imported".to_string());
    let mut name = a.name.clone().unwrap_or(file_name);
    let mut profile: Profile = match toml::from_str(&content) {
        Ok(p) => p,
        Err(_) => {
            // 带 name 字段：先解析提取 name，再解析 Profile 主体
            let name_opt: Option<toml::Value> = toml::from_str(&content).ok();
            if let Some(toml::Value::String(n)) = name_opt.as_ref().and_then(|v| v.get("name")) {
                if a.name.is_none() {
                    name = n.clone();
                }
            }
            match toml::from_str(&content) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("bcr: 解析 {} 失败: {}", a.file, e);
                    return 2;
                }
            }
        }
    };
    // name 字段不应进入 Profile 主体（Profile 无 name 字段，反序列化会忽略未知字段？不——严格模式会报错）
    // 若解析失败是因为 name 字段，则剥离后重试
    if profile.includes.is_empty()
        && !profile.excludes.is_empty()
        && content.contains("name =")
        && toml::from_str::<Profile>(&content).is_err()
    {
        // 保留主体字段：尝试按 Profile 表解析（忽略顶层 name）
        let mut stripped = String::new();
        for line in content.lines() {
            if line.trim_start().starts_with("name =") {
                continue;
            }
            stripped.push_str(line);
            stripped.push('\n');
        }
        match toml::from_str(&stripped) {
            Ok(p) => profile = p,
            Err(e) => {
                eprintln!("bcr: 解析 {} 失败: {}", a.file, e);
                return 2;
            }
        }
    }
    let mut all = load();
    if all.profiles.contains_key(&name) {
        eprintln!("bcr: {}", fmt(Key::ProfileExists, &[&name]));
        return 2;
    }
    all.profiles.insert(name.clone(), profile);
    match save_all(&all) {
        Ok(()) => {
            println!("已导入 Profile '{}' ← {}", name, a.file);
            0
        }
        Err(e) => {
            eprintln!("bcr: {}", fmt(Key::ProfileWriteFailed, &[&e]));
            2
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_roundtrip_toml() {
        let mut p = Profiles::default();
        p.profiles.insert(
            "code".into(),
            Profile {
                includes: vec!["*.rs".into()],
                excludes: vec!["target/**".into()],
                ignore_whitespace: true,
                ignore_trailing: true,
                ignore_case: false,
                encoding: Some("utf-8".into()),
                compare_content: true,
                detect_moves: false,
            },
        );
        let toml_str = toml::to_string_pretty(&p).unwrap();
        let back: Profiles = toml::from_str(&toml_str).unwrap();
        let prof = &back.profiles["code"];
        assert_eq!(prof.includes, vec!["*.rs"]);
        assert_eq!(prof.excludes, vec!["target/**"]);
        assert!(prof.ignore_whitespace);
        assert!(prof.ignore_trailing);
        assert_eq!(prof.encoding.as_deref(), Some("utf-8"));
        assert!(prof.compare_content);
        assert!(!prof.detect_moves);
    }

    #[test]
    fn profile_defaults() {
        let toml_str = r#"
[profiles.demo]
"#;
        let p: Profiles = toml::from_str(toml_str).unwrap();
        let prof = &p.profiles["demo"];
        assert!(prof.detect_moves);
        assert!(!prof.compare_content);
        assert!(prof.includes.is_empty());
        assert!(prof.encoding.is_none());
    }
}
