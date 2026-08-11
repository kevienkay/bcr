//! WebDAV 虚拟文件系统。
//!
//! 通过 HTTP WebDAV 协议（PROPFIND/GET/PUT/DELETE/MKCOL/MOVE）把远程目录
//! 当作本地树。URL 规范：
//! - `webdav://[user[:pass]@]host[:port]/remote/path`（HTTP）
//! - `webdavs://[user[:pass]@]host[:port]/remote/path`（HTTPS）
//!
//! 可读写：read/write/delete/rename/remove_dir；Basic Auth；PROPFIND depth=1
//! 递归扫描（兼容大部分服务器，避免 depth=infinity 被拒）。

use super::Vfs;
use crate::fsscan::{FileMeta, Filter};
use reqwest::blocking::Client;
use reqwest::StatusCode;
use std::collections::BTreeMap;
use std::io;
use std::time::{SystemTime, UNIX_EPOCH};

/// WebDAV 虚拟文件系统
pub struct WebdavVfs {
    desc: String,
    /// 客户端（带 Basic Auth 与重定向）
    client: Client,
    /// 根目录 URL（含尾斜杠）
    root: String,
}

/// 解析 webdav:// 或 webdavs:// URL：返回 (scheme, user, pass, base_url, remote_path)
#[allow(clippy::type_complexity)]
fn parse_url(rest: &str) -> io::Result<(String, Option<String>, Option<String>, String, String)> {
    // 兼容 webdavs:// 前缀（带 s）
    let (scheme, rest) = if let Some(r) = rest.strip_prefix("webdavs://") {
        ("https", r)
    } else if let Some(r) = rest.strip_prefix("webdav://") {
        ("http", r)
    } else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "需要 webdav:// 或 webdavs:// 前缀",
        ));
    };
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
            "webdav:// 缺少主机名",
        ));
    }
    let (host, port) = match host_part.rsplit_once(':') {
        Some((h, p)) => {
            if h.is_empty() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "webdav:// 缺少主机名",
                ));
            }
            (
                h.to_string(),
                p.parse::<u16>().map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidInput, "webdav:// 端口无效")
                })?,
            )
        }
        None => (
            host_part.to_string(),
            if scheme == "https" { 443 } else { 80 },
        ),
    };
    let (user, pass) = match auth {
        Some(a) => match a.split_once(':') {
            Some((u, p)) => (Some(u.to_string()), Some(p.to_string())),
            None => (Some(a.to_string()), None),
        },
        None => (None, None),
    };
    let base = format!("{}://{}:{}", scheme, host, port);
    Ok((scheme.to_string(), user, pass, base, path.to_string()))
}

impl WebdavVfs {
    /// 打开 WebDAV 连接（惰性：实际请求时才建立 HTTP 连接）
    pub fn connect(rest: &str) -> io::Result<Self> {
        let (_scheme, user, pass, base, path) = parse_url(rest)?;
        let mut builder = Client::builder().redirect(reqwest::redirect::Policy::limited(5));
        // basic_auth 在 RequestBuilder 上，ClientBuilder 用 default_headers 预置 Authorization
        if let (Some(u), Some(p)) = (&user, &pass) {
            let auth = reqwest::header::HeaderValue::from_str(&format!(
                "Basic {}",
                base64_encode(&format!("{}:{}", u, p))
            ))
            .unwrap_or_else(|_| reqwest::header::HeaderValue::from_static(""));
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert(reqwest::header::AUTHORIZATION, auth);
            builder = builder.default_headers(headers);
        } else if let Some(u) = &user {
            let auth =
                reqwest::header::HeaderValue::from_str(&format!("Basic {}", base64_encode(u)))
                    .unwrap_or_else(|_| reqwest::header::HeaderValue::from_static(""));
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert(reqwest::header::AUTHORIZATION, auth);
            builder = builder.default_headers(headers);
        }
        let client = builder
            .build()
            .map_err(|e| io::Error::other(format!("HTTP 客户端构建失败: {e}")))?;
        // 根 URL：确保尾斜杠
        let root = format!("{}{}/", base, path.trim_end_matches('/'));
        Ok(WebdavVfs {
            desc: format!("webdav://{}", rest),
            client,
            root,
        })
    }

