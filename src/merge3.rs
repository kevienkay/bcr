//! P28 批次 2：三路文件夹合并（对标 Beyond Compare Pro 的 folder merge）。
//!
//! `bcr merge3 BASE LEFT RIGHT -o OUT`：把 BASE/LEFT/RIGHT 三个目录合并到输出目录。
//!
//! 规则（与 BC 对齐）：
//! - 三侧相同 / 仅一侧存在 / 单侧修改 → 直接复制该侧版本到输出
//! - 双侧都改且内容不同（冲突）→ 文本文件自动三路合并（复用 merge::compute_blocks），
//!   冲突块输出冲突标记（`<<<<<<< LEFT ... ======= ... >>>>>>> RIGHT`）；
//!   二进制文件无法合并 → 复制 LEFT 版本并计为冲突，报告路径
//! - 一侧删除另一侧未改 → 输出中不保留该文件
//! - 一侧删除另一侧修改 → 保留修改侧版本
//!
//! `--dry-run` 只打印计划不执行；`--json` 输出契约 merge3.v1。

use crate::i18n::{fmt, Key};
use crate::vfs::Vfs;
use clap::Args;
use similar::Algorithm;
use std::io;

/// merge3 子命令参数
#[derive(Args, Debug)]
pub struct Merge3Args {
    /// 基线目录
    pub base: String,

    /// 左侧修改目录
    pub left: String,

    /// 右侧修改目录
    pub right: String,

    /// 输出目录（合并结果写入这里）
    #[arg(short = 'o', long)]
    pub output: String,

    /// 只打印合并计划，不执行
    #[arg(long)]
    pub dry_run: bool,

    /// 内容哈希比较（默认快速模式 size+mtime；跨文件系统建议开启）
    #[arg(long)]
    pub compare_content: bool,

    /// 包含 glob（逗号分隔）
    #[arg(long)]
    pub include: Vec<String>,

    /// 排除 glob（逗号分隔）
    #[arg(long)]
    pub exclude: Vec<String>,

    /// diff 算法：myers | patience
    #[arg(long, default_value = "patience", value_parser = ["myers", "patience"])]
    pub algo: String,

    /// 以 JSON 契约输出结果（schema: merge3.v1）
    #[arg(long)]
    pub json: bool,
}

/// 合并计划条目
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Merge3PlanItem {
    pub rel: String,
    /// copy(源侧) | merge(文本三路合并) | conflict(二进制冲突) | delete
    pub op: String,
    /// copy 时的源侧：base/left/right
    pub from: Option<String>,
    /// 冲突文件（op=conflict 时输出左侧版本）
    pub conflicted: bool,
}

/// 合并结果统计
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Merge3Stats {
    pub copied: usize,
    pub merged: usize,
    pub conflicts: usize,
    pub deleted: usize,
    pub same: usize,
}

