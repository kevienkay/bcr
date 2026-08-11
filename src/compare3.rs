//! 三路文件夹对比（P6：Folder Compare 3-way）。
//!
//! `bcr compare3 BASE LEFT RIGHT`：对比三个目录树，对每个相对路径
//! 输出三路状态（类似 Beyond Compare 的三路文件夹对比）。每个文件的
//! 状态基于 base/left/right 三者的存在性与内容：
//! - [S]  三处相同
//! - [B]  仅 base 存在（两侧都删除）
//! - [L]  仅 left 存在（新增）
//! - [R]  仅 right 存在（新增）
//! - [LD] 左侧删除（base==right 内容一致）
//! - [RD] 右侧删除（base==left 内容一致）
//! - [LM] 左侧修改（right==base，left 不同）
//! - [RM] 右侧修改（left==base，right 不同）
//! - [M]  两侧相同修改（left==right != base，无冲突）
//! - [C]  冲突（三处都存在且互不相同）

use crate::fsscan::Filter;
use crate::i18n::{fmt, Key};
use crate::vfs::{self, Vfs};
use clap::Args;
use std::collections::BTreeMap;
use std::io::{self, IsTerminal};

/// compare3 子命令参数
#[derive(Args, Debug)]
pub struct Compare3Args {
    /// BASE 目录
    pub base: String,

    /// LEFT 目录
    pub left: String,

    /// RIGHT 目录
    pub right: String,

    /// 深度比较：对大小相同的文件对做 blake3 哈希比对（默认仅比较大小+修改时间）
    #[arg(long)]
    pub compare_content: bool,

    /// 包含过滤（glob，可重复）
    #[arg(long = "include")]
    pub includes: Vec<String>,

    /// 排除过滤（glob，可重复）
    #[arg(long = "exclude")]
    pub excludes: Vec<String>,

    /// 同时显示相同文件
    #[arg(long)]
    pub show_same: bool,

    /// 输出统计信息
    #[arg(long)]
    pub summary: bool,

    /// 颜色输出：auto | always | never
    #[arg(long, default_value = "auto", value_parser = ["auto", "always", "never"])]
    pub color: String,
}

/// 三路文件状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriStatus {
    Same,
    BaseOnly,
    LeftOnly,
    RightOnly,
    LeftDeleted,
    RightDeleted,
    LeftModified,
    RightModified,
    BothModified,
    Conflict,
}

impl TriStatus {
    pub fn tag(self) -> &'static str {
        match self {
            TriStatus::Same => "S",
            TriStatus::BaseOnly => "B",
            TriStatus::LeftOnly => "L",
            TriStatus::RightOnly => "R",
            TriStatus::LeftDeleted => "LD",
            TriStatus::RightDeleted => "RD",
            TriStatus::LeftModified => "LM",
            TriStatus::RightModified => "RM",
            TriStatus::BothModified => "M",
            TriStatus::Conflict => "C",
        }
    }
}

/// 三路比较条目
#[derive(Debug, Clone)]
pub struct TriEntry {
    pub rel: String,
    pub status: TriStatus,
}

/// 三路统计
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TriStats {
    pub same: usize,
    pub base_only: usize,
    pub left_only: usize,
    pub right_only: usize,
    pub left_deleted: usize,
    pub right_deleted: usize,
    pub left_modified: usize,
    pub right_modified: usize,
    pub both_modified: usize,
    pub conflict: usize,
}

impl TriStats {
    pub fn has_differences(self) -> bool {
        self.base_only
            + self.left_only
            + self.right_only
            + self.left_deleted
            + self.right_deleted
            + self.left_modified
            + self.right_modified
            + self.both_modified
            + self.conflict
            > 0
    }
}

/// 三路目录比较结果
#[derive(Debug, Default)]
pub struct TriResult {
    pub entries: Vec<TriEntry>,
    pub stats: TriStats,
}

/// 对比三个目录树，返回结构化结果。
pub fn compare3_dirs(
    base: &std::path::Path,
    left: &std::path::Path,
    right: &std::path::Path,
    filter: &Filter,
    compare_content: bool,
) -> io::Result<TriResult> {
    let b = vfs::LocalVfs::new(base)?;
    let l = vfs::LocalVfs::new(left)?;
    let r = vfs::LocalVfs::new(right)?;
    compare3_vfs(&b, &l, &r, filter, compare_content)
}