    /// 拼接子资源 URL（rel 相对 root）
    fn url(&self, rel: &str) -> String {
        if rel.is_empty() {
            self.root.clone()
        } else {
            // 逐段拼接避免路径穿越（拒绝 ..）
            if rel.split('/').any(|s| s == "..") {
                return self.root.clone();
            }
            format!("{}{}", self.root, rel)
        }
    }

    /// PROPFIND depth=1 列出目录：返回 (名称, 是否目录, 大小, mtime)
    fn list_dir(&self, url: &str) -> io::Result<Vec<(String, bool, u64, SystemTime)>> {
        let resp = self
            .client
            .request(reqwest::Method::from_bytes(b"PROPFIND").unwrap(), url)
            .header("Depth", "1")
            .header("Content-Type", "application/xml")
            .body(b"<?xml version=\"1.0\"?><d:propfind xmlns:d=\"DAV:\"><d:prop><d:resourcetype/><d:getcontentlength/><d:getlastmodified/></d:prop></d:propfind>".to_vec())
            .send()
            .map_err(|e| io::Error::other(format!("PROPFIND {url} 失败: {e}")))?;
        if !resp.status().is_success() && resp.status() != StatusCode::MULTI_STATUS {
            return Err(io::Error::other(format!(
                "PROPFIND {url} HTTP {}",
                resp.status()
            )));
        }
        let bytes = resp
            .bytes()
            .map_err(|e| io::Error::other(format!("读取 PROPFIND 响应失败: {e}")))?;
        parse_multistatus(&bytes, &self.root)
    }

    /// 递归扫描目录
    fn scan_rec(
        &self,
        url: &str,
        root: &str,
        filter: &Filter,
        out: &mut BTreeMap<String, FileMeta>,
    ) -> io::Result<()> {
        let entries = self.list_dir(url)?;
        for (name, is_dir, size, mtime) in entries {
            if name.is_empty() || name == "." || name == ".." {
                continue;
            }
            let rel = if url == root {
                name.clone()
            } else {
                format!(
                    "{}/{}",
                    url.trim_start_matches(root).trim_start_matches('/'),
                    name
                )
            };
            let rel = rel.trim_start_matches('/').to_string();
            if is_dir {
                if filter.is_excluded(&rel) || filter.is_excluded(&format!("{rel}/")) {
                    continue;
                }
                self.scan_rec(&self.url(&rel), root, filter, out)?;
            } else if filter.accept(&rel) {
                out.insert(
                    rel,
                    FileMeta {
                        size,
                        mtime,
                        mode: None,
                        symlink: None,
                    },
                );
            }
        }
        Ok(())
    }
}

