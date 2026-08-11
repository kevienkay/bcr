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

/// Profile 文件路径：`~/.bcr-profiles.toml`
fn profiles_path() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".bcr-profiles.toml")
}

fn load() -> Profiles {
    let path = profiles_path();
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_all(p: &Profiles) -> Result<(), String> {
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
