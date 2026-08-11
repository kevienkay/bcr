//! 归档虚拟文件系统（tar 系可读写，7z 只读）。
//!
//! 把压缩包当作目录树：
//! - `tar://path/to/x.tar`、`tar://x.tar.gz`、`tar://x.tar.bz2`、`tar://x.tar.xz`
//! - `7z://path/to/x.7z`
//!
//! 打开时全量解压进内存（与 zip 的惰性读取不同；tar 是流式格式无法随机读，
//! 7z 解压同样一次性完成）。超大归档请用 zip 或本地目录。
//!
//! 写支持：tar / tar.gz / tar.xz 支持 write/delete/rename/set_mtime（全量重写）；
//! tar.bz2 无纯 Rust 编码器、7z 写入复杂，保持只读。

use super::Vfs;
use crate::fsscan::{FileMeta, Filter};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io::{self, BufReader, Cursor, Read, Write};
use std::time::{SystemTime, UNIX_EPOCH};

/// 归档内文件表：相对路径 -> (内容, 大小, mtime)
type ArchiveFiles = BTreeMap<String, (Vec<u8>, u64, SystemTime)>;

/// 归档虚拟文件系统（全量解压进内存）
pub struct ArchiveVfs {
    /// 归档路径（仅用于描述）
    path: String,
    /// 格式名（tar.gz / tar.bz2 / tar.xz / 7z …）
    kind: String,
    /// 相对路径 -> (内容, 大小, mtime)；RefCell 供写操作修改
    files: RefCell<ArchiveFiles>,
}

impl ArchiveVfs {
    /// 打开归档。`spec` 为 `tar://...` 或 `7z://...` 去掉前缀后的路径。
    pub fn open(path: &str) -> io::Result<Self> {
        let lower = path.to_ascii_lowercase();
        let data = std::fs::read(path)?;

        // 按扩展名分派：tar 系走 tar 解包，7z 走 sevenz
        if lower.ends_with(".7z") {
            let files = read_sevenz(path)?;
            return Ok(ArchiveVfs {
                path: path.to_string(),
                kind: "7z".to_string(),
                files: RefCell::new(files),
            });
        }
        // tar 系（含压缩变体）
        let kind = if lower.ends_with(".tar.gz") || lower.ends_with(".tgz") {
            "tar.gz"
        } else if lower.ends_with(".tar.bz2") || lower.ends_with(".tbz2") {
            "tar.bz2"
        } else if lower.ends_with(".tar.xz") || lower.ends_with(".txz") {
            "tar.xz"
        } else if lower.ends_with(".tar") {
            "tar"
        } else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "不支持的归档格式（支持 .tar/.tar.gz/.tar.bz2/.tar.xz/.7z）: {}",
                    path
                ),
            ));
        };

        // 解压到 tar 字节流
        let tar_bytes: Vec<u8> = match kind {
            "tar" => data,
            "tar.gz" => {
                let mut out = Vec::new();
                flate2::read::GzDecoder::new(data.as_slice()).read_to_end(&mut out)?;
                out
            }
            "tar.bz2" => {
                let mut out = Vec::new();
                bzip2_rs::DecoderReader::new(data.as_slice()).read_to_end(&mut out)?;
                out
            }
            "tar.xz" => {
                let mut out = Vec::new();
                lzma_rs::xz_decompress(&mut BufReader::new(data.as_slice()), &mut out).map_err(
                    |e| io::Error::new(io::ErrorKind::InvalidData, format!("xz 解压失败: {e}")),
                )?;
                out
            }
            _ => unreachable!(),
        };
        let files = read_tar(&tar_bytes)?;
        Ok(ArchiveVfs {
            path: path.to_string(),
            kind: kind.to_string(),
            files: RefCell::new(files),
        })
    }

    /// 是否支持写入（tar / tar.gz / tar.xz 可重写；tar.bz2 无纯 Rust 编码器、7z 只读）
    fn writable(&self) -> bool {
        matches!(self.kind.as_str(), "tar" | "tar.gz" | "tar.xz")
    }

    /// 全量重写：修改内存表 → 编码回 tar → 按 kind 压缩 → 原子替换文件
    fn rewrite(&self, modify: impl FnOnce(&mut ArchiveFiles)) -> io::Result<()> {
        if !self.writable() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("{} 归档为只读后端（仅 tar/tar.gz/tar.xz 可写）", self.kind),
            ));
        }
        {
            let mut files = self.files.borrow_mut();
            modify(&mut files);
        }
        // 编码 tar 字节流
        let tar_bytes = encode_tar(&self.files.borrow())?;
        // 按 kind 压缩
        let out: Vec<u8> = match self.kind.as_str() {
            "tar" => tar_bytes,
            "tar.gz" => {
                let mut out = Vec::new();
                let mut enc =
                    flate2::write::GzEncoder::new(&mut out, flate2::Compression::default());
                enc.write_all(&tar_bytes)?;
                enc.finish()?;
                out
            }
            "tar.xz" => {
                let mut out = Vec::new();
                lzma_rs::xz_compress(&mut BufReader::new(tar_bytes.as_slice()), &mut out)
                    .map_err(|e| io::Error::other(format!("xz 压缩失败: {e}")))?;
                out
            }
            _ => unreachable!(),
        };
        // 原子替换
        let tmp = format!("{}.bcr-tmp{}", self.path, std::process::id());
        let result = (|| -> io::Result<()> {
            std::fs::write(&tmp, &out)?;
            std::fs::rename(&tmp, &self.path)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
        result
    }
}

