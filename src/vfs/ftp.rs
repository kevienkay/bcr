//! FTP 虚拟文件系统。
//!
//! 通过 suppaftp（纯 Rust FTP 客户端）连接 FTP 服务器，把远程目录当作本地树。
//! URL 规范：`ftp://[user[:pass]@]host[:port]/remote/path`
//!
//! - 支持匿名登录（默认 user=anonymous, pass=空）
//! - 可读写：read/write/delete/rename/remove_dir；set_mtime 发 MFMT（RFC 3659 扩展，
//!   服务器不支持时静默降级）——同步到 FTP 建议加 `--compare-content` 保证准确性
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
    /// A6 FTPS：implicit TLS（true）
    tls: bool,
}

/// 统一明文/TLS 连接（suppaftp 的 FtpStream 与 RustlsFtpStream 类型不同，用 enum 收口）
enum FtpConn {
    Plain(FtpStream),
    Secure(suppaftp::RustlsFtpStream),
}

impl FtpConn {
    fn login(&mut self, user: &str, pass: &str) -> suppaftp::FtpResult<()> {
        match self {
            FtpConn::Plain(f) => f.login(user, pass),
            FtpConn::Secure(f) => f.login(user, pass),
        }
    }

    fn cwd(&mut self, path: &str) -> suppaftp::FtpResult<()> {
        match self {
            FtpConn::Plain(f) => f.cwd(path),
            FtpConn::Secure(f) => f.cwd(path),
        }
    }

    fn quit(&mut self) -> suppaftp::FtpResult<()> {
        match self {
            FtpConn::Plain(f) => f.quit(),
            FtpConn::Secure(f) => f.quit(),
        }
    }

    fn list(&mut self, path: Option<&str>) -> suppaftp::FtpResult<Vec<String>> {
        match self {
            FtpConn::Plain(f) => f.list(path),
            FtpConn::Secure(f) => f.list(path),
        }
    }

    fn nlst(&mut self, path: Option<&str>) -> suppaftp::FtpResult<Vec<String>> {
        match self {
            FtpConn::Plain(f) => f.nlst(path),
            FtpConn::Secure(f) => f.nlst(path),
        }
    }

    fn size(&mut self, path: String) -> suppaftp::FtpResult<usize> {
        match self {
            FtpConn::Plain(f) => f.size(path),
            FtpConn::Secure(f) => f.size(path),
        }
    }

    fn mkdir(&mut self, path: String) -> suppaftp::FtpResult<()> {
        match self {
            FtpConn::Plain(f) => f.mkdir(path),
            FtpConn::Secure(f) => f.mkdir(path),
        }
    }

    fn put_file(&mut self, path: String, data: &mut &[u8]) -> suppaftp::FtpResult<u64> {
        match self {
            FtpConn::Plain(f) => f.put_file(path, data),
            FtpConn::Secure(f) => f.put_file(path, data),
        }
    }

    fn rm(&mut self, path: String) -> suppaftp::FtpResult<()> {
        match self {
            FtpConn::Plain(f) => f.rm(path),
            FtpConn::Secure(f) => f.rm(path),
        }
    }

    fn rmdir(&mut self, path: String) -> suppaftp::FtpResult<()> {
        match self {
            FtpConn::Plain(f) => f.rmdir(path),
            FtpConn::Secure(f) => f.rmdir(path),
        }
    }

    fn rename(&mut self, from: String, to: String) -> suppaftp::FtpResult<()> {
        match self {
            FtpConn::Plain(f) => f.rename(from, to),
            FtpConn::Secure(f) => f.rename(from, to),
        }
    }

    fn custom_command(
        &mut self,
        cmd: String,
        expect: &[suppaftp::Status],
    ) -> suppaftp::FtpResult<suppaftp::types::Response> {
        match self {
            FtpConn::Plain(f) => f.custom_command(cmd, expect),
            FtpConn::Secure(f) => f.custom_command(cmd, expect),
        }
    }

