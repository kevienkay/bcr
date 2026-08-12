//! SVN 虚拟文件系统（A13）。
//!
//! `svn://[user[:pass]@]host[:port]/path` → 通过外部 `svn` 命令行访问 Subversion 仓库。
//!
//! 纯 Rust 无 SVN 协议库，采用外部命令方案（与 BC 的 SVN 集成同思路）：
//! - `scan`：`svn list --xml -R` 递归列出
//! - `read`：`svn cat`
//! - 只读后端：write/delete/rename/set_mtime 返回不支持错误
//!
//! 未安装 `svn` 命令时 connect 报错并给出安装提示。

use super::Vfs;
use crate::fsscan::{FileMeta, Filter};
use std::collections::BTreeMap;
use std::io;
use std::process::Command;
use std::time::SystemTime;

/// SVN 虚拟文件系统
pub struct SvnVfs {
    desc: String,
    /// 仓库 URL（svn://host[:port]/path）
    repo: String,
    /// 认证参数（--username/--password，缺省 None）
    auth: Vec<String>,
}

/// 解析 svn:// URL：返回 (repo_url, auth_args, root_rel)
fn parse_url(rest: &str) -> io::Result<(String, Vec<String>, String)> {
    let (auth, rest) = match rest.rsplit_once('@') {
        Some((a, rest)) => (Some(a), rest),
        None => (None, rest),
    };
    let rest = rest.trim_end_matches('/');
    let (user, pass) = match auth {
        Some(a) => match a.split_once(':') {
            Some((u, p)) => (Some(u.to_string()), Some(p.to_string())),
            None => (Some(a.to_string()), None),
        },
        None => (None, None),
    };
    // host:port/path
    let (host_part, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    if host_part.is_empty() || host_part.starts_with(':') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "svn:// 缺少主机名",
        ));
    }
    let path = if path.is_empty() {
        "/".to_string()
    } else {
        path.to_string()
    };
    let mut auth_args: Vec<String> = Vec::new();
    if let Some(u) = &user {
        auth_args.push("--username".into());
        auth_args.push(u.clone());
    }
    if let Some(p) = &pass {
        auth_args.push("--password".into());
        auth_args.push(p.clone());
    }
    Ok((
        format!("svn://{}{}", host_part, path),
        auth_args,
        String::new(),
    ))
}

impl SvnVfs {
    pub fn connect(rest: &str) -> io::Result<Self> {
        // 探测 svn 命令是否存在
        match Command::new("svn").arg("--version").output() {
            Ok(o) if o.status.success() => {}
            Ok(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    "svn 命令不可用（--version 失败），请安装 Subversion 客户端",
                ));
            }
            Err(e) => {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("未找到 svn 命令: {e}（请安装 Subversion 客户端）"),
                ));
            }
        }
        let (repo, auth, _root) = parse_url(rest)?;
        Ok(SvnVfs {
            desc: format!("svn://{}", rest),
            repo,
            auth,
        })
    }

    /// 执行 svn 命令，返回 stdout（失败时含 stderr 提示）
    fn run(&self, args: &[&str]) -> io::Result<String> {
        let out = Command::new("svn")
            .args(&self.auth)
            .args(args)
            .output()
            .map_err(|e| io::Error::other(format!("svn 执行失败: {e}")))?;
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
            return Err(io::Error::other(format!(
                "svn {} 失败: {}",
                args.first().unwrap_or(&""),
                stderr
            )));
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }

    /// 拼接完整仓库路径（root 相对路径 → 绝对）
    fn abs(&self, rel: &str) -> String {
        if rel.is_empty() {
            return self.repo.clone();
        }
        format!(
            "{}/{}",
            self.repo.trim_end_matches('/'),
            rel.trim_start_matches('/')
        )
    }
}

impl Vfs for SvnVfs {
    fn describe(&self) -> String {
        self.desc.clone()
    }

    fn scan(&self, filter: &Filter) -> io::Result<BTreeMap<String, FileMeta>> {
        // svn list -R 输出相对路径行；目录以 / 结尾
        let out = self.run(&["list", "-R", &self.repo])?;
        let mut map = BTreeMap::new();
        for line in out.lines() {
            let line = line.trim();
            if line.is_empty() || line.ends_with('/') {
                continue; // 目录跳过（scan 只收文件）
            }
            let rel = line.to_string();
            if !filter.accept(&rel) {
                continue;
            }
            let meta = self.meta(&rel)?;
            map.insert(rel, meta);
        }
        Ok(map)
    }

    fn read(&self, rel: &str) -> io::Result<Vec<u8>> {
        let out = Command::new("svn")
            .args(&self.auth)
            .arg("cat")
            .arg(self.abs(rel))
            .output()
            .map_err(|e| io::Error::other(format!("svn cat 失败: {e}")))?;
        if !out.status.success() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("svn cat {} 失败", rel),
            ));
        }
        Ok(out.stdout)
    }

    fn exists(&self, rel: &str) -> io::Result<bool> {
        Ok(self
            .run(&["info", &self.abs(rel)])
            .map(|_| true)
            .unwrap_or(false))
    }

    fn write(&self, _rel: &str, _data: &[u8]) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "SVN 后端只读（写入请用 svn commit）",
        ))
    }

    fn delete(&self, _rel: &str) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "SVN 后端只读（删除请用 svn delete + commit）",
        ))
    }

    fn remove_dir(&self, _rel: &str) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "SVN 后端只读"))
    }

    fn rename(&self, _from: &str, _to: &str) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "SVN 后端只读"))
    }

    fn set_mtime(&self, _rel: &str, _t: SystemTime) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "SVN 后端只读"))
    }
}

impl SvnVfs {
    /// 获取单文件元数据（svn info --xml 解析 size）
    fn meta(&self, rel: &str) -> io::Result<FileMeta> {
        let out = Command::new("svn")
            .args(&self.auth)
            .args(["info", "--xml"])
            .arg(self.abs(rel))
            .output()
            .map_err(|e| io::Error::other(format!("svn info 失败: {e}")))?;
        let xml = String::from_utf8_lossy(&out.stdout);
        // 解析 <size>123</size>
        let size = extract_tag(&xml, "size")
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);
        Ok(FileMeta {
            size,
            mtime: SystemTime::UNIX_EPOCH, // SVN 无本地 mtime 语义
            mode: None,
            symlink: None,
        })
    }
}

/// 提取 XML 标签内容（简易解析，<tag>value</tag>）
fn extract_tag(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let s = xml.find(&open)? + open.len();
    let e = xml[s..].find(&close)? + s;
    Some(xml[s..e].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_url_with_auth() {
        let (repo, auth, root) = parse_url("alice:secret@example.com/svn/proj").unwrap();
        assert_eq!(repo, "svn://example.com/svn/proj");
        assert_eq!(auth, vec!["--username", "alice", "--password", "secret"]);
        assert_eq!(root, "");
    }

    #[test]
    fn parse_url_plain() {
        let (repo, auth, _) = parse_url("svn.example.com/repo").unwrap();
        assert_eq!(repo, "svn://svn.example.com/repo");
        assert!(auth.is_empty());
    }

    #[test]
    fn parse_url_missing_host_errors() {
        assert!(parse_url(":22/x").is_err());
    }

    #[test]
    fn extract_tag_parses_size() {
        let xml = "<entry><size>12345</size></entry>";
        assert_eq!(extract_tag(xml, "size").as_deref(), Some("12345"));
        assert_eq!(extract_tag("<x></x>", "size"), None);
    }
}
