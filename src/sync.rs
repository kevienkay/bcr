use crate::fsscan::Filter;
use crate::i18n::{fmt, t, Key};
use crate::vfs::{self, Vfs};
use clap::Args;
use std::io;
use std::time::SystemTime;

/// sync 子命令参数
#[derive(Args, Debug)]
pub struct SyncArgs {
    /// 左侧目录
    pub left: String,

    /// 右侧目录
    pub right: String,

    /// 同步模式：update（单向复制新增/更新，不删除）| mirror（单向镜像，含删除）| two-way（双向，mtime 新者胜）
    #[arg(long, default_value = "update", value_parser = ["update", "mirror", "two-way"])]
    pub mode: String,

    /// 反转方向：默认 LEFT → RIGHT，加此选项变为 RIGHT → LEFT
    #[arg(long)]
    pub reverse: bool,

    /// 只输出计划，不执行任何操作
    #[arg(long)]
    pub dry_run: bool,

    /// 对大小相同的文件对做 blake3 哈希比对（默认仅比较大小+修改时间）
    #[arg(long)]
    pub compare_content: bool,

    /// 包含过滤（glob，可重复）
    #[arg(long = "include")]
    pub includes: Vec<String>,

    /// 排除过滤（glob，可重复）
    #[arg(long = "exclude")]
    pub excludes: Vec<String>,

    /// 输出统计信息
    #[arg(long)]
    pub summary: bool,
}

