use crate::fsscan::{FileMeta, Filter};
use crate::i18n::{fmt, Key};
use crate::vfs::{LocalVfs, Vfs};
use clap::Args;
use std::io::{self, IsTerminal};
use std::path::Path;

const RED: &str = "\x1b[31m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const BLUE: &str = "\x1b[34m";
const RESET: &str = "\x1b[0m";

/// compare 子命令参数
#[derive(Args, Debug)]
pub struct CompareArgs {
    /// 左侧目录
    pub left: String,

    /// 右侧目录
    pub right: String,

    /// 深度比较：对大小相同的文件对做 blake3 哈希比对（默认仅比较大小+修改时间）
    #[arg(long)]
    pub compare_content: bool,

    /// 包含过滤（glob，可重复），仅比较匹配的文件
    #[arg(long = "include")]
    pub includes: Vec<String>,

    /// 排除过滤（glob，可重复），跳过匹配的文件/目录
    #[arg(long = "exclude")]
    pub excludes: Vec<String>,

    /// 同时显示相同的文件
    #[arg(long)]
    pub show_same: bool,

    /// 输出统计信息
    #[arg(long)]
    pub summary: bool,

    /// 颜色输出：auto | always | never
    #[arg(long, default_value = "auto", value_parser = ["auto", "always", "never"])]
    pub color: String,
}

/// 文件比较状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileStatus {
    Same,
    LeftOnly,
    RightOnly,
    Differ,
}

impl FileStatus {
    pub fn letter(self) -> char {
        match self {
            FileStatus::Same => 'S',
            FileStatus::LeftOnly => 'L',
            FileStatus::RightOnly => 'R',
            FileStatus::Differ => 'C',
        }
    }
}

/// 单个文件的比较条目
#[derive(Debug, Clone)]
pub struct FileEntry {
    pub rel: String,
    pub status: FileStatus,
    pub left: Option<FileMeta>,
    pub right: Option<FileMeta>,
}

/// 比较统计
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CompareStats {
    pub same: usize,
    pub left_only: usize,
    pub right_only: usize,
    pub differ: usize,
}

impl CompareStats {
    pub fn has_differences(self) -> bool {
        self.left_only + self.right_only + self.differ > 0
    }
}

/// 目录比较结果（CLI 与 GUI 共用）
#[derive(Debug, Default)]
pub struct CompareResult {
    /// 排序后的条目（BTreeMap 顺序）
    pub entries: Vec<FileEntry>,
    pub stats: CompareStats,
    /// 比较过程中的非致命警告（如读取失败被跳过）
    pub warnings: Vec<String>,
}

/// 对比两个目录树，返回结构化结果。
///
/// 行为与 CLI 一致：快速模式比较大小+mtime；compare_content 时对大小相同的
/// 文件对做哈希比对。读取失败的文件记入 warnings 并跳过。
pub fn compare_dirs(
    left_dir: &Path,
    right_dir: &Path,
    filter: &Filter,
    compare_content: bool,
) -> io::Result<CompareResult> {
    let left = LocalVfs::new(left_dir)?;
    let right = LocalVfs::new(right_dir)?;
    compare_vfs(&left, &right, filter, compare_content)
}

