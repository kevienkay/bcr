use clap::Args;
use globset::{Glob, GlobSet, GlobSetBuilder};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{self, IsTerminal, Read};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use walkdir::WalkDir;

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

#[derive(Debug)]
struct FileMeta {
    size: u64,
    mtime: SystemTime,
}

struct Filter {
    include: Option<GlobSet>,
    exclude: Option<GlobSet>,
}

impl Filter {
    fn new(includes: &[String], excludes: &[String]) -> Result<Self, globset::Error> {
        Ok(Filter {
            include: build_set(includes)?,
            exclude: build_set(excludes)?,
        })
    }

    /// 文件级过滤：include 白名单 + exclude 黑名单
    fn accept(&self, rel: &str) -> bool {
        if let Some(inc) = &self.include {
            if !inc.is_match(rel) {
                return false;
            }
        }
        !self.is_excluded(rel)
    }

    /// 目录级剪枝：仅检查 exclude（目录不参与 include 匹配）
    fn is_excluded(&self, rel: &str) -> bool {
        self.exclude
            .as_ref()
            .map(|s| s.is_match(rel))
            .unwrap_or(false)
    }
}

fn build_set(patterns: &[String]) -> Result<Option<GlobSet>, globset::Error> {
    if patterns.is_empty() {
        return Ok(None);
    }
    let mut b = GlobSetBuilder::new();
    for p in patterns {
        b.add(Glob::new(p)?);
    }
    Ok(Some(b.build()?))
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

/// 扫描目录树，返回 (相对路径 -> 元数据)，BTreeMap 保证有序
fn scan(root: &Path, filter: &Filter) -> io::Result<BTreeMap<String, FileMeta>> {
    let mut map = BTreeMap::new();
    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            let rel = match e.path().strip_prefix(root) {
                Ok(r) => r,
                Err(_) => return true,
            };
            if rel.as_os_str().is_empty() {
                return true; // 根目录
            }
            if e.file_type().is_dir() {
                let s = rel.to_string_lossy();
                // 两种写法都试，兼容 "sub" 与 "sub/" 模式
                return !(filter.is_excluded(&s) || filter.is_excluded(&format!("{s}/")));
            }
            true
        })
    {
        let entry = match entry {
            Ok(e) => e,
            Err(err) => {
                eprintln!("bcr: 扫描警告: {err}");
                continue;
            }
        };
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = match entry.path().strip_prefix(root) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let rel_str = rel.to_string_lossy().into_owned();
        if !filter.accept(&rel_str) {
            continue;
        }
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(err) => {
                eprintln!("bcr: 元数据读取失败 {rel_str}: {err}");
                continue;
            }
        };
        map.insert(
            rel_str,
            FileMeta {
                size: meta.len(),
                mtime: meta.modified().unwrap_or(UNIX_EPOCH),
            },
        );
    }
    Ok(map)
}

/// 对两侧同路径文件做 blake3 哈希比对
fn content_equal(left_dir: &Path, right_dir: &Path, rel: &str) -> io::Result<bool> {
    Ok(hash_file(&left_dir.join(rel))? == hash_file(&right_dir.join(rel))?)
}

fn hash_file(path: &Path) -> io::Result<blake3::Hash> {
    let mut f = File::open(path)?;
    let mut hasher = blake3::Hasher::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize())
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
