//! M6 虚拟文件系统适配层。
//!
//! 统一抽象本地目录、ZIP 压缩包、SFTP 远程目录三种后端，使 compare/sync
//! 可以跨后端工作。路径规范：
//! - 普通路径 → 本地目录
//! - `zip://path/to/archive.zip` → ZIP 虚拟 FS（只读）
//! - `sftp://[user[:pass]@]host[:port]/remote/path` → SFTP 虚拟 FS
//!
//! 核心操作与 `fsscan` 对齐：scan 返回 (相对路径 -> 元数据) 的有序表，
//! read/write/delete 以相对路径为键，复制跨后端进行（先读后写）。

pub mod archive;
pub mod ftp;
pub mod s3;
pub mod sftp;
pub mod webdav;
pub mod zip;

use crate::fsscan::{FileMeta, Filter};
use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::Path;
use std::time::SystemTime;

/// 虚拟文件系统后端
pub trait Vfs {
    /// 人类可读描述（错误消息用）
    fn describe(&self) -> String;

    /// 扫描根目录树，返回 (相对路径 -> 元数据)，BTreeMap 保证有序
    fn scan(&self, filter: &Filter) -> io::Result<BTreeMap<String, FileMeta>>;

    /// 扫描根目录树中的子目录相对路径集合（不含根），供 mirror 空目录清理。
    /// 默认返回空集（后端不支持时跳过目录清理）。
    fn scan_dirs(&self, _filter: &Filter) -> io::Result<BTreeSet<String>> {
        Ok(Default::default())
    }

    /// 读取文件全部内容
    fn read(&self, rel: &str) -> io::Result<Vec<u8>>;

    /// 流式计算文件 blake3 哈希（分块读取，支持超大文件，内存 O(64KB)）。
    /// 默认实现走 read()，本地/ZIP 后端覆写为真正的流式。
    fn hash(&self, rel: &str) -> io::Result<blake3::Hash> {
        let data = self.read(rel)?;
        Ok(blake3::hash(&data))
    }

    /// 文件是否存在
    #[allow(dead_code)]
    fn exists(&self, rel: &str) -> io::Result<bool>;

    /// 写入文件（覆盖，自动建父目录）
    fn write(&self, rel: &str, data: &[u8]) -> io::Result<()>;

    /// 删除文件
    fn delete(&self, rel: &str) -> io::Result<()>;

    /// 删除空目录（mirror 清理用；非空目录应返回错误）
    fn remove_dir(&self, rel: &str) -> io::Result<()>;

    /// 移动/重命名：优先后端原生实现（本地 rename 零拷贝）；
    /// 默认回退为 读→写→删（跨后端仍可用）。
    fn rename(&self, from: &str, to: &str) -> io::Result<()> {
        let data = self.read(from)?;
        self.write(to, &data)?;
        self.delete(from)
    }

    /// 把本后端的 rel 文件复制到目标后端（跨 FS）
    fn copy_to(&self, rel: &str, dst: &dyn Vfs) -> io::Result<()> {
        let data = self.read(rel)?;
        dst.write(rel, &data)
    }

    /// 设置文件 mtime（幂等同步的关键）
    fn set_mtime(&self, rel: &str, t: SystemTime) -> io::Result<()>;
}

/// 本地目录后端
pub struct LocalVfs {
    root: std::path::PathBuf,
}

impl LocalVfs {
    pub fn new(root: &Path) -> io::Result<Self> {
        if !root.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("不是目录: {}", root.display()),
            ));
        }
        Ok(LocalVfs {
            root: root.to_path_buf(),
        })
    }
}

impl Vfs for LocalVfs {
    fn describe(&self) -> String {
        self.root.display().to_string()
    }

    fn scan(&self, filter: &Filter) -> io::Result<BTreeMap<String, FileMeta>> {
        crate::fsscan::scan(&self.root, filter)
    }

    fn scan_dirs(&self, filter: &Filter) -> io::Result<BTreeSet<String>> {
        crate::fsscan::scan_dirs(&self.root, filter)
    }