/// 三路文件夹合并：生成合并计划（纯逻辑，可单测）。
///
/// 返回按路径排序的计划 + 统计。plan 的执行（写入输出）由 `execute_plan` 完成，
/// 以便 dry-run 只生成计划、不落盘。
pub fn build_merge3_plan(
    base: &dyn Vfs,
    left: &dyn Vfs,
    right: &dyn Vfs,
    filter: &crate::fsscan::Filter,
    compare_content: bool,
) -> io::Result<(Vec<Merge3PlanItem>, Merge3Stats)> {
    // 复用 compare3 的三路扫描/分类逻辑
    let tri = crate::compare3::compare3_vfs(base, left, right, filter, compare_content)
        .map_err(|e| io::Error::new(e.kind(), format!("compare3_vfs: {e}")))?;

    let mut plan: Vec<Merge3PlanItem> = Vec::new();
    let mut stats = Merge3Stats::default();

    for entry in tri.entries {
        let rel = &entry.rel;
        use crate::compare3::TriStatus as S;
        match entry.status {
            S::Same => {
                stats.same += 1;
                // 相同文件不需要复制
            }
            S::BaseOnly => {
                plan.push(Merge3PlanItem {
                    rel: rel.clone(),
                    op: "copy".into(),
                    from: Some("base".into()),
                    conflicted: false,
                });
                stats.copied += 1;
            }
            S::LeftOnly => {
                plan.push(Merge3PlanItem {
                    rel: rel.clone(),
                    op: "copy".into(),
                    from: Some("left".into()),
                    conflicted: false,
                });
                stats.copied += 1;
            }
            S::RightOnly => {
                plan.push(Merge3PlanItem {
                    rel: rel.clone(),
                    op: "copy".into(),
                    from: Some("right".into()),
                    conflicted: false,
                });
                stats.copied += 1;
            }
            S::LeftDeleted => {
                // 左侧删除：右侧未改或已改？
                // classify 语义：LeftDeleted = base==right 且 left 缺失 → 右侧未改 → 删除
                plan.push(Merge3PlanItem {
                    rel: rel.clone(),
                    op: "delete".into(),
                    from: None,
                    conflicted: false,
                });
                stats.deleted += 1;
            }
            S::RightDeleted => {
                plan.push(Merge3PlanItem {
                    rel: rel.clone(),
                    op: "delete".into(),
                    from: None,
                    conflicted: false,
                });
                stats.deleted += 1;
            }
            S::LeftModified => {
                plan.push(Merge3PlanItem {
                    rel: rel.clone(),
                    op: "copy".into(),
                    from: Some("left".into()),
                    conflicted: false,
                });
                stats.copied += 1;
            }
            S::RightModified => {
                plan.push(Merge3PlanItem {
                    rel: rel.clone(),
                    op: "copy".into(),
                    from: Some("right".into()),
                    conflicted: false,
                });
                stats.copied += 1;
            }
            S::BothModified | S::Conflict => {
                // 两侧都改（或两侧新增不同）→ 尝试文本三路合并
                let b_ok = base.read(rel).map(|d| !is_binary(&d)).unwrap_or(false);
                let l_ok = left.read(rel).map(|d| !is_binary(&d)).unwrap_or(false);
                let r_ok = right.read(rel).map(|d| !is_binary(&d)).unwrap_or(false);
                if b_ok && l_ok && r_ok {
                    plan.push(Merge3PlanItem {
                        rel: rel.clone(),
                        op: "merge".into(),
                        from: None,
                        conflicted: false,
                    });
                    stats.merged += 1;
                } else {
                    // 二进制冲突：复制 LEFT 版本并标记冲突
                    plan.push(Merge3PlanItem {
                        rel: rel.clone(),
                        op: "copy".into(),
                        from: Some("left".into()),
                        conflicted: true,
                    });
                    stats.conflicts += 1;
                }
            }
        }
    }
    Ok((plan, stats))
}

/// 判断字节流是否为二进制（复用 encoding 判定：NUL/控制字符启发式）
fn is_binary(data: &[u8]) -> bool {
    crate::encoding::decode(data).is_binary
}

/// 执行合并计划，把结果写入输出目录。返回 (冲突数, 合并数)。
pub fn execute_plan(
    base: &dyn Vfs,
    left: &dyn Vfs,
    right: &dyn Vfs,
    out: &dyn Vfs,
    plan: &[Merge3PlanItem],
    algo: Algorithm,
    dry_run: bool,
) -> io::Result<usize> {
    let mut conflicts = 0usize;
    for item in plan {
        if dry_run {
            match item.op.as_str() {
                "copy" => println!(
                    "  [{}] {}  ← {}",
                    if item.conflicted { "CONFLICT" } else { "copy" },
                    item.rel,
                    item.from.as_deref().unwrap_or("")
                ),
                "merge" => println!("  [merge] {}  (三路文本合并)", item.rel),
                "delete" => println!("  [delete] {}", item.rel),
                _ => {}
            }
            continue;
        }
        match item.op.as_str() {
            "copy" => {
                let src: &dyn Vfs = match item.from.as_deref() {
                    Some("base") => base,
                    Some("right") => right,
                    _ => left,
                };
                src.copy_to(&item.rel, out)?;
            }
            "merge" => {
                let b = base.read(&item.rel)?;
                let l = left.read(&item.rel)?;
                let r = right.read(&item.rel)?;
                let merged = merge_text(&b, &l, &r, algo, &mut conflicts)?;
                out.write(&item.rel, &merged)?;
            }
            "delete" if out.exists(&item.rel).unwrap_or(false) => {
                // 输出目录可能已有该文件（之前合并过）→ 删除；不存在则忽略
                out.delete(&item.rel)?;
            }
            "delete" => {}
            _ => {}
        }
    }
    Ok(conflicts)
}

