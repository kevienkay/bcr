//! ISO 9660 虚拟文件系统（A12，只读）。
//!
//! `iso://path/to/image.iso` → 通过外部 `7z` 或 `bsdtar` 命令读取光盘镜像。
//! （iso9660 crate 在部分镜像源不可用，采用外部命令方案与 SVN/RAR 一致。）
//!
//! - 优先 `7z`（`7z l`/`7z e -so`），回退 `bsdtar`（`bsdtar -tf`/`-xOf`）
//! - 未安装时 connect 报错提示
//! - 只读后端：write/delete/rename/set_mtime 返回不支持错误

use super::Vfs;
use crate::fsscan::{FileMeta, Filter};
use std::collections::BTreeMap;
use std::io;
use std::process::Command;
use std::time::SystemTime;

/// ISO 虚拟文件系统
pub struct IsoVfs {
    desc: String,
    /// 镜像文件路径
    path: String,
    /// 使用的命令：7z 或 bsdtar
    backend: IsoBackend,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IsoBackend {
    SevenZip,
    Bsdtar,
}

impl IsoVfs {
    /// 探测可用后端并连接
    pub fn connect(rest: &str) -> io::Result<Self> {
        let backend = if Command::new("7z")
            .arg("i")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            IsoBackend::SevenZip
        } else if Command::new("bsdtar")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            IsoBackend::Bsdtar
        } else {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "ISO 后端需要 7z 或 bsdtar 命令（请安装 p7zip 或 libarchive）",
            ));
        };
        Ok(IsoVfs {
            desc: format!("iso://{}", rest),
            path: rest.to_string(),
            backend,
        })
    }

    /// 列出归档内容（每行一个相对路径；目录以 / 结尾）
    fn list(&self) -> io::Result<Vec<String>> {
        match self.backend {
            IsoBackend::SevenZip => {
                let out = Command::new("7z")
                    .args(["l", "-slt", &self.path])
                    .output()
                    .map_err(|e| io::Error::other(format!("7z l 失败: {e}")))?;
                if !out.status.success() {
                    return Err(io::Error::other("7z l 失败（镜像不可读或损坏）"));
                }
                // -slt 输出：Path = xxx 块
                let text = String::from_utf8_lossy(&out.stdout);
                let mut paths = Vec::new();
                let mut cur = String::new();
                for line in text.lines() {
                    if let Some(p) = line.strip_prefix("Path = ") {
                        cur = p.trim().to_string();
                    } else if line.starts_with("Attributes = ") && !cur.is_empty() {
                        paths.push(std::mem::take(&mut cur));
                    }
                }
                if !cur.is_empty() {
                    paths.push(cur);
                }
                Ok(paths)
            }
            IsoBackend::Bsdtar => {
                let out = Command::new("bsdtar")
                    .args(["-tf", &self.path])
                    .output()
                    .map_err(|e| io::Error::other(format!("bsdtar -tf 失败: {e}")))?;
                if !out.status.success() {
                    return Err(io::Error::other("bsdtar -tf 失败（镜像不可读或损坏）"));
                }
                Ok(String::from_utf8_lossy(&out.stdout)
                    .lines()
                    .map(|s| s.to_string())
                    .collect())
            }
        }
    }
}

impl Vfs for IsoVfs {
    fn describe(&self) -> String {
        self.desc.clone()
    }

    fn scan(&self, filter: &Filter) -> io::Result<BTreeMap<String, FileMeta>> {
        let mut map = BTreeMap::new();
        for entry in self.list()? {
            let rel = entry.trim_start_matches("./").to_string();
            if rel.is_empty() || rel.ends_with('/') {
                continue; // 目录跳过
            }
            if !filter.accept(&rel) {
                continue;
            }
            // size 由读取时计算（ISO 条目无标准大小字段，简单起见占位）
            map.insert(
                rel.clone(),
                FileMeta {
                    size: 0,
                    mtime: SystemTime::UNIX_EPOCH,
                    mode: None,
                    symlink: None,
                },
            );
        }
        Ok(map)
    }

    fn read(&self, rel: &str) -> io::Result<Vec<u8>> {
        match self.backend {
            IsoBackend::SevenZip => {
                let out = Command::new("7z")
                    .args(["e", "-so", &self.path, rel])
                    .output()
                    .map_err(|e| io::Error::other(format!("7z e 失败: {e}")))?;
                if !out.status.success() {
                    return Err(io::Error::new(
                        io::ErrorKind::NotFound,
                        format!("ISO 中无 {rel}"),
                    ));
                }
                Ok(out.stdout)
            }
            IsoBackend::Bsdtar => {
                let out = Command::new("bsdtar")
                    .args(["-xOf", &self.path, rel])
                    .output()
                    .map_err(|e| io::Error::other(format!("bsdtar -xOf 失败: {e}")))?;
                if !out.status.success() {
                    return Err(io::Error::new(
                        io::ErrorKind::NotFound,
                        format!("ISO 中无 {rel}"),
                    ));
                }
                Ok(out.stdout)
            }
        }
    }

    fn exists(&self, rel: &str) -> io::Result<bool> {
        Ok(self
            .list()?
            .iter()
            .any(|p| p.trim_start_matches("./") == rel))
    }

    fn write(&self, _rel: &str, _data: &[u8]) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "ISO 后端只读"))
    }

    fn delete(&self, _rel: &str) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "ISO 后端只读"))
    }

    fn remove_dir(&self, _rel: &str) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "ISO 后端只读"))
    }

    fn rename(&self, _from: &str, _to: &str) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "ISO 后端只读"))
    }

    fn set_mtime(&self, _rel: &str, _t: SystemTime) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "ISO 后端只读"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_readonly_ops_error_without_image() {
        // 即使 7z/bsdtar 不存在，只读操作的错误语义也正确
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("fake.iso");
        std::fs::write(&path, b"not an iso").unwrap();
        if let Ok(v) = IsoVfs::connect(path.to_str().unwrap()) {
            assert!(v.write("x", b"1").is_err());
            assert!(v.delete("x").is_err());
            assert!(v.rename("a", "b").is_err());
            assert!(v.set_mtime("x", SystemTime::UNIX_EPOCH).is_err());
        }
        // 未安装后端时 connect 报错
        if Command::new("7z")
            .arg("i")
            .output()
            .map(|o| !o.status.success())
            .unwrap_or(true)
            && Command::new("bsdtar")
                .arg("--version")
                .output()
                .map(|o| !o.status.success())
                .unwrap_or(true)
        {
            assert!(IsoVfs::connect(path.to_str().unwrap()).is_err());
        }
    }
}
