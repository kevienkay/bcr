use globset::{Glob, GlobSet, GlobSetBuilder};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use walkdir::WalkDir;

/// 文件元数据（快速比较用）
#[derive(Debug)]
pub struct FileMeta {
    pub size: u64,
    pub mtime: SystemTime,
}

/// glob 过滤规则：include 白名单 + exclude 黑名单
pub struct Filter {
    include: Option<GlobSet>,
    exclude: Option<GlobSet>,
}

impl Filter {
    pub fn new(includes: &[String], excludes: &[String]) -> Result<Self, globset::Error> {
        Ok(Filter {
            include: build_set(includes)?,
            exclude: build_set(excludes)?,
        })
    }

    /// 文件级过滤：include 白名单 + exclude 黑名单
    pub fn accept(&self, rel: &str) -> bool {
        if let Some(inc) = &self.include {
            if !inc.is_match(rel) {
                return false;
            }
        }
        !self.is_excluded(rel)
    }

    /// 目录级剪枝：仅检查 exclude（目录不参与 include 匹配）
    pub fn is_excluded(&self, rel: &str) -> bool {
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

/// 扫描目录树，返回 (相对路径 -> 元数据)，BTreeMap 保证有序
pub fn scan(root: &Path, filter: &Filter) -> io::Result<BTreeMap<String, FileMeta>> {
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
pub fn content_equal(left_dir: &Path, right_dir: &Path, rel: &str) -> io::Result<bool> {
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
