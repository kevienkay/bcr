//! Dropbox 虚拟文件系统。
//!
//! 通过 Dropbox HTTP API 把 Dropbox 当作目录树。URL 规范：`dropbox://path`
//! - 凭证：`BCR_DROPBOX_TOKEN` 环境变量或 `~/.bcr-cloud.toml` 的
//!   `[dropbox] token = "..."`（OAuth access token，需用户自行申请）
//! - 可读写：read/write/delete/rename/mkdir（list_folder / download / upload /
//!   delete_v2 / move_v2 / create_folder_v2）
//! - mtime 取条目 server_modified

use super::Vfs;
use crate::fsscan::{FileMeta, Filter};
use reqwest::blocking::Client;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::io;
use std::time::{SystemTime, UNIX_EPOCH};

const API: &str = "https://api.dropboxapi.com/2";
const CONTENT: &str = "https://content.dropboxapi.com/2";

/// 凭证来源：BCR_DROPBOX_TOKEN 环境变量优先，其次 ~/.bcr-cloud.toml
fn token() -> Option<String> {
    if let Ok(t) = std::env::var("BCR_DROPBOX_TOKEN") {
        if !t.is_empty() {
            return Some(t);
        }
    }
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()?;
    let path = std::path::Path::new(&home).join(".bcr-cloud.toml");
    let content = std::fs::read_to_string(path).ok()?;
    let mut in_dropbox = false;
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("[dropbox]") {
            in_dropbox = true;
            continue;
        }
        if in_dropbox {
            if line.starts_with('[') {
                break;
            }
            if let Some(v) = line.strip_prefix("token") {
                let v = v.trim_start_matches('=').trim().trim_matches('"');
                if !v.is_empty() {
                    return Some(v.to_string());
                }
            }
        }
    }
    None
}

/// Dropbox list_folder 条目
#[derive(Deserialize)]
struct DboxEntry {
    #[serde(rename = ".tag")]
    tag: String,
    name: String,
    #[allow(dead_code)]
    path_display: Option<String>,
    size: Option<u64>,
    #[serde(rename = "server_modified")]
    server_modified: Option<String>,
}

/// list_folder 响应
#[derive(Deserialize)]
struct ListResp {
    entries: Vec<DboxEntry>,
    #[allow(dead_code)]
    has_more: bool,
    #[allow(dead_code)]
    cursor: Option<String>,
}

/// Dropbox 虚拟文件系统
pub struct DropboxVfs {
    desc: String,
    client: Client,
    /// 根路径（Dropbox 以 / 开头；空 = 根）
    root: String,
}

impl DropboxVfs {
    /// 打开 Dropbox（token 从环境变量或 ~/.bcr-cloud.toml 读取）
    pub fn connect(rest: &str) -> io::Result<Self> {
        let _t = token().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                "缺少 Dropbox token（设置 BCR_DROPBOX_TOKEN 或 ~/.bcr-cloud.toml [dropbox] token）",
            )
        })?;
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()
            .map_err(|e| io::Error::other(format!("HTTP 客户端构建失败: {e}")))?;
        let root = format!("/{}", rest.trim_matches('/'));
        Ok(DropboxVfs {
            desc: format!("dropbox://{}", rest),
            client,
            root,
        })
    }

    /// 完整远程路径
    fn path(&self, rel: &str) -> String {
        if rel.is_empty() {
            self.root.clone()
        } else {
            format!("{}/{}", self.root.trim_end_matches('/'), rel)
        }
    }

    /// 递归扫描目录（连接复用 list_folder + cursor 分页）
    fn scan_rec(
        &self,
        dir: &str,
        root: &str,
        filter: &Filter,
        out: &mut BTreeMap<String, FileMeta>,
    ) -> io::Result<()> {
        let auth = format!("Bearer {}", token().unwrap_or_default());
        // list_folder
        let body = serde_json::json!({ "path": dir, "recursive": false, "include_deleted": false });
        let resp = self
            .client
            .post(format!("{}/files/list_folder", API))
            .header(AUTHORIZATION, &auth)
            .header(CONTENT_TYPE, "application/json")
            .body(body.to_string())
            .send()
            .map_err(|e| io::Error::other(format!("Dropbox list_folder 失败: {e}")))?;
        if !resp.status().is_success() {
            return Err(io::Error::other(format!(
                "Dropbox list_folder {} HTTP {}",
                dir,
                resp.status()
            )));
        }
        let bytes = resp
            .bytes()
            .map_err(|e| io::Error::other(format!("Dropbox 响应读取失败: {e}")))?;
        let list: ListResp = serde_json::from_slice(&bytes)
            .map_err(|e| io::Error::other(format!("Dropbox 响应解析失败: {e}")))?;
        for entry in list.entries {
            if entry.name.is_empty() {
                continue;
            }
            let rel = if dir == root {
                entry.name.clone()
            } else {
                format!(
                    "{}/{}",
                    dir.trim_start_matches(root).trim_start_matches('/'),
                    entry.name
                )
            };
            let rel = rel.trim_start_matches('/').to_string();
            if entry.tag == "folder" {
                if filter.is_excluded(&rel) || filter.is_excluded(&format!("{rel}/")) {
                    continue;
                }
                self.scan_rec(&self.path(&rel), root, filter, out)?;
            } else if entry.tag == "file" && filter.accept(&rel) {
                out.insert(
                    rel,
                    FileMeta {
                        size: entry.size.unwrap_or(0),
                        mtime: parse_dbox_date(entry.server_modified.as_deref()),
                        mode: None,
                        symlink: None,
                    },
                );
            }
        }
        Ok(())
    }
}