/// 对比三个虚拟文件系统后端。
pub fn compare3_vfs(
    base: &dyn Vfs,
    left: &dyn Vfs,
    right: &dyn Vfs,
    filter: &Filter,
    compare_content: bool,
) -> io::Result<TriResult> {
    let b_map = base.scan(filter)?;
    let l_map = left.scan(filter)?;
    let r_map = right.scan(filter)?;

    // 合并 key 集合（已排序，保证输出顺序稳定）
    let mut keys: Vec<&String> = Vec::new();
    for k in b_map.keys() {
        keys.push(k);
    }
    for k in l_map.keys() {
        if !keys.contains(&k) {
            keys.push(k);
        }
    }
    for k in r_map.keys() {
        if !keys.contains(&k) {
            keys.push(k);
        }
    }
    keys.sort();

    let mut result = TriResult::default();
    for key in keys {
        let bm = b_map.get(key);
        let lm = l_map.get(key);
        let rm = r_map.get(key);
        let status = classify(base, left, right, key, bm, lm, rm, compare_content)?;
        match status {
            TriStatus::Same => result.stats.same += 1,
            TriStatus::BaseOnly => result.stats.base_only += 1,
            TriStatus::LeftOnly => result.stats.left_only += 1,
            TriStatus::RightOnly => result.stats.right_only += 1,
            TriStatus::LeftDeleted => result.stats.left_deleted += 1,
            TriStatus::RightDeleted => result.stats.right_deleted += 1,
            TriStatus::LeftModified => result.stats.left_modified += 1,
            TriStatus::RightModified => result.stats.right_modified += 1,
            TriStatus::BothModified => result.stats.both_modified += 1,
            TriStatus::Conflict => result.stats.conflict += 1,
        }
        result.entries.push(TriEntry {
            rel: key.clone(),
            status,
        });
    }
    Ok(result)
}

/// 三侧内容比较辅助：都存在的文件对，判断是否一致
fn eq3(
    base: &dyn Vfs,
    left: &dyn Vfs,
    right: &dyn Vfs,
    rel: &str,
    bm: Option<&crate::fsscan::FileMeta>,
    lm: Option<&crate::fsscan::FileMeta>,
    rm: Option<&crate::fsscan::FileMeta>,
    compare_content: bool,
) -> io::Result<(bool, bool, bool)> {
    // 返回 (base==left, base==right, left==right)
    let deep = |a: &dyn Vfs, c: &dyn Vfs| -> io::Result<bool> { Ok(a.hash(rel)? == c.hash(rel)?) };
    let (bl, br, lr) = if compare_content {
        (deep(base, left)?, deep(base, right)?, deep(left, right)?)
    } else {
        // 快速模式：size+mtime 都同 → 相同；size 同但 mtime 不同 → 哈希兜底
        // （三路对比语义要求准确，避免复制导致 mtime 不同被误判为冲突）
        let fast_or_deep = |a: &dyn Vfs,
                            c: &dyn Vfs,
                            am: Option<&crate::fsscan::FileMeta>,
                            cm: Option<&crate::fsscan::FileMeta>|
         -> io::Result<bool> {
            match (am, cm) {
                (Some(x), Some(y)) if x.size == y.size && x.mtime == y.mtime => Ok(true),
                (Some(x), Some(y)) if x.size == y.size => Ok(a.hash(rel)? == c.hash(rel)?),
                _ => Ok(false),
            }
        };
        (
            fast_or_deep(base, left, bm, lm)?,
            fast_or_deep(base, right, bm, rm)?,
            fast_or_deep(left, right, lm, rm)?,
        )
    };
    Ok((bl, br, lr))
}

