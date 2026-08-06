use crate::render;
use clap::Args;
use similar::{capture_diff_slices, Algorithm};
use std::fs;
use std::io::{self, IsTerminal, Read};
use std::path::Path;

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
        Err(e) => {
            eprintln!("bcr: 无法读取 {}: {e}", args.left);
            return 2;
        }
    };
    let right = match read_input(&args.right) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("bcr: 无法读取 {}: {e}", args.right);
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
    let s = if args.ignore_whitespace {
        line.chars().filter(|c| !c.is_whitespace()).collect()
    } else if args.ignore_trailing {
        line.trim_end().to_string()
    } else {
        line.to_string()
    };
    if args.ignore_case {
        s.to_lowercase()
    } else {
        s
    }
}

fn read_input(path: &str) -> io::Result<String> {
    if path == "-" {
        let mut buf = String::new();
        io::stdin().read_to_string(&mut buf)?;
        Ok(buf)
    } else {
        fs::read_to_string(Path::new(path))
    }
}