/// 从内存文件表编码 tar 字节流（保留 mtime；目录项隐式）
fn encode_tar(files: &ArchiveFiles) -> io::Result<Vec<u8>> {
    let mut buf = Vec::new();
    {
        let mut builder = tar::Builder::new(&mut buf);
        for (rel, (data, _, mtime)) in files {
            let mut header = tar::Header::new_gnu();
            header.set_size(data.len() as u64);
            header.set_mode(0o644);
            let secs = mtime
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            header.set_mtime(secs);
            header.set_cksum();
            builder
                .append_data(&mut header, rel, data.as_slice())
                .map_err(|e| io::Error::other(format!("tar 编码失败: {e}")))?;
        }
        builder
            .finish()
            .map_err(|e| io::Error::other(format!("tar 收尾失败: {e}")))?;
    }
    Ok(buf)
}

/// 从 tar 字节流读取全部文件条目（跳过目录/链接/设备）
fn read_tar(data: &[u8]) -> io::Result<ArchiveFiles> {
    let mut ar = tar::Archive::new(Cursor::new(data));
    let mut files = BTreeMap::new();
    let entries = ar
        .entries()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("tar 解析失败: {e}")))?;
    for entry in entries {
        let mut entry = entry.map_err(|e| {
            io::Error::new(io::ErrorKind::InvalidData, format!("tar 条目失败: {e}"))
        })?;
        if !entry.header().entry_type().is_file() {
            continue;
        }
        let path = entry.path().map_err(|e| {
            io::Error::new(io::ErrorKind::InvalidData, format!("tar 路径失败: {e}"))
        })?;
        // 规范化：去 ./ 前缀与结尾斜杠
        let rel = normalize_rel(&path.to_string_lossy());
        if rel.is_empty() {
            continue;
        }
        let mtime = entry.header().mtime().ok().map(secs_to_systemtime);
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf)?;
        let size = buf.len() as u64;
        files.insert(rel, (buf, size, mtime.unwrap_or(UNIX_EPOCH)));
    }
    Ok(files)
}

/// 读取 7z 全部文件条目
fn read_sevenz(path: &str) -> io::Result<ArchiveFiles> {
    let mut reader = sevenz_rust2::SevenZReader::open(path, sevenz_rust2::Password::empty())
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("7z 打开失败: {e}")))?;
    let mut files = BTreeMap::new();
    let mut err: Option<io::Error> = None;
    let r = reader.for_each_entries(|entry, r| {
        if entry.is_directory() || !entry.has_stream() {
            return Ok(true);
        }
        let rel = normalize_rel(entry.name());
        if rel.is_empty() {
            return Ok(true);
        }
        let mut buf = Vec::new();
        if let Err(e) = r.read_to_end(&mut buf) {
            err = Some(e);
            return Ok(false);
        }
        let size = buf.len() as u64;
        let mtime = sevenz_mtime(entry);
        files.insert(rel, (buf, size, mtime));
        Ok(true)
    });
    if let Err(e) = r {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("7z 读取失败: {e}"),
        ));
    }
    if let Some(e) = err {
        return Err(e);
    }
    Ok(files)
}

/// 7z 条目 mtime（nt-time FileTime → SystemTime，失败回退 UNIX_EPOCH）
fn sevenz_mtime(entry: &sevenz_rust2::SevenZArchiveEntry) -> SystemTime {
    let (secs, _nanos) = entry.last_modified_date().to_unix_time(); // (秒, 纳秒)
    secs_to_systemtime(secs.max(0) as u64)
}

/// 规范化归档内相对路径：去 `./` 前缀、`\` → `/`、去结尾 `/`、拒绝 `..`
fn normalize_rel(p: &str) -> String {
    let mut s = p.replace('\\', "/");
    while let Some(stripped) = s.strip_prefix("./") {
        s = stripped.to_string();
    }
    let s = s.trim_start_matches('/');
    if s.contains("..") {
        return String::new();
    }
    s.trim_end_matches('/').to_string()
}

