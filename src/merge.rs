use crate::i18n::{fmt, Key};
use clap::Args;
use similar::{capture_diff_slices, Algorithm, DiffOp, DiffTag};
use std::fs;
use std::io::{self, Read};
use std::ops::Range;
use std::path::Path;

/// merge 子命令参数
#[derive(Args, Debug)]
pub struct MergeArgs {
    /// 基线文件（- 表示 stdin）
    pub base: String,

    /// 左侧修改版本（- 表示 stdin）
    pub left: String,

    /// 右侧修改版本（- 表示 stdin）
    pub right: String,

    /// 输出到文件（默认 stdout）
    #[arg(short = 'o', long)]
    pub output: Option<String>,

    /// diff 算法：myers | patience
    #[arg(long, default_value = "patience", value_parser = ["myers", "patience"])]
    pub algo: String,

    /// 冲突标记标签，最多两个（默认 LEFT / RIGHT）
    #[arg(short = 'L', num_args = 1..=2, default_values = ["LEFT", "RIGHT"])]
    pub labels: Vec<String>,
}

/// 一侧对 base 的一个变更区域：base 行区间 + 该侧的替换内容（可能为空 = 删除）
struct Region<'a> {
    base: Range<usize>,
    side_lines: Vec<&'a str>,
}

/// 一个归并块：覆盖 base 区间 [base)，左侧/右侧应用变更后的行序列
/// （GUI 三路合并视图与 CLI 共用）
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MergeBlock<'a> {
    pub base: Range<usize>,
    pub left: Vec<&'a str>,
    pub right: Vec<&'a str>,
    /// 两侧都改了且内容不同 → 冲突
    pub conflict: bool,
}

