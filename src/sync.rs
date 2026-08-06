use crate::fsscan::{content_equal, scan, Filter};
use clap::Args;
use filetime::FileTime;
use std::fs;
use std::io;
use std::path::Path;

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
    let src_dir = Path::new(src);
    let dst_dir = Path::new(dst);
    if !src_dir.is_dir() {
        eprintln!("bcr: 不是目录: {}", src);
        return 2;
    }
    if !dst_dir.is_dir() {
        eprintln!("bcr: 不是目录: {}", dst);
        return 2;
    }

    let filter = match Filter::new(&args.includes, &args.excludes) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("bcr: 过滤规则错误: {e}");
            return 2;
        }
    };

    let src_map = match scan(src_dir, &filter) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("bcr: 扫描 {} 失败: {e}", src);
            return 2;
        }
    };
    let dst_map = match scan(dst_dir, &filter) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("bcr: 扫描 {} 失败: {e}", dst);
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
                    match content_equal(src_dir, dst_dir, key) {
                        Ok(eq) => eq,
                        Err(e) => {
                            eprintln!("bcr: 读取 {} 失败: {e}", key);
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
                                reason: "目标侧较新",
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
                    reason: "仅存在于目标侧",
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
                    (src_dir, dst_dir)
                } else {
                    (dst_dir, src_dir)
                };
                println!("[COPY]   {rel} -> {}", to.display());
                if !args.dry_run {
                    if let Err(e) = do_copy(&from.join(rel), &to.join(rel)) {
                        eprintln!("bcr: 复制 {rel} 失败: {e}");
                        n_error += 1;
                    }
                }
            }
            Plan::Delete { rel } => {
                n_delete += 1;
                println!("[DELETE] {rel}");
                if !args.dry_run {
                    if let Err(e) = fs::remove_file(dst_dir.join(rel)) {
                        eprintln!("bcr: 删除 {rel} 失败: {e}");
                        n_error += 1;
                    }
                }
            }
            Plan::Skip { rel, reason } => {
                n_skip += 1;
                println!("[SKIP]   {rel} ({reason})");
            }
            Plan::Conflict { rel } => {
                n_conflict += 1;
                println!("[CONFLICT] {rel} (两侧同时修改且无法判定新者，跳过)");
            }
        }
    }

    if args.summary {
        println!(
            "统计: {} 复制, {} 删除, {} 跳过, {} 冲突, {} 错误",
            n_copy, n_delete, n_skip, n_conflict, n_error
        );
    }

    if n_error > 0 {
        2
    } else if n_conflict > 0 {
        1
    } else if args.dry_run && n_copy + n_delete + n_conflict > 0 {
        1
    } else {
        0
    }
}

/// 复制文件并保留源 mtime（避免下次同步误判为过时）
fn do_copy(from: &Path, to: &Path) -> io::Result<()> {
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(from, to)?;
    let mtime = fs::metadata(from)?.modified()?;
    filetime::set_file_mtime(to, FileTime::from_system_time(mtime))?;
    Ok(())
}
