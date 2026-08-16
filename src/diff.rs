use crate::i18n::{fmt, Key};
use crate::render;
use clap::Args;
use similar::{capture_diff_slices, Algorithm};
use std::io::{self, IsTerminal};

/// diff 子命令参数
#[derive(Args, Debug, Clone)]
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

    /// 忽略行尾 CR/LF 差异（CRLF vs LF）
    #[arg(long)]
    pub ignore_crlf: bool,

    /// 转换后比较（A9）：比较前统一换行符（CRLF/CR → LF）与编码（→ UTF-8）
    #[arg(long)]
    pub convert: bool,

    /// 外部工具对比（A14）：不按文本 diff，改用 ~/.bcr-external.toml 中配置的
    /// 命令对比（按扩展名匹配，如 docx/pdf/xlsx），适合未支持的二进制格式
    #[arg(long)]
    pub external: bool,

    /// 忽略匹配正则的行（内容过滤：如版本号/时间戳行，可重复）
    #[arg(long = "ignore-lines")]
    pub ignore_lines: Vec<String>,

    /// 颜色输出：auto | always | never
    #[arg(long, default_value = "auto", value_parser = ["auto", "always", "never"])]
    pub color: String,

    /// 上下文行语法着色（需要 color，按左侧文件扩展名识别语言）
    #[arg(long)]
    pub highlight: bool,

    /// 输出标签，最多两个（对应左右两侧），默认使用文件名
    #[arg(short = 'L', num_args = 1..=2)]
    pub labels: Vec<String>,

    /// 复用已保存的规则 Profile（忽略选项等）
    #[arg(long)]
    pub profile: Option<String>,

    /// 输出 JSON 契约（diff.v1，P27 自动化格式）
    #[arg(long)]
    pub json: bool,
}

