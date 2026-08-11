//! SFTP 虚拟文件系统。
//!
//! 通过 russh（纯 Rust SSH 实现）连接 SFTP 服务器，把远程目录当作本地树。
//! URL 规范：`sftp://[user[:pass]@]host[:port]/remote/path`
//!
//! 注意：首次连接不校验服务器 host key（等价 `StrictHostKeyChecking=no`），
//! 仅适用于受信环境；生产使用请自行校验。

use super::Vfs;
use crate::fsscan::{FileMeta, Filter};
use russh::client::Handler;
use russh_sftp::client::SftpSession;
use std::collections::BTreeMap;
use std::io;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// 不校验 host key 的 handler（M6 最小实现）
struct NoVerify;

impl Handler for NoVerify {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &russh::keys::ssh_key::PublicKey,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

/// 解析 sftp:// URL：返回 (user, pass, host, port, remote_path)
fn parse_url(rest: &str) -> io::Result<(String, Option<String>, String, u16, String)> {
    let rest = rest.trim_end_matches('/');
    let (auth, host_port_path) = match rest.rsplit_once('@') {
        Some((a, rest)) => (Some(a), rest),
        None => (None, rest),
    };
    let (host_part, path) = match host_port_path.find('/') {
        Some(i) => (&host_port_path[..i], &host_port_path[i..]),
        None => (host_port_path, "/"),
    };
    let (host, port) = match host_part.rsplit_once(':') {
        Some((h, p)) if p.chars().all(|c| c.is_ascii_digit()) => (
            h,
            p.parse::<u16>()
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "非法端口"))?,
        ),
        _ => (host_part, 22),
    };
    if host.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "sftp:// 缺少主机名",
        ));
    }
    let (user, pass) = match auth {
        Some(a) => match a.split_once(':') {
            Some((u, p)) => (u.to_string(), Some(p.to_string())),
            None => (a.to_string(), None),
        },
        None => ("root".to_string(), None),
    };
    let path = if path.is_empty() {
        "/".to_string()
    } else {
        path.to_string()
    };
    Ok((user, pass, host.to_string(), port, path))
}

/// 拼接远程绝对路径（独立函数便于测试）
fn join_root(root: &str, rel: &str) -> String {
    let root = root.trim_end_matches('/');
    if rel.is_empty() || rel == "." {
        return root.to_string();
    }
    let rel = rel.trim_start_matches('/');
    if rel.is_empty() {
        return root.to_string();
    }
    format!("{root}/{rel}")
}

pub struct SftpVfs {
    /// URL 描述
    desc: String,
    /// tokio runtime（block_on 包装异步 russh 调用）
    rt: tokio::runtime::Runtime,
    session: SftpSession,
    /// 远程根路径（绝对路径）
    root: String,
}

impl SftpVfs {
    /// 连接并建立 SFTP 会话（阻塞）
    pub fn connect(url_rest: &str) -> io::Result<Self> {
        let (user, pass, host, port, root) = parse_url(url_rest)?;
        let desc = format!("sftp://{user}@{host}:{port}{root}");

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| io::Error::other(format!("tokio runtime: {e}")))?;

        let session = rt.block_on(async {
            let config = Arc::new(russh::client::Config::default());
            let mut session = russh::client::connect(config, (host.as_str(), port), NoVerify)
                .await
                .map_err(|e| io::Error::other(format!("SSH 连接失败: {e}")))?;

            let authed = match &pass {
                Some(p) => session.authenticate_password(&user, p).await.map_err(|e| {
                    io::Error::new(io::ErrorKind::PermissionDenied, format!("认证失败: {e}"))
                })?,
                None => session.authenticate_none(&user).await.map_err(|e| {
                    io::Error::new(io::ErrorKind::PermissionDenied, format!("认证失败: {e}"))
                })?,
            };
            if !authed.success() {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "SSH 认证未通过（URL 需包含密码或用密钥）",
                ));
            }

            let channel = session
                .channel_open_session()
                .await
                .map_err(|e| io::Error::other(format!("通道失败: {e}")))?;
            let stream = channel.into_stream();
            SftpSession::new(stream)
                .await
                .map_err(|e| io::Error::other(format!("SFTP 握手失败: {e}")))
        })?;

        Ok(SftpVfs {
            desc,
            rt,
            session,
            root,
        })
    }

    /// 拼接远程绝对路径
    fn abs(&self, rel: &str) -> String {
        join_root(&self.root, rel)
    }

    /// 递归扫描目录（相对路径键）
    fn scan_rec(
        &self,
        dir: &str,
        filter: &Filter,
        out: &mut BTreeMap<String, FileMeta>,
    ) -> io::Result<()> {
        let rd = self
            .rt
            .block_on(self.session.read_dir(dir))
            .map_err(|e| io::Error::other(format!("read_dir {dir}: {e}")))?;
        for entry in rd {
            let name = entry.file_name();
            if name == "." || name == ".." {
                continue;
            }
            let rel = if dir == self.root {
                name.clone()
            } else {
                format!(
                    "{}/{}",
                    dir.trim_start_matches(&self.root).trim_start_matches('/'),
                    name
                )
            };
            let rel = rel.trim_start_matches('/').to_string();
            let meta = entry.metadata();
            if meta.file_type().is_dir() {
                if filter.is_excluded(&rel) || filter.is_excluded(&format!("{rel}/")) {
                    continue;
                }
                self.scan_rec(&self.abs(&rel), filter, out)?;
            } else if meta.file_type().is_file() && filter.accept(&rel) {
                out.insert(
                    rel,
                    FileMeta {
                        size: meta.len(),
                        mtime: meta.modified().unwrap_or(UNIX_EPOCH),
                    },
                );
            }
        }
        Ok(())
    }
}