    fn retr_as_buffer(&mut self, path: &str) -> suppaftp::FtpResult<std::io::Cursor<Vec<u8>>> {
        match self {
            FtpConn::Plain(f) => f.retr_as_buffer(path),
            FtpConn::Secure(f) => f.retr_as_buffer(path),
        }
    }
}

/// 解析 ftp:// 或 ftps:// URL：返回 (user, pass, host, port, remote_path)
/// 注：tls 由调用方（vfs::open 按前缀）决定，此函数只解析主机信息。
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
    // 调用方负责剥离 ftps:// 前缀并传入 rest；这里统一返回 tls=false（由前缀决定）
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
    /// 打开 FTP 连接并登录、进入根目录。tls=true 时按 implicit FTPS（端口默认 990）
    pub fn connect(rest: &str, tls: bool) -> io::Result<Self> {
        let (user, pass, host, port, root) = parse_url(rest)?;
        // implicit TLS 默认端口 990（显式指定端口则尊重用户）
        let port = if tls && port == 21 { 990 } else { port };
        Ok(FtpVfs {
            desc: format!("{}://{}", if tls { "ftps" } else { "ftp" }, rest),
            user,
            pass,
            host,
            port,
            root,
            tls,
        })
    }

    /// 建立新连接并登录、进入 root（每次操作调用，用完 quit）
    fn session(&self) -> io::Result<FtpConn> {
        let mut ftp = if self.tls {
            // A6 FTPS：implicit TLS（RFC 4217，直接 TLS 握手，端口默认 990）
            let config = rustls::ClientConfig::builder()
                .with_root_certificates(rustls::RootCertStore::empty())
                .with_no_client_auth();
            let connector = suppaftp::RustlsConnector::from(std::sync::Arc::new(config));
            FtpConn::Secure(
                suppaftp::RustlsFtpStream::connect_secure_implicit(
                    (self.host.as_str(), self.port),
                    connector,
                    &self.host,
                )
                .map_err(|e| {
                    io::Error::new(
                        io::ErrorKind::NotFound,
                        format!("FTPS 连接 {}:{} 失败: {e}", self.host, self.port),
                    )
                })?,
            )
        } else {
            FtpConn::Plain(
                FtpStream::connect((self.host.as_str(), self.port)).map_err(|e| {
                    io::Error::new(
                        io::ErrorKind::NotFound,
                        format!("FTP 连接 {}:{} 失败: {e}", self.host, self.port),
                    )
                })?,
            )
        };
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
        ftp: &mut FtpConn,
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

    fn set_mtime(&self, rel: &str, t: SystemTime) -> io::Result<()> {
        // MFMT（RFC 3659 扩展）设置 mtime：MFMT YYYYMMDDHHMMSS path
        // 服务器不支持（错误码 500/502）时静默降级，不影响同步主流程
        let stamp = mfmt_stamp(t);
        let mut ftp = self.session()?;
        let r = ftp
            .custom_command(
                format!("MFMT {} {}", stamp, join_root(&self.root, rel)),
                &[suppaftp::Status::File], // 213 File status
            )
            .map_err(|e| io::Error::other(format!("FTP 设置 mtime: {e}")));
        let _ = ftp.quit();
        // 不支持 MFMT 的服务器返回 500/502，静默降级为 Ok
        let _ = r;
        Ok(())
    }
}

/// SystemTime → MFMT 时间戳（UTC，YYYYMMDDHHMMSS）
fn mfmt_stamp(t: SystemTime) -> String {
    let secs = t
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let days = secs.div_euclid(86400);
    let rem = secs.rem_euclid(86400);
    let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    // 儒略日 → 公历(Howard Hinnant 算法，与 jsonout.rs 一致)
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let yy = if mo <= 2 { y + 1 } else { y };
    format!("{yy:04}{mo:02}{d:02}{h:02}{m:02}{s:02}")
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
