//! FTP 虚拟文件系统。
//!
//! 通过 suppaftp（纯 Rust FTP 客户端）连接 FTP 服务器，把远程目录当作本地树。
//! URL 规范：`ftp://[user[:pass]@]host[:port]/remote/path`
//!
//! - 支持匿名登录（默认 user=anonymous, pass=空）
//! - 可读写：read/write/delete/rename/remove_dir；FTP 无标准设置 mtime 的
//!   命令（MFMT 非标准），set_mtime 静默忽略 —— 同步到 FTP 建议加
//!   `--compare-content` 保证准确性
//! - 被动模式（Passive），适配大多数 NAT/防火墙环境

use super::Vfs;
use crate::fsscan::{FileMeta, Filter};
use std::collections::BTreeMap;
use std::io::{self, Read};
use std::time::SystemTime;
use suppaftp::list::{File, ListParser};
use suppaftp::FtpStream;

/// FTP 虚拟文件系统（每次操作建立短连接，协议简单无状态）
pub struct FtpVfs {
    desc: String,
    user: String,
    pass: String,
    host: String,
    port: u16,
    /// 远程根目录（绝对路径）
    root: String,
}

/// 解析 ftp:// URL：返回 (user, pass, host, port, remote_path)
fn parse_url(rest: &str) -> io::Result<(String, String, String, u16, String)> {
    let rest = rest.trim_end_matches('/');
    let (auth, host_port_path) = match rest.rsplit_once('@') {
        Some((a, rest)) => (Some(a), rest),
        None => (None, rest),
    };
    let (host_part, path) = match host_port_path.find('/') {
        Some(i) => (&host_port_path[..i], &host_port_path[i..]),
        None => (host_port_path, "/"),
    };
    let host_part = host_part.trim_start_matches('/');
    if host_part.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "ftp:// 缺少主机名",
        ));
    }
    let (host, port) = match host_part.rsplit_once(':') {
        Some((h, p)) => {
            if h.is_empty() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "ftp:// 缺少主机名",
                ));
            }
            (
                h.to_string(),
                p.parse::<u16>()
                    .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "ftp:// 端口无效"))?,
            )
        }
        None => (host_part.to_string(), 21),
    };
    // 用户信息：user:pass@ 或 user@；缺省 anonymous
    let (user, pass) = match auth {
        Some(a) => match a.split_once(':') {
            Some((u, p)) => (u.to_string(), p.to_string()),
            None => (a.to_string(), String::new()),
        },
        None => ("anonymous".to_string(), String::new()),
    };
    Ok((user, pass, host, port, path.to_string()))
}

/// 拼接远程绝对路径
fn join_root(root: &str, rel: &str) -> String {
    if rel.is_empty() {
        return root.trim_end_matches('/').to_string();
    }
    format!("{}/{}", root.trim_end_matches('/'), rel)
}

impl FtpVfs {
    /// 打开 FTP 连接并登录、进入根目录
    pub fn connect(rest: &str) -> io::Result<Self> {
        let (user, pass, host, port, root) = parse_url(rest)?;
        Ok(FtpVfs {
            desc: format!("ftp://{}", rest),
            user,
            pass,
            host,
            port,
            root,
        })
    }

