//! P27:统一 JSON 输出框架 — 所有子命令 `--json` 的结构化契约。
//!
//! 设计原则(见 docs/P27-python-binding-design.md):
//! - 通用信封:`{ schema, ok, command, args, result, warnings, error }`
//! - schema 版本化:`compare.v1` / `sync.v1` 等,保证外部脚本稳定
//! - mtime 统一 ISO-8601 UTC 字符串(Python `datetime.fromisoformat` 直接解析)
//! - status 统一 snake_case 字符串
//! - stdout 只输出 JSON;错误时 ok=false + error 字段,退出码 2
//!
//! 纯函数:输入现有结果结构,输出 serde_json::Value,与 CLI 输出解耦。

use crate::compare::{CompareResult, FileStatus};
use crate::fsscan::FileMeta;
use crate::sync::SyncOp;
use serde_json::{json, Map, Value};
use std::time::{SystemTime, UNIX_EPOCH};

/// 信封:包一层 schema/ok/args
fn envelope(schema: &str, command: &str, args: &[(&str, Value)], result: Value) -> Value {
    let mut arg_map = Map::new();
    for (k, v) in args {
        arg_map.insert(k.to_string(), v.clone());
    }
    json!({
        "schema": schema,
        "ok": true,
        "command": command,
        "args": Value::Object(arg_map),
        "result": result,
        "warnings": [],
        "error": null,
    })
}

/// 错误信封(ok=false,result=null,error=消息)
pub fn error_envelope(schema: &str, command: &str, message: &str) -> Value {
    json!({
        "schema": schema,
        "ok": false,
        "command": command,
        "args": {},
        "result": null,
        "warnings": [],
        "error": message,
    })
}

/// SystemTime → ISO-8601 UTC(如 2026-08-11T10:00:00Z)
fn iso8601(t: SystemTime) -> String {
    let secs = t
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    // 手动格式化为 UTC:秒级精度足够契约使用
    let days = secs.div_euclid(86400);
    let rem = secs.rem_euclid(86400);
    let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    // 儒略日 → 公历(Howard Hinnant 算法)
    let (y, mo, d) = civil_from_days(days);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{m:02}:{s:02}Z")
}

/// 天数 → 公历日期(Howard Hinnant 的 civil_from_days)
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m as i64, d as i64)
}

/// FileStatus → snake_case 字符串
pub fn status_str(s: FileStatus) -> &'static str {
    match s {
        FileStatus::Same => "same",
        FileStatus::LeftOnly => "left_only",
        FileStatus::RightOnly => "right_only",
        FileStatus::Differ => "differ",
        FileStatus::Moved => "moved",
    }
}

/// FileMeta → JSON(ISO-8601 mtime)
fn meta_json(m: &FileMeta) -> Value {
    json!({
        "size": m.size,
        "mtime": iso8601(m.mtime),
        "mode": m.mode,
        "symlink": m.symlink,
    })
}

/// compare 结果 → JSON 契约(compare.v1)
pub fn compare_json(left: &str, right: &str, result: &CompareResult, include_same: bool) -> Value {
    let entries: Vec<Value> = result
        .entries
        .iter()
        .filter(|e| include_same || e.status != FileStatus::Same)
        .map(|e| {
            json!({
                "rel": e.rel,
                "status": status_str(e.status),
                "left": e.left.as_ref().map(meta_json),
                "right": e.right.as_ref().map(meta_json),
                "moved_to": e.moved_to,
                "attrs_differ": e.attrs_differ,
            })
        })
        .collect();
    let s = result.stats;
    let stats = json!({
        "same": s.same,
        "left_only": s.left_only,
        "right_only": s.right_only,
        "differ": s.differ,
        "moved": s.moved,
    });
    envelope(
        "compare.v1",
        "compare",
        &[("left", json!(left)), ("right", json!(right))],
        json!({
            "stats": stats,
            "has_differences": s.has_differences(),
            "entries": entries,
        }),
    )
}

/// SyncOp → JSON(仅计划/执行列表项)
fn op_json(op: &SyncOp) -> Value {
    match op {
        SyncOp::Copy { rel, from_src } => json!({
            "op": "copy",
            "rel": rel,
            "from": if *from_src { "left" } else { "right" },
        }),
        SyncOp::Delete { rel } => json!({ "op": "delete", "rel": rel }),
        SyncOp::Rename { from, to } => json!({ "op": "rename", "rel": from, "to": to }),
        SyncOp::RmDir { rel } => json!({ "op": "rmdir", "rel": rel }),
        SyncOp::Skip { rel, reason } => json!({ "op": "skip", "rel": rel, "reason": reason }),
        SyncOp::Conflict { rel } => json!({ "op": "conflict", "rel": rel }),
    }
}

