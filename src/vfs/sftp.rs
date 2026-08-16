//! SFTP 虚拟文件系统。
//!
//! 通过 russh（纯 Rust SSH 实现）连接 SFTP 服务器，把远程目录当作本地树。
//! URL 规范：`sftp://[user[:pass]@]host[:port]/remote/path`
//!
//! host key 校验（C3）：默认 TOFU（Trust On First Use）——
//! 首次连接把服务器 host key 保存到 `~/.bcr-known-hosts`，后续连接校验；
//! 同时加载 `~/.ssh/known_hosts` 参与匹配。
//! 兼容旧行为：`sftp+insecure://` 前缀跳过校验（等价 StrictHostKeyChecking=no）。

use super::Vfs;
use crate::fsscan::{FileMeta, Filter};
use russh::client::Handler;
use russh_sftp::client::SftpSession;
use std::collections::BTreeMap;
use std::io;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// C3 TOFU host key 校验：首次保存到 ~/.bcr-known-hosts，后续校验；
/// 也匹配 ~/.ssh/known_hosts 中已记录的密钥。insecure=true 时跳过校验。
struct KeyVerify {
    /// `[host]:port` 形式的连接目标（known_hosts 条目格式）
    target: String,
    /// 纯主机名（兼容无端口条目）
    host: String,
    /// 跳过校验（sftp+insecure:// 兼容旧行为）
    insecure: bool,
}

impl KeyVerify {
    /// known_hosts 文件路径
    fn known_hosts_paths() -> Vec<std::path::PathBuf> {
        let home = std::env::var("HOME")
            .map(std::path::PathBuf::from)
            .unwrap_or_default();
        vec![home.join(".ssh/known_hosts"), home.join(".bcr-known-hosts")]
    }

    /// 目标是否匹配某条 known_hosts 记录的主机模式（支持 [host]:port、host、host,host 逗号列表）
    fn target_matches(pattern: &str, target: &str, host: &str) -> bool {
        for pat in pattern.split(',') {
            let pat = pat.trim();
            if pat.is_empty() {
                continue;
            }
            if pat == target || pat == host {
                return true;
            }
            // 通配符：* 匹配任意非分隔符序列
            let (pat, host_to_check) = if pat.starts_with('[') {
                // [host]:port 形式，仅匹配主机部分通配
                (pat, target)
            } else {
                (pat, host)
            };
            if wildcard_match(pat, host_to_check) {
                return true;
            }
        }
        false
    }
}

impl Handler for KeyVerify {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &russh::keys::ssh_key::PublicKey,
    ) -> Result<bool, Self::Error> {
        if self.insecure {
            return Ok(true);
        }
        let key_openssh = server_public_key.to_openssh().unwrap_or_default();
        // OpenSSH 格式形如 "ssh-ed25519 AAAA..."；known_hosts 行把类型与密钥
        // 分成两列存储，比较时必须用 base64 段（整串比较永远不相等）
        let (_ktype, key_b64) = match key_openssh.split_once(' ') {
            Some((t, k)) => (t.to_string(), k.to_string()),
            None => (String::new(), key_openssh.clone()),
        };
        let mut matched = false;
        for path in Self::known_hosts_paths() {
            let Ok(content) = std::fs::read_to_string(&path) else {
                continue;
            };
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                if line.starts_with("@cert-authority") || line.starts_with("@revoked") {
                    continue;
                }
                let mut parts = line.split_whitespace();
                let (Some(hosts), Some(_ktype), Some(key)) =
                    (parts.next(), parts.next(), parts.next())
                else {
                    continue;
                };
                if !Self::target_matches(hosts, &self.target, &self.host) {
                    continue;
                }
                matched = true;
                // 密钥一致 → 可信（按 base64 段比较）
                if key == key_b64 {
                    return Ok(true);
                }
            }
        }
        if matched {
            // 有条目但密钥不匹配 → 拒绝（可能中间人攻击或 host key 更换）
            eprintln!(
                "bcr: host key 不匹配 {}(known_hosts 已记录不同密钥；如确认服务器重装，请删除 ~/.bcr-known-hosts 对应条目)",
                self.target
            );
            return Ok(false);
        }
        // TOFU：首次连接，保存到 ~/.bcr-known-hosts 并接受
        let home = std::env::var("HOME")
            .map(std::path::PathBuf::from)
            .unwrap_or_default();
        let path = home.join(".bcr-known-hosts");
        // 三段格式（target 类型 base64），与 known_hosts 一致，可被自身读取回环
        let line = format!("{} {} {}\n", self.target, _ktype, key_b64);
        if std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .and_then(|mut f| std::io::Write::write_all(&mut f, line.as_bytes()))
            .is_ok()
        {
            eprintln!(
                "bcr: 首次连接，host key 已保存到 {}（TOFU）",
                path.display()
            );
        }
        Ok(true)
    }
}

