//! CAB 虚拟文件系统（A12，只读）。
//!
//! `cab://path/to/file.cab` → 通过 `cab` crate（纯 Rust）读取 Microsoft CAB 归档。
//! CAB 无目录层级（扁平文件名），scan 返回顶层文件名条目。
//!
//! 只读后端：write/delete/rename/set_mtime 返回不支持错误。

use super::Vfs;
use crate::fsscan::{FileMeta, Filter};
use cab::Cabinet;
use std::collections::BTreeMap;
use std::io::{self, Read};
use std::time::SystemTime;

/// CAB 虚拟文件系统（打开时全量读入内存并解压索引）
pub struct CabVfs {
    desc: String,
    /// 文件名 → 内容（CAB 一般不大，一次性解压便于随机读取）
    files: BTreeMap<String, Vec<u8>>,
}

impl CabVfs {
    /// 打开 CAB 归档（rest 为文件路径）
    pub fn connect(rest: &str) -> io::Result<Self> {
        let data = std::fs::read(rest)?;
        let mut cabinet = Cabinet::new(io::Cursor::new(data))
            .map_err(|e| io::Error::other(format!("CAB 解析失败: {e}")))?;
        // 先收集全部文件名（folder_entries 借用 cabinet），再逐个读取（需要 &mut）
        let mut names: Vec<String> = Vec::new();
        for folder in cabinet.folder_entries() {
            for file in folder.file_entries() {
                names.push(file.name().to_string());
            }
        }
        let mut files = BTreeMap::new();
        for name in names {
            let mut reader = cabinet
                .read_file(&name)
                .map_err(|e| io::Error::other(format!("CAB 读取 {name} 失败: {e}")))?;
            let mut buf = Vec::new();
            reader.read_to_end(&mut buf)?;
            files.insert(name, buf);
        }
        Ok(CabVfs {
            desc: format!("cab://{}", rest),
            files,
        })
    }
}

impl Vfs for CabVfs {
    fn describe(&self) -> String {
        self.desc.clone()
    }

    fn scan(&self, filter: &Filter) -> io::Result<BTreeMap<String, FileMeta>> {
        let mut map = BTreeMap::new();
        for (name, data) in &self.files {
            if !filter.accept(name) {
                continue;
            }
            map.insert(
                name.clone(),
                FileMeta {
                    size: data.len() as u64,
                    mtime: SystemTime::UNIX_EPOCH,
                    mode: None,
                    symlink: None,
                },
            );
        }
        Ok(map)
    }

    fn read(&self, rel: &str) -> io::Result<Vec<u8>> {
        self.files
            .get(rel)
            .cloned()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("CAB 中无 {rel}")))
    }

    fn exists(&self, rel: &str) -> io::Result<bool> {
        Ok(self.files.contains_key(rel))
    }

    fn write(&self, _rel: &str, _data: &[u8]) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "CAB 后端只读"))
    }

    fn delete(&self, _rel: &str) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "CAB 后端只读"))
    }

    fn remove_dir(&self, _rel: &str) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "CAB 后端只读"))
    }

    fn rename(&self, _from: &str, _to: &str) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "CAB 后端只读"))
    }

    fn set_mtime(&self, _rel: &str, _t: SystemTime) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "CAB 后端只读"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cab::{CabinetBuilder, CompressionType};
    use std::io::Write;
    use tempfile::tempdir;

    /// 构造一个含两个文件的 CAB（MSZIP 压缩）
    fn build_cab(dir: &std::path::Path) -> String {
        let path = dir.join("test.cab");
        let out = std::fs::File::create(&path).unwrap();
        let mut builder = CabinetBuilder::new();
        let folder = builder.add_folder(CompressionType::MsZip);
        folder.add_file("a.txt");
        folder.add_file("b.txt");
        let mut writer = builder.build(out).unwrap();
        // 按文件名顺序写入
        for (name, content) in [("a.txt", "hello cab a"), ("b.txt", "hello cab b")] {
            let mut fw = writer.next_file().unwrap().expect("cab writer 应产出文件");
            assert_eq!(fw.file_name(), name);
            fw.write_all(content.as_bytes()).unwrap();
        }
        writer.finish().unwrap();
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn cab_scan_and_read() {
        let d = tempdir().unwrap();
        let path = build_cab(d.path());
        let v = CabVfs::connect(&path).unwrap();
        let f = Filter::new(&[], &[]).unwrap();
        let map = v.scan(&f).unwrap();
        assert!(map.contains_key("a.txt"));
        assert!(map.contains_key("b.txt"));
        assert_eq!(map["a.txt"].size, "hello cab a".len() as u64);
        assert_eq!(v.read("a.txt").unwrap(), b"hello cab a");
        assert_eq!(v.read("b.txt").unwrap(), b"hello cab b");
        assert!(v.read("missing.txt").is_err());
    }

    #[test]
    fn cab_readonly_ops_error() {
        let d = tempdir().unwrap();
        let path = build_cab(d.path());
        let v = CabVfs::connect(&path).unwrap();
        assert!(v.write("x", b"1").is_err());
        assert!(v.delete("a.txt").is_err());
        assert!(v.rename("a.txt", "c.txt").is_err());
    }
}