/// 解析 PROPFIND 207 Multi-Status 响应：提取每个条目的 href（最后一段）与属性
fn parse_multistatus(data: &[u8], root: &str) -> io::Result<Vec<(String, bool, u64, SystemTime)>> {
    let mut out: Vec<(String, bool, u64, SystemTime)> = Vec::new();
    let mut reader = quick_xml::Reader::from_reader(data);
    let mut buf = Vec::new();
    // 当前响应条目状态
    let mut cur_href: Option<String> = None;
    let mut cur_is_dir = false;
    let mut cur_size: u64 = 0;
    let mut cur_mtime: Option<SystemTime> = None;
    // 文本收集
    let mut text = String::new();
    let mut in_resourcetype = false;
    let mut in_collection = false;
    let mut in_size = false;
    let mut in_mtime = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(quick_xml::events::Event::Start(e)) => {
                // 去掉命名空间前缀（<d:href> → href）
                let name = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                match name.as_str() {
                    "response" => {
                        cur_href = None;
                        cur_is_dir = false;
                        cur_size = 0;
                        cur_mtime = None;
                    }
                    "resourcetype" => in_resourcetype = true,
                    "collection" => in_collection = true,
                    "getcontentlength" => in_size = true,
                    "getlastmodified" => in_mtime = true,
                    _ => {}
                }
            }
            Ok(quick_xml::events::Event::Text(t)) => {
                text.push_str(&String::from_utf8_lossy(t.as_ref()));
            }
            Ok(quick_xml::events::Event::Empty(e)) => {
                // 自闭合标签（如 <d:collection/>）
                let name = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                if name == "collection" {
                    cur_is_dir = true;
                }
            }
            Ok(quick_xml::events::Event::End(e)) => {
                let name = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                match name.as_str() {
                    "href" => {
                        let href = text.trim().to_string();
                        // 取 URL 最后一段作为名称（解码 %20 等；目录结尾斜杠先去掉）
                        let decoded = percent_decode(&href);
                        let trimmed = decoded.trim_end_matches('/');
                        // 根目录自身（href == root）→ 空名，扫描时跳过
                        let seg = if trimmed == root.trim_end_matches('/') {
                            String::new()
                        } else {
                            trimmed.rsplit('/').next().unwrap_or("").to_string()
                        };
                        cur_href = Some(seg);
                    }
                    "resourcetype" => in_resourcetype = false,
                    "collection" => in_collection = false,
                    "getcontentlength" => {
                        if let Ok(v) = text.trim().parse::<u64>() {
                            cur_size = v;
                        }
                        in_size = false;
                    }
                    "getlastmodified" => {
                        cur_mtime = parse_http_date(text.trim());
                        in_mtime = false;
                    }
                    "response" => {
                        if let Some(name) = cur_href.take() {
                            out.push((name, cur_is_dir, cur_size, cur_mtime.unwrap_or(UNIX_EPOCH)));
                        }
                        cur_is_dir = false;
                    }
                    _ => {}
                }
                text.clear();
            }
            Ok(quick_xml::events::Event::Eof) => break,
            Err(e) => {
                return Err(io::Error::other(format!("PROPFIND XML 解析失败: {e}")));
            }
            _ => {}
        }
        buf.clear();
    }
    let _ = (in_resourcetype, in_collection, in_size, in_mtime);
    let _ = root;
    Ok(out)
}

/// 极简 base64 编码（Basic Auth 用；避免引入额外依赖）
fn base64_encode(data: &str) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes = data.as_bytes();
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[(n >> 18) as usize & 63] as char);
        out.push(TABLE[(n >> 12) as usize & 63] as char);
        if chunk.len() > 1 {
            out.push(TABLE[(n >> 6) as usize & 63] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[n as usize & 63] as char);
        } else {
            out.push('=');
        }
    }
    out
}