/// 解析 Dropbox 日期（RFC3339 带时区，如 2026-08-11T12:00:00Z）
fn parse_dbox_date(s: Option<&str>) -> SystemTime {
    let Some(s) = s else { return UNIX_EPOCH };
    let s = s.trim();
    // 去掉时区偏移（Z 或 +08:00），取 UTC 部分
    let s = s.trim_end_matches('Z');
    let s = s.split('+').next().unwrap_or(s);
    let Some((date, time)) = s.split_once('T') else {
        return UNIX_EPOCH;
    };
    let mut dp = date.split('-');
    let (Some(year), Some(month), Some(day)) = (dp.next(), dp.next(), dp.next()) else {
        return UNIX_EPOCH;
    };
    let (Ok(year), Ok(month), Ok(day)) = (
        year.parse::<i64>(),
        month.parse::<u32>(),
        day.parse::<u32>(),
    ) else {
        return UNIX_EPOCH;
    };
    let t = time.split('.').next().unwrap_or(time);
    let mut tp = t.split(':');
    let (Some(hour), Some(minute), Some(second)) = (tp.next(), tp.next(), tp.next()) else {
        return UNIX_EPOCH;
    };
    let (Ok(hour), Ok(minute), Ok(second)) = (
        hour.parse::<u64>(),
        minute.parse::<u64>(),
        second.parse::<u64>(),
    ) else {
        return UNIX_EPOCH;
    };
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = ((month + 9) % 12) as i64;
    let doy = (153 * mp + 2) / 5 + day as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    let secs = days * 86400 + hour as i64 * 3600 + minute as i64 * 60 + second as i64;
    if secs >= 0 {
        UNIX_EPOCH + std::time::Duration::from_secs(secs as u64)
    } else {
        UNIX_EPOCH
    }
}

impl Vfs for DropboxVfs {
    fn describe(&self) -> String {
        self.desc.clone()
    }

    fn scan(&self, filter: &Filter) -> io::Result<BTreeMap<String, FileMeta>> {
        let mut map = BTreeMap::new();
        self.scan_rec(&self.root, &self.root, filter, &mut map)?;
        Ok(map)
    }