/// 对比两个虚拟文件系统后端，返回结构化结果（CLI/GUI/远程共用）。
pub fn compare_vfs(
    left: &dyn Vfs,
    right: &dyn Vfs,
    filter: &Filter,
    compare_content: bool,
) -> io::Result<CompareResult> {
    let left_map = left.scan(filter)?;
    let right_map = right.scan(filter)?;

    // 合并 key 集合（已排序，保证输出顺序稳定）
    let mut keys: Vec<&String> = Vec::with_capacity(left_map.len() + right_map.len());
    for k in left_map.keys() {
        keys.push(k);
    }
    for k in right_map.keys() {
        if !left_map.contains_key(k) {
            keys.push(k);
        }
    }
    keys.sort();

    let mut result = CompareResult::default();
    for key in keys {
        match (left_map.get(key), right_map.get(key)) {
            (Some(l), Some(r)) => {
                let same = if l.size != r.size {
                    false
                } else if compare_content {
                    match crate::vfs::content_equal_vfs(left, right, key) {
                        Ok(eq) => eq,
                        Err(e) => {
                            result.warnings.push(format!("读取 {key} 失败: {e}"));
                            continue;
                        }
                    }
                } else {
                    l.mtime == r.mtime
                };
                if same {
                    result.stats.same += 1;
                    result.entries.push(FileEntry {
                        rel: key.clone(),
                        status: FileStatus::Same,
                        left: Some(l.clone()),
                        right: Some(r.clone()),
                    });
                } else {
                    result.stats.differ += 1;
                    result.entries.push(FileEntry {
                        rel: key.clone(),
                        status: FileStatus::Differ,
                        left: Some(l.clone()),
                        right: Some(r.clone()),
                    });
                }
            }
            (Some(l), None) => {
                result.stats.left_only += 1;
                result.entries.push(FileEntry {
                    rel: key.clone(),
                    status: FileStatus::LeftOnly,
                    left: Some(l.clone()),
                    right: None,
                });
            }
            (None, Some(r)) => {
                result.stats.right_only += 1;
                result.entries.push(FileEntry {
                    rel: key.clone(),
                    status: FileStatus::RightOnly,
                    left: None,
                    right: Some(r.clone()),
                });
            }
            (None, None) => unreachable!(),
        }
    }
    Ok(result)
}