/// 百分号解码（%20 → 空格 等）
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push(h * 16 + l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// 解析 HTTP 日期（RFC 1123/850/asctime 简化：只处理 RFC 1123 格式）
fn parse_http_date(s: &str) -> Option<SystemTime> {
    // "Mon, 02 Jan 2006 15:04:05 GMT"
    let s = s.trim();
    let rest = s.split_once(", ").map(|(_, r)| r).unwrap_or(s);
    let mut parts = rest.split_whitespace();
    let mday: u32 = parts.next()?.parse().ok()?; // "02"
    let mon = match parts.next()? {
        "Jan" => 1,
        "Feb" => 2,
        "Mar" => 3,
        "Apr" => 4,
        "May" => 5,
        "Jun" => 6,
        "Jul" => 7,
        "Aug" => 8,
        "Sep" => 9,
        "Oct" => 10,
        "Nov" => 11,
        "Dec" => 12,
        _ => return None,
    };
    let year: i64 = parts.next()?.parse().ok()?;
    let time = parts.next()?;
    let mut tp = time.split(':');
    let hour: u64 = tp.next()?.parse().ok()?;
    let minute: u64 = tp.next()?.parse().ok()?;
    let second: u64 = tp.next()?.parse().ok()?;
    // days-from-civil
    let y = if mon <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (mon + 9) % 12;
    let doy = (153 * mp + 2) / 5 + mday as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    let secs = days * 86400 + hour as i64 * 3600 + minute as i64 * 60 + second as i64;
    if secs >= 0 {
        Some(UNIX_EPOCH + std::time::Duration::from_secs(secs as u64))
    } else {
        Some(UNIX_EPOCH)
    }
}

impl Vfs for WebdavVfs {
    fn describe(&self) -> String {
        self.desc.clone()
    }

    fn scan(&self, filter: &Filter) -> io::Result<BTreeMap<String, FileMeta>> {
        let mut map = BTreeMap::new();
        self.scan_rec(&self.root, &self.root, filter, &mut map)?;
        Ok(map)
    }

    fn read(&self, rel: &str) -> io::Result<Vec<u8>> {
        let url = self.url(rel);
        let resp = self
            .client
            .get(&url)
            .send()
            .map_err(|e| io::Error::other(format!("GET {rel} 失败: {e}")))?;
        if !resp.status().is_success() {
            return Err(io::Error::other(format!(
                "GET {rel} HTTP {}",
                resp.status()
            )));
        }
        resp.bytes()
            .map(|b| b.to_vec())
            .map_err(|e| io::Error::other(format!("读取 {rel} 失败: {e}")))
    }

    fn hash(&self, rel: &str) -> io::Result<blake3::Hash> {
        let data = self.read(rel)?;
        Ok(blake3::hash(&data))
    }

    fn exists(&self, rel: &str) -> io::Result<bool> {
        let url = self.url(rel);
        let resp = self
            .client
            .get(&url)
            .send()
            .map_err(|e| io::Error::other(format!("HEAD {rel} 失败: {e}")))?;
        Ok(resp.status().is_success())
    }

    fn write(&self, rel: &str, data: &[u8]) -> io::Result<()> {
        let url = self.url(rel);
        // 逐级 MKCOL 父目录
        if let Some(parent) = rel.rsplit_once('/') {
            let mut cur = String::new();
            for seg in parent.0.split('/') {
                if seg.is_empty() {
                    continue;
                }
                cur.push_str(seg);
                cur.push('/');
                let _ = self
                    .client
                    .request(
                        reqwest::Method::from_bytes(b"MKCOL").unwrap(),
                        self.url(&cur),
                    )
                    .send();
            }
        }
        let resp = self
            .client
            .put(&url)
            .body(data.to_vec())
            .send()
            .map_err(|e| io::Error::other(format!("PUT {rel} 失败: {e}")))?;
        if !resp.status().is_success() {
            return Err(io::Error::other(format!(
                "PUT {rel} HTTP {}",
                resp.status()
            )));
        }
        Ok(())
    }

    fn delete(&self, rel: &str) -> io::Result<()> {
        let url = self.url(rel);
        let resp = self
            .client
            .delete(&url)
            .send()
            .map_err(|e| io::Error::other(format!("DELETE {rel} 失败: {e}")))?;
        if !resp.status().is_success() {
            return Err(io::Error::other(format!(
                "DELETE {rel} HTTP {}",
                resp.status()
            )));
        }
        Ok(())
    }

    fn remove_dir(&self, rel: &str) -> io::Result<()> {
        // WebDAV 删除目录用 DELETE（部分服务器需先清空）
        self.delete(rel)
    }

    fn rename(&self, from: &str, to: &str) -> io::Result<()> {
        let src = self.url(from);
        let dst = self.url(to);
        let resp = self
            .client
            .request(reqwest::Method::from_bytes(b"MOVE").unwrap(), &src)
            .header("Destination", dst)
            .send()
            .map_err(|e| io::Error::other(format!("MOVE {from} 失败: {e}")))?;
        if !resp.status().is_success() {
            return Err(io::Error::other(format!(
                "MOVE {from} HTTP {}",
                resp.status()
            )));
        }
        Ok(())
    }

    fn set_mtime(&self, _rel: &str, _t: SystemTime) -> io::Result<()> {
        // WebDAV 标准无设置 mtime 的可靠方法（PROPPATCH 非标准），静默忽略
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_url_full_http() {
        let (s, u, p, base, path) =
            parse_url("webdav://alice:secret@example.com:8080/dav/pub").unwrap();
        assert_eq!(s, "http");
        assert_eq!(u.as_deref(), Some("alice"));
        assert_eq!(p.as_deref(), Some("secret"));
        assert_eq!(base, "http://example.com:8080");
        assert_eq!(path, "/dav/pub");
    }

    #[test]
    fn parse_url_https_default_port() {
        let (s, _, _, base, path) = parse_url("webdavs://host/srv").unwrap();
        assert_eq!(s, "https");
        assert_eq!(base, "https://host:443");
        assert_eq!(path, "/srv");
    }

    #[test]
    fn parse_url_no_auth_http_default() {
        let (s, u, p, base, path) = parse_url("webdav://files.example.com/share").unwrap();
        assert_eq!(s, "http");
        assert!(u.is_none());
        assert!(p.is_none());
        assert_eq!(base, "http://files.example.com:80");
        assert_eq!(path, "/share");
    }

    #[test]
    fn parse_url_missing_host_errors() {
        assert!(parse_url("webdav://:8080/x").is_err());
        assert!(parse_url("http://host/x").is_err());
    }

    #[test]
    fn url_join_and_traversal_guard() {
        let v = WebdavVfs::connect("webdav://user@host/root").unwrap();
        assert_eq!(v.url("a.txt"), "http://host:80/root/a.txt");
        assert_eq!(v.url("sub/b.txt"), "http://host:80/root/sub/b.txt");
        assert_eq!(v.url(""), "http://host:80/root/");
        // 拒绝路径穿越
        assert_eq!(v.url("../evil"), "http://host:80/root/");
    }

    #[test]
    fn percent_decode_basic() {
        assert_eq!(percent_decode("a%20b.txt"), "a b.txt");
        assert_eq!(percent_decode("plain.txt"), "plain.txt");
        assert_eq!(percent_decode("100%25"), "100%");
    }

    #[test]
    fn parse_http_date_rfc1123() {
        let t = parse_http_date("Mon, 02 Jan 2006 15:04:05 GMT").unwrap();
        let secs = t.duration_since(UNIX_EPOCH).unwrap().as_secs();
        // 2006-01-02 15:04:05 UTC ≈ 1136214245
        assert!(secs > 1_136_000_000 && secs < 1_137_000_000, "secs={secs}");
    }

    #[test]
    fn multistatus_parses_entries() {
        let xml = br#"<?xml version="1.0"?>
<d:multistatus xmlns:d="DAV:">
  <d:response>
    <d:href>http://host/root/</d:href>
    <d:propstat><d:prop>
      <d:resourcetype><d:collection/></d:resourcetype>
    </d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat>
  </d:response>
  <d:response>
    <d:href>http://host/root/a.txt</d:href>
    <d:propstat><d:prop>
      <d:resourcetype/>
      <d:getcontentlength>1234</d:getcontentlength>
      <d:getlastmodified>Mon, 02 Jan 2006 15:04:05 GMT</d:getlastmodified>
    </d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat>
  </d:response>
  <d:response>
    <d:href>http://host/root/sub/</d:href>
    <d:propstat><d:prop>
      <d:resourcetype><d:collection/></d:resourcetype>
    </d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat>
  </d:response>
</d:multistatus>"#;
        let entries = parse_multistatus(xml, "http://host/root/").unwrap();
        assert_eq!(entries.len(), 3);
        // 根目录
        assert_eq!(entries[0].0, "");
        assert!(entries[0].1);
        // 文件
        assert_eq!(entries[1].0, "a.txt");
        assert!(!entries[1].1);
        assert_eq!(entries[1].2, 1234);
        // 子目录
        assert_eq!(entries[2].0, "sub");
        assert!(entries[2].1);
    }
}
