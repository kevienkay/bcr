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
        SyncOp::Copy {
            rel,
            src_rel,
            from_src,
        } => json!({
            "op": "copy",
            "rel": rel,
            "src_rel": src_rel,
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

/// imgcmp 结果 → JSON 契约(imgcmp.v1)
#[allow(dead_code)]
#[allow(clippy::too_many_arguments)]
pub fn envelope_imgcmp(
    left: &str,
    right: &str,
    left_w: u32,
    left_h: u32,
    right_w: u32,
    right_h: u32,
    size_differs: bool,
    diff_pixels: u64,
    total_pixels: u64,
    diff_ratio: f64,
    bounds: Option<(u32, u32, u32, u32)>,
) -> Value {
    envelope(
        "imgcmp.v1",
        "imgcmp",
        &[("left", json!(left)), ("right", json!(right))],
        json!({
            "left_size": [left_w, left_h],
            "right_size": [right_w, right_h],
            "size_differs": size_differs,
            "diff_pixels": diff_pixels,
            "total_pixels": total_pixels,
            "diff_ratio": diff_ratio,
            "bounds": bounds.map(|(x, y, w, h)| json!([x, y, w, h])),
            "has_differences": size_differs || diff_pixels > 0,
        }),
    )
}

/// diff 结果 → JSON 契约(diff.v1)
/// ops 为 (tag, old_start, old_end, new_start, new_end) 五元组，
/// tag ∈ equal/delete/insert/replace，range 为左/右行号区间（0 基，半开）。
/// no_newline_l/no_newline_r：对应侧文件是否不以换行结尾（GNU diff 兼容：
/// 仅行尾换行不同也是差异，has_differences 与退出码需一并反映）。
pub fn envelope_diff(
    left: &str,
    right: &str,
    ops: &[(String, usize, usize, usize, usize)],
    no_newline_l: bool,
    no_newline_r: bool,
) -> Value {
    envelope(
        "diff.v1",
        "diff",
        &[("left", json!(left)), ("right", json!(right))],
        json!({
            "ops": ops
                .iter()
                .map(|(tag, os, oe, ns, ne)| json!({
                    "tag": tag,
                    "old_range": [os, oe],
                    "new_range": [ns, ne],
                }))
                .collect::<Vec<_>>(),
            "no_newline": {
                "left": no_newline_l,
                "right": no_newline_r,
            },
            "has_differences": ops.iter().any(|(t, ..)| t != "equal")
                || no_newline_l != no_newline_r,
        }),
    )
}

/// hex 结果 → JSON 契约(hex.v1)
/// rows 为 (offset, left_hex, right_hex, diff) 四元组，仅含差异行（与 CLI 默认一致）。
pub fn envelope_hex(
    left: &str,
    right: &str,
    rows: &[(usize, String, String, bool)],
    diff_rows: usize,
    diff_bytes: u64,
    left_size: u64,
    right_size: u64,
) -> Value {
    envelope(
        "hex.v1",
        "hex",
        &[("left", json!(left)), ("right", json!(right))],
        json!({
            "rows": rows
                .iter()
                .map(|(offset, l, r, diff)| json!({
                    "offset": offset,
                    "left": l,
                    "right": r,
                    "diff": diff,
                }))
                .collect::<Vec<_>>(),
            "stats": {
                "diff_rows": diff_rows,
                "diff_bytes": diff_bytes,
                "left_size": left_size,
                "right_size": right_size,
            },
            "has_differences": diff_rows > 0,
        }),
    )
}

/// media 结果 → JSON 契约(media.v1)
/// fields 为 (name, left, right) 字段级差异列表，left/right 为字符串化值。
pub fn envelope_media(
    left: &str,
    right: &str,
    left_format: Option<String>,
    right_format: Option<String>,
    fields: &[(String, Option<String>, Option<String>)],
) -> Value {
    envelope(
        "media.v1",
        "media",
        &[("left", json!(left)), ("right", json!(right))],
        json!({
            "left_format": left_format,
            "right_format": right_format,
            "fields": fields
                .iter()
                .map(|(name, l, r)| json!({
                    "name": name,
                    "left": l,
                    "right": r,
                    "diff": l != r,
                }))
                .collect::<Vec<_>>(),
            "has_differences": !fields.is_empty(),
        }),
    )
}

/// mp3tag 结果 → JSON 契约(mp3tag.v1)
#[allow(dead_code)]
pub fn envelope_mp3tag(
    left: &str,
    right: &str,
    fields: &[(String, Option<String>, Option<String>)],
    has_differences: bool,
) -> Value {
    let list: Vec<Value> = fields
        .iter()
        .map(|(name, l, r)| {
            json!({
                "name": name,
                "left": l,
                "right": r,
                "diff": l != r,
            })
        })
        .collect();
    envelope(
        "mp3tag.v1",
        "mp3tag",
        &[("left", json!(left)), ("right", json!(right))],
        json!({
            "fields": list,
            "has_differences": has_differences,
        }),
    )
}

/// csv 结果 → JSON 契约(csv.v1)
#[allow(dead_code)]
pub fn envelope_csv(
    left: &str,
    right: &str,
    same: usize,
    left_only: usize,
    right_only: usize,
    modified: usize,
) -> Value {
    envelope(
        "csv.v1",
        "csv",
        &[("left", json!(left)), ("right", json!(right))],
        json!({
            "stats": {
                "same": same,
                "left_only": left_only,
                "right_only": right_only,
                "modified": modified,
            },
            "has_differences": left_only + right_only + modified > 0,
        }),
    )
}

/// compare3 结果 → JSON 契约(compare3.v1)
#[allow(dead_code)]
pub fn envelope_compare3(
    base: &str,
    left: &str,
    right: &str,
    entries: &[(String, &'static str)],
    stats: &crate::compare3::TriStats,
) -> Value {
    let list: Vec<Value> = entries
        .iter()
        .map(|(rel, status)| json!({ "rel": rel, "status": status }))
        .collect();
    envelope(
        "compare3.v1",
        "compare3",
        &[
            ("base", json!(base)),
            ("left", json!(left)),
            ("right", json!(right)),
        ],
        json!({
            "stats": {
                "same": stats.same,
                "base_only": stats.base_only,
                "left_only": stats.left_only,
                "right_only": stats.right_only,
                "left_deleted": stats.left_deleted,
                "right_deleted": stats.right_deleted,
                "left_modified": stats.left_modified,
                "right_modified": stats.right_modified,
                "both_modified": stats.both_modified,
                "conflict": stats.conflict,
            },
            "has_differences": stats.has_differences(),
            "entries": list,
        }),
    )
}

/// merge 结果 → JSON 契约(merge.v1)
#[allow(dead_code)]
pub fn envelope_merge(
    base: &str,
    left: &str,
    right: &str,
    conflicts: usize,
    output: Option<&str>,
) -> Value {
    envelope(
        "merge.v1",
        "merge",
        &[
            ("base", json!(base)),
            ("left", json!(left)),
            ("right", json!(right)),
        ],
        json!({
            "conflicts": conflicts,
            "output": output,
            "has_conflicts": conflicts > 0,
        }),
    )
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
            src_rel: None,
            from_src: true,
        };
        assert_eq!(op_json(&op)["op"], "copy");
        assert_eq!(op_json(&op)["from"], "left");
        let d = SyncOp::Delete { rel: "x".into() };
        assert_eq!(op_json(&d)["op"], "delete");
    }

    #[test]
    fn csv_envelope_shape() {
        let v = envelope_csv("/a.csv", "/b.csv", 2, 1, 0, 3);
        assert_eq!(v["schema"], "csv.v1");
        assert_eq!(v["result"]["stats"]["modified"], 3);
        assert_eq!(v["result"]["has_differences"], true);
    }

    #[test]
    fn mp3tag_envelope_shape() {
        let fields = vec![(
            "title".to_string(),
            Some("A".to_string()),
            Some("B".to_string()),
        )];
        let v = envelope_mp3tag("/a.mp3", "/b.mp3", &fields, true);
        assert_eq!(v["schema"], "mp3tag.v1");
        assert_eq!(v["result"]["fields"][0]["diff"], true);
        assert_eq!(v["result"]["fields"][0]["name"], "title");
    }

    #[test]
    fn imgcmp_envelope_shape() {
        let v = envelope_imgcmp(
            "/a.png",
            "/b.png",
            4,
            4,
            4,
            4,
            false,
            8,
            16,
            0.5,
            Some((1, 1, 2, 2)),
        );
        assert_eq!(v["schema"], "imgcmp.v1");
        assert_eq!(v["result"]["diff_pixels"], 8);
        assert_eq!(v["result"]["bounds"].as_array().unwrap().len(), 4);
        assert_eq!(v["result"]["has_differences"], true);
    }

    #[test]
    fn compare3_envelope_shape() {
        use crate::compare3::TriStats;
        let stats = TriStats {
            same: 1,
            base_only: 0,
            left_only: 0,
            right_only: 0,
            left_deleted: 0,
            right_deleted: 0,
            left_modified: 0,
            right_modified: 0,
            both_modified: 0,
            conflict: 1,
        };
        let entries = vec![("f.txt".to_string(), "C")];
        let v = envelope_compare3("/b", "/l", "/r", &entries, &stats);
        assert_eq!(v["schema"], "compare3.v1");
        assert_eq!(v["result"]["stats"]["conflict"], 1);
        assert_eq!(v["result"]["entries"][0]["status"], "C");
    }

    #[test]
    fn merge_envelope_shape() {
        let v = envelope_merge("/base", "/l", "/r", 2, Some("/out.txt"));
        assert_eq!(v["schema"], "merge.v1");
        assert_eq!(v["result"]["conflicts"], 2);
        assert_eq!(v["result"]["has_conflicts"], true);
    }

    #[test]
    fn diff_envelope_shape() {
        let ops = vec![
            ("equal".to_string(), 0usize, 2usize, 0usize, 2usize),
            ("replace".to_string(), 2usize, 3usize, 2usize, 3usize),
        ];
        let v = envelope_diff("/l.txt", "/r.txt", &ops, false, false);
        assert_eq!(v["schema"], "diff.v1");
        assert_eq!(v["result"]["has_differences"], true);
        assert_eq!(v["result"]["ops"][1]["tag"], "replace");
        assert_eq!(v["result"]["ops"][0]["old_range"][0], 0);
        assert_eq!(v["result"]["no_newline"]["left"], false);
    }

    #[test]
    fn diff_envelope_no_difference() {
        let ops = vec![("equal".to_string(), 0usize, 2usize, 0usize, 2usize)];
        let v = envelope_diff("/l.txt", "/r.txt", &ops, false, false);
        assert_eq!(v["result"]["has_differences"], false);
    }

    #[test]
    fn diff_envelope_newline_only_difference() {
        // 仅行尾换行不同（左侧无结尾换行、右侧有）→ has_differences=true（GNU diff 兼容）
        let ops = vec![("equal".to_string(), 0usize, 2usize, 0usize, 2usize)];
        let v = envelope_diff("/l.txt", "/r.txt", &ops, true, false);
        assert_eq!(v["result"]["has_differences"], true);
        assert_eq!(v["result"]["no_newline"]["left"], true);
        assert_eq!(v["result"]["no_newline"]["right"], false);
    }

    #[test]
    fn hex_envelope_shape() {
        let rows = vec![(0usize, "00 01".to_string(), "00 02".to_string(), true)];
        let v = envelope_hex("/l.bin", "/r.bin", &rows, 1, 2, 2, 2);
        assert_eq!(v["schema"], "hex.v1");
        assert_eq!(v["result"]["stats"]["diff_rows"], 1);
        assert_eq!(v["result"]["rows"][0]["left"], "00 01");
        assert_eq!(v["result"]["has_differences"], true);
    }

    #[test]
    fn media_envelope_shape() {
        let fields = vec![(
            "sample_rate".to_string(),
            Some("44100 Hz".to_string()),
            Some("48000 Hz".to_string()),
        )];
        let v = envelope_media(
            "/l.wav",
            "/r.wav",
            Some("WAV".to_string()),
            Some("WAV".to_string()),
            &fields,
        );
        assert_eq!(v["schema"], "media.v1");
        assert_eq!(v["result"]["left_format"], "WAV");
        assert_eq!(v["result"]["fields"][0]["name"], "sample_rate");
        assert_eq!(v["result"]["has_differences"], true);
    }
}
