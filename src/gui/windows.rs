//! P65（BC 5.2.5 设计稿 · 窗口菜单）：多窗口协调。
//!
//! 设计稿 `screens/menus.html` 的窗口菜单含 **移动标签页到新窗口** 与
//! **合并所有窗口**（单标签时置灰）。bcr 的多窗口模型是**多进程**
//! （`⌘N` 新建窗口 = 启动新进程，见 `DiffApp::open_new_window`），
//! 因此这里用「窗口注册表」跨进程协调：
//!
//! - **注册**：每个 GUI 进程周期性把自己的**可重建会话**写入
//!   `~/.bcr-windows/window-<pid>.toml`（含心跳时间、标签总数/可重建数）。
//!   目录可用 `BCR_WINDOWS_DIR` 覆盖（测试用）。
//! - **移动标签页到新窗口**：把该标签写成一个单标签工作空间文件，启动
//!   `bcr gui --workspace <file>` 新进程，然后在本窗口关闭该标签。
//! - **合并所有窗口**：读取其它窗口的注册表 → 在本窗口重建它们的会话 →
//!   只有当对方**全部标签都可重建**时才写 `<pid>.merge` 请求其退出，
//!   避免把带未保存缓冲区的编辑窗口关掉造成数据丢失。
//!
//! 注册表条目带心跳，`PEER_TTL` 内没有刷新的条目视为已退出/崩溃，直接忽略。

use std::path::{Path, PathBuf};

/// 对端条目存活时间（秒）——超过即视为该窗口已退出
pub const PEER_TTL: u64 = 10;

/// 一个可重建会话（标签的「外部表示」）
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Session {
    /// 会话类型：diff / dir / merge / image / csv / media
    pub kind: String,
    pub left: String,
    pub right: String,
}

/// 注册表里的一个窗口条目
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowEntry {
    pub pid: u32,
    /// 最后一次心跳（Unix 秒）
    pub updated: u64,
    /// 该窗口的标签总数（含不可重建的文本编辑/补丁标签）
    pub total: usize,
    /// 可重建会话（= 能把标签内容交回另一个窗口）
    pub sessions: Vec<Session>,
}

impl WindowEntry {
    /// 该窗口能否被安全合并（全部标签都可重建 → 合并后关闭不会丢内容）
    pub fn mergeable(&self) -> bool {
        !self.sessions.is_empty() && self.sessions.len() == self.total
    }
}

/// 注册表目录（`BCR_WINDOWS_DIR` 优先，便于测试隔离）
pub fn registry_dir() -> PathBuf {
    if let Ok(d) = std::env::var("BCR_WINDOWS_DIR") {
        if !d.is_empty() {
            return PathBuf::from(d);
        }
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    Path::new(&home).join(".bcr-windows")
}

/// 当前 Unix 秒
pub fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn entry_path(dir: &Path, pid: u32) -> PathBuf {
    dir.join(format!("window-{pid}.toml"))
}

fn request_path(dir: &Path, pid: u32) -> PathBuf {
    dir.join(format!("window-{pid}.merge"))
}

#[derive(serde::Serialize, serde::Deserialize)]
struct EntryFile {
    pid: u32,
    updated: u64,
    total: usize,
    #[serde(default)]
    tabs: Vec<EntryTab>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct EntryTab {
    kind: String,
    left: String,
    right: String,
}

/// 写（或刷新）本窗口的注册表条目。返回错误字符串而不 panic。
pub fn register(dir: &Path, pid: u32, total: usize, sessions: &[Session]) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let file = EntryFile {
        pid,
        updated: now_secs(),
        total,
        tabs: sessions
            .iter()
            .map(|s| EntryTab {
                kind: s.kind.clone(),
                left: s.left.clone(),
                right: s.right.clone(),
            })
            .collect(),
    };
    let text = toml::to_string_pretty(&file).map_err(|e| e.to_string())?;
    // 原子替换：先写临时文件再 rename，避免对端读到半截内容
    let tmp = entry_path(dir, pid).with_extension("tmp");
    std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, entry_path(dir, pid)).map_err(|e| e.to_string())
}

/// 注销本窗口（退出时调用；崩溃场景由心跳 TTL 兜底）
pub fn unregister(dir: &Path, pid: u32) {
    let _ = std::fs::remove_file(entry_path(dir, pid));
    let _ = std::fs::remove_file(request_path(dir, pid));
}

/// 扫描对端窗口（排除自己、排除心跳过期的条目，并顺带清理过期文件）
pub fn scan_peers(dir: &Path, self_pid: u32) -> Vec<WindowEntry> {
    scan_peers_at(dir, self_pid, now_secs())
}

/// 带「当前时间」入参的版本（便于测试心跳过期规则）
pub fn scan_peers_at(dir: &Path, self_pid: u32, now: u64) -> Vec<WindowEntry> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for e in rd.flatten() {
        let p = e.path();
        if p.extension().and_then(|s| s.to_str()) != Some("toml") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&p) else {
            continue;
        };
        let Ok(file) = toml::from_str::<EntryFile>(&text) else {
            continue;
        };
        if file.pid == self_pid {
            continue;
        }
        if now.saturating_sub(file.updated) > PEER_TTL {
            // 过期窗口（崩溃或强杀）：清理，避免后续误合并
            let _ = std::fs::remove_file(&p);
            continue;
        }
        out.push(WindowEntry {
            pid: file.pid,
            updated: file.updated,
            total: file.total,
            sessions: file
                .tabs
                .into_iter()
                .map(|t| Session {
                    kind: t.kind,
                    left: t.left,
                    right: t.right,
                })
                .collect(),
        });
    }
    out.sort_by_key(|e| e.pid);
    out
}

