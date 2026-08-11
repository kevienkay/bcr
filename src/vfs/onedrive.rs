//! OneDrive 虚拟文件系统。
//!
//! 通过 Microsoft Graph API 把 OneDrive 当作目录树。URL 规范：`onedrive://path`
//! - 凭证：`BCR_ONEDRIVE_TOKEN` 环境变量或 `~/.bcr-cloud.toml` 的
//!   `[onedrive] token = "..."`（OAuth access token，需用户自行申请）
//! - 可读写：read/write/delete/rename/mkdir；mtime 取 Graph 的 lastModifiedDateTime
//! - 路径规范：`/me/drive/root:/path` 冒号语法

use super::Vfs;
use crate::fsscan::{FileMeta, Filter};
use reqwest::blocking::Client;
use reqwest::header::AUTHORIZATION;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::io;
use std::time::{SystemTime, UNIX_EPOCH};

/// 凭证来源：BCR_ONEDRIVE_TOKEN 环境变量优先，其次 ~/.bcr-cloud.toml
fn token() -> Option<String> {
    if let Ok(t) = std::env::var("BCR_ONEDRIVE_TOKEN") {
        if !t.is_empty() {
            return Some(t);
        }
    }
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()?;
    let path = std::path::Path::new(&home).join(".bcr-cloud.toml");
    let content = std::fs::read_to_string(path).ok()?;
    // 简单解析 [onedrive] 段的 token
    let mut in_onedrive = false;
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("[onedrive]") {
            in_onedrive = true;
            continue;
        }
        if in_onedrive {
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

/// Graph API 目录项
#[derive(Deserialize)]
struct GraphItem {
    name: String,
    folder: Option<serde_json::Value>,
    size: Option<u64>,
    #[serde(rename = "lastModifiedDateTime")]
    last_modified: Option<String>,
}

/// Graph API children 响应
#[derive(Deserialize)]
struct ChildrenResp {
    value: Vec<GraphItem>,
}

/// OneDrive 虚拟文件系统
pub struct OneDriveVfs {
    desc: String,
    client: Client,
    /// Graph 根路径（`root:/path:` 形式，尾冒号）
    root: String,
}

impl OneDriveVfs {
    /// 打开 OneDrive（token 从环境变量或 ~/.bcr-cloud.toml 读取）
    pub fn connect(rest: &str) -> io::Result<Self> {
        let _t = token().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                "缺少 OneDrive token（设置 BCR_ONEDRIVE_TOKEN 或 ~/.bcr-cloud.toml [onedrive] token）",
            )
        })?;
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()
            .map_err(|e| io::Error::other(format!("HTTP 客户端构建失败: {e}")))?;
        // 路径规范化：去掉首尾斜杠 → `root:/path:`
        let path = rest.trim_matches('/');
        let root = if path.is_empty() {
            "root:".to_string()
        } else {
            format!("root:/{}:", path)
        };
        Ok(OneDriveVfs {
            desc: format!("onedrive://{}", rest),
            client,
            root,
        })
    }

    /// Graph 资源 URL（冒号语法）
    fn item_url(&self, rel: &str) -> String {
        if rel.is_empty() {
            format!("https://graph.microsoft.com/v1.0/me/drive/{}", self.root)
        } else {
            format!(
                "https://graph.microsoft.com/v1.0/me/drive/{}{}",
                self.root,
                rel.split('/').collect::<Vec<_>>().join("/")
            )
        }
    }

    /// 读取目录（children）
    fn list_dir(&self, url: &str) -> io::Result<Vec<GraphItem>> {
        let resp = self
            .client
            .get(format!("{}/children", url.trim_end_matches(':')))
            .header(
                AUTHORIZATION,
                format!("Bearer {}", token().unwrap_or_default()),
            )
            .send()
            .map_err(|e| io::Error::other(format!("Graph 请求失败: {e}")))?;
        if !resp.status().is_success() {
            return Err(io::Error::other(format!(
                "Graph {} HTTP {}",
                url,
                resp.status()
            )));
        }
        let bytes = resp
            .bytes()
            .map_err(|e| io::Error::other(format!("Graph 响应读取失败: {e}")))?;
        let body: ChildrenResp = serde_json::from_slice(&bytes)
            .map_err(|e| io::Error::other(format!("Graph 响应解析失败: {e}")))?;
        Ok(body.value)
    }

    /// 递归扫描
    fn scan_rec(
        &self,
        url: &str,
        root: &str,
        filter: &Filter,
        out: &mut BTreeMap<String, FileMeta>,
    ) -> io::Result<()> {
        let items = self.list_dir(url)?;
        for item in items {
            if item.name.is_empty() {
                continue;
            }
            let rel = if url == root {
                item.name.clone()
            } else {
                format!(
                    "{}/{}",
                    url.trim_start_matches(root)
                        .trim_start_matches(":/")
                        .trim_start_matches('/'),
                    item.name
                )
            };
            let rel = rel
                .trim_start_matches(":/")
                .trim_start_matches('/')
                .to_string();
            if item.folder.is_some() {
                if filter.is_excluded(&rel) || filter.is_excluded(&format!("{rel}/")) {
                    continue;
                }
                // 目录 URL：root:/dir: 形式
                let dir_url = format!("{}:{}", url.trim_end_matches(':'), rel);
                self.scan_rec(&dir_url, root, filter, out)?;
            } else if filter.accept(&rel) {
                out.insert(
                    rel,
                    FileMeta {
                        size: item.size.unwrap_or(0),
                        mtime: parse_graph_date(item.last_modified.as_deref()),
                        mode: None,
                        symlink: None,
                    },
                );
            }
        }
        Ok(())
    }
}