/// 计算三路归并块序列（公共区 + 变更区，按 base 行号有序）。
///
/// 输出与 `run()` 的归并结果逐块对应：非冲突块取 left（内容等于 right 或未改侧）；
/// 冲突块由调用方决定如何输出/渲染。
pub(crate) fn compute_blocks<'a>(
    base: &[&'a str],
    left: &[&'a str],
    right: &[&'a str],
    algo: Algorithm,
) -> Vec<MergeBlock<'a>> {
    let ops_l = capture_diff_slices(algo, base, left);
    let ops_r = capture_diff_slices(algo, base, right);
    let regions_l = extract_regions(&ops_l, base, left);
    let regions_r = extract_regions(&ops_r, base, right);

    let mut blocks: Vec<MergeBlock<'a>> = Vec::new();
    let mut cur = 0usize;
    let mut i = 0usize;
    let mut j = 0usize;

    while i < regions_l.len() || j < regions_r.len() {
        // 下一个变更区域的 base 起点
        let next = match (regions_l.get(i), regions_r.get(j)) {
            (Some(a), Some(b)) => a.base.start.min(b.base.start),
            (Some(a), None) => a.base.start,
            (None, Some(b)) => b.base.start,
            (None, None) => break,
        };
        // 公共区
        if next > cur {
            blocks.push(MergeBlock {
                base: cur..next,
                left: base[cur..next].to_vec(),
                right: base[cur..next].to_vec(),
                conflict: false,
            });
        }

        // 收集重叠（传递闭包）区域，构成一个处理块
        let (block_l, block_r, end) = collect_block(&regions_l, &regions_r, &mut i, &mut j, next);
        let lv = apply_regions(base, &block_l, next, end);
        let rv = apply_regions(base, &block_r, next, end);
        let conflict = !block_l.is_empty() && !block_r.is_empty() && lv != rv;
        blocks.push(MergeBlock {
            base: next..end,
            left: lv,
            right: rv,
            conflict,
        });
        cur = end;
    }
    // 尾部公共区
    if cur < base.len() {
        blocks.push(MergeBlock {
            base: cur..base.len(),
            left: base[cur..].to_vec(),
            right: base[cur..].to_vec(),
            conflict: false,
        });
    }
    blocks
}

/// 运行 merge 子命令，返回进程退出码（0=无冲突，1=有冲突，2=错误）
pub fn run(args: &MergeArgs) -> i32 {
    let base = match read_input(&args.base) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("bcr: {}", fmt(Key::CannotRead, &[&args.base, &e.to_string()]));
            return 2;
        }
    };
    let left = match read_input(&args.left) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("bcr: {}", fmt(Key::CannotRead, &[&args.left, &e.to_string()]));
            return 2;
        }
    };
    let right = match read_input(&args.right) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("bcr: {}", fmt(Key::CannotRead, &[&args.right, &e.to_string()]));
            return 2;
        }
    };

    let algo = match args.algo.as_str() {
        "myers" => Algorithm::Myers,
        _ => Algorithm::Patience,
    };

    let base_lines: Vec<&str> = base.lines().collect();
    let left_lines: Vec<&str> = left.lines().collect();
    let right_lines: Vec<&str> = right.lines().collect();

    // 归并为块序列（公共区 + 变更区，按 base 行号有序）
    let blocks = compute_blocks(&base_lines, &left_lines, &right_lines, algo);

    let label_l = args.labels.first().cloned().unwrap_or_else(|| "LEFT".to_string());
    let label_r = args
        .labels
        .get(1)
        .cloned()
        .unwrap_or_else(|| "RIGHT".to_string());

    let mut out: Vec<String> = Vec::new();
    let mut conflicts = 0usize;

    for blk in &blocks {
        if blk.conflict {
            conflicts += 1;
            out.push(format!("<<<<<<< {label_l}"));
            out.extend(blk.left.iter().map(|s| s.to_string()));
            out.push("=======".to_string());
            out.extend(blk.right.iter().map(|s| s.to_string()));
            out.push(format!(">>>>>>> {label_r}"));
        } else if blk.left == blk.right {
            // 公共区或两侧相同修改
            out.extend(blk.left.iter().map(|s| s.to_string()));
        } else if blk.left == base_lines[blk.base.clone()] {
            // 左侧未改 → 只有右侧改动
            out.extend(blk.right.iter().map(|s| s.to_string()));
        } else {
            // 右侧未改 → 只有左侧改动
            out.extend(blk.left.iter().map(|s| s.to_string()));
        }
    }

    // 输出
    if let Some(path) = &args.output {
        // 换行风格跟随 base 源文件：Windows CRLF 文件合并后保持 CRLF
        let nl = if detect_crlf(&args.base) { "\r\n" } else { "\n" };
        let mut content = out.join(nl);
        if !content.is_empty() {
            content.push_str(nl);
        }
        if let Err(e) = fs::write(Path::new(path), content) {
            eprintln!("bcr: {}", fmt(Key::WriteFailed, &[path, &e.to_string()]));
            return 2;
        }
    } else {
        for l in &out {
            println!("{l}");
        }
    }

    if conflicts > 0 {
        1
    } else {
        0
    }
}

/// 从 diff ops 提取一侧的变更区域列表（按 base 行号升序）
fn extract_regions<'a>(
    ops: &[DiffOp],
    base: &[&'a str],
    side: &[&'a str],
) -> Vec<Region<'a>> {
    let _ = base;
    let mut regions = Vec::new();
    for op in ops {
        match op.tag() {
            DiffTag::Equal => {}
            DiffTag::Delete => regions.push(Region {
                base: op.old_range(),
                side_lines: Vec::new(),
            }),
            DiffTag::Insert => regions.push(Region {
                base: op.old_range(), // 空区间：插在 base.start 行之前
                side_lines: side[op.new_range()].to_vec(),
            }),
            DiffTag::Replace => regions.push(Region {
                base: op.old_range(),
                side_lines: side[op.new_range()].to_vec(),
            }),
        }
    }
    regions
}