    /// 建立新连接并登录、进入 root（每次操作调用，用完 quit）
    fn session(&self) -> io::Result<FtpStream> {
        let mut ftp = FtpStream::connect((self.host.as_str(), self.port)).map_err(|e| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("FTP 连接 {}:{} 失败: {e}", self.host, self.port),
            )
        })?;
        ftp.login(&self.user, &self.pass).map_err(|e| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("FTP 登录失败: {e}"),
            )
        })?;
        ftp.cwd(&self.root).map_err(|e| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("FTP 进入目录 {} 失败: {e}", self.root),
            )
        })?;
        Ok(ftp)
    }

    /// 解析一行 LIST 输出（unix 优先，回退 dos；无法解析返回 None）
    fn parse_line(line: &str) -> Option<File> {
        // 跳过 "total N" 等摘要行
        if line.is_empty() || line.starts_with("total ") {
            return None;
        }
        ListParser::parse_posix(line)
            .or_else(|_| ListParser::parse_dos(line))
            .ok()
    }

    /// 递归扫描目录（连接复用）
    fn scan_rec(
        ftp: &mut FtpStream,
        abs: &str,
        root: &str,
        filter: &Filter,
        out: &mut BTreeMap<String, FileMeta>,
    ) -> io::Result<()> {
        let lines = ftp
            .list(Some(abs))
            .map_err(|e| io::Error::other(format!("FTP list {abs} 失败: {e}")))?;
        for line in lines {
            let Some(f) = Self::parse_line(&line) else {
                continue;
            };
            let name = f.name().to_string();
            if name == "." || name == ".." || name.is_empty() {
                continue;
            }
            let rel = if abs == root {
                name.clone()
            } else {
                format!(
                    "{}/{}",
                    abs.trim_start_matches(root).trim_start_matches('/'),
                    name
                )
            };
            if f.is_directory() {
                if filter.is_excluded(&rel) || filter.is_excluded(&format!("{rel}/")) {
                    continue;
                }
                Self::scan_rec(ftp, &join_root(root, &rel), root, filter, out)?;
            } else if f.is_file() && filter.accept(&rel) {
                out.insert(
                    rel,
                    FileMeta {
                        size: f.size() as u64,
                        mtime: f.modified(),
                        mode: None,
                        symlink: None,
                    },
                );
            }
        }
        Ok(())
    }
}

impl Vfs for FtpVfs {
    fn describe(&self) -> String {
        self.desc.clone()
    }

    fn scan(&self, filter: &Filter) -> io::Result<BTreeMap<String, FileMeta>> {
        let mut ftp = self.session()?;
        let mut map = BTreeMap::new();
        let r = Self::scan_rec(&mut ftp, &self.root, &self.root, filter, &mut map);
        let _ = ftp.quit();
        r?;
        Ok(map)
    }

    fn read(&self, rel: &str) -> io::Result<Vec<u8>> {
        let mut ftp = self.session()?;
        let r = (|| -> io::Result<Vec<u8>> {
            let mut cursor = ftp
                .retr_as_buffer(&join_root(&self.root, rel))
                .map_err(|e| {
                    io::Error::new(io::ErrorKind::NotFound, format!("FTP 读取 {rel}: {e}"))
                })?;
            let mut buf = Vec::new();
            cursor.read_to_end(&mut buf)?;
            Ok(buf)
        })();
        let _ = ftp.quit();
        r
    }

    fn hash(&self, rel: &str) -> io::Result<blake3::Hash> {
        let data = self.read(rel)?;
        Ok(blake3::hash(&data))
    }

    fn exists(&self, rel: &str) -> io::Result<bool> {
        let mut ftp = self.session()?;
        let r = ftp
            .size(join_root(&self.root, rel))
            .map(|_| true)
            .or_else(|_| {
                // 目录会报错；用 nlst 判断是否存在
                ftp.nlst(Some(&join_root(&self.root, rel))).map(|_| true)
            });
        let _ = ftp.quit();
        Ok(r.unwrap_or(false))
    }

    fn write(&self, rel: &str, data: &[u8]) -> io::Result<()> {
        let mut ftp = self.session()?;
        let r = (|| -> io::Result<()> {
            // 逐级创建父目录
            let abs = join_root(&self.root, rel);
            let parent = abs.rsplit_once('/').map(|(p, _)| p.to_string());
            if let Some(p) = parent {
                if !p.is_empty() && p != self.root {
                    let mut cur = String::new();
                    for seg in p.split('/') {
                        if seg.is_empty() {
                            continue;
                        }
                        cur.push('/');
                        cur.push_str(seg);
                        let _ = ftp.mkdir(cur.clone()); // 已存在则忽略错误
                    }
                }
            }
            let mut data = data;
            ftp.put_file(abs, &mut data)
                .map_err(|e| io::Error::other(format!("FTP 写入 {rel}: {e}")))?;
            Ok(())
        })();
        let _ = ftp.quit();
        r
    }