fn secs_to_systemtime(secs: u64) -> SystemTime {
    UNIX_EPOCH + std::time::Duration::from_secs(secs)
}

impl Vfs for ArchiveVfs {
    fn describe(&self) -> String {
        format!("{}://{}", self.kind, self.path)
    }

    fn scan(&self, filter: &Filter) -> io::Result<BTreeMap<String, FileMeta>> {
        let files = self.files.borrow();
        let mut map = BTreeMap::new();
        for (rel, (_, size, mtime)) in files.iter() {
            if !filter.accept(rel) {
                continue;
            }
            map.insert(
                rel.clone(),
                FileMeta {
                    size: *size,
                    mtime: *mtime,
                    mode: None,
                    symlink: None,
                },
            );
        }
        Ok(map)
    }

    fn read(&self, rel: &str) -> io::Result<Vec<u8>> {
        self.files
            .borrow()
            .get(rel)
            .map(|(buf, _, _)| buf.clone())
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("{}: 无此文件", rel)))
    }

    fn hash(&self, rel: &str) -> io::Result<blake3::Hash> {
        self.files
            .borrow()
            .get(rel)
            .map(|(buf, _, _)| blake3::hash(buf))
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("{}: 无此文件", rel)))
    }

    fn exists(&self, rel: &str) -> io::Result<bool> {
        Ok(self.files.borrow().contains_key(rel))
    }

    fn write(&self, rel: &str, data: &[u8]) -> io::Result<()> {
        let rel = rel.to_string();
        let data = data.to_vec();
        self.rewrite(|files| {
            let len = data.len() as u64;
            files.insert(rel, (data, len, UNIX_EPOCH));
        })
    }

    fn delete(&self, rel: &str) -> io::Result<()> {
        let rel = rel.to_string();
        if !self.files.borrow().contains_key(&rel) {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("{}: 不存在", rel),
            ));
        }
        self.rewrite(|files| {
            files.remove(&rel);
        })
    }

    fn remove_dir(&self, _rel: &str) -> io::Result<()> {
        // tar 无显式目录树，目录是隐式的；无需清理
        Ok(())
    }

    fn rename(&self, from: &str, to: &str) -> io::Result<()> {
        if from == to {
            return Ok(());
        }
        let data = self.read(from)?;
        let from = from.to_string();
        let to = to.to_string();
        self.rewrite(|files| {
            if let Some((_, _, mtime)) = files.remove(&from) {
                let len = data.len() as u64;
                files.insert(to, (data.clone(), len, mtime));
            }
        })
    }

    fn set_mtime(&self, rel: &str, t: SystemTime) -> io::Result<()> {
        let rel = rel.to_string();
        if !self.files.borrow().contains_key(&rel) {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("{}: 不存在", rel),
            ));
        }
        self.rewrite(|files| {
            if let Some(entry) = files.get_mut(&rel) {
                entry.2 = t;
            }
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

    /// 构建一个 tar：a.txt、sub/b.txt、sub/deep/c.txt
    fn make_tar(path: &Path) {
        let file = fs::File::create(path).unwrap();
        let mut ar = tar::Builder::new(file);
        for (name, content) in [
            ("a.txt", "hello"),
            ("sub/b.txt", "world"),
            ("sub/deep/c.txt", "deep"),
        ] {
            let mut header = tar::Header::new_gnu();
            header.set_size(content.len() as u64);
            header.set_mode(0o644);
            header.set_mtime(1_700_000_000);
            header.set_cksum();
            ar.append_data(&mut header, name, content.as_bytes())
                .unwrap();
        }
        ar.finish().unwrap();
    }

    /// 构建一个 7z（sevenz-rust2 writer）
    fn make_7z(path: &Path) {
        let mut writer = sevenz_rust2::SevenZWriter::new(fs::File::create(path).unwrap()).unwrap();
        writer
            .push_archive_entry(
                sevenz_rust2::SevenZArchiveEntry::new_file("a.txt"),
                Some("hello".as_bytes()),
            )
            .unwrap();
        writer
            .push_archive_entry(
                sevenz_rust2::SevenZArchiveEntry::new_file("sub/b.txt"),
                Some("world".as_bytes()),
            )
            .unwrap();
        writer.finish().unwrap();
    }

    #[test]
    fn tar_scan_and_read() {
        let d = tempdir().unwrap();
        let tp = d.path().join("t.tar");
        make_tar(&tp);
        let v = ArchiveVfs::open(tp.to_str().unwrap()).unwrap();
        let f = Filter::new(&[], &[]).unwrap();
        let map = v.scan(&f).unwrap();
        assert_eq!(map.len(), 3);
        assert!(map.contains_key("a.txt"));
        assert!(map.contains_key("sub/b.txt"));
        assert!(map.contains_key("sub/deep/c.txt"));
        assert_eq!(map["a.txt"].size, 5);
        assert_eq!(v.read("sub/b.txt").unwrap(), b"world");
        assert_eq!(v.hash("a.txt").unwrap(), blake3::hash(b"hello"));
        assert!(v.exists("a.txt").unwrap());
        assert!(!v.exists("nope.txt").unwrap());
        // mtime 已填充（1970 之后）
        assert!(map["a.txt"].mtime > UNIX_EPOCH);
    }

    #[test]
    fn tar_gz_roundtrip() {
        let d = tempdir().unwrap();
        let tp = d.path().join("t.tar");
        make_tar(&tp);
        let gz = d.path().join("t.tar.gz");
        let raw = fs::read(&tp).unwrap();
        let mut out = Vec::new();
        {
            let mut enc = flate2::write::GzEncoder::new(&mut out, flate2::Compression::default());
            enc.write_all(&raw).unwrap();
            enc.finish().unwrap();
        }
        fs::write(&gz, &out).unwrap();
        let v = ArchiveVfs::open(gz.to_str().unwrap()).unwrap();
        let f = Filter::new(&[], &[]).unwrap();
        let map = v.scan(&f).unwrap();
        assert_eq!(map.len(), 3);
        assert_eq!(v.read("a.txt").unwrap(), b"hello");
    }

    #[test]
    fn sevenz_scan_and_read() {
        let d = tempdir().unwrap();
        let zp = d.path().join("t.7z");
        make_7z(&zp);
        let v = ArchiveVfs::open(zp.to_str().unwrap()).unwrap();
        let f = Filter::new(&[], &[]).unwrap();
        let map = v.scan(&f).unwrap();
        assert_eq!(map.len(), 2);
        assert_eq!(v.read("a.txt").unwrap(), b"hello");
        assert_eq!(v.read("sub/b.txt").unwrap(), b"world");
    }

    #[test]
    fn unsupported_format_errors() {
        let d = tempdir().unwrap();
        let p = d.path().join("x.rar");
        fs::write(&p, "fake rar").unwrap();
        assert!(ArchiveVfs::open(p.to_str().unwrap()).is_err());
    }

    #[test]
    fn write_delete_mtime_roundtrip() {
        let d = tempdir().unwrap();
        let tp = d.path().join("t.tar");
        make_tar(&tp);
        let v = ArchiveVfs::open(tp.to_str().unwrap()).unwrap();
        // tar 可写：新增/覆盖/删除/设置 mtime
        v.write("new.txt", b"new-content").unwrap();
        assert_eq!(v.read("new.txt").unwrap(), b"new-content");
        v.write("a.txt", b"updated").unwrap();
        assert_eq!(v.read("a.txt").unwrap(), b"updated");
        v.delete("sub/b.txt").unwrap();
        assert!(!v.exists("sub/b.txt").unwrap());
        let t = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
        v.set_mtime("a.txt", t).unwrap();
        let f = Filter::new(&[], &[]).unwrap();
        let map = v.scan(&f).unwrap();
        assert_eq!(map["a.txt"].mtime, t);
        // 重写后重新打开仍一致
        drop(v);
        let v2 = ArchiveVfs::open(tp.to_str().unwrap()).unwrap();
        assert_eq!(v2.read("a.txt").unwrap(), b"updated");
        assert!(!v2.exists("sub/b.txt").unwrap());
        assert!(v2.exists("new.txt").unwrap());
    }

    #[test]
    fn sevenz_remains_readonly() {
        let d = tempdir().unwrap();
        let zp = d.path().join("t.7z");
        make_7z(&zp);
        let v = ArchiveVfs::open(zp.to_str().unwrap()).unwrap();
        assert!(v.write("x.txt", b"x").is_err());
        assert!(v.delete("a.txt").is_err());
        assert!(v.set_mtime("a.txt", UNIX_EPOCH).is_err());
    }

    #[test]
    fn normalize_rel_strips_prefixes() {
        assert_eq!(normalize_rel("./a/b.txt"), "a/b.txt");
        assert_eq!(normalize_rel("a\\b.txt"), "a/b.txt");
        assert_eq!(normalize_rel("dir/"), "dir");
        assert_eq!(normalize_rel("../evil.txt"), "");
        assert_eq!(normalize_rel("a/../b.txt"), "");
    }
}
