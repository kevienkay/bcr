use similar::{capture_diff_slices, Algorithm, DiffOp};

/// hunk 上下文行数
const CTX: usize = 3;

const RED: &str = "\x1b[31m";
const GREEN: &str = "\x1b[32m";
const BOLD: &str = "\x1b[1m";
const BG_RED: &str = "\x1b[41m";
const BG_GREEN: &str = "\x1b[42m";
const RESET: &str = "\x1b[0m";

/// 渲染 unified diff（带行内高亮）
pub fn render_unified(
    ops: &[DiffOp],
    lines_l: &[&str],
    lines_r: &[&str],
    label_l: &str,
    label_r: &str,
    color: bool,
) {
    println!("--- {label_l}");
    println!("+++ {label_r}");

    // 将 ops 分组为 hunk：相邻 op 间隔超过 2*CTX 行则断开
    let hunks = group_hunks(ops);

    for group in &hunks {
        let first = group[0];
        let last = group[group.len() - 1];

        let old_start = ops[first].old_range().start.saturating_sub(CTX);
        let new_start = ops[first].new_range().start.saturating_sub(CTX);
        let old_end = (ops[last].old_range().end + CTX).min(lines_l.len());
        let new_end = (ops[last].new_range().end + CTX).min(lines_r.len());

        println!(
            "@@ -{},{} +{},{} @@",
            old_start + 1,
            old_end - old_start,
            new_start + 1,
            new_end - new_start
        );

        // 逐 op 输出，op 之间的间隔区补上下文行
        let mut old_idx = old_start;
        let mut new_idx = new_start;
        for &op_i in group {
            let op = &ops[op_i];
            while old_idx < op.old_range().start {
                emit_plain(' ', lines_l[old_idx], color);
                old_idx += 1;
                new_idx += 1;
            }
            emit_op(op, lines_l, lines_r, &mut old_idx, &mut new_idx, color);
        }
        while old_idx < old_end {
            emit_plain(' ', lines_l[old_idx], color);
            old_idx += 1;
        }
        let _ = new_idx; // hunk 内部与 new 侧行号保持同步即可，hunk 结束后不再使用
    }
}

fn emit_op(
    op: &DiffOp,
    lines_l: &[&str],
    lines_r: &[&str],
    old_idx: &mut usize,
    new_idx: &mut usize,
    color: bool,
) {
    use similar::DiffTag::*;
    match op.tag() {
        Equal => {
            for _ in 0..(op.old_range().end - op.old_range().start) {
                emit_plain(' ', lines_l[*old_idx], color);
                *old_idx += 1;
                *new_idx += 1;
            }
        }
        Delete => {
            for i in op.old_range().start..op.old_range().end {
                emit_plain('-', lines_l[i], color);
                *old_idx += 1;
            }
        }
        Insert => {
            for i in op.new_range().start..op.new_range().end {
                emit_plain('+', lines_r[i], color);
                *new_idx += 1;
            }
        }
        Replace => {
            // 删除行与插入行按顺序配对，配对行之间做字符级 diff（行内高亮）
            let d = op.old_range().end - op.old_range().start;
            let i = op.new_range().end - op.new_range().start;
            let paired = d.min(i);
            for k in 0..paired {
                let (segs_l, segs_r) = intra_line(
                    lines_l[op.old_range().start + k],
                    lines_r[op.new_range().start + k],
                );
                emit_segments('-', &segs_l, color);
                emit_segments('+', &segs_r, color);
                *old_idx += 1;
                *new_idx += 1;
            }
            for k in paired..d {
                emit_plain('-', lines_l[op.old_range().start + k], color);
                *old_idx += 1;
            }
            for k in paired..i {
                emit_plain('+', lines_r[op.new_range().start + k], color);
                *new_idx += 1;
            }
        }
    }
}