    fn delete(&self, rel: &str) -> io::Result<()> {
        let mut ftp = self.session()?;
        let r = ftp
            .rm(join_root(&self.root, rel))
            .map_err(|e| io::Error::other(format!("FTP 删除 {rel}: {e}")));
        let _ = ftp.quit();
        r
    }

    fn remove_dir(&self, rel: &str) -> io::Result<()> {
        let mut ftp = self.session()?;
        let r = ftp
            .rmdir(join_root(&self.root, rel))
            .map_err(|e| io::Error::other(format!("FTP 删除目录 {rel}: {e}")));
        let _ = ftp.quit();
        r
    }

    fn rename(&self, from: &str, to: &str) -> io::Result<()> {
        let mut ftp = self.session()?;
        let r = ftp
            .rename(join_root(&self.root, from), join_root(&self.root, to))
            .map_err(|e| io::Error::other(format!("FTP 重命名 {from} -> {to}: {e}")));
        let _ = ftp.quit();
        r
    }

    fn set_mtime(&self, _rel: &str, _t: SystemTime) -> io::Result<()> {
        // FTP 无标准设置 mtime 命令（MFMT 非标准），静默忽略；
        // 同步到 FTP 建议 --compare-content
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_url_full() {
        let (u, p, h, port, path) = parse_url("alice:secret@example.com:2121/pub").unwrap();
        assert_eq!(u, "alice");
        assert_eq!(p, "secret");
        assert_eq!(h, "example.com");
        assert_eq!(port, 2121);
        assert_eq!(path, "/pub");
    }

    #[test]
    fn parse_url_anonymous_defaults() {
        let (u, p, h, port, path) = parse_url("ftp.example.com/data").unwrap();
        assert_eq!(u, "anonymous");
        assert_eq!(p, "");
        assert_eq!(h, "ftp.example.com");
        assert_eq!(port, 21);
        assert_eq!(path, "/data");
    }

    #[test]
    fn parse_url_no_pass() {
        let (u, p, _, _, _) = parse_url("bob@example.com/srv").unwrap();
        assert_eq!(u, "bob");
        assert_eq!(p, "");
    }

    #[test]
    fn parse_url_missing_host_errors() {
        assert!(parse_url(":21/x").is_err());
    }

    #[test]
    fn parse_url_trailing_slash() {
        let (_, _, _, _, path) = parse_url("user@host/data/").unwrap();
        assert_eq!(path, "/data");
    }

    #[test]
    fn join_root_paths() {
        assert_eq!(join_root("/ftp", "a.txt"), "/ftp/a.txt");
        assert_eq!(join_root("/ftp", "sub/b.txt"), "/ftp/sub/b.txt");
        assert_eq!(join_root("/ftp", ""), "/ftp");
        assert_eq!(join_root("/ftp/", "x.txt"), "/ftp/x.txt");
        assert_eq!(join_root("/", "a.txt"), "/a.txt");
    }

    #[test]
    fn parse_line_skips_total_and_parses_posix() {
        assert!(FtpVfs::parse_line("total 12").is_none());
        assert!(FtpVfs::parse_line("").is_none());
        let f =
            FtpVfs::parse_line("-rw-r--r-- 1 user group 1234 Nov  5 13:46 example.txt").unwrap();
        assert_eq!(f.name(), "example.txt");
        assert!(f.is_file());
        assert_eq!(f.size(), 1234);
        let d = FtpVfs::parse_line("drwxr-xr-x 2 user group 4096 Nov  5 13:46 subdir").unwrap();
        assert!(d.is_directory());
        assert_eq!(d.name(), "subdir");
    }

    #[test]
    fn parse_line_dos_format() {
        let f = FtpVfs::parse_line("11-05-26  01:46PM       <DIR>          subdir").unwrap();
        assert!(f.is_directory());
        let f2 = FtpVfs::parse_line("11-05-26  01:46PM              1234 example.txt").unwrap();
        assert!(f2.is_file());
        assert_eq!(f2.size(), 1234);
    }
}