/// 运行 diff 子命令，返回进程退出码（0=无差异，1=有差异，2=错误）
pub fn run(args: &DiffArgs) -> i32 {
    // A14 外部工具对比：跳过内置文本 diff，直接调用配置的外部命令
    if args.external {
        let tools = crate::external::ExternalTools::load();
        let Some(tpl) = tools.command_for(&args.left) else {
            eprintln!(
                "bcr: 未找到 {} 的外部对比工具配置（~/.bcr-external.toml [tools] 表，如 \"docx\" = \"soffice --diff {{left}} {{right}}\"）",
                std::path::Path::new(&args.left)
                    .extension()
                    .map(|e| e.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "未知扩展名".to_string())
            );
            return 2;
        };
        return match crate::external::ExternalTools::run(tpl, &args.left, &args.right) {
            Some(code) => code,
            None => {
                eprintln!("bcr: 外部命令启动失败：{}", tpl);
                2
            }
        };
    }

    // Profile 合并：忽略选项默认值来自 Profile，命令显式参数优先
    let merged = merge_profile(args);
    let args = &merged;
    // 原始文本（--convert 之前）用于计算 No newline 标记：
    // --convert 会把末尾 CR 转成 LF，若按转换后文本判定会凭空出现"行尾换行差异"，
    // 与 --convert 归一化行尾的语义冲突（两侧原始行尾风格不同 ≠ 内容不同）。
    let left_raw = match read_input(&args.left) {
        Ok(s) => s,
        Err(ReadErr::Binary) => {
            eprintln!("bcr: {}", fmt(Key::BinaryFile, &[&args.left]));
            return 2;
        }
        Err(ReadErr::TooLarge) => {
            eprintln!(
                "bcr: {}",
                fmt(Key::FileTooLarge, &[&args.left, &max_size_mb()])
            );
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
    let right_raw = match read_input(&args.right) {
        Ok(s) => s,
        Err(ReadErr::Binary) => {
            eprintln!("bcr: {}", fmt(Key::BinaryFile, &[&args.right]));
            return 2;
        }
        Err(ReadErr::TooLarge) => {
            eprintln!(
                "bcr: {}",
                fmt(Key::FileTooLarge, &[&args.right, &max_size_mb()])
            );
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
    // 文件是否不以换行结尾（GNU diff 的 No newline 标记；按原文判定）
    let no_newline_l = !left_raw.ends_with('\n') && !left_raw.is_empty();
    let no_newline_r = !right_raw.ends_with('\n') && !right_raw.is_empty();
    let left = if args.convert {
        normalize_eol(&left_raw)
    } else {
        left_raw
    };
    let right = if args.convert {
        normalize_eol(&right_raw)
    } else {
        right_raw
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
    // 语法高亮：--highlight 且彩色输出时启用，按左侧文件扩展名识别语言
    let syntax = if args.highlight && color {
        crate::highlight::syntax_for(&args.left)
    } else {
        None
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
    // 编译内容过滤正则（--ignore-lines）
    let ignore_lines: Vec<regex::Regex> = args
        .ignore_lines
        .iter()
        .filter_map(|p| regex::Regex::new(p).ok())
        .collect();
    let lines_l: Vec<&str> = left.lines().collect();
    let lines_r: Vec<&str> = right.lines().collect();
    let keys_l: Vec<String> = lines_l
        .iter()
        .map(|l| normalize(l, args, &ignore_lines))
        .collect();
    let keys_r: Vec<String> = lines_r
        .iter()
        .map(|l| normalize(l, args, &ignore_lines))
        .collect();

    let ops = capture_diff_slices(algo, &keys_l, &keys_r);

    // P49-2：--json 输出 diff.v1 契约（行号区间，自动化友好），退出码仍按差异语义
    if args.json {
        let json_ops: Vec<(String, usize, usize, usize, usize)> = ops
            .iter()
            .map(|op| {
                let (tag, r0, r1) = match op.tag() {
                    similar::DiffTag::Equal => ("equal", op.old_range(), op.new_range()),
                    similar::DiffTag::Delete => ("delete", op.old_range(), op.new_range()),
                    similar::DiffTag::Insert => ("insert", op.old_range(), op.new_range()),
                    similar::DiffTag::Replace => ("replace", op.old_range(), op.new_range()),
                };
                (tag.to_string(), r0.start, r0.end, r1.start, r1.end)
            })
            .collect();
        let v = crate::jsonout::envelope_diff(
            &args.left,
            &args.right,
            &json_ops,
            no_newline_l,
            no_newline_r,
        );
        println!("{}", serde_json::to_string(&v).unwrap_or_default());
        let has_diff = json_ops.iter().any(|(t, ..)| t != "equal") || no_newline_l != no_newline_r;
        return if has_diff { 1 } else { 0 };
    }

    // capture_diff_slices 对完全相同的输入返回全 Equal op，而非空 vec，需显式判断。
    // 注意：仅行尾换行不同（一侧缺结尾换行）也是差异（GNU diff 兼容），
    // 此时把最后一行合成为 Replace op 渲染并输出 \ No newline 标记，退出码 1。
    if ops.iter().all(|op| op.tag() == similar::DiffTag::Equal) {
        let newline_only_diff = no_newline_l != no_newline_r;
        if !newline_only_diff {
            return 0; // 无差异
        }
        if !lines_l.is_empty() && !lines_r.is_empty() {
            let last = lines_l.len() - 1;
            let fake = vec![
                similar::DiffOp::Equal {
                    old_index: 0,
                    new_index: 0,
                    len: last,
                },
                similar::DiffOp::Replace {
                    old_index: last,
                    old_len: 1,
                    new_index: last,
                    new_len: 1,
                },
            ];
            render::render_unified(
                &fake,
                &lines_l,
                &lines_r,
                &label_l,
                &label_r,
                color,
                syntax,
                no_newline_l,
                no_newline_r,
            );
        }
        return 1;
    }

    render::render_unified(
        &ops,
        &lines_l,
        &lines_r,
        &label_l,
        &label_r,
        color,
        syntax,
        no_newline_l,
        no_newline_r,
    );
    1 // 有差异
}

/// 合并 Profile 到 diff 参数（仅合并忽略选项）
fn merge_profile(args: &DiffArgs) -> DiffArgs {
    let Some(name) = &args.profile else {
        return args.clone();
    };
    let Ok(p) = crate::profile::get(name) else {
        eprintln!(
            "bcr: {}",
            crate::i18n::fmt(crate::i18n::Key::ProfileNotFound, &[name])
        );
        std::process::exit(2);
    };
    let mut out = args.clone();
    if !out.ignore_whitespace && p.ignore_whitespace {
        out.ignore_whitespace = true;
    }
    if !out.ignore_trailing && p.ignore_trailing {
        out.ignore_trailing = true;
    }
    if !out.ignore_case && p.ignore_case {
        out.ignore_case = true;
    }
    if let Some(enc) = p.encoding {
        if std::env::var("BCR_ENCODING")
            .map(|v| v.is_empty())
            .unwrap_or(true)
        {
            unsafe { std::env::set_var("BCR_ENCODING", enc) };
        }
    }
    out
}

/// 按忽略选项归一化一行，仅用于匹配，不改变输出内容
fn normalize(line: &str, args: &DiffArgs, ignore_lines: &[regex::Regex]) -> String {
    normalize_line(
        line,
        args.ignore_whitespace,
        args.ignore_trailing,
        args.ignore_case,
        args.ignore_crlf,
        ignore_lines,
    )
}

/// A9 转换后比较：统一换行符（CRLF/CR → LF），比较前对全文做转换
fn normalize_eol(s: &str) -> String {
    s.replace("\r\n", "\n").replace('\r', "\n")
}

/// 归一化原语（GUI 并排视图与 CLI 共用）
pub(crate) fn normalize_line(
    line: &str,
    ignore_whitespace: bool,
    ignore_trailing: bool,
    ignore_case: bool,
    ignore_crlf: bool,
    ignore_lines: &[regex::Regex],
) -> String {
    // 内容过滤：匹配忽略正则的行 → 空比较键（两侧都空则视为相同）
    for re in ignore_lines {
        if re.is_match(line) {
            return String::new();
        }
    }
    // 先剥离行尾 CR（CRLF vs LF 归一），再走其他忽略选项
    let line = if ignore_crlf {
        line.strip_suffix('\r').unwrap_or(line)
    } else {
        line
    };
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

/// 读取错误：二进制文件 / 文件过大 / IO 错误
#[derive(Debug)]
enum ReadErr {
    Binary,
    TooLarge,
    Io(io::Error),
}

/// 当前大小上限（MB，用于错误提示）
fn max_size_mb() -> String {
    std::env::var("BCR_MAX_SIZE")
        .ok()
        .unwrap_or_else(|| (crate::encoding::DEFAULT_MAX_TEXT_BYTES / 1024 / 1024).to_string())
}

fn read_input(path: &str) -> Result<String, ReadErr> {
    let tf = crate::encoding::read_input(path).map_err(|e| {
        if e.kind() == io::ErrorKind::FileTooLarge {
            ReadErr::TooLarge
        } else {
            ReadErr::Io(e)
        }
    })?;
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
            ignore_crlf: false,
            convert: false,
            external: false,
            ignore_lines: vec![],
            color: "never".into(),
            highlight: false,
            labels: vec![],
            profile: None,
            json: false,
        }
    }

    #[test]
    fn normalize_default_keeps_line() {
        let a = args();
        assert_eq!(normalize("  Hello World  ", &a, &[]), "  Hello World  ");
    }

    #[test]
    fn normalize_ignore_whitespace_strips_all() {
        let mut a = args();
        a.ignore_whitespace = true;
        assert_eq!(normalize("  a \t b \n c ", &a, &[]), "abc");
    }

    #[test]
    fn normalize_ignore_trailing_trims_end() {
        let mut a = args();
        a.ignore_trailing = true;
        assert_eq!(normalize("hello   ", &a, &[]), "hello");
        // 行首空白保留
        assert_eq!(normalize("  hello  ", &a, &[]), "  hello");
    }

    #[test]
    fn normalize_ignore_case_lowercases() {
        let mut a = args();
        a.ignore_case = true;
        assert_eq!(normalize("Hello World", &a, &[]), "hello world");
    }

    #[test]
    fn normalize_combined_options() {
        let mut a = args();
        a.ignore_whitespace = true;
        a.ignore_case = true;
        assert_eq!(normalize("  Foo Bar ", &a, &[]), "foobar");
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
    fn run_newline_only_diff_exit_one() {
        // 仅行尾换行不同（左侧无结尾换行、右侧有）：GNU diff 兼容，退出码应为 1
        let dir = tempdir().unwrap();
        let l = dir.path().join("l.txt");
        let r = dir.path().join("r.txt");
        fs::write(&l, "a\nb").unwrap(); // 无结尾换行
        fs::write(&r, "a\nb\n").unwrap();
        let mut a = args();
        a.left = l.to_str().unwrap().into();
        a.right = r.to_str().unwrap().into();
        assert_eq!(run(&a), 1, "行尾换行差异应退出码 1");
    }

    #[test]
    fn run_newline_only_diff_json_has_differences() {
        // --json：仅行尾换行差异 → has_differences=true 且退出码 1（契约含 no_newline 字段）
        let dir = tempdir().unwrap();
        let l = dir.path().join("l.txt");
        let r = dir.path().join("r.txt");
        fs::write(&l, "a\nb").unwrap();
        fs::write(&r, "a\nb\n").unwrap();
        let mut a = args();
        a.left = l.to_str().unwrap().into();
        a.right = r.to_str().unwrap().into();
        a.json = true;
        // run 的 json 分支 println 输出到 stdout（测试日志可见），退出码可断言
        let code = run(&a);
        assert_eq!(code, 1);
    }

    #[test]
    fn run_same_newline_no_diff() {
        // 两侧都有（或都无）结尾换行且行一致 → 无差异
        let dir = tempdir().unwrap();
        let l = dir.path().join("l.txt");
        let r = dir.path().join("r.txt");
        fs::write(&l, "a\nb\n").unwrap();
        fs::write(&r, "a\nb\n").unwrap();
        let mut a = args();
        a.left = l.to_str().unwrap().into();
        a.right = r.to_str().unwrap().into();
        assert_eq!(run(&a), 0);
        fs::write(&l, "a\nb").unwrap();
        fs::write(&r, "a\nb").unwrap();
        assert_eq!(run(&a), 0);
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

#[cfg(test)]
mod crlf_tests {
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
            ignore_crlf: false,
            convert: false,
            external: false,
            ignore_lines: vec![],
            color: "never".into(),
            highlight: false,
            labels: vec![],
            profile: None,
            json: false,
        }
    }

    #[test]
    fn ignore_crlf_normalizes_cr() {
        // lines() 已剥离 \r\n 的 \n；单独 \r 行尾会残留（如末行 "b\r"）
        assert_eq!(
            normalize_line("abc\r", false, false, false, true, &[]),
            "abc"
        );
        assert_eq!(normalize_line("abc", false, false, false, true, &[]), "abc");
        // 不忽略时保留 CR
        assert_eq!(
            normalize_line("abc\r", false, false, false, false, &[]),
            "abc\r"
        );
    }

    #[test]
    fn crlf_diff_ignored_matches() {
        let d = tempdir().unwrap();
        let l = d.path().join("l.txt");
        let r = d.path().join("r.txt");
        // 左侧末行残留 \r（CRLF 文件），右侧纯 LF
        fs::write(&l, "a\r\nb\r").unwrap();
        fs::write(&r, "a\nb").unwrap();
        let mut a = args();
        a.left = l.to_str().unwrap().into();
        a.right = r.to_str().unwrap().into();
        // 默认：末行 \r 差异
        assert_eq!(run(&a), 1);
        // --ignore-crlf：视为相同
        a.ignore_crlf = true;
        assert_eq!(run(&a), 0);
    }

    #[test]
    fn convert_normalizes_crlf_before_compare() {
        let d = tempdir().unwrap();
        let l = d.path().join("l.txt");
        let r = d.path().join("r.txt");
        // 左侧 CRLF 且末行残留 \r，右侧纯 LF（末行无 \r）
        fs::write(&l, "a\r\nb\r").unwrap();
        fs::write(&r, "a\nb").unwrap();
        let mut a = args();
        a.left = l.to_str().unwrap().into();
        a.right = r.to_str().unwrap().into();
        // 默认：末行 \r 差异存在
        assert_eq!(run(&a), 1);
        // --convert：统一换行后视为相同
        a.convert = true;
        assert_eq!(run(&a), 0);
    }
}

#[cfg(test)]
mod content_filter_tests {
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
            ignore_crlf: false,
            convert: false,
            external: false,
            ignore_lines: vec![],
            color: "never".into(),
            highlight: false,
            labels: vec![],
            profile: None,
            json: false,
        }
    }

    #[test]
    fn ignore_lines_compiles_and_matches() {
        let re = regex::Regex::new(r"time: \d{4}-\d{2}-\d{2}").unwrap();
        // 匹配 → 空键
        assert_eq!(
            normalize_line(
                "time: 2026-08-11 12:00:00",
                false,
                false,
                false,
                false,
                std::slice::from_ref(&re)
            ),
            ""
        );
        // 不匹配 → 原样
        assert_eq!(
            normalize_line("hello", false, false, false, false, &[re]),
            "hello"
        );
    }

    #[test]
    fn ignore_lines_merges_diff() {
        let d = tempdir().unwrap();
        let l = d.path().join("l.txt");
        let r = d.path().join("r.txt");
        // 两文件仅时间戳行不同（version 行相同）
        fs::write(&l, "version: 1.0\ntime: 2026-08-11 12:00:00\n").unwrap();
        fs::write(&r, "version: 1.0\ntime: 2026-08-11 13:00:00\n").unwrap();
        let mut a = args();
        a.left = l.to_str().unwrap().into();
        a.right = r.to_str().unwrap().into();
        // 默认：时间戳行不同 → 有差异
        assert_eq!(run(&a), 1);
        // --ignore-lines 忽略时间戳行 → 无差异
        a.ignore_lines = vec![r"time: \d{4}-\d{2}-\d{2}".to_string()];
        assert_eq!(run(&a), 0);
    }

    #[test]
    fn ignore_lines_still_detects_real_diff() {
        let d = tempdir().unwrap();
        let l = d.path().join("l.txt");
        let r = d.path().join("r.txt");
        fs::write(&l, "time: 2026-08-11 12:00:00\nreal line\n").unwrap();
        fs::write(&r, "time: 2026-08-11 13:00:00\nREAL CHANGED\n").unwrap();
        let mut a = args();
        a.left = l.to_str().unwrap().into();
        a.right = r.to_str().unwrap().into();
        a.ignore_lines = vec![r"time: \d{4}-\d{2}-\d{2}".to_string()];
        // 时间戳被忽略，但 real line 仍不同 → 有差异
        assert_eq!(run(&a), 1);
    }
}