/// 三路文本合并：复用 merge::compute_blocks，冲突块输出冲突标记。
fn merge_text(
    base: &[u8],
    left: &[u8],
    right: &[u8],
    algo: Algorithm,
    conflicts: &mut usize,
) -> io::Result<Vec<u8>> {
    // 按各自编码解码为文本（合并结果以 UTF-8 输出，冲突标记 ASCII 安全）
    let b = crate::encoding::decode(base);
    let l = crate::encoding::decode(left);
    let r = crate::encoding::decode(right);
    let b_lines: Vec<&str> = b.text.lines().collect();
    let l_lines: Vec<&str> = l.text.lines().collect();
    let r_lines: Vec<&str> = r.text.lines().collect();
    let blocks = crate::merge::compute_blocks(&b_lines, &l_lines, &r_lines, algo);

    let mut out: Vec<String> = Vec::new();
    for blk in &blocks {
        if blk.conflict {
            *conflicts += 1;
            out.push("<<<<<<< LEFT".to_string());
            out.extend(blk.left.iter().map(|s| s.to_string()));
            out.push("=======".to_string());
            out.extend(blk.right.iter().map(|s| s.to_string()));
            out.push(">>>>>>> RIGHT".to_string());
        } else if blk.left == blk.right {
            out.extend(blk.left.iter().map(|s| s.to_string()));
        } else if blk.left == b_lines[blk.base.clone()] {
            out.extend(blk.right.iter().map(|s| s.to_string()));
        } else {
            out.extend(blk.left.iter().map(|s| s.to_string()));
        }
    }
    let mut content = out.join("\n");
    if !content.is_empty() {
        content.push('\n');
    }
    Ok(content.into_bytes())
}

