//! Amazon S3 虚拟文件系统。
//!
//! 把 S3 bucket 当作目录树。URL 规范：`s3://bucket[/prefix]`
//! - 凭证：AWS_ACCESS_KEY_ID / AWS_SECRET_ACCESS_KEY 环境变量（aws-creds 自动读取，
//!   也支持 ~/.aws/credentials）；区域取 AWS_REGION（默认 us-east-1）
//! - 支持 MinIO 等 S3 兼容服务：AWS_ENDPOINT 环境变量指定自定义 endpoint
//! - 可读写：read/write/delete/rename（copy+delete）；S3 无 mtime 概念，
//!   scan 用对象 LastModified，set_mtime 静默忽略
//! - 目录为隐式（key 前缀），remove_dir 无操作

use super::Vfs;
use crate::fsscan::{FileMeta, Filter};
use s3::bucket::Bucket;
use s3::creds::Credentials;
use s3::region::Region;
use std::collections::BTreeMap;
use std::io;
use std::time::{SystemTime, UNIX_EPOCH};

/// S3 虚拟文件系统
pub struct S3Vfs {
    desc: String,
    bucket: Bucket,
    /// 根前缀（相对 bucket 的目录前缀，含尾斜杠；空 = bucket 根）
    prefix: String,
}

/// 解析 s3:// URL：返回 (bucket, prefix)
fn parse_url(rest: &str) -> io::Result<(String, String)> {
    let rest = rest.trim_end_matches('/');
    let (bucket, prefix) = match rest.find('/') {
        Some(i) => (&rest[..i], rest[i + 1..].to_string()),
        None => (rest, String::new()),
    };
    if bucket.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "s3:// 缺少 bucket 名",
        ));
    }
    Ok((bucket.to_string(), prefix))
}

impl S3Vfs {
    /// 打开 S3 bucket（凭证/区域/endpoint 从环境变量读取）
    pub fn connect(rest: &str) -> io::Result<Self> {
        let (bucket_name, prefix) = parse_url(rest)?;
        let credentials = Credentials::default()
            .or_else(|_| Credentials::from_env())
            .map_err(|e| {
                io::Error::other(format!(
                    "AWS 凭证读取失败（设置 AWS_ACCESS_KEY_ID/AWS_SECRET_ACCESS_KEY）: {e}"
                ))
            })?;
        // 区域：AWS_REGION 环境变量或 us-east-1；AWS_ENDPOINT 支持 MinIO
        let region_str = std::env::var("AWS_REGION").unwrap_or_else(|_| "us-east-1".to_string());
        let region = match std::env::var("AWS_ENDPOINT") {
            Ok(endpoint) => Region::Custom {
                region: region_str,
                endpoint,
            },
            Err(_) => region_str
                .parse::<Region>()
                .map_err(|e| io::Error::other(format!("无效 AWS 区域 {region_str}: {e}")))?,
        };
        let bucket = Bucket::new(&bucket_name, region, credentials)
            .map_err(|e| io::Error::other(format!("S3 bucket 打开失败: {e}")))?;
        // prefix 规范化：不以斜杠开头、以斜杠结尾（空 = bucket 根）
        let prefix = prefix.trim_start_matches('/').to_string();
        let prefix = if prefix.is_empty() {
            String::new()
        } else {
            format!("{}/", prefix.trim_end_matches('/'))
        };
        Ok(S3Vfs {
            desc: format!("s3://{}", rest),
            bucket: *bucket,
            prefix,
        })
    }

    /// 完整 key（prefix + rel）
    fn key(&self, rel: &str) -> String {
        format!("{}{}", self.prefix, rel)
    }

    /// 递归列出 prefix 下的全部对象（分页）
    fn list_objects(&self) -> io::Result<BTreeMap<String, (u64, SystemTime)>> {
        let mut map = BTreeMap::new();
        let mut token: Option<String> = None;
        loop {
            let page = self
                .bucket
                .list_page_blocking(self.prefix.clone(), None, token, None, None)
                .map_err(|e| io::Error::other(format!("S3 list 失败: {e}")))?;
            let (result, _) = page;
            for obj in &result.contents {
                // 去掉 prefix 前缀得到 rel；跳过"目录占位"对象（key 以 / 结尾）
                let key = obj.key.trim_start_matches(&self.prefix);
                if key.is_empty() || key.ends_with('/') {
                    continue;
                }
                let mtime = parse_s3_date(&obj.last_modified);
                map.insert(key.to_string(), (obj.size, mtime));
            }
            token = result.next_continuation_token.clone();
            if token.is_none() {
                break;
            }
        }
        Ok(map)
    }
}

