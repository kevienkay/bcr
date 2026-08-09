use crate::i18n::{fmt, Key};
use crate::render;
use clap::Args;
use similar::{capture_diff_slices, Algorithm};
use std::io::{self, IsTerminal};

/// diff 子命令参数
#[derive(Args, Debug)]
pub struct DiffArgs {
    /// 左侧文件（- 表示从 stdin 读取）
    pub left: String,

    /// 右侧文件（- 表示从 stdin 读取）
    pub right: String,

    /// diff 算法：myers | patience
    #[arg(long, default_value = "patience", value_parser = ["myers", "patience"])]
    pub algo: String,

    /// 忽略所有空白差异
    #[arg(long)]
    pub ignore_whitespace: bool,

    /// 忽略行尾空白差异
    #[arg(long)]
    pub ignore_trailing: bool,

    /// 忽略大小写差异
    #[arg(long)]
    pub ignore_case: bool,

    /// 颜色输出：auto | always | never
    #[arg(long, default_value = "auto", value_parser = ["auto", "always", "never"])]
    pub color: String,

    /// 输出标签，最多两个（对应左右两侧），默认使用文件名
    #[arg(short = 'L', num_args = 1..=2)]
    pub labels: Vec<String>,
}

/// 运行 diff 子命令，返回进程退出码（0=无差异，1=有差异，2=错误）
pub fn run(args: &DiffArgs) -> i32 {
    let left = match read_input(&args.left) {
        Ok(s) => s,
        Err(ReadErr::Binary) => {
            eprintln!("bcr: {}", fmt(Key::BinaryFile, &[&args.left]));
            return 2;
        }
        Err(ReadErr::Io(e)) => {
            eprintln!(
                "bcr: {}",
                fmt(Key::CannotRead, &[&args.left, &e.to_string()])
            );
            return 2;
        }
    };
    let right = match read_input(&args.right) {
        Ok(s) => s,
        Err(ReadErr::Binary) => {
            eprintln!("bcr: {}", fmt(Key::BinaryFile, &[&args.right]));
            return 2;
        }
        Err(ReadErr::Io(e)) => {
            eprintln!(
                "bcr: {}",
                fmt(Key::CannotRead, &[&args.right, &e.to_string()])
            );
            return 2;
        }
    };

    let algo = match args.algo.as_str() {
        "myers" => Algorithm::Myers,
        _ => Algorithm::Patience,
    };

    let color = match args.color.as_str() {
        "always" => true,
        "never" => false,
        _ => io::stdout().is_terminal(),
    };

    let label_l = args
        .labels
        .first()
        .cloned()
        .unwrap_or_else(|| args.left.clone());
    let label_r = args
        .labels
        .get(1)
        .cloned()
        .unwrap_or_else(|| args.right.clone());

    // 原始行（用于输出）与比较键（用于 diff，按选项归一化）
    let lines_l: Vec<&str> = left.lines().collect();
    let lines_r: Vec<&str> = right.lines().collect();
    let keys_l: Vec<String> = lines_l.iter().map(|l| normalize(l, args)).collect();
    let keys_r: Vec<String> = lines_r.iter().map(|l| normalize(l, args)).collect();

    let ops = capture_diff_slices(algo, &keys_l, &keys_r);
    // capture_diff_slices 对完全相同的输入返回全 Equal op，而非空 vec，需显式判断
    if ops.iter().all(|op| op.tag() == similar::DiffTag::Equal) {
        return 0; // 无差异
    }

    render::render_unified(&ops, &lines_l, &lines_r, &label_l, &label_r, color);
    1 // 有差异
}

/// 按忽略选项归一化一行，仅用于匹配，不改变输出内容
fn normalize(line: &str, args: &DiffArgs) -> String {
    normalize_line(
        line,
        args.ignore_whitespace,
        args.ignore_trailing,
        args.ignore_case,
    )
}

/// 归一化原语（GUI 并排视图与 CLI 共用）
pub(crate) fn normalize_line(
    line: &str,
    ignore_whitespace: bool,
    ignore_trailing: bool,
    ignore_case: bool,
) -> String {
    let s = if ignore_whitespace {
        line.chars().filter(|c| !c.is_whitespace()).collect()
    } else if ignore_trailing {
        line.trim_end().to_string()
    } else {
        line.to_string()
    };
    if ignore_case {
        s.to_lowercase()
    } else {
        s
    }
}

/// 读取错误：二进制文件 / IO 错误
#[derive(Debug)]
enum ReadErr {
    Binary,
    Io(io::Error),
}