/// merge3 子命令入口
pub fn run(args: &Merge3Args) -> i32 {
    for (label, p) in [
        ("base", &args.base),
        ("left", &args.left),
        ("right", &args.right),
    ] {
        if !crate::vfs::is_remote(p) && !std::path::Path::new(p).is_dir() {
            eprintln!("bcr: {} ({label})", fmt(Key::NotDir, &[p]));
            return 2;
        }
    }
    let filter = match crate::fsscan::Filter::new(&args.include, &args.exclude) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("bcr: {}", fmt(Key::FilterError, &[&e.to_string()]));
            return 2;
        }
    };
    let b = match crate::vfs::open(&args.base) {
        Ok(v) => v,
        Err(e) => {
            eprintln!(
                "bcr: {}",
                fmt(Key::OpenFailed, &[&args.base, &e.to_string()])
            );
            return 2;
        }
    };
    let l = match crate::vfs::open(&args.left) {
        Ok(v) => v,
        Err(e) => {
            eprintln!(
                "bcr: {}",
                fmt(Key::OpenFailed, &[&args.left, &e.to_string()])
            );
            return 2;
        }
    };
    let r = match crate::vfs::open(&args.right) {
        Ok(v) => v,
        Err(e) => {
            eprintln!(
                "bcr: {}",
                fmt(Key::OpenFailed, &[&args.right, &e.to_string()])
            );
            return 2;
        }
    };

    let (plan, stats) = match build_merge3_plan(
        b.as_ref(),
        l.as_ref(),
        r.as_ref(),
        &filter,
        args.compare_content,
    ) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("bcr: {}", fmt(Key::ScanFailed, &[&e.to_string()]));
            return 2;
        }
    };

    if args.json {
        let items: Vec<serde_json::Value> = plan
            .iter()
            .map(|p| {
                serde_json::json!({
                    "op": p.op,
                    "rel": p.rel,
                    "from": p.from,
                    "conflicted": p.conflicted,
                })
            })
            .collect();
        let v = serde_json::json!({
            "schema": "merge3.v1",
            "ok": true,
            "command": "merge3",
            "args": { "base": args.base, "left": args.left, "right": args.right, "output": args.output },
            "result": {
                "dry_run": args.dry_run,
                "plan": items,
                "stats": {
                    "copied": stats.copied,
                    "merged": stats.merged,
                    "conflicts": stats.conflicts,
                    "deleted": stats.deleted,
                    "same": stats.same,
                },
                "has_conflicts": stats.conflicts > 0,
            },
            "warnings": [],
            "error": null,
        });
        println!("{}", serde_json::to_string(&v).unwrap_or_default());
        return if stats.conflicts > 0 { 1 } else { 0 };
    }

    if !args.dry_run {
        println!(
            "▶ 合并计划: {} 项（复制 {} / 合并 {} / 删除 {} / 冲突 {}）",
            plan.len(),
            stats.copied,
            stats.merged,
            stats.deleted,
            stats.conflicts
        );
    }

    let algo = if args.algo == "myers" {
        Algorithm::Myers
    } else {
        Algorithm::Patience
    };
    let out = match crate::vfs::open(&args.output) {
        Ok(v) => v,
        Err(e) => {
            eprintln!(
                "bcr: {}",
                fmt(Key::OpenFailed, &[&args.output, &e.to_string()])
            );
            return 2;
        }
    };
    match execute_plan(
        b.as_ref(),
        l.as_ref(),
        r.as_ref(),
        out.as_ref(),
        &plan,
        algo,
        args.dry_run,
    ) {
        Ok(conflicts) => {
            if args.dry_run {
                println!("[dry-run] 未执行任何操作");
            } else {
                println!(
                    "✓ 合并完成: 复制 {} / 合并 {} / 删除 {} / 冲突 {}",
                    stats.copied, stats.merged, stats.deleted, conflicts
                );
            }
            if conflicts > 0 {
                1
            } else {
                0
            }
        }
        Err(e) => {
            eprintln!(
                "bcr: {}",
                fmt(Key::WriteFailed, &[&args.output, &e.to_string()])
            );
            2
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fsscan::Filter;
    use std::fs;
    use tempfile::tempdir;

    fn write(dir: &std::path::Path, rel: &str, content: &str) {
        let p = dir.join(rel);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(p, content).unwrap();
    }

    #[test]
    fn plan_copies_single_side_and_same() {
        let d = tempdir().unwrap();
        let base = d.path().join("base");
        let left = d.path().join("left");
        let right = d.path().join("right");
        fs::create_dir_all(&base).unwrap();
        fs::create_dir_all(&left).unwrap();
        fs::create_dir_all(&right).unwrap();
        write(&base, "same.txt", "x");
        write(&left, "same.txt", "x");
        write(&right, "same.txt", "x");
        write(&left, "only_left.txt", "L");
        write(&right, "only_right.txt", "R");
        write(&base, "deleted.txt", "d");
        write(&left, "deleted.txt", "d");
        // right 缺 deleted.txt → RightDeleted

        let b = crate::vfs::LocalVfs::new(&base).unwrap();
        let l = crate::vfs::LocalVfs::new(&left).unwrap();
        let r = crate::vfs::LocalVfs::new(&right).unwrap();
        let f = Filter::new(&[], &[]).unwrap();
        let (plan, stats) = match build_merge3_plan(&b, &l, &r, &f, true) {
            Ok(x) => x,
            Err(e) => {
                eprintln!("DBG plan err: {e:?}");
                panic!("plan err");
            }
        };

        assert_eq!(stats.same, 1);
        assert_eq!(stats.copied, 2); // only_left + only_right
        assert_eq!(stats.deleted, 1); // deleted.txt
        let ops: Vec<&str> = plan.iter().map(|p| p.op.as_str()).collect();
        assert!(ops.contains(&"copy"));
        assert!(ops.contains(&"delete"));
        // 无 merge/conflict
        assert!(!ops.contains(&"merge"));
        assert!(!ops.contains(&"conflict"));
    }

    #[test]
    fn plan_marks_text_conflict_as_merge_and_binary_conflict() {
        let d = tempdir().unwrap();
        let base = d.path().join("base");
        let left = d.path().join("left");
        let right = d.path().join("right");
        fs::create_dir_all(&base).unwrap();
        fs::create_dir_all(&left).unwrap();
        fs::create_dir_all(&right).unwrap();
        // 文本冲突：base 1 行，两侧改成不同内容
        write(&base, "t.txt", "a\n");
        write(&left, "t.txt", "L\n");
        write(&right, "t.txt", "R\n");
        // 二进制冲突：两侧不同字节（含 NUL，确保被判为二进制）
        write(&base, "bin.dat", "AAAA\u{0}BBBB");
        write(&left, "bin.dat", "CCCC\u{0}DDDD");
        write(&right, "bin.dat", "EEEE\u{0}FFFF");

        let b = crate::vfs::LocalVfs::new(&base).unwrap();
        let l = crate::vfs::LocalVfs::new(&left).unwrap();
        let r = crate::vfs::LocalVfs::new(&right).unwrap();
        let f = Filter::new(&[], &[]).unwrap();
        let (plan, stats) = build_merge3_plan(&b, &l, &r, &f, true).unwrap();

        assert_eq!(stats.merged, 1); // t.txt
        assert_eq!(stats.conflicts, 1); // bin.dat
        let t = plan.iter().find(|p| p.rel == "t.txt").unwrap();
        assert_eq!(t.op, "merge");
        let bin = plan.iter().find(|p| p.rel == "bin.dat").unwrap();
        assert_eq!(bin.op, "copy");
        assert!(bin.conflicted);
    }

    #[test]
    fn execute_merge_writes_conflict_markers() {
        let d = tempdir().unwrap();
        let base = d.path().join("base");
        let left = d.path().join("left");
        let right = d.path().join("right");
        let out = d.path().join("out");
        fs::create_dir_all(&base).unwrap();
        fs::create_dir_all(&left).unwrap();
        fs::create_dir_all(&right).unwrap();
        write(&base, "t.txt", "a\nb\n");
        write(&left, "t.txt", "A\nb\n");
        write(&right, "t.txt", "a\nB\n");
        write(&right, "new.txt", "new");
        fs::create_dir_all(&out).unwrap();

        let b = crate::vfs::LocalVfs::new(&base).unwrap();
        let l = crate::vfs::LocalVfs::new(&left).unwrap();
        let r = crate::vfs::LocalVfs::new(&right).unwrap();
        let f = Filter::new(&[], &[]).unwrap();
        let (plan, _) = build_merge3_plan(&b, &l, &r, &f, true).unwrap();
        let o = crate::vfs::LocalVfs::new(&out).unwrap();
        let conflicts = execute_plan(&b, &l, &r, &o, &plan, Algorithm::Patience, false).unwrap();

        // 两侧不同行修改 → 无冲突，自动合并 A+B
        assert_eq!(conflicts, 0);
        let merged = fs::read_to_string(out.join("t.txt")).unwrap();
        assert_eq!(merged, "A\nB\n");
        // new.txt 被复制
        assert!(out.join("new.txt").exists());
    }

    #[test]
    fn execute_merge_same_line_conflict_emits_markers() {
        let d = tempdir().unwrap();
        let base = d.path().join("base");
        let left = d.path().join("left");
        let right = d.path().join("right");
        let out = d.path().join("out");
        fs::create_dir_all(&base).unwrap();
        fs::create_dir_all(&left).unwrap();
        fs::create_dir_all(&right).unwrap();
        write(&base, "t.txt", "x\n");
        write(&left, "t.txt", "L\n");
        write(&right, "t.txt", "R\n");
        fs::create_dir_all(&out).unwrap();

        let b = crate::vfs::LocalVfs::new(&base).unwrap();
        let l = crate::vfs::LocalVfs::new(&left).unwrap();
        let r = crate::vfs::LocalVfs::new(&right).unwrap();
        let f = Filter::new(&[], &[]).unwrap();
        let (plan, _) = build_merge3_plan(&b, &l, &r, &f, true).unwrap();
        let o = crate::vfs::LocalVfs::new(&out).unwrap();
        let conflicts = execute_plan(&b, &l, &r, &o, &plan, Algorithm::Patience, false).unwrap();

        assert_eq!(conflicts, 1);
        let merged = fs::read_to_string(out.join("t.txt")).unwrap();
        assert!(merged.contains("<<<<<<< LEFT"));
        assert!(merged.contains(">>>>>>> RIGHT"));
        assert!(merged.contains("L\n"));
        assert!(merged.contains("R\n"));
    }
}
