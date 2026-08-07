use globset::{Glob, GlobSet, GlobSetBuilder};
use std::collections::BTreeMap;
use std::io;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use walkdir::WalkDir;

#[cfg(test)]
use std::fs::File;
#[cfg(test)]
use std::io::Read;

/// 文件元数据（快速比较用）
#[derive(Debug, Clone)]
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
///
/// 生产代码已改用 [`crate::vfs::content_equal_vfs`]，此函数仅保留给 fsscan 自身测试。
#[cfg(test)]
pub fn content_equal(left_dir: &Path, right_dir: &Path, rel: &str) -> io::Result<bool> {
    Ok(hash_file(&left_dir.join(rel))? == hash_file(&right_dir.join(rel))?)
}

#[cfg(test)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn filter(includes: &[&str], excludes: &[&str]) -> Filter {
        let inc: Vec<String> = includes.iter().map(|s| s.to_string()).collect();
        let exc: Vec<String> = excludes.iter().map(|s| s.to_string()).collect();
        Filter::new(&inc, &exc).unwrap()
    }

    #[test]
    fn filter_empty_accepts_all() {
        let f = filter(&[], &[]);
        assert!(f.accept("a.txt"));
        assert!(f.accept("sub/b.rs"));
        assert!(!f.is_excluded("anything"));
    }

    #[test]
    fn filter_include_whitelist() {
        let f = filter(&["*.rs"], &[]);
        assert!(f.accept("main.rs"));
        assert!(!f.accept("main.c"));
        assert!(!f.accept("sub/mod.txt"));
    }

    #[test]
    fn filter_exclude_blacklist() {
        let f = filter(&[], &["*.log", "target/**"]);
        assert!(!f.accept("x.log"));
        assert!(f.accept("x.txt"));
        assert!(!f.accept("target/debug/bcr"));
        // globset 的 dir/** 匹配目录本身时需要尾斜杠写法（scan 会两种都试）
        assert!(f.is_excluded("target/"));
        assert!(f.is_excluded("target/debug"));
    }

    #[test]
    fn filter_include_and_exclude_combined() {
        let f = filter(&["*.rs"], &["test/**"]);
        assert!(f.accept("src/main.rs"));
        assert!(!f.accept("test/mod.rs"));
    }

    #[test]
    fn filter_invalid_glob_errors() {
        let inc = vec!["[".to_string()]; // 非法 glob
        assert!(Filter::new(&inc, &[]).is_err());
    }

    #[test]
    fn scan_indexes_files_recursively() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("sub/deep")).unwrap();
        fs::write(dir.path().join("a.txt"), "aaa").unwrap();
        fs::write(dir.path().join("sub/b.txt"), "bb").unwrap();
        fs::write(dir.path().join("sub/deep/c.txt"), "c").unwrap();
        let f = filter(&[], &[]);
        let map = scan(dir.path(), &f).unwrap();
        assert_eq!(map.len(), 3);
        assert!(map.contains_key("a.txt"));
        assert!(map.contains_key("sub/b.txt"));
        assert!(map.contains_key("sub/deep/c.txt"));
        assert_eq!(map["a.txt"].size, 3);
        assert_eq!(map["sub/b.txt"].size, 2);
    }

    #[test]
    fn scan_respects_exclude_on_dir_and_file() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("skip")).unwrap();
        fs::write(dir.path().join("keep.txt"), "k").unwrap();
        fs::write(dir.path().join("skip/drop.txt"), "d").unwrap();
        fs::write(dir.path().join("skip.log"), "l").unwrap();
        let f = filter(&[], &["skip/**", "*.log"]);
        let map = scan(dir.path(), &f).unwrap();
        assert!(map.contains_key("keep.txt"));
        assert!(!map.contains_key("skip/drop.txt"));
        assert!(!map.contains_key("skip.log"));
    }

    #[test]
    fn scan_respects_include_whitelist() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("a.rs"), "1").unwrap();
        fs::write(dir.path().join("b.txt"), "2").unwrap();
        let f = filter(&["*.rs"], &[]);
        let map = scan(dir.path(), &f).unwrap();
        assert!(map.contains_key("a.rs"));
        assert!(!map.contains_key("b.txt"));
    }

    #[test]
    fn scan_empty_dir_returns_empty_map() {
        let dir = tempdir().unwrap();
        let f = filter(&[], &[]);
        assert!(scan(dir.path(), &f).unwrap().is_empty());
    }

    #[test]
    fn content_equal_same_content_true() {
        let d1 = tempdir().unwrap();
        let d2 = tempdir().unwrap();
        fs::write(d1.path().join("f.txt"), "hello").unwrap();
        fs::write(d2.path().join("f.txt"), "hello").unwrap();
        assert!(content_equal(d1.path(), d2.path(), "f.txt").unwrap());
    }

    #[test]
    fn content_equal_different_content_false() {
        let d1 = tempdir().unwrap();
        let d2 = tempdir().unwrap();
        fs::write(d1.path().join("f.txt"), "hello").unwrap();
        fs::write(d2.path().join("f.txt"), "world").unwrap();
        assert!(!content_equal(d1.path(), d2.path(), "f.txt").unwrap());
    }

    #[test]
    fn content_equal_large_file() {
        let d1 = tempdir().unwrap();
        let d2 = tempdir().unwrap();
        let big = "x".repeat(200_000); // 跨多个 64KB 缓冲块
        fs::write(d1.path().join("f.txt"), &big).unwrap();
        fs::write(d2.path().join("f.txt"), &big).unwrap();
        assert!(content_equal(d1.path(), d2.path(), "f.txt").unwrap());
    }

    #[test]
    fn content_equal_missing_file_errors() {
        let d1 = tempdir().unwrap();
        let d2 = tempdir().unwrap();
        assert!(content_equal(d1.path(), d2.path(), "nope.txt").is_err());
    }
}
