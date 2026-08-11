//! 目录比较结果缓存（P17）：mtime+size 快照比对。
//!
//! 记录"两侧目录的扫描快照 + 比较结果"，存于 `~/.bcr-cache.toml`。
//! 二次比较同一对目录时，若两侧快照（每个文件 size+mtime）与缓存一致，
//! 直接复用缓存结果，跳过扫描与哈希——大目录/远程对比秒开。
//!
//! 失效策略：任何一侧文件变更（size/mtime 变化、增删）→ 快照不一致 → 重算并更新缓存。
//! 缓存按 LRU 上限裁剪（默认 64 条），防止无限增长。

use crate::compare::CompareResult;
use crate::fsscan::FileMeta;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};
use std::path::PathBuf;

/// 单侧目录快照：相对路径 -> (size, mtime 秒)
pub type Snapshot = BTreeMap<String, (u64, i64)>;

/// 缓存条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    /// 左侧目录快照
    pub left: Snapshot,
    /// 右侧目录快照
    pub right: Snapshot,
    /// 上次比较结果（serde 持久化）
    pub result: CompareResult,
    /// 最近使用时间（unix 秒，LRU 裁剪用）
    pub last_used: u64,
}

/// 缓存文件
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Cache {
    #[serde(default)]
    pub entries: BTreeMap<String, CacheEntry>,
}

/// 缓存上限条目数
const MAX_ENTRIES: usize = 64;

/// 缓存文件路径：`~/.bcr-cache.toml`
pub fn cache_path() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".bcr-cache.toml")
}

pub fn load() -> Cache {
    let path = cache_path();
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save(c: &Cache) {
    let path = cache_path();
    if let Ok(s) = toml::to_string_pretty(c) {
        let _ = std::fs::write(path, s);
    }
}

/// FileMeta 表 → 快照（size + mtime 秒，1970 前为负）
pub fn snapshot_of(meta: &BTreeMap<String, FileMeta>) -> Snapshot {
    meta.iter()
        .map(|(rel, m)| {
            let secs = m
                .mtime
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or_else(|e| -(e.duration().as_secs() as i64));
            (rel.clone(), (m.size, secs))
        })
        .collect()
}

/// 缓存键：两侧路径 + 过滤/选项的稳定指纹（避免跨目录误命中）
pub fn key_for(
    left: &str,
    right: &str,
    includes: &[String],
    excludes: &[String],
    opts: &str,
) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    left.hash(&mut h);
    right.hash(&mut h);
    includes.hash(&mut h);
    excludes.hash(&mut h);
    opts.hash(&mut h);
    format!("{:016x}", h.finish())
}

/// 查询缓存：命中且快照一致 → 返回缓存结果
pub fn lookup(key: &str, left_snap: &Snapshot, right_snap: &Snapshot) -> Option<CompareResult> {
    let cache = load();
    let entry = cache.entries.get(key)?;
    if &entry.left == left_snap && &entry.right == right_snap {
        Some(entry.result.clone())
    } else {
        None
    }
}

/// 写入缓存（LRU 裁剪），失败静默
pub fn insert(key: &str, left: Snapshot, right: Snapshot, result: CompareResult) {
    let mut cache = load();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // 删除同名旧条目（更新 last_used）
    cache.entries.remove(key);
    cache.entries.insert(
        key.to_string(),
        CacheEntry {
            left,
            right,
            result,
            last_used: now,
        },
    );
    // LRU 裁剪：超出上限时移除最久未用的
    while cache.entries.len() > MAX_ENTRIES {
        let oldest = cache
            .entries
            .iter()
            .min_by_key(|(_, e)| e.last_used)
            .map(|(k, _)| k.clone());
        if let Some(k) = oldest {
            cache.entries.remove(&k);
        } else {
            break;
        }
    }
    save(&cache);
    let _ = VecDeque::<u8>::new(); // 避免未用导入警告
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compare::{CompareResult, CompareStats};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn snap(entries: &[(&str, u64, i64)]) -> Snapshot {
        entries
            .iter()
            .map(|(r, s, m)| (r.to_string(), (*s, *m)))
            .collect()
    }

    fn result() -> CompareResult {
        CompareResult {
            entries: vec![],
            stats: CompareStats {
                same: 3,
                ..Default::default()
            },
            warnings: vec![],
        }
    }

    #[test]
    fn snapshot_extracts_size_and_mtime() {
        let mut meta = BTreeMap::new();
        meta.insert(
            "a.txt".to_string(),
            FileMeta {
                size: 10,
                mtime: UNIX_EPOCH + std::time::Duration::from_secs(100),
                mode: None,
                symlink: None,
            },
        );
        let s = snapshot_of(&meta);
        assert_eq!(s.get("a.txt"), Some(&(10, 100)));
    }

    #[test]
    fn cache_roundtrip_and_hit() {
        // 用临时 HOME 指向避免污染真实缓存
        let dir = tempfile::tempdir().unwrap();
        let fake_home = dir.path().to_str().unwrap().to_string();
        std::env::set_var("HOME", &fake_home);
        let key = "test-key";
        let l = snap(&[("a.txt", 10, 100)]);
        let r = snap(&[("a.txt", 10, 100)]);
        insert(key, l.clone(), r.clone(), result());
        // 快照一致 → 命中
        assert!(lookup(key, &l, &r).is_some());
        // 快照变化 → 未命中
        let l2 = snap(&[("a.txt", 11, 100)]);
        assert!(lookup(key, &l2, &r).is_none());
        std::env::remove_var("HOME");
    }

    #[test]
    fn mtime_before_epoch_serializes() {
        // 1970 之前的 SystemTime 也能序列化（负秒）
        let t = UNIX_EPOCH - std::time::Duration::from_secs(5);
        let meta = FileMeta {
            size: 1,
            mtime: t,
            mode: None,
            symlink: None,
        };
        let mut map = BTreeMap::new();
        map.insert("x".to_string(), meta);
        let s = snapshot_of(&map);
        assert_eq!(s.get("x"), Some(&(1, -5)));
        let _ = SystemTime::now();
    }
}
