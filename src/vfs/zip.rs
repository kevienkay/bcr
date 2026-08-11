//! ZIP 虚拟文件系统（只读）。
//!
//! 把 .zip 压缩包当作一个目录树：`zip://path/to/archive.zip`。
//! scan 列出全部文件条目（目录项过滤掉），read 按条目名读取。
//! mtime 取 zip 条目时间；写入/删除不支持（只读后端）。

use super::Vfs;
use crate::fsscan::{FileMeta, Filter};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io::{self, Read};
use std::time::{SystemTime, UNIX_EPOCH};
use zip::ZipArchive;

pub struct ZipVfs {
    /// 压缩包路径（仅用于描述）
    path: String,
    /// by_name 需要 &mut，用 RefCell 提供内部可变性
    archive: RefCell<ZipArchive<std::fs::File>>,
    /// 条目名 -> (大小, mtime)
    index: BTreeMap<String, (u64, Option<SystemTime>)>,
}

impl ZipVfs {
    /// 打开 zip 文件并建立条目索引
    pub fn open(path: &str) -> io::Result<Self> {
        let file = std::fs::File::open(path)?;
        let mut archive = ZipArchive::new(file).map_err(|e| {
            io::Error::new(io::ErrorKind::InvalidData, format!("ZIP 解析失败: {e}"))
        })?;

        let mut index = BTreeMap::new();
        for i in 0..archive.len() {
            let entry = archive.by_index(i).map_err(|e| {
                io::Error::new(io::ErrorKind::InvalidData, format!("ZIP 条目失败: {e}"))
            })?;
            if entry.is_dir() {
                continue;
            }
            let name = entry.name().trim_start_matches('/').to_string();
            if name.is_empty() {
                continue;
            }
            let mtime = entry.last_modified().map(zip_dt_to_systemtime);
            index.insert(name, (entry.size(), mtime));
        }

        Ok(ZipVfs {
            path: path.to_string(),
            archive: RefCell::new(archive),
            index,
        })
    }
}

/// 把 zip DateTime 转为 SystemTime（zip 8 未提供公共转换，手动按字段构造）
fn zip_dt_to_systemtime(dt: zip::DateTime) -> SystemTime {
    let year = dt.year() as i64;
    let month = dt.month() as i64;
    let day = dt.day() as i64;
    let hour = dt.hour() as i64;
    let minute = dt.minute() as i64;
    let second = dt.second() as i64;
    // 使用简单的 days-from-civil 算法（Howard Hinnant）
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400; // [0, 399]
    let mp = (month + 9) % 12; // [0, 11]
    let doy = (153 * mp + 2) / 5 + day - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    let days = era * 146097 + doe - 719468; // days since 1970-01-01
    let secs = days * 86400 + hour * 3600 + minute * 60 + second;
    if secs >= 0 {
        UNIX_EPOCH + std::time::Duration::from_secs(secs as u64)
    } else {
        UNIX_EPOCH
    }
}

impl Vfs for ZipVfs {
    fn describe(&self) -> String {
        format!("zip://{}", self.path)
    }

    fn scan(&self, filter: &Filter) -> io::Result<BTreeMap<String, FileMeta>> {
        let mut map = BTreeMap::new();
        for (rel, (size, mtime)) in &self.index {
            if !filter.accept(rel) {
                continue;
            }
            map.insert(
                rel.clone(),
                FileMeta {
                    size: *size,
                    mtime: mtime.unwrap_or(UNIX_EPOCH),
                },
            );
        }
        Ok(map)
    }