/// sync 计划 → JSON 契约(sync.v1,未执行/dry-run 用)
pub fn sync_plan_json(left: &str, right: &str, mode: &str, plan: &[SyncOp]) -> Value {
    let mut stats = Map::new();
    for k in ["copy", "delete", "rename", "rmdir", "skip", "conflict"] {
        stats.insert(k.to_string(), json!(0));
    }
    let list: Vec<Value> = plan.iter().map(op_json).collect();
    envelope(
        "sync.v1",
        "sync",
        &[
            ("left", json!(left)),
            ("right", json!(right)),
            ("mode", json!(mode)),
        ],
        json!({
            "dry_run": true,
            "mode": mode,
            "plan": list,
            "stats": Value::Object(stats),
        }),
    )
}

/// sync 执行结果 → JSON 契约(sync.v1)
pub fn sync_result_json(
    left: &str,
    right: &str,
    mode: &str,
    plan: &[SyncOp],
    stats: &crate::sync::SyncStats,
) -> Value {
    let list: Vec<Value> = plan.iter().map(op_json).collect();
    envelope(
        "sync.v1",
        "sync",
        &[
            ("left", json!(left)),
            ("right", json!(right)),
            ("mode", json!(mode)),
        ],
        json!({
            "dry_run": false,
            "mode": mode,
            "plan": list,
            "stats": {
                "copy": stats.copy,
                "delete": stats.delete,
                "rename": stats.rename,
                "rmdir": stats.rmdir,
                "skip": stats.skip,
                "conflict": stats.conflict,
                "errors": stats.error,
            },
        }),
    )
}

/// fsscan::FileMeta → JSON(供其他子命令复用)
#[allow(dead_code)] // 其他子命令 --json(Phase 2)使用
pub fn fsscan_meta_json(m: &FileMeta) -> Value {
    json!({
        "size": m.size,
        "mtime": iso8601(m.mtime),
        "mode": m.mode,
        "symlink": m.symlink,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso8601_known_timestamp() {
        // 2026-08-11T10:00:00Z = 1786442400
        let t = UNIX_EPOCH + std::time::Duration::from_secs(1_786_442_400);
        assert_eq!(iso8601(t), "2026-08-11T10:00:00Z");
    }

    #[test]
    fn iso8601_epoch() {
        assert_eq!(iso8601(UNIX_EPOCH), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn civil_from_days_known() {
        // 2026-08-11 的儒略日差 = 20676 (days since epoch)
        let (y, m, d) = civil_from_days(20676);
        assert_eq!((y, m, d), (2026, 8, 11));
    }

    #[test]
    fn status_mapping() {
        assert_eq!(status_str(FileStatus::Same), "same");
        assert_eq!(status_str(FileStatus::LeftOnly), "left_only");
        assert_eq!(status_str(FileStatus::RightOnly), "right_only");
        assert_eq!(status_str(FileStatus::Differ), "differ");
        assert_eq!(status_str(FileStatus::Moved), "moved");
    }

    #[test]
    fn compare_json_envelope_shape() {
        let mut result = CompareResult::default();
        result.stats.same = 1;
        result.stats.differ = 2;
        let v = compare_json("/a", "/b", &result, false);
        assert_eq!(v["schema"], "compare.v1");
        assert_eq!(v["ok"], true);
        assert_eq!(v["result"]["stats"]["same"], 1);
        assert_eq!(v["result"]["has_differences"], true);
        assert_eq!(v["error"], Value::Null);
    }

    #[test]
    fn error_envelope_shape() {
        let v = error_envelope("compare.v1", "compare", "bad path");
        assert_eq!(v["ok"], false);
        assert_eq!(v["error"], "bad path");
        assert_eq!(v["result"], Value::Null);
    }

    #[test]
    fn op_json_forms() {
        let op = SyncOp::Copy {
            rel: "a.rs".into(),
            from_src: true,
        };
        assert_eq!(op_json(&op)["op"], "copy");
        assert_eq!(op_json(&op)["from"], "left");
        let d = SyncOp::Delete { rel: "x".into() };
        assert_eq!(op_json(&d)["op"], "delete");
    }
}