/// 收集与 [start, end) 重叠（含传递闭包）的两侧区域，返回块内容与块结束行号
fn collect_block<'a, 'b>(
    regions_l: &'b [Region<'a>],
    regions_r: &'b [Region<'a>],
    i: &mut usize,
    j: &mut usize,
    start: usize,
) -> (Vec<&'b Region<'a>>, Vec<&'b Region<'a>>, usize) {
    let mut bl: Vec<&Region<'a>> = Vec::new();
    let mut br: Vec<&Region<'a>> = Vec::new();
    let mut end = start;
    loop {
        let mut changed = false;
        if let Some(a) = regions_l.get(*i) {
            if overlap(&a.base, start, end) {
                // 空区间（纯插入）不扩展块尾，避免吞掉相邻公共区；
                // 非空区间才把块尾推进到其结束行。
                if !a.base.is_empty() {
                    end = end.max(a.base.end);
                }
                bl.push(a);
                *i += 1;
                changed = true;
            }
        }
        if let Some(b) = regions_r.get(*j) {
            if overlap(&b.base, start, end) {
                if !b.base.is_empty() {
                    end = end.max(b.base.end);
                }
                br.push(b);
                *j += 1;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    (bl, br, end)
}

/// 区间与 [start, end) 是否重叠（空区间视作单点，用于插入定位）
///
/// 注意：相邻但不重叠的区间（如 [8,9) 与 [9,10)）必须判为不重叠，
/// 否则两侧对相邻行的独立修改会被误判为冲突。
fn overlap(r: &Range<usize>, start: usize, end: usize) -> bool {
    // 空块 [s, s) 视作单点 [s, s+1)，保证起始 region 能加入
    let block_eff_end = if end == start { start + 1 } else { end };
    r.start < block_eff_end && start < eff_end(r)
}

/// 空区间 [s, s) 的有效结束位置视为 s+1（插入发生在第 s 行之前）
fn eff_end(r: &Range<usize>) -> usize {
    if r.is_empty() {
        r.start + 1
    } else {
        r.end
    }
}

/// 应用一侧的区域列表，重建该侧在 [start, end) 范围内的完整行序列
fn apply_regions<'a>(
    base: &[&'a str],
    regions: &[&Region<'a>],
    start: usize,
    end: usize,
) -> Vec<&'a str> {
    let mut out = Vec::new();
    let mut cur = start;
    for r in regions {
        out.extend_from_slice(&base[cur..r.base.start]);
        out.extend_from_slice(&r.side_lines);
        cur = r.base.end;
    }
    out.extend_from_slice(&base[cur..end]);
    out
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

/// 检测文件是否使用 CRLF 换行（二进制安全：只扫前 64KB）
fn detect_crlf(path: &str) -> bool {
    let Ok(mut f) = std::fs::File::open(path) else {
        return false;
    };
    let mut buf = [0u8; 65536];
    use std::io::Read;
    let Ok(n) = f.read(&mut buf) else {
        return false;
    };
    let bytes = &buf[..n];
    let crlf = bytes.windows(2).filter(|w| w == b"\r\n").count();
    let lf = bytes.iter().filter(|&&b| b == b'\n').count();
    crlf > 0 && crlf * 2 >= lf // CRLF 占换行的大多数才判定为 CRLF 文件
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn args(base: &str, left: &str, right: &str) -> MergeArgs {
        MergeArgs {
            base: base.into(),
            left: left.into(),
            right: right.into(),
            output: None,
            algo: "patience".into(),
            labels: vec![],
        }
    }

    fn write_file(dir: &std::path::Path, name: &str, content: &str) -> String {
        let p = dir.join(name);
        fs::write(&p, content).unwrap();
        p.to_str().unwrap().to_string()
    }

    #[test]
    fn run_no_conflict_exit_zero() {
        let d = tempdir().unwrap();
        let b = write_file(d.path(), "base.txt", "l1\nl2\nl3\nl4\nl5\n");
        let l = write_file(d.path(), "left.txt", "L1\nl2\nl3\nl4\nl5\n");
        let r = write_file(d.path(), "right.txt", "l1\nl2\nl3\nl4\nR5\n");
        let a = args(&b, &l, &r);
        assert_eq!(run(&a), 0);
    }

    #[test]
    fn run_conflict_exit_one_with_markers() {
        let d = tempdir().unwrap();
        let b = write_file(d.path(), "base.txt", "l1\nX3\nl3\n");
        let l = write_file(d.path(), "left.txt", "l1\nL3\nl3\n");
        let r = write_file(d.path(), "right.txt", "l1\nR3\nl3\n");
        let out = d.path().join("out.txt");
        let out_s = out.to_str().unwrap().to_string();
        let mut a = args(&b, &l, &r);
        a.output = Some(out_s);
        assert_eq!(run(&a), 1);
        let content = fs::read_to_string(&out).unwrap();
        assert!(content.contains("<<<<<<< LEFT"));
        assert!(content.contains("======="));
        assert!(content.contains(">>>>>>> RIGHT"));
    }

    #[test]
    fn run_identical_changes_no_conflict() {
        let d = tempdir().unwrap();
        let b = write_file(d.path(), "base.txt", "l1\nX3\nl3\n");
        let l = write_file(d.path(), "left.txt", "l1\nZ3\nl3\n");
        let r = write_file(d.path(), "right.txt", "l1\nZ3\nl3\n");
        let a = args(&b, &l, &r);
        assert_eq!(run(&a), 0);
    }

    #[test]
    fn run_single_side_change_merged() {
        let d = tempdir().unwrap();
        let b = write_file(d.path(), "base.txt", "l1\nl2\nl3\n");
        let l = write_file(d.path(), "left.txt", "l1\nIA\nl2\nl3\n");
        let r = write_file(d.path(), "right.txt", "l1\nl2\nl3\n");
        let out = d.path().join("out.txt");
        let out_s = out.to_str().unwrap().to_string();
        let mut a = args(&b, &l, &r);
        a.output = Some(out_s);
        assert_eq!(run(&a), 0);
        let content = fs::read_to_string(&out).unwrap();
        assert!(content.contains("IA"));
    }

    #[test]
    fn run_missing_file_exit_two() {
        let d = tempdir().unwrap();
        let b = write_file(d.path(), "base.txt", "l1\n");
        let a = args(&b, "/nonexistent/l", "/nonexistent/r");
        assert_eq!(run(&a), 2);
    }

    #[test]
    fn run_adjacent_independent_edits_no_conflict() {
        // 经典 diff3 语义：两侧对相邻行的独立修改不应冲突
        let d = tempdir().unwrap();
        let b = write_file(d.path(), "base.txt", "l1\nl2\nl3\n");
        let l = write_file(d.path(), "left.txt", "L2\nl2\nl3\n");
        let r = write_file(d.path(), "right.txt", "l1\nR2\nl3\n");
        let a = args(&b, &l, &r);
        assert_eq!(run(&a), 0);
    }

    #[test]
    fn run_stdin_left_side_supported() {
        // 无法直接注入 stdin，这里验证文件路径读取路径本身正常
        let d = tempdir().unwrap();
        let b = write_file(d.path(), "base.txt", "l1\nX2\nl3\n");
        let l = write_file(d.path(), "left.txt", "l1\nX2\nl3\n");
        let r = write_file(d.path(), "right.txt", "l1\nX2\nl3\n");
        let a = args(&b, &l, &r);
        assert_eq!(run(&a), 0);
    }

    #[test]
    fn extract_regions_replace() {
        let base: Vec<&str> = vec!["a", "b", "c"];
        let side: Vec<&str> = vec!["a", "X", "c"];
        let ops = capture_diff_slices(Algorithm::Myers, &base, &side);
        let regions = extract_regions(&ops, &base, &side);
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].base, 1..2);
        assert_eq!(regions[0].side_lines, vec!["X"]);
    }

    #[test]
    fn extract_regions_insert() {
        let base: Vec<&str> = vec!["a", "b"];
        let side: Vec<&str> = vec!["a", "X", "b"];
        let ops = capture_diff_slices(Algorithm::Myers, &base, &side);
        let regions = extract_regions(&ops, &base, &side);
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].base, 1..1); // 空区间，插入点
        assert_eq!(regions[0].side_lines, vec!["X"]);
    }

    #[test]
    fn extract_regions_delete() {
        let base: Vec<&str> = vec!["a", "b", "c"];
        let side: Vec<&str> = vec!["a", "c"];
        let ops = capture_diff_slices(Algorithm::Myers, &base, &side);
        let regions = extract_regions(&ops, &base, &side);
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].base, 1..2);
        assert!(regions[0].side_lines.is_empty());
    }

    #[test]
    fn overlap_semantics() {
        // 重叠
        assert!(overlap(&(1..3), 1, 3));
        assert!(overlap(&(0..2), 1, 3));
        assert!(overlap(&(2..4), 1, 3));
        // 相邻但不重叠（关键语义）
        assert!(!overlap(&(8..9), 9, 10));
        assert!(!overlap(&(9..10), 8, 9));
        // 空区间视作单点
        assert!(overlap(&(1..1), 1, 1));
        assert!(!overlap(&(3..3), 1, 2));
    }

    #[test]
    fn eff_end_empty_range_plus_one() {
        assert_eq!(eff_end(&(3..3)), 4);
        assert_eq!(eff_end(&(3..5)), 5);
    }

    #[test]
    fn apply_regions_rebuilds_lines() {
        let base: Vec<&str> = vec!["a", "b", "c", "d", "e"];
        let r1 = Region { base: 1..2, side_lines: vec!["X", "Y"] };
        let r2 = Region { base: 3..4, side_lines: vec![] };
        let out = apply_regions(&base, &[&r1, &r2], 0, 5);
        assert_eq!(out, vec!["a", "X", "Y", "c", "e"]);
    }

    #[test]
    fn collect_block_gathers_overlapping_regions() {
        // 左区域 1..3 与右区域 2..4 重叠 → 构成一个块
        let rl = Region { base: 1..3, side_lines: vec!["L"] };
        let rr = Region { base: 2..4, side_lines: vec!["R"] };
        let regions_l = vec![rl];
        let regions_r = vec![rr];
        let mut i = 0;
        let mut j = 0;
        let (bl, br, end) = collect_block(&regions_l, &regions_r, &mut i, &mut j, 1);
        assert_eq!(bl.len(), 1);
        assert_eq!(br.len(), 1);
        assert_eq!(end, 4);
        assert_eq!(i, 1);
        assert_eq!(j, 1);
    }

    #[test]
    fn collect_block_adjacent_regions_not_overlapping() {
        // 相邻但不重叠（8..9 与 9..10）→ 不应合并进同一块
        let rl = Region { base: 8..9, side_lines: vec!["L"] };
        let rr = Region { base: 9..10, side_lines: vec!["R"] };
        let regions_l = vec![rl];
        let regions_r = vec![rr];
        let mut i = 0;
        let mut j = 0;
        let (bl, br, end) = collect_block(&regions_l, &regions_r, &mut i, &mut j, 8);
        assert_eq!(bl.len(), 1);
        assert!(br.is_empty());
        assert_eq!(end, 9);
    }

    #[test]
    fn collect_block_transitive_closure() {
        // 左 1..2 + 右 1..3 → 左 2..3 因传递闭包也并入
        let rl1 = Region { base: 1..2, side_lines: vec!["L1"] };
        let rl2 = Region { base: 2..3, side_lines: vec!["L2"] };
        let rr = Region { base: 1..3, side_lines: vec!["R"] };
        let regions_l = vec![rl1, rl2];
        let regions_r = vec![rr];
        let mut i = 0;
        let mut j = 0;
        let (bl, br, end) = collect_block(&regions_l, &regions_r, &mut i, &mut j, 1);
        assert_eq!(bl.len(), 2);
        assert_eq!(br.len(), 1);
        assert_eq!(end, 3);
        assert_eq!(i, 2);
        assert_eq!(j, 1);
    }
}
