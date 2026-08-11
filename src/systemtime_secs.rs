//! SystemTime ↔ unix 秒的 serde 序列化助手（用于缓存文件持久化）。
//!
//! `#[serde(with = "crate::systemtime_secs")]` 把 SystemTime 存为 i64 秒，
//! 避免 serde 对 SystemTime 无默认实现的问题。

use serde::{Deserialize, Deserializer, Serializer};
use std::time::{SystemTime, UNIX_EPOCH};

/// 序列化为 unix 秒（i64；1970 之前为负）
pub fn serialize<S: Serializer>(t: &SystemTime, s: S) -> Result<S::Ok, S::Error> {
    let secs = t
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_else(|e| -(e.duration().as_secs() as i64));
    s.serialize_i64(secs)
}

/// 从 unix 秒反序列化
pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<SystemTime, D::Error> {
    let secs = i64::deserialize(d)?;
    if secs >= 0 {
        Ok(UNIX_EPOCH + std::time::Duration::from_secs(secs as u64))
    } else {
        Ok(UNIX_EPOCH - std::time::Duration::from_secs((-secs) as u64))
    }
}