/// 对单个 key 分类三路状态
fn classify(
    base: &dyn Vfs,
    left: &dyn Vfs,
    right: &dyn Vfs,
    rel: &str,
    bm: Option<&crate::fsscan::FileMeta>,
    lm: Option<&crate::fsscan::FileMeta>,
    rm: Option<&crate::fsscan::FileMeta>,
    compare_content: bool,
) -> io::Result<TriStatus> {
    match (bm, lm, rm) {
        (Some(_), Some(_), Some(_)) => {
            let (bl, br, lr) = eq3(base, left, right, rel, bm, lm, rm, compare_content)?;
            if bl && br {
                Ok(TriStatus::Same)
            } else if lr && !bl {
                // left == right != base：两侧相同修改
                Ok(TriStatus::BothModified)
            } else if bl {
                // base==left，right 不同
                Ok(TriStatus::RightModified)
            } else if br {
                // base==right，left 不同
                Ok(TriStatus::LeftModified)
            } else {
                Ok(TriStatus::Conflict)
            }
        }
        (Some(_), Some(_), None) => {
            // 左侧存在、右侧缺失：base==left 内容一致 → 右侧删除；否则左侧修改
            let bl = eq3(base, left, right, rel, bm, lm, rm, compare_content)?.0;
            if bl {
                Ok(TriStatus::RightDeleted)
            } else {
                Ok(TriStatus::LeftModified)
            }
        }
        (Some(_), None, Some(_)) => {
            let br = eq3(base, left, right, rel, bm, lm, rm, compare_content)?.1;
            if br {
                Ok(TriStatus::LeftDeleted)
            } else {
                Ok(TriStatus::RightModified)
            }
        }
        (Some(_), None, None) => Ok(TriStatus::BaseOnly),
        (None, Some(_), Some(_)) => {
            // 左侧与右侧都存在、base 缺失：两侧新增
            let lr = eq3(base, left, right, rel, bm, lm, rm, compare_content)?.2;
            if lr {
                // 两侧新增相同内容 → 归为相同修改（无冲突）
                Ok(TriStatus::BothModified)
            } else {
                Ok(TriStatus::Conflict)
            }
        }
        (None, Some(_), None) => Ok(TriStatus::LeftOnly),
        (None, None, Some(_)) => Ok(TriStatus::RightOnly),
        (None, None, None) => unreachable!(),
    }
}

/// 运行 compare3 子命令，返回进程退出码（0=无差异，1=有差异，2=错误）
pub fn run(args: &Compare3Args) -> i32 {
    for (label, p) in [
        ("base", &args.base),
        ("left", &args.left),
        ("right", &args.right),
    ] {
        if !vfs::is_remote(p) && !std::path::Path::new(p).is_dir() {
            eprintln!("bcr: {} ({label})", fmt(Key::NotDir, &[p]));
            return 2;
        }
    }

    let filter = match Filter::new(&args.includes, &args.excludes) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("bcr: {}", fmt(Key::FilterError, &[&e.to_string()]));
            return 2;
        }
    };

    let b = match vfs::open(&args.base) {
        Ok(v) => v,
        Err(e) => {
            eprintln!(
                "bcr: {}",
                fmt(Key::OpenFailed, &[&args.base, &e.to_string()])
            );
            return 2;
        }
    };
    let l = match vfs::open(&args.left) {
        Ok(v) => v,
        Err(e) => {
            eprintln!(
                "bcr: {}",
                fmt(Key::OpenFailed, &[&args.left, &e.to_string()])
            );
            return 2;
        }
    };
    let r = match vfs::open(&args.right) {
        Ok(v) => v,
        Err(e) => {
            eprintln!(
                "bcr: {}",
                fmt(Key::OpenFailed, &[&args.right, &e.to_string()])
            );
            return 2;
        }
    };

    let result = match compare3_vfs(
        b.as_ref(),
        l.as_ref(),
        r.as_ref(),
        &filter,
        args.compare_content,
    ) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("bcr: {}", fmt(Key::ScanFailed, &[&e.to_string()]));
            return 2;
        }
    };

    let color = match args.color.as_str() {
        "always" => true,
        "never" => false,
        _ => io::stdout().is_terminal(),
    };

    for e in &result.entries {
        if e.status == TriStatus::Same && !args.show_same {
            continue;
        }
        let tag = e.status.tag();
        if color {
            let c = match e.status {
                TriStatus::Same => "\x1b[32m",
                TriStatus::BaseOnly => "\x1b[90m",
                TriStatus::LeftOnly | TriStatus::LeftDeleted | TriStatus::LeftModified => {
                    "\x1b[31m"
                }
                TriStatus::RightOnly | TriStatus::RightDeleted | TriStatus::RightModified => {
                    "\x1b[34m"
                }
                TriStatus::BothModified => "\x1b[33m",
                TriStatus::Conflict => "\x1b[35m",
            };
            println!("{c}[{tag}]{RESET} {}", e.rel);
        } else {
            println!("[{tag}] {}", e.rel);
        }
    }

    if args.summary {
        let s = result.stats;
        println!(
            "{}",
            fmt(
                Key::SummaryCompare3,
                &[
                    &s.same.to_string(),
                    &s.base_only.to_string(),
                    &s.left_only.to_string(),
                    &s.right_only.to_string(),
                    &(s.left_deleted + s.right_deleted).to_string(),
                    &(s.left_modified + s.right_modified + s.both_modified).to_string(),
                    &s.conflict.to_string(),
                ]
            )
        );
    }

    if result.stats.has_differences() {
        1
    } else {
        0
    }
}