/// 运行 compare 子命令，返回进程退出码（0=无差异，1=有差异，2=错误）
/// 运行 compare 子命令，返回进程退出码（0=无差异，1=有差异，2=错误）
pub fn run(args: &CompareArgs) -> i32 {
    // 本地路径需要是目录；zip:// 与 sftp:// 交给 vfs::open 处理
    if !crate::vfs::is_remote(&args.left) && !Path::new(&args.left).is_dir() {
        eprintln!("bcr: {}", fmt(Key::NotDir, &[&args.left]));
        return 2;
    }
    if !crate::vfs::is_remote(&args.right) && !Path::new(&args.right).is_dir() {
        eprintln!("bcr: {}", fmt(Key::NotDir, &[&args.right]));
        return 2;
    }

    let filter = match Filter::new(&args.includes, &args.excludes) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("bcr: {}", fmt(Key::FilterError, &[&e.to_string()]));
            return 2;
        }
    };

    let left = match crate::vfs::open(&args.left) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("bcr: {}", fmt(Key::OpenFailed, &[&args.left, &e.to_string()]));
            return 2;
        }
    };
    let right = match crate::vfs::open(&args.right) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("bcr: {}", fmt(Key::OpenFailed, &[&args.right, &e.to_string()]));
            return 2;
        }
    };

    let result = match compare_vfs(left.as_ref(), right.as_ref(), &filter, args.compare_content) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("bcr: {}", fmt(Key::ScanFailed, &[&e.to_string()]));
            return 2;
        }
    };

    for w in &result.warnings {
        eprintln!("bcr: {w}");
    }

    let color = match args.color.as_str() {
        "always" => true,
        "never" => false,
        _ => io::stdout().is_terminal(),
    };

    for entry in &result.entries {
        if entry.status == FileStatus::Same && !args.show_same {
            continue;
        }
        emit(entry.status.letter(), &entry.rel, color);
    }

    if args.summary {
        let s = result.stats;
        println!(
            "{}",
            fmt(
                Key::SummaryCompare,
                &[
                    &s.same.to_string(),
                    &s.left_only.to_string(),
                    &s.right_only.to_string(),
                    &s.differ.to_string(),
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

fn emit(status: char, rel: &str, color: bool) {
    if color {
        let c = match status {
            'L' => RED,
            'R' => BLUE,
            'C' => YELLOW,
            _ => GREEN,
        };
        println!("{c}[{status}]{RESET} {rel}");
    } else {
        println!("[{status}] {rel}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn args(left: &str, right: &str) -> CompareArgs {
        CompareArgs {
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

    /// 构建目录树：entries 为 (相对路径, 内容)，统一写入固定 mtime 保证快速模式可比
    fn make_tree(dir: &std::path::Path, entries: &[(&str, &str)]) {
        let fixed = filetime::FileTime::from_unix_time(1_700_000_000, 0);
        for (rel, content) in entries {
            let p = dir.join(rel);
            if let Some(parent) = p.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(p, content).unwrap();
            filetime::set_file_mtime(&dir.join(rel), fixed).unwrap();
        }
    }

    fn empty_filter() -> Filter {
        Filter::new(&[], &[]).unwrap()
    }

    #[test]
    fn run_identical_dirs_exit_zero() {
        let d1 = tempdir().unwrap();
        let d2 = tempdir().unwrap();
        make_tree(d1.path(), &[("a.txt", "x"), ("sub/b.txt", "y")]);
        make_tree(d2.path(), &[("a.txt", "x"), ("sub/b.txt", "y")]);
        let a = args(d1.path().to_str().unwrap(), d2.path().to_str().unwrap());
        assert_eq!(run(&a), 0);
    }

    #[test]
    fn run_different_dirs_exit_one() {
        let d1 = tempdir().unwrap();
        let d2 = tempdir().unwrap();
        make_tree(d1.path(), &[("only_l.txt", "x")]);
        make_tree(d2.path(), &[("only_r.txt", "x")]);
        let a = args(d1.path().to_str().unwrap(), d2.path().to_str().unwrap());
        assert_eq!(run(&a), 1);
    }

    #[test]
    fn run_missing_dir_exit_two() {
        let d = tempdir().unwrap();
        let a = args(d.path().to_str().unwrap(), "/nonexistent/bcr-dir");
        assert_eq!(run(&a), 2);
    }

    #[test]
    fn run_quick_mode_mtime_diff_counts_as_differ() {
        let d1 = tempdir().unwrap();
        let d2 = tempdir().unwrap();
        make_tree(d1.path(), &[("f.txt", "same")]);
        make_tree(d2.path(), &[("f.txt", "same")]);
        // 内容相同但 mtime 不同 → 快速模式判为不同
        let old = filetime::FileTime::from_unix_time(1_600_000_000, 0);
        filetime::set_file_mtime(d1.path().join("f.txt"), old).unwrap();
        let new = filetime::FileTime::from_unix_time(1_700_000_000, 0);
        filetime::set_file_mtime(d2.path().join("f.txt"), new).unwrap();
        let a = args(d1.path().to_str().unwrap(), d2.path().to_str().unwrap());
        assert_eq!(run(&a), 1);
    }

    #[test]
    fn run_content_mode_ignores_mtime() {
        let d1 = tempdir().unwrap();
        let d2 = tempdir().unwrap();
        make_tree(d1.path(), &[("f.txt", "same")]);
        make_tree(d2.path(), &[("f.txt", "same")]);
        let old = filetime::FileTime::from_unix_time(1_600_000_000, 0);
        filetime::set_file_mtime(d1.path().join("f.txt"), old).unwrap();
        let new = filetime::FileTime::from_unix_time(1_700_000_000, 0);
        filetime::set_file_mtime(d2.path().join("f.txt"), new).unwrap();
        let mut a = args(d1.path().to_str().unwrap(), d2.path().to_str().unwrap());
        a.compare_content = true;
        assert_eq!(run(&a), 0);
    }

    #[test]
    fn run_content_mode_detects_different_content() {
        let d1 = tempdir().unwrap();
        let d2 = tempdir().unwrap();
        make_tree(d1.path(), &[("f.txt", "aaa")]);
        make_tree(d2.path(), &[("f.txt", "bbb")]);
        let t = filetime::FileTime::from_unix_time(1_700_000_000, 0);
        filetime::set_file_mtime(d1.path().join("f.txt"), t).unwrap();
        filetime::set_file_mtime(d2.path().join("f.txt"), t).unwrap();
        let mut a = args(d1.path().to_str().unwrap(), d2.path().to_str().unwrap());
        a.compare_content = true;
        assert_eq!(run(&a), 1);
    }

    #[test]
    fn run_include_filter_limits_comparison() {
        let d1 = tempdir().unwrap();
        let d2 = tempdir().unwrap();
        make_tree(d1.path(), &[("keep.txt", "x"), ("drop.txt", "y")]);
        make_tree(d2.path(), &[("keep.txt", "x")]);
        // 不带 include：drop.txt 仅左侧 → 有差异
        let a = args(d1.path().to_str().unwrap(), d2.path().to_str().unwrap());
        assert_eq!(run(&a), 1);
        // 带 include：drop.txt 被过滤 → 无差异
        let mut b = args(d1.path().to_str().unwrap(), d2.path().to_str().unwrap());
        b.includes = vec!["keep.txt".into()];
        assert_eq!(run(&b), 0);
    }

    #[test]
    fn run_exclude_filter_ignores_files() {
        let d1 = tempdir().unwrap();
        let d2 = tempdir().unwrap();
        make_tree(d1.path(), &[("keep.txt", "x"), ("skip.txt", "y")]);
        make_tree(d2.path(), &[("keep.txt", "x")]);
        let mut a = args(d1.path().to_str().unwrap(), d2.path().to_str().unwrap());
        a.excludes = vec!["skip.txt".into()];
        assert_eq!(run(&a), 0);
    }

    #[test]
    fn emit_formats_plain_status() {
        // 不校验 stdout，只保证不 panic，且颜色路径逻辑可用
        emit('L', "a.txt", false);
        emit('R', "a.txt", false);
        emit('C', "a.txt", false);
        emit('S', "a.txt", false);
    }

    // ---- compare_dirs 结构化 API ----

    #[test]
    fn compare_dirs_reports_statuses_and_stats() {
        let d1 = tempdir().unwrap();
        let d2 = tempdir().unwrap();
        make_tree(d1.path(), &[("same.txt", "x"), ("diff.txt", "v1"), ("only_l.txt", "a")]);
        make_tree(d2.path(), &[("same.txt", "x"), ("diff.txt", "v22"), ("only_r.txt", "b")]);
        let r = compare_dirs(d1.path(), d2.path(), &empty_filter(), false).unwrap();
        let by_rel: std::collections::BTreeMap<&str, FileStatus> = r
            .entries
            .iter()
            .map(|e| (e.rel.as_str(), e.status))
            .collect();
        assert_eq!(by_rel["same.txt"], FileStatus::Same);
        assert_eq!(by_rel["diff.txt"], FileStatus::Differ);
        assert_eq!(by_rel["only_l.txt"], FileStatus::LeftOnly);
        assert_eq!(by_rel["only_r.txt"], FileStatus::RightOnly);
        assert_eq!(r.stats.same, 1);
        assert_eq!(r.stats.differ, 1);
        assert_eq!(r.stats.left_only, 1);
        assert_eq!(r.stats.right_only, 1);
        assert!(r.stats.has_differences());
        // 顺序稳定（BTreeMap 排序）
        let rels: Vec<&str> = r.entries.iter().map(|e| e.rel.as_str()).collect();
        assert_eq!(rels, vec!["diff.txt", "only_l.txt", "only_r.txt", "same.txt"]);
    }

    #[test]
    fn compare_dirs_identical_dirs_no_differences() {
        let d = tempdir().unwrap();
        make_tree(d.path(), &[("a.txt", "x"), ("sub/b.txt", "y")]);
        let r = compare_dirs(d.path(), d.path(), &empty_filter(), false).unwrap();
        assert!(!r.stats.has_differences());
        assert_eq!(r.stats.same, 2);
        assert!(r.warnings.is_empty());
    }

    #[test]
    fn compare_dirs_entries_carry_metadata() {
        let d1 = tempdir().unwrap();
        let d2 = tempdir().unwrap();
        make_tree(d1.path(), &[("f.txt", "12345")]);
        make_tree(d2.path(), &[("f.txt", "12345")]);
        let r = compare_dirs(d1.path(), d2.path(), &empty_filter(), false).unwrap();
        let e = &r.entries[0];
        assert_eq!(e.left.as_ref().unwrap().size, 5);
        assert_eq!(e.right.as_ref().unwrap().size, 5);
    }
}