    fn read(&self, rel: &str) -> io::Result<Vec<u8>> {
        let mut archive = self.archive.borrow_mut();
        let mut entry = archive.by_name(rel).map_err(|e| {
            io::Error::new(io::ErrorKind::NotFound, format!("ZIP 读取 {rel} 失败: {e}"))
        })?;
        let mut buf = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut buf)?;
        Ok(buf)
    }

    fn hash(&self, rel: &str) -> io::Result<blake3::Hash> {
        let mut archive = self.archive.borrow_mut();
        let mut entry = archive.by_name(rel).map_err(|e| {
            io::Error::new(io::ErrorKind::NotFound, format!("ZIP 读取 {rel} 失败: {e}"))
        })?;
        let mut hasher = blake3::Hasher::new();
        let mut buf = [0u8; 65536];
        loop {
            let n = entry.read(&mut buf)?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }
        Ok(hasher.finalize())
    }

    fn exists(&self, rel: &str) -> io::Result<bool> {
        Ok(self.index.contains_key(rel))
    }

    fn write(&self, _rel: &str, _data: &[u8]) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "ZIP 为只读后端，不支持写入",
        ))
    }

    fn delete(&self, _rel: &str) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "ZIP 为只读后端，不支持删除",
        ))
    }

    fn remove_dir(&self, _rel: &str) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "ZIP 为只读后端，不支持删除目录",
        ))
    }

    fn rename(&self, _from: &str, _to: &str) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "ZIP 为只读后端，不支持重命名",
        ))
    }

    fn set_mtime(&self, _rel: &str, _t: SystemTime) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "ZIP 为只读后端，不支持修改时间",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use std::path::Path;
    use tempfile::tempdir;
    use zip::write::SimpleFileOptions;

    /// 创建一个测试 zip：a.txt、sub/b.txt、sub/deep/c.txt（跳过空目录）
    fn make_zip(path: &Path) {
        let file = fs::File::create(path).unwrap();
        let mut w = zip::ZipWriter::new(file);
        let opts = SimpleFileOptions::default();
        for (name, content) in [
            ("a.txt", "hello"),
            ("sub/b.txt", "world"),
            ("sub/deep/c.txt", "deep"),
        ] {
            w.start_file(name, opts).unwrap();
            w.write_all(content.as_bytes()).unwrap();
        }
        w.finish().unwrap();
    }

    #[test]
    fn scan_lists_entries_with_meta() {
        let d = tempdir().unwrap();
        let zp = d.path().join("t.zip");
        make_zip(&zp);
        let v = ZipVfs::open(zp.to_str().unwrap()).unwrap();
        let f = Filter::new(&[], &[]).unwrap();
        let map = v.scan(&f).unwrap();
        assert_eq!(map.len(), 3);
        assert!(map.contains_key("a.txt"));
        assert!(map.contains_key("sub/b.txt"));
        assert!(map.contains_key("sub/deep/c.txt"));
        assert_eq!(map["a.txt"].size, 5);
        assert_eq!(map["sub/b.txt"].size, 5);
        assert_eq!(map["sub/deep/c.txt"].size, 4);
        // mtime 已填充（1970 之后）
        assert!(map["a.txt"].mtime > UNIX_EPOCH);
    }

    #[test]
    fn scan_respects_filter() {
        let d = tempdir().unwrap();
        let zp = d.path().join("t.zip");
        make_zip(&zp);
        let v = ZipVfs::open(zp.to_str().unwrap()).unwrap();
        let f = Filter::new(&["*.txt".to_string()], &[]).unwrap();
        let map = v.scan(&f).unwrap();
        assert_eq!(map.len(), 3);
        let f2 = Filter::new(&[], &["sub/**".to_string()]).unwrap();
        let map2 = v.scan(&f2).unwrap();
        assert_eq!(map2.len(), 1);
        assert!(map2.contains_key("a.txt"));
    }

    #[test]
    fn read_returns_content() {
        let d = tempdir().unwrap();
        let zp = d.path().join("t.zip");
        make_zip(&zp);
        let v = ZipVfs::open(zp.to_str().unwrap()).unwrap();
        assert_eq!(v.read("a.txt").unwrap(), b"hello");
        assert_eq!(v.read("sub/deep/c.txt").unwrap(), b"deep");
        assert!(v.exists("a.txt").unwrap());
        assert!(!v.exists("nope.txt").unwrap());
        assert!(v.read("nope.txt").is_err());
    }

    #[test]
    fn write_delete_rejected_readonly() {
        let d = tempdir().unwrap();
        let zp = d.path().join("t.zip");
        make_zip(&zp);
        let v = ZipVfs::open(zp.to_str().unwrap()).unwrap();
        assert!(v.write("x.txt", b"x").is_err());
        assert!(v.delete("a.txt").is_err());
        assert!(v.set_mtime("a.txt", UNIX_EPOCH).is_err());
    }

    #[test]
    fn open_invalid_zip_errors() {
        let d = tempdir().unwrap();
        let zp = d.path().join("bad.zip");
        fs::write(&zp, "not a zip").unwrap();
        assert!(ZipVfs::open(zp.to_str().unwrap()).is_err());
    }

    #[test]
    fn zip_dt_conversion_basic() {
        // 2020-01-02 03:04:05
        let dt = zip::DateTime::from_date_and_time(2020, 1, 2, 3, 4, 5).unwrap();
        let t = zip_dt_to_systemtime(dt);
        let secs = t.duration_since(UNIX_EPOCH).unwrap().as_secs();
        // 2020-01-02 03:04:05 UTC ≈ 1577927045
        assert!(secs > 1_577_000_000 && secs < 1_578_000_000, "secs={secs}");
    }
}