const RESET: &str = "\x1b[0m";

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn args(base: &str, left: &str, right: &str) -> Compare3Args {
        Compare3Args {
            base: base.into(),
            left: left.into(),
            right: right.into(),
            compare_content: false,
            includes: vec![],
            excludes: vec![],
            show_same: false,
            summary: false,
            color: "never".into(),
        }
    }

    fn make_tree(dir: &std::path::Path, entries: &[(&str, &str)]) {
        for (rel, content) in entries {
            let p = dir.join(rel);
            if let Some(parent) = p.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(&p, content).unwrap();
            // 按内容派生 mtime：同内容同 mtime，不同内容不同 mtime，
            // 保证快速模式（size+mtime）能正确区分内容差异
            let h = blake3::hash(content.as_bytes());
            let mut arr = [0u8; 8];
            arr.copy_from_slice(&h.as_bytes()[..8]);
            let secs = 1_700_000_000i64 + (u64::from_le_bytes(arr) % 100_000_000) as i64;
            filetime::set_file_mtime(&p, filetime::FileTime::from_unix_time(secs, 0)).unwrap();
        }
    }

    fn empty_filter() -> Filter {
        Filter::new(&[], &[]).unwrap()
    }

    fn statuses(r: &TriResult) -> std::collections::BTreeMap<&str, TriStatus> {
        r.entries
            .iter()
            .map(|e| (e.rel.as_str(), e.status))
            .collect()
    }

    #[test]
    fn all_three_identical() {
        let b = tempdir().unwrap();
        let l = tempdir().unwrap();
        let r = tempdir().unwrap();
        make_tree(b.path(), &[("a.txt", "x")]);
        make_tree(l.path(), &[("a.txt", "x")]);
        make_tree(r.path(), &[("a.txt", "x")]);
        let res = compare3_dirs(b.path(), l.path(), r.path(), &empty_filter(), false).unwrap();
        let m = statuses(&res);
        assert_eq!(m["a.txt"], TriStatus::Same);
        assert!(!res.stats.has_differences());
        assert_eq!(res.stats.same, 1);
    }

    #[test]
    fn left_modified_right_unchanged() {
        let b = tempdir().unwrap();
        let l = tempdir().unwrap();
        let r = tempdir().unwrap();
        make_tree(b.path(), &[("a.txt", "v1")]);
        make_tree(l.path(), &[("a.txt", "v2")]);
        make_tree(r.path(), &[("a.txt", "v1")]);
        let res = compare3_dirs(b.path(), l.path(), r.path(), &empty_filter(), false).unwrap();
        assert_eq!(statuses(&res)["a.txt"], TriStatus::LeftModified);
        assert_eq!(res.stats.left_modified, 1);
    }

    #[test]
    fn right_modified_left_unchanged() {
        let b = tempdir().unwrap();
        let l = tempdir().unwrap();
        let r = tempdir().unwrap();
        make_tree(b.path(), &[("a.txt", "v1")]);
        make_tree(l.path(), &[("a.txt", "v1")]);
        make_tree(r.path(), &[("a.txt", "v2")]);
        let res = compare3_dirs(b.path(), l.path(), r.path(), &empty_filter(), false).unwrap();
        assert_eq!(statuses(&res)["a.txt"], TriStatus::RightModified);
    }

    #[test]
    fn both_modified_same_content() {
        let b = tempdir().unwrap();
        let l = tempdir().unwrap();
        let r = tempdir().unwrap();
        make_tree(b.path(), &[("a.txt", "v1")]);
        make_tree(l.path(), &[("a.txt", "v2")]);
        make_tree(r.path(), &[("a.txt", "v2")]);
        let res = compare3_dirs(b.path(), l.path(), r.path(), &empty_filter(), false).unwrap();
        assert_eq!(statuses(&res)["a.txt"], TriStatus::BothModified);
    }

    #[test]
    fn conflict_all_differ() {
        let b = tempdir().unwrap();
        let l = tempdir().unwrap();
        let r = tempdir().unwrap();
        make_tree(b.path(), &[("a.txt", "v1")]);
        make_tree(l.path(), &[("a.txt", "v2")]);
        make_tree(r.path(), &[("a.txt", "v3")]);
        let res = compare3_dirs(b.path(), l.path(), r.path(), &empty_filter(), false).unwrap();
        assert_eq!(statuses(&res)["a.txt"], TriStatus::Conflict);
        assert_eq!(res.stats.conflict, 1);
    }

    #[test]
    fn left_deleted_right_unchanged() {
        let b = tempdir().unwrap();
        let l = tempdir().unwrap();
        let r = tempdir().unwrap();
        make_tree(b.path(), &[("a.txt", "v1")]);
        make_tree(r.path(), &[("a.txt", "v1")]);
        // left 无 a.txt
        let res = compare3_dirs(b.path(), l.path(), r.path(), &empty_filter(), false).unwrap();
        assert_eq!(statuses(&res)["a.txt"], TriStatus::LeftDeleted);
    }

    #[test]
    fn right_deleted_left_unchanged() {
        let b = tempdir().unwrap();
        let l = tempdir().unwrap();
        let r = tempdir().unwrap();
        make_tree(b.path(), &[("a.txt", "v1")]);
        make_tree(l.path(), &[("a.txt", "v1")]);
        let res = compare3_dirs(b.path(), l.path(), r.path(), &empty_filter(), false).unwrap();
        assert_eq!(statuses(&res)["a.txt"], TriStatus::RightDeleted);
    }

    #[test]
    fn base_only_and_additions() {
        let b = tempdir().unwrap();
        let l = tempdir().unwrap();
        let r = tempdir().unwrap();
        make_tree(b.path(), &[("gone.txt", "x")]);
        make_tree(l.path(), &[("new_l.txt", "a")]);
        make_tree(r.path(), &[("new_r.txt", "b")]);
        let res = compare3_dirs(b.path(), l.path(), r.path(), &empty_filter(), false).unwrap();
        let m = statuses(&res);
        assert_eq!(m["gone.txt"], TriStatus::BaseOnly);
        assert_eq!(m["new_l.txt"], TriStatus::LeftOnly);
        assert_eq!(m["new_r.txt"], TriStatus::RightOnly);
        assert_eq!(res.stats.base_only, 1);
        assert_eq!(res.stats.left_only, 1);
        assert_eq!(res.stats.right_only, 1);
    }

    #[test]
    fn exit_codes() {
        let b = tempdir().unwrap();
        let l = tempdir().unwrap();
        let r = tempdir().unwrap();
        make_tree(b.path(), &[("a.txt", "v1")]);
        make_tree(l.path(), &[("a.txt", "v1")]);
        make_tree(r.path(), &[("a.txt", "v1")]);
        let a = args(
            b.path().to_str().unwrap(),
            l.path().to_str().unwrap(),
            r.path().to_str().unwrap(),
        );
        assert_eq!(run(&a), 0);

        let l2 = tempdir().unwrap();
        make_tree(l2.path(), &[("a.txt", "v2")]);
        let a2 = args(
            b.path().to_str().unwrap(),
            l2.path().to_str().unwrap(),
            r.path().to_str().unwrap(),
        );
        assert_eq!(run(&a2), 1);
    }

    #[test]
    fn missing_dir_exit_two() {
        let b = tempdir().unwrap();
        let a = args(
            b.path().to_str().unwrap(),
            "/nonexistent/bcr-left",
            "/nonexistent/bcr-right",
        );
        assert_eq!(run(&a), 2);
    }
}
