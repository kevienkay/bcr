//! ZIP 虚拟文件系统（只读）。
//!
//! 把 .zip 压缩包当作一个目录树：`zip://path/to/archive.zip`。
//! scan 列出全部文件条目（目录项过滤掉），read 按条目名读取。
//! mtime 取 zip 条目时间；写入/删除不支持（只读后端）。

use super::Vfs;
use crate::fsscan::{FileMeta, Filter};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io::{self, Read, Write};
use std::time::{SystemTime, UNIX_EPOCH};
use zip::ZipArchive;

pub struct ZipVfs {
    /// 压缩包路径（仅用于描述）
    path: String,
    /// by_name 需要 &mut，用 RefCell 提供内部可变性；
    /// 写入/删除时 take 出来释放文件句柄，重写完成后再装回（Windows 上替换文件需要先关闭）
    archive: RefCell<Option<ZipArchive<std::fs::File>>>,
    /// 条目名 -> (大小, mtime)
    index: RefCell<BTreeMap<String, (u64, Option<SystemTime>)>>,
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
            archive: RefCell::new(Some(archive)),
            index: RefCell::new(index),
        })
    }

    /// 重写整个 zip：复制旧条目（跳过 skip），再执行 write_fn 写新条目，原子替换原文件并重建索引。
    /// 通过 take 释放旧文件句柄，保证 Windows 上可以替换被占用的文件。
    fn rewrite(
        &self,
        skip: Option<&str>,
        write_fn: impl FnOnce(&mut zip::ZipWriter<std::fs::File>) -> io::Result<()>,
    ) -> io::Result<()> {
        // 1. 释放旧句柄并取出旧条目列表
        let mut old = self
            .archive
            .borrow_mut()
            .take()
            .ok_or_else(|| io::Error::other("zip 后端未初始化"))?;
        let mut old_entries: Vec<(String, bool)> = Vec::new(); // (name, is_dir)
        for i in 0..old.len() {
            let e = old.by_index(i).map_err(|e| {
                io::Error::new(io::ErrorKind::InvalidData, format!("ZIP 条目失败: {e}"))
            })?;
            old_entries.push((e.name().to_string(), e.is_dir()));
        }

        // 2. 写临时文件
        let tmp_path = format!("{}.bcr-tmp{}", self.path, std::process::id());
        let result = (|| -> io::Result<()> {
            let out = std::fs::File::create(&tmp_path)?;
            let mut writer = zip::ZipWriter::new(out);
            // 复制旧条目（跳过目标条目）
            for (name, is_dir) in &old_entries {
                if skip.is_some() && skip == Some(name.as_str()) {
                    continue;
                }
                if *is_dir {
                    // 目录条目：仅当无同名文件时重建
                    if !old_entries.iter().any(|(n, d)| !d && n == name) {
                        let idx = old_entries.iter().position(|(n, _)| n == name).unwrap();
                        if let Ok(e) = old.by_index(idx) {
                            let _ = writer.raw_copy_file(e);
                        }
                    }
                    continue;
                }
                let idx = old_entries.iter().position(|(n, _)| n == name).unwrap();
                let e = old.by_index(idx).map_err(|e| {
                    io::Error::new(io::ErrorKind::InvalidData, format!("ZIP 条目失败: {e}"))
                })?;
                writer
                    .raw_copy_file(e)
                    .map_err(|e| io::Error::other(format!("ZIP 复制失败: {e}")))?;
            }
            // 写新条目
            write_fn(&mut writer)?;
            writer
                .finish()
                .map_err(|e| io::Error::other(format!("ZIP 写入失败: {e}")))?;
            // 3. 原子替换
            std::fs::rename(&tmp_path, &self.path)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&tmp_path);
        }
        // 4. 重建（无论成功失败都重新打开，失败时恢复原状）
        let reopened = Self::open(&self.path);
        match reopened {
            Ok(v) => {
                *self.archive.borrow_mut() = Some(v.archive.borrow_mut().take().unwrap());
                *self.index.borrow_mut() = std::mem::take(&mut *v.index.borrow_mut());
                result
            }
            Err(e) => {
                // 重写失败且重建也失败：保持空态，返回原始错误
                if result.is_err() {
                    result
                } else {
                    Err(e)
                }
            }
        }
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

/// 把 SystemTime 转回 zip DateTime（zip 8 无公共转换，手动按字段构造，精度秒）
fn systemtime_to_zip_dt(t: SystemTime) -> zip::DateTime {
    let secs = t.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    let days = secs / 86400;
    let rem = secs % 86400;
    let (hour, minute, second) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    // days-from-civil 逆运算（Howard Hinnant）
    let z = days as i64 + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    zip::DateTime::from_date_and_time(
        year as u16,
        m as u8,
        d as u8,
        hour as u8,
        minute as u8,
        second as u8,
    )
    .unwrap_or_default()
}

impl Vfs for ZipVfs {
    fn describe(&self) -> String {
        format!("zip://{}", self.path)
    }

    fn scan(&self, filter: &Filter) -> io::Result<BTreeMap<String, FileMeta>> {
        let index = self.index.borrow();
        let mut map = BTreeMap::new();
        for (rel, (size, mtime)) in index.iter() {
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
        let mut guard = self.archive.borrow_mut();
        let archive = guard
            .as_mut()
            .ok_or_else(|| io::Error::other("zip 后端未初始化"))?;
        let mut entry = archive.by_name(rel).map_err(|e| {
            io::Error::new(io::ErrorKind::NotFound, format!("ZIP 读取 {rel} 失败: {e}"))
        })?;
        let mut buf = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut buf)?;
        Ok(buf)
    }

    fn hash(&self, rel: &str) -> io::Result<blake3::Hash> {
        let mut guard = self.archive.borrow_mut();
        let archive = guard
            .as_mut()
            .ok_or_else(|| io::Error::other("zip 后端未初始化"))?;
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
        Ok(self.index.borrow().contains_key(rel))
    }

    fn write(&self, rel: &str, data: &[u8]) -> io::Result<()> {
        let rel = rel.to_string();
        let data = data.to_vec();
        self.rewrite(Some(&rel), |writer| {
            let opts = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated)
                .last_modified_time(zip::DateTime::default());
            writer
                .start_file(&rel, opts)
                .map_err(|e| io::Error::other(format!("ZIP 写入 {rel} 失败: {e}")))?;
            writer
                .write_all(&data)
                .map_err(|e| io::Error::other(format!("ZIP 写入 {rel} 失败: {e}")))?;
            Ok(())
        })
    }

    fn delete(&self, rel: &str) -> io::Result<()> {
        if !self.exists(rel)? {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("ZIP 删除 {rel}: 不存在"),
            ));
        }
        let rel = rel.to_string();
        self.rewrite(Some(&rel), |_| Ok(()))
    }

    fn remove_dir(&self, _rel: &str) -> io::Result<()> {
        // ZIP 没有显式目录树，目录是隐式的；无需清理
        Ok(())
    }

    fn rename(&self, from: &str, to: &str) -> io::Result<()> {
        if from == to {
            return Ok(());
        }
        let data = self.read(from)?;
        self.write(to, &data)?;
        self.delete(from)
    }

    fn set_mtime(&self, rel: &str, t: SystemTime) -> io::Result<()> {
        let rel = rel.to_string();
        // 读取原内容，重写该条目并设置新时间
        let data = self.read(&rel)?;
        let dt = systemtime_to_zip_dt(t);
        self.rewrite(Some(&rel), |writer| {
            let opts = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated)
                .last_modified_time(dt);
            writer
                .start_file(&rel, opts)
                .map_err(|e| io::Error::other(format!("ZIP 写时间 {rel} 失败: {e}")))?;
            writer
                .write_all(&data)
                .map_err(|e| io::Error::other(format!("ZIP 写时间 {rel} 失败: {e}")))?;
            Ok(())
        })
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
    fn write_delete_mtime_roundtrip() {
        let d = tempdir().unwrap();
        let zp = d.path().join("t.zip");
        make_zip(&zp);
        let v = ZipVfs::open(zp.to_str().unwrap()).unwrap();
        // 新增条目
        v.write("new.txt", b"new-content").unwrap();
        assert_eq!(v.read("new.txt").unwrap(), b"new-content");
        // 覆盖已有条目
        v.write("a.txt", b"updated").unwrap();
        assert_eq!(v.read("a.txt").unwrap(), b"updated");
        // 删除条目
        v.delete("sub/b.txt").unwrap();
        assert!(!v.exists("sub/b.txt").unwrap());
        assert!(v.exists("sub/deep/c.txt").unwrap());
        assert!(v.exists("a.txt").unwrap());
        // 设置 mtime
        let t = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
        v.set_mtime("a.txt", t).unwrap();
        let f = Filter::new(&[], &[]).unwrap();
        let map = v.scan(&f).unwrap();
        let a_mtime = map["a.txt"].mtime;
        // zip 秒级精度，误差 < 2s
        let diff = a_mtime
            .duration_since(t)
            .unwrap_or_else(|_| t.duration_since(a_mtime).unwrap())
            .as_secs();
        assert!(diff <= 2, "mtime 偏差 {diff}s");
    }

    #[test]
    fn write_delete_rejected_readonly() {
        // 旧用例：写/删现在支持，改为验证行为
        let d = tempdir().unwrap();
        let zp = d.path().join("t.zip");
        make_zip(&zp);
        let v = ZipVfs::open(zp.to_str().unwrap()).unwrap();
        v.write("x.txt", b"x").unwrap();
        v.delete("a.txt").unwrap();
        v.set_mtime("x.txt", UNIX_EPOCH).unwrap();
    }

    #[test]
    fn zip_cross_backend_copy_to() {
        let d = tempdir().unwrap();
        let zp = d.path().join("t.zip");
        make_zip(&zp);
        let v = ZipVfs::open(zp.to_str().unwrap()).unwrap();
        // 复制到本地目录
        let out = tempdir().unwrap();
        let local = crate::vfs::LocalVfs::new(out.path()).unwrap();
        v.copy_to("a.txt", &local).unwrap();
        assert_eq!(
            fs::read_to_string(out.path().join("a.txt")).unwrap(),
            "hello"
        );
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