/// 同步计划项。from_src=true 表示从 src 复制到 dst，false 反之（仅 two-way 会出现）
enum Plan {
    Copy { rel: String, from_src: bool },
    Delete { rel: String },
    Skip { rel: String, reason: &'static str },
    Conflict { rel: String },
}

/// 运行 sync 子命令，返回进程退出码（0=成功，1=有冲突/有计划(dry-run)，2=错误）
pub fn run(args: &SyncArgs) -> i32 {
    // src/dst 已按 --reverse 归一：单向模式下所有写入都发生在 dst
    let (src, dst) = if args.reverse {
        (&args.right, &args.left)
    } else {
        (&args.left, &args.right)
    };

    let src_vfs = match vfs::open(src) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("bcr: {}", fmt(Key::OpenFailed, &[src, &e.to_string()]));
            return 2;
        }
    };
    let dst_vfs = match vfs::open(dst) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("bcr: {}", fmt(Key::OpenFailed, &[dst, &e.to_string()]));
            return 2;
        }
    };

    let filter = match Filter::new(&args.includes, &args.excludes) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("bcr: {}", fmt(Key::FilterError, &[&e.to_string()]));
            return 2;
        }
    };

    let src_map = match src_vfs.scan(&filter) {
        Ok(m) => m,
        Err(e) => {
            eprintln!(
                "bcr: {}",
                fmt(Key::ScanFailed, &[&format!("{}: {}", src, e)])
            );
            return 2;
        }
    };
    let dst_map = match dst_vfs.scan(&filter) {
        Ok(m) => m,
        Err(e) => {
            eprintln!(
                "bcr: {}",
                fmt(Key::ScanFailed, &[&format!("{}: {}", dst, e)])
            );
            return 2;
        }
    };

    let mode = args.mode.as_str();
    let mut plan: Vec<Plan> = Vec::new();

    // 合并 key 集合（已排序，保证输出顺序稳定）
    let mut keys: Vec<&String> = Vec::with_capacity(src_map.len() + dst_map.len());
    for k in src_map.keys() {
        keys.push(k);
    }
    for k in dst_map.keys() {
        if !src_map.contains_key(k) {
            keys.push(k);
        }
    }
    keys.sort();

    for key in keys {
        match (src_map.get(key), dst_map.get(key)) {
            (Some(s), Some(d)) => {
                let same = if s.size != d.size {
                    false
                } else if args.compare_content {
                    match vfs::content_equal_vfs(src_vfs.as_ref(), dst_vfs.as_ref(), key) {
                        Ok(eq) => eq,
                        Err(e) => {
                            eprintln!("bcr: {}", fmt(Key::ReadFailed, &[key, &e.to_string()]));
                            return 2;
                        }
                    }
                } else {
                    s.mtime == d.mtime
                };
                if same {
                    continue; // 已一致
                }
                match mode {
                    // 镜像：以源为准，无条件覆盖
                    "mirror" => plan.push(Plan::Copy {
                        rel: key.clone(),
                        from_src: true,
                    }),
                    // 更新：源新才覆盖，目标新则跳过
                    "update" => {
                        if s.mtime >= d.mtime {
                            plan.push(Plan::Copy {
                                rel: key.clone(),
                                from_src: true,
                            });
                        } else {
                            plan.push(Plan::Skip {
                                rel: key.clone(),
                                reason: t(Key::ReasonDstNewer),
                            });
                        }
                    }
                    // 双向：mtime 新者胜，无法判定则冲突
                    _ => {
                        if s.mtime > d.mtime {
                            plan.push(Plan::Copy {
                                rel: key.clone(),
                                from_src: true,
                            });
                        } else if d.mtime > s.mtime {
                            plan.push(Plan::Copy {
                                rel: key.clone(),
                                from_src: false,
                            });
                        } else {
                            plan.push(Plan::Conflict { rel: key.clone() });
                        }
                    }
                }
            }
            (Some(_), None) => plan.push(Plan::Copy {
                rel: key.clone(),
                from_src: true,
            }),
            (None, Some(_)) => match mode {
                "mirror" => plan.push(Plan::Delete { rel: key.clone() }),
                "update" => plan.push(Plan::Skip {
                    rel: key.clone(),
                    reason: t(Key::ReasonDstOnly),
                }),
                _ => plan.push(Plan::Copy {
                    rel: key.clone(),
                    from_src: false, // two-way：目标独有 → 复制回源
                }),
            },
            (None, None) => unreachable!(),
        }
    }

    // 输出并执行
    let mut n_copy = 0usize;
    let mut n_delete = 0usize;
    let mut n_skip = 0usize;
    let mut n_conflict = 0usize;
    let mut n_error = 0usize;

    for p in &plan {
        match p {
            Plan::Copy { rel, from_src } => {
                n_copy += 1;
                let (from, to) = if *from_src {
                    (src_vfs.as_ref(), dst_vfs.as_ref())
                } else {
                    (dst_vfs.as_ref(), src_vfs.as_ref())
                };
                println!("{}", fmt(Key::TagCopy, &[rel, &to.describe()]));
                if !args.dry_run {
                    if let Err(e) = do_copy_vfs(from, to, rel) {
                        eprintln!("bcr: {}", fmt(Key::CopyFailed, &[rel, &e.to_string()]));
                        n_error += 1;
                    }
                }
            }
            Plan::Delete { rel } => {
                n_delete += 1;
                println!("{}", fmt(Key::TagDelete, &[rel]));
                if !args.dry_run {
                    if let Err(e) = dst_vfs.delete(rel) {
                        eprintln!("bcr: {}", fmt(Key::DeleteFailed, &[rel, &e.to_string()]));
                        n_error += 1;
                    }
                }
            }
            Plan::Skip { rel, reason } => {
                n_skip += 1;
                println!("{}", fmt(Key::TagSkip, &[rel, reason]));
            }
            Plan::Conflict { rel } => {
                n_conflict += 1;
                println!("{}", fmt(Key::TagConflict, &[rel]));
            }
        }
    }

    if args.summary {
        println!(
            "{}",
            fmt(
                Key::SummarySync,
                &[
                    &n_copy.to_string(),
                    &n_delete.to_string(),
                    &n_skip.to_string(),
                    &n_conflict.to_string(),
                    &n_error.to_string(),
                ]
            )
        );
    }

    if n_error > 0 {
        2
    } else if n_conflict > 0 || (args.dry_run && n_copy + n_delete + n_conflict > 0) {
        1
    } else {
        0
    }
}

