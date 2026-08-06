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

/// 运行 merge 子命令，返回进程退出码（0=无冲突，1=有冲突，2=错误）
pub fn run(args: &MergeArgs) -> i32 {
    let base = match read_input(&args.base) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("bcr: 无法读取 {}: {e}", args.base);
            return 2;
        }
    };
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

    let base_lines: Vec<&str> = base.lines().collect();
    let left_lines: Vec<&str> = left.lines().collect();
    let right_lines: Vec<&str> = right.lines().collect();

    // 两个 diff 都基于 base 行号，归并即可得到三路合并结果
    let ops_l = capture_diff_slices(algo, &base_lines, &left_lines);
    let ops_r = capture_diff_slices(algo, &base_lines, &right_lines);
    let regions_l = extract_regions(&ops_l, &base_lines, &left_lines);
    let regions_r = extract_regions(&ops_r, &base_lines, &right_lines);

    let label_l = args.labels.first().cloned().unwrap_or_else(|| "LEFT".to_string());
    let label_r = args
        .labels
        .get(1)
        .cloned()
        .unwrap_or_else(|| "RIGHT".to_string());

    let mut out: Vec<String> = Vec::new();
    let mut conflicts = 0usize;
    let mut cur = 0usize; // base 游标
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
        // 两侧都未改动的公共区
        for line in &base_lines[cur..next] {
            out.push((*line).to_string());
        }

        // 收集与 [next, end) 重叠（传递闭包）的变更区域，构成一个处理块
        let (block_l, block_r, end) = collect_block(&regions_l, &regions_r, &mut i, &mut j, next);
        let lv = apply_regions(&base_lines, &block_l, next, end);
        let rv = apply_regions(&base_lines, &block_r, next, end);

        if block_l.is_empty() {
            // 只有右侧改动
            out.extend(rv.iter().map(|s| s.to_string()));
        } else if block_r.is_empty() {
            // 只有左侧改动
            out.extend(lv.iter().map(|s| s.to_string()));
        } else if lv == rv {
            // 两侧改动相同 → 无冲突
            out.extend(lv.iter().map(|s| s.to_string()));
        } else {
            // 冲突
            conflicts += 1;
            out.push(format!("<<<<<<< {label_l}"));
            out.extend(lv.iter().map(|s| s.to_string()));
            out.push("=======".to_string());
            out.extend(rv.iter().map(|s| s.to_string()));
            out.push(format!(">>>>>>> {label_r}"));
        }
        cur = end;
    }
    // 尾部公共区
    for line in &base_lines[cur..] {
        out.push((*line).to_string());
    }

    // 输出
    if let Some(path) = &args.output {
        let mut content = out.join("\n");
        if !content.is_empty() {
            content.push('\n');
        }
        if let Err(e) = fs::write(Path::new(path), content) {
            eprintln!("bcr: 写入 {} 失败: {e}", path);
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
fn collect_block<'a>(
    regions_l: &'a [Region<'a>],
    regions_r: &'a [Region<'a>],
    i: &mut usize,
    j: &mut usize,
    start: usize,
) -> (Vec<&'a Region<'a>>, Vec<&'a Region<'a>>, usize) {
    let mut bl: Vec<&Region<'a>> = Vec::new();
    let mut br: Vec<&Region<'a>> = Vec::new();
    let mut end = start;
    loop {
        let mut changed = false;
        if let Some(a) = regions_l.get(*i) {
            if overlap(&a.base, start, end) {
                end = end.max(eff_end(&a.base));
                bl.push(a);
                *i += 1;
                changed = true;
            }
        }
        if let Some(b) = regions_r.get(*j) {
            if overlap(&b.base, start, end) {
                end = end.max(eff_end(&b.base));
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