    fn hash(&self, rel: &str) -> io::Result<blake3::Hash> {
        use std::io::Read;
        let p = self.root.join(rel);
        let mut f = std::fs::File::open(&p)?;
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

    fn read(&self, rel: &str) -> io::Result<Vec<u8>> {
        let p = self.root.join(rel);
        let f = std::fs::File::open(&p)?;
        // mmap 只读映射后立即复制为 Vec：减少一次内核→用户缓冲拷贝，
        // 且不长期持有映射（避免外部修改文件触发 SIGBUS）
        unsafe { Ok(memmap2::Mmap::map(&f)?.to_vec()) }
    }

    fn exists(&self, rel: &str) -> io::Result<bool> {
        Ok(self.root.join(rel).exists())
    }

    fn write(&self, rel: &str, data: &[u8]) -> io::Result<()> {
        let p = self.root.join(rel);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(p, data)
    }

    fn delete(&self, rel: &str) -> io::Result<()> {
        std::fs::remove_file(self.root.join(rel))
    }

    fn remove_dir(&self, rel: &str) -> io::Result<()> {
        std::fs::remove_dir(self.root.join(rel))
    }

    fn rename(&self, from: &str, to: &str) -> io::Result<()> {
        let src = self.root.join(from);
        let dst = self.root.join(to);
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::rename(&src, &dst)
    }

    fn set_mtime(&self, rel: &str, t: SystemTime) -> io::Result<()> {
        filetime::set_file_mtime(self.root.join(rel), filetime::FileTime::from_system_time(t))
    }
}

/// 解析路径规范为后端。非 URL 形式一律视为本地目录。
pub fn open(spec: &str) -> io::Result<Box<dyn Vfs>> {
    if let Some(rest) = spec.strip_prefix("zip://") {
        let z = zip::ZipVfs::open(rest)?;
        return Ok(Box::new(z));
    }
    if let Some(rest) = spec.strip_prefix("tar://") {
        let a = archive::ArchiveVfs::open(rest)?;
        return Ok(Box::new(a));
    }
    if let Some(rest) = spec.strip_prefix("7z://") {
        let a = archive::ArchiveVfs::open(rest)?;
        return Ok(Box::new(a));
    }
    if let Some(rest) = spec.strip_prefix("sftp://") {
        let s = sftp::SftpVfs::connect(rest)?;
        return Ok(Box::new(s));
    }
    if let Some(rest) = spec.strip_prefix("ftp://") {
        let f = ftp::FtpVfs::connect(rest)?;
        return Ok(Box::new(f));
    }
    if let Some(rest) = spec.strip_prefix("webdav://") {
        let w = webdav::WebdavVfs::connect(rest)?;
        return Ok(Box::new(w));
    }
    if let Some(rest) = spec.strip_prefix("webdavs://") {
        let w = webdav::WebdavVfs::connect(rest)?;
        return Ok(Box::new(w));
    }
    if let Some(rest) = spec.strip_prefix("s3://") {
        let s = s3::S3Vfs::connect(rest)?;
        return Ok(Box::new(s));
    }
    Ok(Box::new(LocalVfs::new(Path::new(spec))?))
}

/// 判断是否使用了虚拟后端（用于错误提示）
pub fn is_remote(spec: &str) -> bool {
    spec.starts_with("zip://")
        || spec.starts_with("tar://")
        || spec.starts_with("7z://")
        || spec.starts_with("sftp://")
        || spec.starts_with("ftp://")
        || spec.starts_with("webdav://")
        || spec.starts_with("webdavs://")
        || spec.starts_with("s3://")
}

/// 跨后端内容比对：流式计算两侧 blake3 哈希比较（内存 O(64KB)，支持超大文件）
pub fn content_equal_vfs(left: &dyn Vfs, right: &dyn Vfs, rel: &str) -> io::Result<bool> {
    Ok(left.hash(rel)? == right.hash(rel)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn open_local_dir() {
        let d = tempdir().unwrap();
        fs::write(d.path().join("a.txt"), "x").unwrap();
        let v = open(d.path().to_str().unwrap()).unwrap();
        let f = Filter::new(&[], &[]).unwrap();
        let map = v.scan(&f).unwrap();
        assert!(map.contains_key("a.txt"));
        assert_eq!(v.read("a.txt").unwrap(), b"x");
    }

    #[test]
    fn open_missing_dir_errors() {
        assert!(open("/nonexistent/bcr-vfs-dir").is_err());
    }

    #[test]
    fn open_zip_scheme() {
        let d = tempdir().unwrap();
        let zp = d.path().join("t.zip");
        {
            let file = fs::File::create(&zp).unwrap();
            let mut w = ::zip::ZipWriter::new(file);
            w.start_file("a.txt", ::zip::write::SimpleFileOptions::default())
                .unwrap();
            use std::io::Write;
            w.write_all(b"zip-content").unwrap();
            w.finish().unwrap();
        }
        let spec = format!("zip://{}", zp.display());
        assert!(is_remote(&spec));
        let v = open(&spec).unwrap();
        let f = Filter::new(&[], &[]).unwrap();
        let map = v.scan(&f).unwrap();
        assert!(map.contains_key("a.txt"));
        assert_eq!(v.read("a.txt").unwrap(), b"zip-content");
    }

    #[test]
    fn local_write_delete_mtime() {
        let d = tempdir().unwrap();
        let v = LocalVfs::new(d.path()).unwrap();
        v.write("sub/f.txt", b"data").unwrap();
        assert!(d.path().join("sub/f.txt").exists());
        assert!(v.exists("sub/f.txt").unwrap());
        assert_eq!(v.read("sub/f.txt").unwrap(), b"data");
        let t = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
        v.set_mtime("sub/f.txt", t).unwrap();
        let meta = fs::metadata(d.path().join("sub/f.txt")).unwrap();
        assert_eq!(meta.modified().unwrap(), t);
        v.delete("sub/f.txt").unwrap();
        assert!(!d.path().join("sub/f.txt").exists());
    }

    #[test]
    fn local_copy_to_cross_backend() {
        let d1 = tempdir().unwrap();
        let d2 = tempdir().unwrap();
        fs::write(d1.path().join("a.txt"), "cross").unwrap();
        let src = LocalVfs::new(d1.path()).unwrap();
        let dst = LocalVfs::new(d2.path()).unwrap();
        src.copy_to("a.txt", &dst).unwrap();
        assert_eq!(
            fs::read_to_string(d2.path().join("a.txt")).unwrap(),
            "cross"
        );
    }
}