/// 跨后端复制并保留源 mtime（避免下次同步误判为过时）
fn do_copy_vfs(from: &dyn Vfs, to: &dyn Vfs, rel: &str) -> io::Result<()> {
    // 先读源 mtime
    let mtime = {
        let filter = Filter::new(&[], &[]).unwrap();
        let map = from.scan(&filter)?;
        map.get(rel)
            .map(|m| m.mtime)
            .unwrap_or(SystemTime::UNIX_EPOCH)
    };
    from.copy_to(rel, to)?;
    to.set_mtime(rel, mtime)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn args(left: &str, right: &str, mode: &str) -> SyncArgs {
        SyncArgs {
            left: left.into(),
            right: right.into(),
            mode: mode.into(),
            reverse: false,
            dry_run: false,
            compare_content: false,
            includes: vec![],
            excludes: vec![],
            summary: false,
        }
    }

    fn write_tree(dir: &std::path::Path, entries: &[(&str, &str, i64)]) {
        for (rel, content, mtime_secs) in entries {
            let p = dir.join(rel);
            if let Some(parent) = p.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(&p, content).unwrap();
            let t = filetime::FileTime::from_unix_time(*mtime_secs, 0);
            filetime::set_file_mtime(&p, t).unwrap();
        }
    }

    #[test]
    fn update_mode_copies_new_files() {
        let src = tempdir().unwrap();
        let dst = tempdir().unwrap();
        write_tree(src.path(), &[("new.txt", "hello", 1_700_000_000)]);
        let a = args(
            src.path().to_str().unwrap(),
            dst.path().to_str().unwrap(),
            "update",
        );
        assert_eq!(run(&a), 0);
        assert_eq!(
            fs::read_to_string(dst.path().join("new.txt")).unwrap(),
            "hello"
        );
    }

    #[test]
    fn update_mode_skips_newer_dst() {
        let src = tempdir().unwrap();
        let dst = tempdir().unwrap();
        // 源旧目标新 → update 应跳过
        write_tree(src.path(), &[("f.txt", "old", 1_700_000_000)]);
        write_tree(dst.path(), &[("f.txt", "newer", 1_700_000_100)]);
        let a = args(
            src.path().to_str().unwrap(),
            dst.path().to_str().unwrap(),
            "update",
        );
        assert_eq!(run(&a), 0);
        assert_eq!(
            fs::read_to_string(dst.path().join("f.txt")).unwrap(),
            "newer"
        );
    }

    #[test]
    fn update_mode_overwrites_when_src_newer() {
        let src = tempdir().unwrap();
        let dst = tempdir().unwrap();
        write_tree(src.path(), &[("f.txt", "newer", 1_700_000_100)]);
        write_tree(dst.path(), &[("f.txt", "old", 1_700_000_000)]);
        let a = args(
            src.path().to_str().unwrap(),
            dst.path().to_str().unwrap(),
            "update",
        );
        assert_eq!(run(&a), 0);
        assert_eq!(
            fs::read_to_string(dst.path().join("f.txt")).unwrap(),
            "newer"
        );
    }

    #[test]
    fn update_mode_keeps_dst_only_files() {
        let src = tempdir().unwrap();
        let dst = tempdir().unwrap();
        write_tree(dst.path(), &[("only.txt", "x", 1_700_000_000)]);
        let a = args(
            src.path().to_str().unwrap(),
            dst.path().to_str().unwrap(),
            "update",
        );
        assert_eq!(run(&a), 0);
        assert!(dst.path().join("only.txt").exists());
    }

    #[test]
    fn dry_run_does_not_write() {
        let src = tempdir().unwrap();
        let dst = tempdir().unwrap();
        write_tree(src.path(), &[("new.txt", "hello", 1_700_000_000)]);
        let mut a = args(
            src.path().to_str().unwrap(),
            dst.path().to_str().unwrap(),
            "update",
        );
        a.dry_run = true;
        assert_eq!(run(&a), 1); // 有计划 → 退出码 1
        assert!(!dst.path().join("new.txt").exists());
    }

    #[test]
    fn mirror_mode_deletes_dst_only_files() {
        let src = tempdir().unwrap();
        let dst = tempdir().unwrap();
        write_tree(dst.path(), &[("old.txt", "x", 1_700_000_000)]);
        let a = args(
            src.path().to_str().unwrap(),
            dst.path().to_str().unwrap(),
            "mirror",
        );
        assert_eq!(run(&a), 0);
        assert!(!dst.path().join("old.txt").exists());
    }

    #[test]
    fn mirror_mode_overwrites_regardless_of_mtime() {
        let src = tempdir().unwrap();
        let dst = tempdir().unwrap();
        // 目标更新也会被镜像覆盖
        write_tree(src.path(), &[("f.txt", "src", 1_700_000_000)]);
        write_tree(dst.path(), &[("f.txt", "dst", 1_700_000_100)]);
        let a = args(
            src.path().to_str().unwrap(),
            dst.path().to_str().unwrap(),
            "mirror",
        );
        assert_eq!(run(&a), 0);
        assert_eq!(fs::read_to_string(dst.path().join("f.txt")).unwrap(), "src");
    }

    #[test]
    fn two_way_syncs_both_sides() {
        let src = tempdir().unwrap();
        let dst = tempdir().unwrap();
        write_tree(src.path(), &[("l.txt", "L", 1_700_000_000)]);
        write_tree(dst.path(), &[("r.txt", "R", 1_700_000_000)]);
        let a = args(
            src.path().to_str().unwrap(),
            dst.path().to_str().unwrap(),
            "two-way",
        );
        assert_eq!(run(&a), 0);
        assert!(dst.path().join("l.txt").exists());
        assert!(src.path().join("r.txt").exists());
    }

    #[test]
    fn two_way_newer_wins() {
        let src = tempdir().unwrap();
        let dst = tempdir().unwrap();
        // 左侧新 → 覆盖右侧
        write_tree(src.path(), &[("both.txt", "L", 1_700_000_100)]);
        write_tree(dst.path(), &[("both.txt", "R", 1_700_000_000)]);
        let a = args(
            src.path().to_str().unwrap(),
            dst.path().to_str().unwrap(),
            "two-way",
        );
        assert_eq!(run(&a), 0);
        assert_eq!(
            fs::read_to_string(dst.path().join("both.txt")).unwrap(),
            "L"
        );
        // 右侧新 → 覆盖左侧
        write_tree(src.path(), &[("both2.txt", "L", 1_700_000_000)]);
        write_tree(dst.path(), &[("both2.txt", "R", 1_700_000_100)]);
        let b = args(
            src.path().to_str().unwrap(),
            dst.path().to_str().unwrap(),
            "two-way",
        );
        assert_eq!(run(&b), 0);
        assert_eq!(
            fs::read_to_string(src.path().join("both2.txt")).unwrap(),
            "R"
        );
    }

    #[test]
    fn two_way_conflict_on_same_mtime_different_size() {
        let src = tempdir().unwrap();
        let dst = tempdir().unwrap();
        // 快速模式下同大小同 mtime 会被判为一致；这里用不同大小触发冲突分支
        write_tree(src.path(), &[("f.txt", "LONGER", 1_700_000_000)]);
        write_tree(dst.path(), &[("f.txt", "R", 1_700_000_000)]);
        let a = args(
            src.path().to_str().unwrap(),
            dst.path().to_str().unwrap(),
            "two-way",
        );
        assert_eq!(run(&a), 1); // 冲突 → 退出码 1
    }

    #[test]
    fn two_way_conflict_compare_content_same_size() {
        let src = tempdir().unwrap();
        let dst = tempdir().unwrap();
        // 同大小同 mtime 但内容不同：快速模式判为一致，compare-content 模式判冲突
        write_tree(src.path(), &[("f.txt", "L", 1_700_000_000)]);
        write_tree(dst.path(), &[("f.txt", "R", 1_700_000_000)]);
        let mut a = args(
            src.path().to_str().unwrap(),
            dst.path().to_str().unwrap(),
            "two-way",
        );
        a.compare_content = true;
        assert_eq!(run(&a), 1);
    }

    #[test]
    fn two_way_compare_content_resolves_same_content() {
        let src = tempdir().unwrap();
        let dst = tempdir().unwrap();
        // mtime 相同但内容相同 → compare-content 模式判定一致，不冲突
        write_tree(src.path(), &[("f.txt", "same", 1_700_000_000)]);
        write_tree(dst.path(), &[("f.txt", "same", 1_700_000_000)]);
        let mut a = args(
            src.path().to_str().unwrap(),
            dst.path().to_str().unwrap(),
            "two-way",
        );
        a.compare_content = true;
        assert_eq!(run(&a), 0);
    }

    #[test]
    fn reverse_swaps_direction() {
        let src = tempdir().unwrap();
        let dst = tempdir().unwrap();
        // 反转后 dst 成为源：dst 独有的文件应被复制回 src
        write_tree(dst.path(), &[("only.txt", "x", 1_700_000_000)]);
        let mut a = args(
            src.path().to_str().unwrap(),
            dst.path().to_str().unwrap(),
            "update",
        );
        a.reverse = true;
        assert_eq!(run(&a), 0);
        assert!(src.path().join("only.txt").exists());
    }

    #[test]
    fn do_copy_vfs_creates_parents_and_preserves_mtime() {
        let dir = tempdir().unwrap();
        let from = dir.path().join("from.txt");
        fs::write(&from, "data").unwrap();
        let t = filetime::FileTime::from_unix_time(1_700_000_000, 0);
        filetime::set_file_mtime(&from, t).unwrap();
        let src = crate::vfs::LocalVfs::new(dir.path()).unwrap();
        let dst_dir = tempdir().unwrap();
        let dst = crate::vfs::LocalVfs::new(dst_dir.path()).unwrap();
        do_copy_vfs(&src, &dst, "from.txt").unwrap();
        let to = dst_dir.path().join("from.txt");
        assert_eq!(fs::read_to_string(&to).unwrap(), "data");
        let mtime = fs::metadata(&to).unwrap().modified().unwrap();
        let expected: std::time::SystemTime = t.into();
        assert_eq!(mtime, expected);
    }

    #[test]
    fn do_copy_vfs_missing_source_errors() {
        let dir = tempdir().unwrap();
        let src = crate::vfs::LocalVfs::new(dir.path()).unwrap();
        let dst_dir = tempdir().unwrap();
        let dst = crate::vfs::LocalVfs::new(dst_dir.path()).unwrap();
        assert!(do_copy_vfs(&src, &dst, "missing.txt").is_err());
    }
}