/// 解析 Graph ISO 8601 日期（含毫秒与 Z）
fn parse_graph_date(s: Option<&str>) -> SystemTime {
    let Some(s) = s else { return UNIX_EPOCH };
    let s = s.trim();
    let Some((date, time)) = s.split_once('T') else {
        return UNIX_EPOCH;
    };
    let time = time.trim_end_matches('Z');
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

impl Vfs for OneDriveVfs {
    fn describe(&self) -> String {
        self.desc.clone()
    }

    fn scan(&self, filter: &Filter) -> io::Result<BTreeMap<String, FileMeta>> {
        let mut map = BTreeMap::new();
        self.scan_rec(&self.root, &self.root, filter, &mut map)?;
        Ok(map)
    }

    fn read(&self, rel: &str) -> io::Result<Vec<u8>> {
        let url = format!("{}/content", self.item_url(rel).trim_end_matches(':'));
        let resp = self
            .client
            .get(&url)
            .header(
                AUTHORIZATION,
                format!("Bearer {}", token().unwrap_or_default()),
            )
            .send()
            .map_err(|e| io::Error::other(format!("Graph 读取 {rel} 失败: {e}")))?;
        if !resp.status().is_success() {
            return Err(io::Error::other(format!(
                "Graph 读取 {rel} HTTP {}",
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
        let url = self.item_url(rel);
        let resp = self
            .client
            .get(&url)
            .header(
                AUTHORIZATION,
                format!("Bearer {}", token().unwrap_or_default()),
            )
            .send()
            .map_err(|e| io::Error::other(format!("Graph exists {rel}: {e}")))?;
        Ok(resp.status().is_success())
    }

    fn write(&self, rel: &str, data: &[u8]) -> io::Result<()> {
        let url = format!("{}/content", self.item_url(rel).trim_end_matches(':'));
        let resp = self
            .client
            .put(&url)
            .header(
                AUTHORIZATION,
                format!("Bearer {}", token().unwrap_or_default()),
            )
            .body(data.to_vec())
            .send()
            .map_err(|e| io::Error::other(format!("Graph 写入 {rel} 失败: {e}")))?;
        if !resp.status().is_success() {
            return Err(io::Error::other(format!(
                "Graph 写入 {rel} HTTP {}",
                resp.status()
            )));
        }
        Ok(())
    }

    fn delete(&self, rel: &str) -> io::Result<()> {
        let url = self.item_url(rel);
        let resp = self
            .client
            .delete(&url)
            .header(
                AUTHORIZATION,
                format!("Bearer {}", token().unwrap_or_default()),
            )
            .send()
            .map_err(|e| io::Error::other(format!("Graph 删除 {rel} 失败: {e}")))?;
        if !resp.status().is_success() {
            return Err(io::Error::other(format!(
                "Graph 删除 {rel} HTTP {}",
                resp.status()
            )));
        }
        Ok(())
    }

    fn remove_dir(&self, rel: &str) -> io::Result<()> {
        // OneDrive 目录为空时 DELETE 即可
        self.delete(rel)
    }

    fn rename(&self, from: &str, to: &str) -> io::Result<()> {
        if from == to {
            return Ok(());
        }
        // 读→写→删（Graph PATCH 移动需要 parentReference，简化实现）
        let data = self.read(from)?;
        self.write(to, &data)?;
        self.delete(from)
    }

    fn set_mtime(&self, _rel: &str, _t: SystemTime) -> io::Result<()> {
        // Graph 无可靠设置 mtime 的简单途径，静默忽略
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_url_builds_colon_syntax() {
        // 不实际连接（无 token 时 connect 会失败），直接构造验证 URL 拼接
        let v = OneDriveVfs {
            desc: "onedrive://".into(),
            client: Client::new(),
            root: "root:/docs:".into(),
        };
        assert_eq!(
            v.item_url(""),
            "https://graph.microsoft.com/v1.0/me/drive/root:/docs:"
        );
        assert_eq!(
            v.item_url("a.txt"),
            "https://graph.microsoft.com/v1.0/me/drive/root:/docs:a.txt"
        );
        assert_eq!(
            v.item_url("sub/b.txt"),
            "https://graph.microsoft.com/v1.0/me/drive/root:/docs:sub/b.txt"
        );
    }

    #[test]
    fn graph_date_parses() {
        let t = parse_graph_date(Some("2026-08-11T12:34:56.789Z"));
        let secs = t.duration_since(UNIX_EPOCH).unwrap().as_secs();
        assert!(secs > 1_780_000_000, "secs={secs}");
        assert_eq!(parse_graph_date(None), UNIX_EPOCH);
        assert_eq!(parse_graph_date(Some("garbage")), UNIX_EPOCH);
    }
}
