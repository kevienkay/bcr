//! 会话保存/恢复（P4）。
//!
//! 把一次目录对比的完整配置（左右路径 + 选项）持久化为可复用 Session，
//! 存于 `~/.bcr-sessions.toml`，支持 list/run/save/delete，类似 Beyond
//! Compare 的 Session 概念：保存后可用一条命令复跑同一比较。

use crate::compare::CompareArgs;
use crate::i18n::{fmt, Key};
use clap::Args;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// 一个已保存的会话
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub left: String,
    pub right: String,
    #[serde(default)]
    pub compare_content: bool,
    #[serde(default = "default_true")]
    pub detect_moves: bool,
    #[serde(default)]
    pub includes: Vec<String>,
    #[serde(default)]
    pub excludes: Vec<String>,
}

fn default_true() -> bool {
    true
}

/// 全部会话（BTreeMap 保证 list 顺序稳定）
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Sessions {
    #[serde(default)]
    pub sessions: BTreeMap<String, Session>,
}

/// session 子命令参数
#[derive(Args, Debug)]
pub struct SessionArgs {
    #[command(subcommand)]
    pub cmd: SessionCmd,
}

#[derive(clap::Subcommand, Debug)]
pub enum SessionCmd {
    /// 保存当前比较为会话：bcr session save <name> <left> <right> [选项]
    Save(SaveArgs),
    /// 列出全部会话
    List,
    /// 运行已保存的会话（等价于带原选项执行 compare）
    Run(RunArgs),
    /// 删除会话
    Delete(DeleteArgs),
}

#[derive(Args, Debug)]
pub struct SaveArgs {
    /// 会话名
    pub name: String,
    /// 左侧目录
    pub left: String,
    /// 右侧目录
    pub right: String,
    /// 深度比较（内容哈希）
    #[arg(long)]
    pub compare_content: bool,
    /// 关闭移动检测（默认开启）
    #[arg(long)]
    pub no_detect_moves: bool,
    /// 包含过滤（glob，可重复）
    #[arg(long = "include")]
    pub includes: Vec<String>,
    /// 排除过滤（glob，可重复）
    #[arg(long = "exclude")]
    pub excludes: Vec<String>,
}

#[derive(Args, Debug)]
pub struct RunArgs {
    /// 会话名
    pub name: String,
}

#[derive(Args, Debug)]
pub struct DeleteArgs {
    /// 会话名
    pub name: String,
}

/// 会话文件路径：`~/.bcr-sessions.toml`
pub fn sessions_path() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".bcr-sessions.toml")
}

pub fn load() -> Sessions {
    let path = sessions_path();
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_all(s: &Sessions) -> Result<(), String> {
    let path = sessions_path();
    let toml_str = toml::to_string_pretty(s).map_err(|e| e.to_string())?;
    std::fs::write(&path, toml_str).map_err(|e| e.to_string())
}

/// 运行 session 子命令，返回进程退出码
pub fn run(args: &SessionArgs) -> i32 {
    match &args.cmd {
        SessionCmd::Save(sa) => run_save(sa),
        SessionCmd::List => run_list(),
        SessionCmd::Run(ra) => run_run(ra),
        SessionCmd::Delete(da) => run_delete(da),
    }
}

fn run_save(a: &SaveArgs) -> i32 {
    let mut all = load();
    if all.sessions.contains_key(&a.name) {
        eprintln!("bcr: {}", fmt(Key::SessionExists, &[&a.name]));
        return 2;
    }
    all.sessions.insert(
        a.name.clone(),
        Session {
            left: a.left.clone(),
            right: a.right.clone(),
            compare_content: a.compare_content,
            detect_moves: !a.no_detect_moves,
            includes: a.includes.clone(),
            excludes: a.excludes.clone(),
        },
    );
    match save_all(&all) {
        Ok(()) => {
            println!("{}", fmt(Key::SessionSaved, &[&a.name]));
            0
        }
        Err(e) => {
            eprintln!("bcr: {}", fmt(Key::SessionWriteFailed, &[&e]));
            2
        }
    }
}

fn run_list() -> i32 {
    let all = load();
    if all.sessions.is_empty() {
        println!("{}", fmt(Key::SessionEmpty, &[]));
        return 0;
    }
    for (name, s) in &all.sessions {
        let opts = format!(
            "{} ↔ {}{}{}",
            s.left,
            s.right,
            if s.compare_content {
                " --compare-content"
            } else {
                ""
            },
            if s.includes.is_empty() {
                String::new()
            } else {
                format!(" --include {}", s.includes.join(","))
            },
        );
        println!("{name}\t{opts}");
    }
    0
}

fn run_run(a: &RunArgs) -> i32 {
    let all = load();
    let Some(s) = all.sessions.get(&a.name) else {
        eprintln!("bcr: {}", fmt(Key::SessionNotFound, &[&a.name]));
        return 2;
    };
    let args = CompareArgs {
        left: s.left.clone(),
        right: s.right.clone(),
        compare_content: s.compare_content,
        includes: s.includes.clone(),
        excludes: s.excludes.clone(),
        show_same: false,
        detect_moves: s.detect_moves,
        summary: false,
        html: None,
        profile: None,
        color: "auto".into(),
    };
    crate::compare::run(&args)
}

fn run_delete(a: &DeleteArgs) -> i32 {
    let mut all = load();
    if all.sessions.remove(&a.name).is_none() {
        eprintln!("bcr: {}", fmt(Key::SessionNotFound, &[&a.name]));
        return 2;
    }
    match save_all(&all) {
        Ok(()) => {
            println!("{}", fmt(Key::SessionDeleted, &[&a.name]));
            0
        }
        Err(e) => {
            eprintln!("bcr: {}", fmt(Key::SessionWriteFailed, &[&e]));
            2
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_roundtrip_toml() {
        let mut s = Sessions::default();
        s.sessions.insert(
            "backup".into(),
            Session {
                left: "/a".into(),
                right: "/b".into(),
                compare_content: true,
                detect_moves: false,
                includes: vec!["*.rs".into()],
                excludes: vec!["target/**".into()],
            },
        );
        let toml_str = toml::to_string_pretty(&s).unwrap();
        let back: Sessions = toml::from_str(&toml_str).unwrap();
        let sess = &back.sessions["backup"];
        assert_eq!(sess.left, "/a");
        assert_eq!(sess.right, "/b");
        assert!(sess.compare_content);
        assert!(!sess.detect_moves);
        assert_eq!(sess.includes, vec!["*.rs"]);
        assert_eq!(sess.excludes, vec!["target/**"]);
    }

    #[test]
    fn session_defaults_detect_moves_true() {
        let toml_str = r#"
[sessions.demo]
left = "/l"
right = "/r"
"#;
        let s: Sessions = toml::from_str(toml_str).unwrap();
        let sess = &s.sessions["demo"];
        assert!(sess.detect_moves);
        assert!(!sess.compare_content);
        assert!(sess.includes.is_empty());
    }
}
