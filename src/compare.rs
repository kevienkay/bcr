use crate::fsscan::{content_equal, scan, Filter};
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

/// 运行 compare 子命令，返回进程退出码（0=无差异，1=有差异，2=错误）
pub fn run(args: &CompareArgs) -> i32 {
    let left_dir = Path::new(&args.left);
    let right_dir = Path::new(&args.right);
    if !left_dir.is_dir() {
        eprintln!("bcr: 不是目录: {}", args.left);
        return 2;
    }
    if !right_dir.is_dir() {
        eprintln!("bcr: 不是目录: {}", args.right);
        return 2;
    }

    let filter = match Filter::new(&args.includes, &args.excludes) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("bcr: 过滤规则错误: {e}");
            return 2;
        }
    };

    let left = match scan(left_dir, &filter) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("bcr: 扫描 {} 失败: {e}", args.left);
            return 2;
        }
    };
    let right = match scan(right_dir, &filter) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("bcr: 扫描 {} 失败: {e}", args.right);
            return 2;
        }
    };

    let color = match args.color.as_str() {
        "always" => true,
        "never" => false,
        _ => io::stdout().is_terminal(),
    };

    // 合并 key 集合（已排序，保证输出顺序稳定）
    let mut keys: Vec<&String> = Vec::with_capacity(left.len() + right.len());
    for k in left.keys() {
        keys.push(k);
    }
    for k in right.keys() {
        if !left.contains_key(k) {
            keys.push(k);
        }
    }
    keys.sort();

    let mut n_same = 0usize;
    let mut n_left_only = 0usize;
    let mut n_right_only = 0usize;
    let mut n_differ = 0usize;

    for key in keys {
        match (left.get(key), right.get(key)) {
            (Some(l), Some(r)) => {
                let same = if l.size != r.size {
                    false
                } else if args.compare_content {
                    match content_equal(left_dir, right_dir, key) {
                        Ok(eq) => eq,
                        Err(e) => {
                            eprintln!("bcr: 读取 {} 失败: {e}", key);
                            continue;
                        }
                    }
                } else {
                    l.mtime == r.mtime
                };
                if same {
                    n_same += 1;
                    if args.show_same {
                        emit('S', key, color);
                    }
                } else {
                    n_differ += 1;
                    emit('C', key, color);
                }
            }
            (Some(_), None) => {
                n_left_only += 1;
                emit('L', key, color);
            }
            (None, Some(_)) => {
                n_right_only += 1;
                emit('R', key, color);
            }
            (None, None) => unreachable!(),
        }
    }

    if args.summary {
        println!(
            "统计: {} 相同, {} 仅左侧, {} 仅右侧, {} 内容不同",
            n_same, n_left_only, n_right_only, n_differ
        );
    }

    if n_left_only + n_right_only + n_differ > 0 {
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
}