impl Vfs for SftpVfs {
    fn describe(&self) -> String {
        self.desc.clone()
    }

    fn scan(&self, filter: &Filter) -> io::Result<BTreeMap<String, FileMeta>> {
        let mut map = BTreeMap::new();
        self.scan_rec(&self.root, filter, &mut map)?;
        Ok(map)
    }

    fn read(&self, rel: &str) -> io::Result<Vec<u8>> {
        self.rt
            .block_on(self.session.read(self.abs(rel)))
            .map_err(|e| io::Error::other(format!("读取 {rel}: {e}")))
    }

    fn exists(&self, rel: &str) -> io::Result<bool> {
        self.rt
            .block_on(self.session.try_exists(self.abs(rel)))
            .map_err(|e| io::Error::other(format!("exists {rel}: {e}")))
    }

    fn write(&self, rel: &str, data: &[u8]) -> io::Result<()> {
        // 逐级创建父目录
        let abs = self.abs(rel);
        let parent = abs
            .rsplit_once('/')
            .map(|(p, _)| p.to_string())
            .unwrap_or_default();
        if !parent.is_empty() {
            let mut cur = String::new();
            for seg in parent.split('/') {
                if seg.is_empty() {
                    continue;
                }
                cur.push('/');
                cur.push_str(seg);
                let _ = self.rt.block_on(self.session.create_dir(cur.clone()));
            }
        }
        self.rt
            .block_on(self.session.write(abs, data))
            .map_err(|e| io::Error::other(format!("写入 {rel}: {e}")))
    }

    fn delete(&self, rel: &str) -> io::Result<()> {
        self.rt
            .block_on(self.session.remove_file(self.abs(rel)))
            .map_err(|e| io::Error::other(format!("删除 {rel}: {e}")))
    }

    fn remove_dir(&self, rel: &str) -> io::Result<()> {
        self.rt
            .block_on(self.session.remove_dir(self.abs(rel)))
            .map_err(|e| io::Error::other(format!("删除目录 {rel}: {e}")))
    }

    fn rename(&self, from: &str, to: &str) -> io::Result<()> {
        // 确保目标父目录存在（与 write 一致的逐级建目录逻辑）
        let dst_abs = self.abs(to);
        let parent = dst_abs
            .rsplit_once('/')
            .map(|(p, _)| p.to_string())
            .unwrap_or_default();
        if !parent.is_empty() {
            let mut cur = String::new();
            for seg in parent.split('/') {
                if seg.is_empty() {
                    continue;
                }
                cur.push('/');
                cur.push_str(seg);
                let _ = self.rt.block_on(self.session.create_dir(cur.clone()));
            }
        }
        self.rt
            .block_on(self.session.rename(self.abs(from), dst_abs))
            .map_err(|e| io::Error::other(format!("重命名 {from} -> {to}: {e}")))
    }

    fn set_mtime(&self, rel: &str, t: SystemTime) -> io::Result<()> {
        // 用 unix 秒构造 FileAttributes（SFTP mtime 为 u32 秒）
        let mut attrs = russh_sftp::protocol::FileAttributes::empty();
        let secs = t
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            .min(u32::MAX as u64) as u32;
        attrs.mtime = Some(secs);
        self.rt
            .block_on(self.session.set_metadata(self.abs(rel), attrs))
            .map_err(|e| io::Error::other(format!("set_mtime {rel}: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_url_full() {
        let (u, p, h, port, path) =
            parse_url("alice:secret@example.com:2222/home/alice/proj").unwrap();
        assert_eq!(u, "alice");
        assert_eq!(p.as_deref(), Some("secret"));
        assert_eq!(h, "example.com");
        assert_eq!(port, 2222);
        assert_eq!(path, "/home/alice/proj");
    }

    #[test]
    fn parse_url_no_pass() {
        let (u, p, h, port, path) = parse_url("bob@example.com/srv").unwrap();
        assert_eq!(u, "bob");
        assert!(p.is_none());
        assert_eq!(h, "example.com");
        assert_eq!(port, 22);
        assert_eq!(path, "/srv");
    }

    #[test]
    fn parse_url_no_auth_default_user() {
        let (u, _, h, port, path) = parse_url("example.com:2222/data").unwrap();
        assert_eq!(u, "root");
        assert_eq!(h, "example.com");
        assert_eq!(port, 2222);
        assert_eq!(path, "/data");
    }

    #[test]
    fn parse_url_missing_host_errors() {
        assert!(parse_url(":2222/x").is_err());
    }

    #[test]
    fn parse_url_trailing_slash() {
        let (_, _, _, _, path) = parse_url("user@host/data/").unwrap();
        assert_eq!(path, "/data");
    }

    #[test]
    fn abs_joins_root() {
        assert_eq!(join_root("/root", "a.txt"), "/root/a.txt");
        assert_eq!(join_root("/root", "sub/b.txt"), "/root/sub/b.txt");
        assert_eq!(join_root("/root", ""), "/root");
        assert_eq!(join_root("/root/", "x.txt"), "/root/x.txt");
        assert_eq!(join_root("/", "a.txt"), "/a.txt");
    }
}