/// 解析 S3 日期（ISO 8601 带毫秒，如 2026-08-11T12:00:00.000Z）
fn parse_s3_date(s: &str) -> SystemTime {
    // "YYYY-MM-DDTHH:MM:SS[.mmm]Z"
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
    let t = time.split('.').next().unwrap_or(time); // 去掉毫秒
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
    // days-from-civil
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

impl Vfs for S3Vfs {
    fn describe(&self) -> String {
        self.desc.clone()
    }

    fn scan(&self, filter: &Filter) -> io::Result<BTreeMap<String, FileMeta>> {
        let objs = self.list_objects()?;
        let mut map = BTreeMap::new();
        for (rel, (size, mtime)) in objs {
            if !filter.accept(&rel) {
                continue;
            }
            map.insert(
                rel,
                FileMeta {
                    size,
                    mtime,
                    mode: None,
                    symlink: None,
                },
            );
        }
        Ok(map)
    }

    fn read(&self, rel: &str) -> io::Result<Vec<u8>> {
        let data = self
            .bucket
            .get_object_blocking(self.key(rel))
            .map_err(|e| io::Error::other(format!("S3 读取 {rel} 失败: {e}")))?;
        Ok(data.to_vec())
    }

    fn hash(&self, rel: &str) -> io::Result<blake3::Hash> {
        let data = self.read(rel)?;
        Ok(blake3::hash(&data))
    }

    fn exists(&self, rel: &str) -> io::Result<bool> {
        self.bucket
            .object_exists_blocking(self.key(rel))
            .map_err(|e| io::Error::other(format!("S3 exists {rel}: {e}")))
    }

    fn write(&self, rel: &str, data: &[u8]) -> io::Result<()> {
        let resp = self
            .bucket
            .put_object_blocking(self.key(rel), data)
            .map_err(|e| io::Error::other(format!("S3 写入 {rel} 失败: {e}")))?;
        if resp.status_code() != 200 {
            return Err(io::Error::other(format!(
                "S3 写入 {rel} HTTP {}",
                resp.status_code()
            )));
        }
        Ok(())
    }

    fn delete(&self, rel: &str) -> io::Result<()> {
        self.bucket
            .delete_object_blocking(self.key(rel))
            .map_err(|e| io::Error::other(format!("S3 删除 {rel} 失败: {e}")))?;
        Ok(())
    }

    fn remove_dir(&self, _rel: &str) -> io::Result<()> {
        // S3 无显式目录，无需清理
        Ok(())
    }

    fn rename(&self, from: &str, to: &str) -> io::Result<()> {
        if from == to {
            return Ok(());
        }
        // S3 无对象级原子重命名：读→写→删（Vfs 默认实现相同语义）
        let data = self.read(from)?;
        self.write(to, &data)?;
        self.delete(from)
    }

    fn set_mtime(&self, _rel: &str, _t: SystemTime) -> io::Result<()> {
        // S3 对象无 mtime 概念（LastModified 只读），静默忽略
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_url_bucket_only() {
        let (b, p) = parse_url("my-bucket").unwrap();
        assert_eq!(b, "my-bucket");
        assert_eq!(p, "");
    }

    #[test]
    fn parse_url_with_prefix() {
        let (b, p) = parse_url("my-bucket/backups/2026/").unwrap();
        assert_eq!(b, "my-bucket");
        assert_eq!(p, "backups/2026");
    }

    #[test]
    fn parse_url_missing_bucket_errors() {
        assert!(parse_url("/only-prefix").is_err());
    }

    #[test]
    fn s3_date_parses_iso8601() {
        let t = parse_s3_date("2026-08-11T12:00:00.000Z");
        let secs = t.duration_since(UNIX_EPOCH).unwrap().as_secs();
        // 2026-08-11 ≈ 1786 百万秒量级
        assert!(secs > 1_780_000_000, "secs={secs}");
    }

    #[test]
    fn s3_date_invalid_returns_epoch() {
        assert_eq!(parse_s3_date("garbage"), UNIX_EPOCH);
        assert_eq!(parse_s3_date(""), UNIX_EPOCH);
    }
}