/// 对单行做字符级 diff，返回左右两侧的分段 (文本, 是否变更)
fn intra_line(old: &str, new: &str) -> (Vec<(String, bool)>, Vec<(String, bool)>) {
    let old_chars: Vec<char> = old.chars().collect();
    let new_chars: Vec<char> = new.chars().collect();
    let ops = capture_diff_slices(Algorithm::Myers, &old_chars, &new_chars);

    let mut left: Vec<(String, bool)> = Vec::new();
    let mut right: Vec<(String, bool)> = Vec::new();
    for op in ops {
        match op.tag() {
            similar::DiffTag::Equal => {
                for c in &old_chars[op.old_range()] {
                    left.push((c.to_string(), false));
                }
                for c in &new_chars[op.new_range()] {
                    right.push((c.to_string(), false));
                }
            }
            similar::DiffTag::Delete => {
                for c in &old_chars[op.old_range()] {
                    left.push((c.to_string(), true));
                }
            }
            similar::DiffTag::Insert => {
                for c in &new_chars[op.new_range()] {
                    right.push((c.to_string(), true));
                }
            }
            similar::DiffTag::Replace => {
                for c in &old_chars[op.old_range()] {
                    left.push((c.to_string(), true));
                }
                for c in &new_chars[op.new_range()] {
                    right.push((c.to_string(), true));
                }
            }
        }
    }
    (left, right)
}

fn emit_plain(sign: char, text: &str, color: bool) {
    if !color {
        println!("{sign}{text}");
        return;
    }
    match sign {
        '-' => println!("{RED}{sign}{text}{RESET}"),
        '+' => println!("{GREEN}{sign}{text}{RESET}"),
        _ => println!(" {text}"),
    }
}

/// 带行内高亮的分段输出：变更段加粗 + 背景色
fn emit_segments(sign: char, segs: &[(String, bool)], color: bool) {
    if !color {
        let text: String = segs.iter().map(|(s, _)| s.as_str()).collect();
        println!("{sign}{text}");
        return;
    }
    // 合并相邻同标志段，减少转义序列数量
    let mut merged: Vec<(String, bool)> = Vec::new();
    for (s, changed) in segs {
        if let Some(last) = merged.last_mut() {
            if last.1 == *changed {
                last.0.push_str(s);
                continue;
            }
        }
        merged.push((s.clone(), *changed));
    }
    let (fg, bg) = match sign {
        '-' => (RED, BG_RED),
        _ => (GREEN, BG_GREEN),
    };
    let mut out = String::new();
    out.push_str(fg);
    out.push(sign);
    for (s, changed) in &merged {
        if *changed {
            out.push_str(BOLD);
            out.push_str(bg);
            out.push_str(s);
            out.push_str(RESET);
            out.push_str(fg);
        } else {
            out.push_str(s);
        }
    }
    out.push_str(RESET);
    println!("{out}");
}

fn group_hunks(ops: &[DiffOp]) -> Vec<Vec<usize>> {
    let mut hunks: Vec<Vec<usize>> = Vec::new();
    let mut cur: Vec<usize> = Vec::new();
    let mut prev_change_end_old = 0usize;
    let mut prev_change_end_new = 0usize;
    let mut seen_change = false;
    for (i, op) in ops.iter().enumerate() {
        // Equal op 是变更之间的间隔区，不参与分组；间隔大小由变更 op 的行号差决定
        if op.tag() == similar::DiffTag::Equal {
            continue;
        }
        if !seen_change {
            cur.push(i);
            seen_change = true;
        } else {
            let gap_old = op.old_range().start.saturating_sub(prev_change_end_old);
            let gap_new = op.new_range().start.saturating_sub(prev_change_end_new);
            if gap_old <= CTX * 2 && gap_new <= CTX * 2 {
                cur.push(i);
            } else {
                hunks.push(std::mem::take(&mut cur));
                cur.push(i);
            }
        }
        prev_change_end_old = op.old_range().end;
        prev_change_end_new = op.new_range().end;
    }
    if seen_change {
        hunks.push(cur);
    }
    hunks
}