fn read_input(path: &str) -> Result<String, ReadErr> {
    let tf = crate::encoding::read_input(path).map_err(ReadErr::Io)?;
    if tf.is_binary {
        return Err(ReadErr::Binary);
    }
    Ok(tf.text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn args() -> DiffArgs {
        DiffArgs {
            left: String::new(),
            right: String::new(),
            algo: "patience".into(),
            ignore_whitespace: false,
            ignore_trailing: false,
            ignore_case: false,
            color: "never".into(),
            labels: vec![],
        }
    }

    #[test]
    fn normalize_default_keeps_line() {
        let a = args();
        assert_eq!(normalize("  Hello World  ", &a), "  Hello World  ");
    }

    #[test]
    fn normalize_ignore_whitespace_strips_all() {
        let mut a = args();
        a.ignore_whitespace = true;
        assert_eq!(normalize("  a \t b \n c ", &a), "abc");
    }

    #[test]
    fn normalize_ignore_trailing_trims_end() {
        let mut a = args();
        a.ignore_trailing = true;
        assert_eq!(normalize("hello   ", &a), "hello");
        // 行首空白保留
        assert_eq!(normalize("  hello  ", &a), "  hello");
    }

    #[test]
    fn normalize_ignore_case_lowercases() {
        let mut a = args();
        a.ignore_case = true;
        assert_eq!(normalize("Hello World", &a), "hello world");
    }

    #[test]
    fn normalize_combined_options() {
        let mut a = args();
        a.ignore_whitespace = true;
        a.ignore_case = true;
        assert_eq!(normalize("  Foo Bar ", &a), "foobar");
    }

    #[test]
    fn run_identical_files_exit_zero() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("a.txt");
        fs::write(&p, "x\ny\n").unwrap();
        let mut a = args();
        a.left = p.to_str().unwrap().into();
        a.right = p.to_str().unwrap().into();
        assert_eq!(run(&a), 0);
    }

    #[test]
    fn run_different_files_exit_one() {
        let dir = tempdir().unwrap();
        let l = dir.path().join("l.txt");
        let r = dir.path().join("r.txt");
        fs::write(&l, "a\nb\n").unwrap();
        fs::write(&r, "a\nc\n").unwrap();
        let mut a = args();
        a.left = l.to_str().unwrap().into();
        a.right = r.to_str().unwrap().into();
        assert_eq!(run(&a), 1);
    }

    #[test]
    fn run_missing_file_exit_two() {
        let mut a = args();
        a.left = "/nonexistent/bcr-test-l".into();
        a.right = "/nonexistent/bcr-test-r".into();
        assert_eq!(run(&a), 2);
    }

    #[test]
    fn run_ignore_whitespace_affects_exit_code() {
        let dir = tempdir().unwrap();
        let l = dir.path().join("l.txt");
        let r = dir.path().join("r.txt");
        fs::write(&l, "a b\n").unwrap();
        fs::write(&r, "ab\n").unwrap();
        let mut a = args();
        a.left = l.to_str().unwrap().into();
        a.right = r.to_str().unwrap().into();
        assert_eq!(run(&a), 1);
        a.ignore_whitespace = true;
        assert_eq!(run(&a), 0);
    }

    #[test]
    fn run_ignore_case_affects_exit_code() {
        let dir = tempdir().unwrap();
        let l = dir.path().join("l.txt");
        let r = dir.path().join("r.txt");
        fs::write(&l, "Hello\n").unwrap();
        fs::write(&r, "hello\n").unwrap();
        let mut a = args();
        a.left = l.to_str().unwrap().into();
        a.right = r.to_str().unwrap().into();
        assert_eq!(run(&a), 1);
        a.ignore_case = true;
        assert_eq!(run(&a), 0);
    }

    #[test]
    fn run_algo_myers_and_patience_both_work() {
        let dir = tempdir().unwrap();
        let l = dir.path().join("l.txt");
        let r = dir.path().join("r.txt");
        fs::write(&l, "one\ntwo\nthree\n").unwrap();
        fs::write(&r, "one\nTWO\nthree\n").unwrap();
        for algo in ["myers", "patience"] {
            let mut a = args();
            a.algo = algo.into();
            a.left = l.to_str().unwrap().into();
            a.right = r.to_str().unwrap().into();
            assert_eq!(run(&a), 1, "algo={algo}");
        }
    }

    #[test]
    fn read_input_reads_file() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("in.txt");
        fs::write(&p, "line1\nline2\n").unwrap();
        assert_eq!(read_input(p.to_str().unwrap()).unwrap(), "line1\nline2\n");
    }
}