/// 简单通配符匹配：* 匹配任意字符序列（不含路径分隔符语义，足够 host 匹配用）
fn wildcard_match(pattern: &str, s: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = s.chars().collect();
    let (mut pi, mut ti) = (0usize, 0usize);
    let (mut star, mut mark) = (usize::MAX, 0usize);
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = pi;
            mark = ti;
            pi += 1;
        } else if star != usize::MAX {
            pi = star + 1;
            mark += 1;
            ti = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
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
    /// 连接并建立 SFTP 会话（阻塞）。insecure=true 时跳过 host key 校验。
    pub fn connect(url_rest: &str) -> io::Result<Self> {
        Self::connect_impl(url_rest, false)
    }

    /// sftp+insecure:// 前缀：跳过 host key 校验（兼容旧行为）
    pub fn connect_insecure(url_rest: &str) -> io::Result<Self> {
        Self::connect_impl(url_rest, true)
    }

    fn connect_impl(url_rest: &str, insecure: bool) -> io::Result<Self> {
        let (user, pass, host, port, root) = parse_url(url_rest)?;
        let desc = format!("sftp://{user}@{host}:{port}{root}");

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| io::Error::other(format!("tokio runtime: {e}")))?;

        let session = rt.block_on(async {
            let config = Arc::new(russh::client::Config::default());
            let target = format!("[{}]:{}", host, port);
            let mut session = russh::client::connect(
                config,
                (host.as_str(), port),
                KeyVerify {
                    target,
                    host: host.clone(),
                    insecure,
                },
            )
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
                        mode: None,
                        symlink: None,
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

    // ---- C3 host key 校验（纯逻辑） ----

    #[test]
    fn wildcard_match_basic() {
        assert!(wildcard_match("*", "anything"));
        assert!(wildcard_match("git*.com", "github.com"));
        assert!(wildcard_match("*.example.com", "a.example.com"));
        assert!(!wildcard_match("*.example.com", "example.org"));
        assert!(wildcard_match("host", "host"));
        assert!(!wildcard_match("host", "other"));
        assert!(wildcard_match("h?st", "host"));
        assert!(!wildcard_match("h?st", "haast"));
    }

    #[test]
    fn target_matches_port_and_wildcard() {
        // [host]:port 精确匹配
        assert!(KeyVerify::target_matches(
            "[example.com]:22",
            "[example.com]:22",
            "example.com"
        ));
        // 纯 host 条目匹配（忽略端口）
        assert!(KeyVerify::target_matches(
            "example.com",
            "[example.com]:22",
            "example.com"
        ));
        // 逗号列表
        assert!(KeyVerify::target_matches(
            "a.com,b.com",
            "[b.com]:2222",
            "b.com"
        ));
        // 通配符 host 条目
        assert!(KeyVerify::target_matches(
            "*.example.com",
            "[sub.example.com]:22",
            "sub.example.com"
        ));
        // 不匹配
        assert!(!KeyVerify::target_matches(
            "other.com",
            "[example.com]:22",
            "example.com"
        ));
        // 空模式
        assert!(!KeyVerify::target_matches(
            "",
            "[example.com]:22",
            "example.com"
        ));
    }

    // ---- C3 host key 匹配修复（回归） ----

    /// 固定 ed25519 测试公钥（ssh-key crate 测试向量，authorized_keys 格式）
    const TEST_KEY_A: &str =
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAILM+rvN+ot98qgEN796jTiQfZfG1KaT0PtFDJ/XFSqti";
    /// 另一把 ed25519 公钥（密钥不同，用于不匹配用例）
    const TEST_KEY_B: &str =
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIB9dG4kjRhQTtWTVzd2t27+t0DEHBPW7iOD23TUiYLio";

    fn test_pubkey(openssh: &str) -> russh::keys::ssh_key::PublicKey {
        russh::keys::ssh_key::PublicKey::from_openssh(openssh).expect("固定测试密钥应可解析")
    }

    /// HOME 环境变量互斥锁：三个 host key 测试都改写 HOME，
    /// 并行执行会互相污染（读到对方临时目录的 known_hosts），必须串行。
    /// 用 tokio Mutex：await 点持有 std MutexGuard 会被 clippy 拒绝。
    static HOME_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    #[tokio::test]
    async fn host_key_matches_known_hosts_entry() {
        let _guard = HOME_LOCK.lock().await;
        // 已知主机（~/.ssh/known_hosts 记录正确密钥）→ 应接受。
        // 回归：此前用整串 openssh 格式比较，known_hosts 的 base64 段永远不等 → 误拒。
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("HOME", dir.path());
        let pk = test_pubkey(TEST_KEY_A);
        let b64 = TEST_KEY_A.rsplit(' ').next().unwrap();
        let kh_dir = dir.path().join(".ssh");
        std::fs::create_dir_all(&kh_dir).unwrap();
        std::fs::write(
            kh_dir.join("known_hosts"),
            format!("[example.com]:2222 ssh-ed25519 {}\n", b64),
        )
        .unwrap();
        let mut kv = KeyVerify {
            target: "[example.com]:2222".to_string(),
            host: "example.com".to_string(),
            insecure: false,
        };
        assert!(
            kv.check_server_key(&pk).await.unwrap(),
            "known_hosts 中正确密钥应通过校验"
        );
        std::env::remove_var("HOME");
    }

    #[tokio::test]
    async fn host_key_mismatch_rejected() {
        let _guard = HOME_LOCK.lock().await;
        // 已知主机但密钥不同（中间人/换钥）→ 拒绝
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("HOME", dir.path());
        let pk = test_pubkey(TEST_KEY_A);
        let b642 = TEST_KEY_B.rsplit(' ').next().unwrap();
        let kh_dir = dir.path().join(".ssh");
        std::fs::create_dir_all(&kh_dir).unwrap();
        std::fs::write(
            kh_dir.join("known_hosts"),
            format!("[example.com]:2222 ssh-ed25519 {}\n", b642),
        )
        .unwrap();
        let mut kv = KeyVerify {
            target: "[example.com]:2222".to_string(),
            host: "example.com".to_string(),
            insecure: false,
        };
        assert!(
            !kv.check_server_key(&pk).await.unwrap(),
            "密钥不同应拒绝连接"
        );
        std::env::remove_var("HOME");
    }

    #[tokio::test]
    async fn host_key_tofu_saves_and_roundtrips() {
        let _guard = HOME_LOCK.lock().await;
        // TOFU：首次连接保存三段格式（target 类型 base64），第二次连接读回可匹配
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("HOME", dir.path());
        let pk = test_pubkey(TEST_KEY_A);
        let mut kv = KeyVerify {
            target: "[tofu.example.com]:22".to_string(),
            host: "tofu.example.com".to_string(),
            insecure: false,
        };
        // 首次：无记录 → TOFU 接受并保存
        assert!(kv.check_server_key(&pk).await.unwrap());
        let saved = std::fs::read_to_string(dir.path().join(".bcr-known-hosts")).unwrap();
        let toks: Vec<&str> = saved.split_whitespace().collect();
        assert_eq!(toks.len(), 3, "TOFU 保存应为三段格式: {saved}");
        assert_eq!(toks[0], "[tofu.example.com]:22");
        assert_eq!(toks[1], "ssh-ed25519");
        // 第二次：读回保存的条目 → 密钥一致 → 接受（回归：整串比较会误拒）
        assert!(
            kv.check_server_key(&pk).await.unwrap(),
            "TOFU 保存后二次连接应通过校验"
        );
        std::env::remove_var("HOME");
    }
}