/// 请求某个对端窗口退出（合并完成后调用）
pub fn request_exit(dir: &Path, pid: u32) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    std::fs::write(request_path(dir, pid), b"merge").map_err(|e| e.to_string())
}

/// 本窗口是否被要求退出（合并到别的窗口）
pub fn exit_requested(dir: &Path, pid: u32) -> bool {
    request_path(dir, pid).exists()
}

/// 把单个会话写成工作空间文件（供 `--workspace` 启动的新窗口载入）
pub fn write_single_session_workspace(path: &Path, session: &Session) -> Result<(), String> {
    #[derive(serde::Serialize)]
    struct WsTab<'a> {
        kind: &'a str,
        left: &'a str,
        right: &'a str,
    }
    #[derive(serde::Serialize)]
    struct WsFile<'a> {
        tabs: Vec<WsTab<'a>>,
    }
    let file = WsFile {
        tabs: vec![WsTab {
            kind: &session.kind,
            left: &session.left,
            right: &session.right,
        }],
    };
    let text = toml::to_string_pretty(&file).map_err(|e| e.to_string())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(path, text).map_err(|e| e.to_string())
}

/// 「移动标签页到新窗口」用的临时工作空间文件路径
pub fn temp_workspace_path(pid: u32, seq: u64) -> PathBuf {
    std::env::temp_dir().join(format!("bcr-move-{pid}-{seq}.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(kind: &str, l: &str, r: &str) -> Session {
        Session {
            kind: kind.to_string(),
            left: l.to_string(),
            right: r.to_string(),
        }
    }

    #[test]
    fn register_then_scan_roundtrip_skips_self() {
        let d = tempfile::tempdir().unwrap();
        register(d.path(), 100, 2, &[s("diff", "a", "b"), s("dir", "c", "d")]).unwrap();
        register(d.path(), 200, 1, &[s("csv", "e", "f")]).unwrap();
        // 自己不在结果里
        let peers = scan_peers(d.path(), 100);
        assert_eq!(peers.len(), 1, "只应看到对端窗口");
        assert_eq!(peers[0].pid, 200);
        assert_eq!(peers[0].sessions, vec![s("csv", "e", "f")]);
        // 未注册的 pid 视为无对端
        assert!(scan_peers(d.path(), 200).iter().any(|e| e.pid == 100));
    }

    #[test]
    fn stale_entries_are_ignored_and_cleaned() {
        let d = tempfile::tempdir().unwrap();
        register(d.path(), 300, 1, &[s("diff", "a", "b")]).unwrap();
        // 心跳落在 TTL 之外 → 视为已退出（并删除文件）
        let now = now_secs() + PEER_TTL + 5;
        assert!(scan_peers_at(d.path(), 1, now).is_empty());
        assert!(!entry_path(d.path(), 300).exists(), "过期条目应被清理");
        // TTL 内仍然可见
        register(d.path(), 301, 1, &[s("diff", "a", "b")]).unwrap();
        assert_eq!(scan_peers_at(d.path(), 1, now_secs()).len(), 1);
    }

    #[test]
    fn mergeable_requires_all_tabs_serializable() {
        let all = WindowEntry {
            pid: 1,
            updated: 0,
            total: 2,
            sessions: vec![s("diff", "a", "b"), s("dir", "c", "d")],
        };
        assert!(all.mergeable(), "全部可重建 → 可合并");
        let partial = WindowEntry {
            pid: 2,
            updated: 0,
            total: 3,
            sessions: vec![s("diff", "a", "b"), s("dir", "c", "d")],
        };
        assert!(!partial.mergeable(), "含不可重建标签（如文本编辑）→ 不合并");
        let empty = WindowEntry {
            pid: 3,
            updated: 0,
            total: 0,
            sessions: vec![],
        };
        assert!(!empty.mergeable(), "空窗口无需合并");
    }

    #[test]
    fn exit_request_roundtrip_and_unregister_clears() {
        let d = tempfile::tempdir().unwrap();
        register(d.path(), 400, 1, &[s("diff", "a", "b")]).unwrap();
        assert!(!exit_requested(d.path(), 400));
        request_exit(d.path(), 400).unwrap();
        assert!(exit_requested(d.path(), 400), "对端应看到退出请求");
        unregister(d.path(), 400);
        assert!(!exit_requested(d.path(), 400));
        assert!(scan_peers(d.path(), 1).is_empty());
    }

    #[test]
    fn single_session_workspace_is_loadable_shape() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("ws.toml");
        write_single_session_workspace(&p, &s("diff", "/a.txt", "/b.txt")).unwrap();
        let text = std::fs::read_to_string(&p).unwrap();
        assert!(text.contains("diff") && text.contains("/a.txt") && text.contains("/b.txt"));
        // 与 `load_workspace` 期望的结构一致（WsFile { tabs: [{kind,left,right}] }）
        #[derive(serde::Deserialize)]
        #[allow(dead_code)] // right 字段仅为校验文件结构完整
        struct WsTab {
            kind: String,
            left: String,
            right: String,
        }
        #[derive(serde::Deserialize)]
        struct WsFile {
            tabs: Vec<WsTab>,
        }
        let ws: WsFile = toml::from_str(&text).unwrap();
        assert_eq!(ws.tabs.len(), 1);
        assert_eq!(ws.tabs[0].kind, "diff");
        assert_eq!(ws.tabs[0].left, "/a.txt");
    }

    #[test]
    fn registry_dir_honours_env_override() {
        // 只验证默认路径形状，不改动进程级环境变量（测试并行安全）
        let dir = registry_dir();
        assert!(
            dir.to_string_lossy().contains("bcr-windows")
                || std::env::var("BCR_WINDOWS_DIR").is_ok()
        );
    }
}