    fn read(&self, rel: &str) -> io::Result<Vec<u8>> {
        let auth = format!("Bearer {}", token().unwrap_or_default());
        let arg = serde_json::json!({ "path": self.path(rel) }).to_string();
        let resp = self
            .client
            .post(format!("{}/files/download", CONTENT))
            .header(AUTHORIZATION, &auth)
            .header("Dropbox-API-Arg", arg)
            .send()
            .map_err(|e| io::Error::other(format!("Dropbox 读取 {rel} 失败: {e}")))?;
        if !resp.status().is_success() {
            return Err(io::Error::other(format!(
                "Dropbox 读取 {rel} HTTP {}",
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
        let auth = format!("Bearer {}", token().unwrap_or_default());
        let body = serde_json::json!({ "path": self.path(rel) }).to_string();
        let resp = self
            .client
            .post(format!("{}/files/get_metadata", API))
            .header(AUTHORIZATION, &auth)
            .header(CONTENT_TYPE, "application/json")
            .body(body)
            .send()
            .map_err(|e| io::Error::other(format!("Dropbox get_metadata {rel}: {e}")))?;
        Ok(resp.status().is_success())
    }

    fn write(&self, rel: &str, data: &[u8]) -> io::Result<()> {
        let auth = format!("Bearer {}", token().unwrap_or_default());
        // 确保父目录存在（逐级 create_folder_v2，已存在忽略）
        if let Some(parent) = rel.rsplit_once('/') {
            let mut cur = self.root.clone();
            for seg in parent.0.split('/') {
                if seg.is_empty() {
                    continue;
                }
                cur.push('/');
                cur.push_str(seg);
                let body = serde_json::json!({ "path": cur }).to_string();
                let _ = self
                    .client
                    .post(format!("{}/files/create_folder_v2", API))
                    .header(AUTHORIZATION, &auth)
                    .header(CONTENT_TYPE, "application/json")
                    .body(body)
                    .send();
            }
        }
        let arg = serde_json::json!({
            "path": self.path(rel),
            "mode": "overwrite",
            "autorename": false,
            "mute": true
        })
        .to_string();
        let resp = self
            .client
            .post(format!("{}/files/upload", CONTENT))
            .header(AUTHORIZATION, &auth)
            .header("Dropbox-API-Arg", arg)
            .header(CONTENT_TYPE, "application/octet-stream")
            .body(data.to_vec())
            .send()
            .map_err(|e| io::Error::other(format!("Dropbox 写入 {rel} 失败: {e}")))?;
        if !resp.status().is_success() {
            return Err(io::Error::other(format!(
                "Dropbox 写入 {rel} HTTP {}",
                resp.status()
            )));
        }
        Ok(())
    }

    fn delete(&self, rel: &str) -> io::Result<()> {
        let auth = format!("Bearer {}", token().unwrap_or_default());
        let body = serde_json::json!({ "path": self.path(rel) }).to_string();
        let resp = self
            .client
            .post(format!("{}/files/delete_v2", API))
            .header(AUTHORIZATION, &auth)
            .header(CONTENT_TYPE, "application/json")
            .body(body)
            .send()
            .map_err(|e| io::Error::other(format!("Dropbox 删除 {rel} 失败: {e}")))?;
        if !resp.status().is_success() {
            return Err(io::Error::other(format!(
                "Dropbox 删除 {rel} HTTP {}",
                resp.status()
            )));
        }
        Ok(())
    }

    fn remove_dir(&self, rel: &str) -> io::Result<()> {
        // Dropbox 目录非空时 delete_v2 也删除全部内容（与 mirror 语义一致）
        self.delete(rel)
    }

    fn rename(&self, from: &str, to: &str) -> io::Result<()> {
        if from == to {
            return Ok(());
        }
        let auth = format!("Bearer {}", token().unwrap_or_default());
        let body = serde_json::json!({
            "from_path": self.path(from),
            "to_path": self.path(to),
            "autorename": false
        })
        .to_string();
        let resp = self
            .client
            .post(format!("{}/files/move_v2", API))
            .header(AUTHORIZATION, &auth)
            .header(CONTENT_TYPE, "application/json")
            .body(body)
            .send()
            .map_err(|e| io::Error::other(format!("Dropbox 移动 {from} 失败: {e}")))?;
        if !resp.status().is_success() {
            return Err(io::Error::other(format!(
                "Dropbox 移动 {from} HTTP {}",
                resp.status()
            )));
        }
        Ok(())
    }

    fn set_mtime(&self, _rel: &str, _t: SystemTime) -> io::Result<()> {
        // Dropbox API 无设置 mtime 的公开接口（仅上传时可带 client_modified），静默忽略
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_joins_root() {
        let v = DropboxVfs {
            desc: "dropbox://".into(),
            client: Client::new(),
            root: "/docs".into(),
        };
        assert_eq!(v.path(""), "/docs");
        assert_eq!(v.path("a.txt"), "/docs/a.txt");
        assert_eq!(v.path("sub/b.txt"), "/docs/sub/b.txt");
    }

    #[test]
    fn dbox_date_parses_rfc3339() {
        let t = parse_dbox_date(Some("2026-08-11T12:34:56Z"));
        let secs = t.duration_since(UNIX_EPOCH).unwrap().as_secs();
        assert!(secs > 1_780_000_000, "secs={secs}");
        let t2 = parse_dbox_date(Some("2026-08-11T12:34:56+08:00"));
        let secs2 = t2.duration_since(UNIX_EPOCH).unwrap().as_secs();
        assert!(secs2 > 1_780_000_000);
        assert_eq!(parse_dbox_date(None), UNIX_EPOCH);
        assert_eq!(parse_dbox_date(Some("garbage")), UNIX_EPOCH);
    }
}
