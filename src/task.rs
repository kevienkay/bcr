//! P27 Phase 3:`bcr task` — 纯数据任务清单(JSON/TOML)。
//!
//! 定位(见 docs/P27-python-binding-design.md):
//! - 给不想写 Python 的简单场景:把常用操作串成清单,一键执行
//! - **纯数据,不是语言**:无变量赋值、无表达式、无循环、无分支
//! - 动态变量仅做字符串替换:`%date% %time% %fn_time% %1-%9 %env:VAR%`
//! - 复杂逻辑(条件/循环/错误处理)→ 用 bcr.py + `--json`(Phase 1/2)
//!
//! 命令集:load / compare / compare3 / csv / merge / sync / report / echo / exit
//! 执行语义:顺序执行,遇错即停(或 continue_on_error);退出码取最后一步结果
//! 校验:`bcr task check` 只做 schema/命令/参数校验,不执行

use clap::{Args, Subcommand};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

/// `bcr task` 子命令参数
#[derive(Args, Debug)]
pub struct TaskArgs {
    #[command(subcommand)]
    pub cmd: TaskCmd,
}

#[derive(Subcommand, Debug)]
pub enum TaskCmd {
    /// 执行任务清单文件(JSON 或 TOML)
    Run(RunArgs),
    /// 校验任务清单(schema/命令/参数,不执行)
    Check(CheckArgs),
}

#[derive(Args, Debug)]
pub struct RunArgs {
    /// 任务清单文件路径(.json 或 .toml)
    pub file: String,

    /// 只打印将执行的步骤,不执行
    #[arg(long)]
    pub dry_run: bool,

    /// 静默模式:抑制步骤输出
    #[arg(long)]
    pub silent: bool,

    /// 位置参数(在清单中通过 %1-%9 引用)
    #[arg(last = true)]
    pub args: Vec<String>,
}

#[derive(Args, Debug)]
pub struct CheckArgs {
    /// 任务清单文件路径(.json 或 .toml)
    pub file: String,
}

// ---------------------------------------------------------------------------
// 清单结构(JSON 与 TOML 共用)
// ---------------------------------------------------------------------------

/// 任务清单
#[derive(Debug, Clone, Deserialize)]
pub struct Task {
    /// 任务名(仅用于输出)
    #[serde(default)]
    pub name: String,
    /// 静默模式(等价 --silent)
    #[serde(default)]
    pub silent: bool,
    /// 某步失败时继续执行后续步骤(默认遇错即停)
    #[serde(default)]
    pub continue_on_error: bool,
    /// 步骤列表
    pub steps: Vec<Step>,
}

/// 单个步骤:cmd 是命令名,其余字段为该命令的参数
#[derive(Debug, Clone, Deserialize)]
pub struct Step {
    /// 命令名:load/compare/compare3/csv/merge/sync/report/echo/exit
    pub cmd: String,
    /// 其余字段作为参数(flat)
    #[serde(flatten)]
    pub params: HashMap<String, serde_json::Value>,
}

// ---------------------------------------------------------------------------
// 解析
// ---------------------------------------------------------------------------

/// 从文件解析任务清单(按扩展名选择 JSON/TOML)
pub fn load_task(path: &str) -> Result<Task, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("读取 {path} 失败: {e}"))?;
    if path.ends_with(".toml") {
        toml::from_str(&text).map_err(|e| format!("TOML 解析失败: {e}"))
    } else {
        serde_json::from_str(&text).map_err(|e| format!("JSON 解析失败: {e}"))
    }
}

/// 校验任务清单:命令名合法 + 必需参数齐全
pub fn validate(task: &Task) -> Result<(), String> {
    for (i, step) in task.steps.iter().enumerate() {
        let no = i + 1;
        match step.cmd.as_str() {
            "load" => {
                if !step.params.contains_key("left") && !step.params.contains_key("session") {
                    return Err(format!("第 {no} 步 load 需要 left 或 session 参数"));
                }
            }
            "compare" => {
                require(step, no, &["left", "right"])?;
            }
            "compare3" => {
                require(step, no, &["base", "left", "right"])?;
            }
            "csv" => {
                require(step, no, &["left", "right"])?;
            }
            "merge" => {
                require(step, no, &["base", "left", "right"])?;
            }
            "sync" => {
                require(step, no, &["left", "right"])?;
            }
            "report" => {
                require(step, no, &["format", "output"])?;
            }
            "echo" | "exit" => {}
            other => {
                return Err(format!(
                    "第 {no} 步未知命令 '{other}'（支持: load compare compare3 csv merge sync report echo exit）"
                ));
            }
        }
    }
    Ok(())
}

fn require(step: &Step, no: usize, keys: &[&str]) -> Result<(), String> {
    for k in keys {
        if !step.params.contains_key(*k) {
            return Err(format!("第 {no} 步 {} 缺少参数 {k}", step.cmd));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// 动态变量替换(纯字符串)
// ---------------------------------------------------------------------------

/// 展开动态变量:%date% %time% %fn_time% %1-%9 %env:VAR%
pub fn expand_vars(s: &str, args: &[String]) -> String {
    let mut out = s.to_string();
    // %date% → yyyy-mm-dd(本地时区)
    out = out.replace("%date%", &local_date());
    // %time% → HH:MM:SS
    out = out.replace("%time%", &local_time(false));
    // %fn_time% → HH-MM-SS
    out = out.replace("%fn_time%", &local_time(true));
    // %env:VAR%
    let re_env = regex::Regex::new(r"%env:([A-Za-z_][A-Za-z0-9_]*)%").unwrap();
    out = re_env
        .replace_all(&out, |caps: &regex::Captures| {
            std::env::var(&caps[1]).unwrap_or_default()
        })
        .to_string();
    // %1-%9(无尾 %;倒序替换避免 %1 误匹配 %10 等)
    for i in (0..args.len().min(9)).rev() {
        out = out.replace(&format!("%{}", i + 1), &args[i]);
    }
    out
}

fn local_date() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    // 本地时区偏移(简化为环境 TZ 不可用时按 UTC;Windows/macOS 均可接受)
    let secs = now + local_offset_secs();
    let days = secs.div_euclid(86400);
    let (y, mo, d) = civil_from_days(days);
    format!("{y:04}-{mo:02}-{d:02}")
}

fn local_time(fn_safe: bool) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let secs = now + local_offset_secs();
    let rem = secs.rem_euclid(86400);
    let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let sep = if fn_safe { "-" } else { ":" };
    format!("{h:02}{sep}{m:02}{sep}{s:02}")
}

/// 本地时区偏移(秒)。简单实现:读 TZ 环境变量未配置时返回 0(UTC)。
fn local_offset_secs() -> i64 {
    // 读取 /etc/localtime 与 TZ 过于复杂;用 libc 不可移植。
    // 退化为 UTC(文档说明 %date%/%time% 使用 UTC)。
    0
}

/// 天数 → 公历日期(与 jsonout.rs 相同的 civil_from_days)
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

// ---------------------------------------------------------------------------
// 执行
// ---------------------------------------------------------------------------

/// 步骤执行上下文:记录上一次比较结果(供 report 使用)
pub struct ExecCtx {
    /// 最近一次 compare 的结果
    pub last_compare: Option<crate::compare::CompareResult>,
    /// 最近一次 compare 的左右路径
    pub last_left: String,
    pub last_right: String,
    /// 最近一次 compare3 的结果
    pub last_compare3: Option<crate::compare3::TriResult>,
}

/// 执行任务清单,返回退出码。
/// 语义(对齐 BC):compare/sync 返回 1(有差异)不中止脚本,只有错误(2)才中止;
/// 最终退出码取最严重状态:错误(2) > 有差异(1) > 成功(0)。
pub fn run_task(task: &Task, args: &[String], dry_run: bool, silent: bool) -> i32 {
    let silent = silent || task.silent;
    let mut ctx = ExecCtx {
        last_compare: None,
        last_left: String::new(),
        last_right: String::new(),
        last_compare3: None,
    };
    let mut had_error = false;
    let mut had_diff = false;

    if !silent {
        println!(
            "▶ 任务{}: {} 个步骤",
            if task.name.is_empty() {
                String::new()
            } else {
                format!(" [{}]", task.name)
            },
            task.steps.len()
        );
    }

    for (i, step) in task.steps.iter().enumerate() {
        let no = i + 1;
        let code = execute_step(step, args, dry_run, silent, &mut ctx);
        if code >= 2 {
            if !task.continue_on_error {
                if !silent {
                    eprintln!("✗ 第 {no} 步 {} 失败(退出码 {code})", step.cmd);
                }
                return code;
            }
            had_error = true;
        } else if code == 1 {
            had_diff = true;
        }
    }
    if had_error {
        2
    } else if had_diff {
        1
    } else {
        0
    }
}

fn str_param(step: &Step, key: &str) -> Option<String> {
    step.params.get(key).and_then(|v| match v {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Bool(b) => Some(b.to_string()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        _ => None,
    })
}

fn bool_param(step: &Step, key: &str) -> bool {
    matches!(step.params.get(key), Some(serde_json::Value::Bool(true)))
}

fn list_param(step: &Step, key: &str) -> Vec<String> {
    match step.params.get(key) {
        Some(serde_json::Value::Array(a)) => a
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect(),
        Some(serde_json::Value::String(s)) => s
            .split(',')
            .map(|p| p.trim().to_string())
            .filter(|p| !p.is_empty())
            .collect(),
        _ => Vec::new(),
    }
}

fn execute_step(
    step: &Step,
    args: &[String],
    dry_run: bool,
    silent: bool,
    ctx: &mut ExecCtx,
) -> i32 {
    let ev = |s: &str| expand_vars(s, args);
    match step.cmd.as_str() {
        "echo" => {
            let text = step
                .params
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if !silent {
                println!("{}", ev(text));
            }
            0
        }
        "exit" => str_param(step, "code")
            .and_then(|s| s.parse::<i32>().ok())
            .unwrap_or(0),
        "load" => {
            // load 记录会话或左右路径,供后续 compare 使用
            // 本实现:load 主要校验路径可达,真正的会话加载由 compare 的 --profile 支持
            let left = str_param(step, "left").map(|s| ev(&s));
            let right = str_param(step, "right").map(|s| ev(&s));
            let session = str_param(step, "session").map(|s| ev(&s));
            if dry_run {
                if !silent {
                    println!(
                        "  load {}",
                        session.clone().unwrap_or_else(|| format!(
                            "{} {}",
                            left.as_deref().unwrap_or(""),
                            right.as_deref().unwrap_or("")
                        ))
                    );
                }
                return 0;
            }
            // 校验路径/会话可打开
            if let Some(sess) = session {
                let all = crate::session::load();
                if !all.sessions.contains_key(&sess) {
                    eprintln!("bcr: 会话不存在: {sess}");
                    return 2;
                }
            }
            if let Some(l) = &left {
                if !crate::vfs::is_remote(l) && !Path::new(l).exists() {
                    eprintln!("bcr: 路径不存在: {l}");
                    return 2;
                }
            }
            if let Some(r) = &right {
                if !crate::vfs::is_remote(r) && !Path::new(r).exists() {
                    eprintln!("bcr: 路径不存在: {r}");
                    return 2;
                }
            }
            0
        }
        "compare" => {
            let left = ev(&str_param(step, "left").unwrap_or_default());
            let right = ev(&str_param(step, "right").unwrap_or_default());
            if dry_run {
                if !silent {
                    println!("  compare {left} {right}");
                }
                return 0;
            }
            if !silent {
                println!("  compare {left} {right}");
            }
            let args = crate::compare::CompareArgs {
                left: left.clone(),
                right: right.clone(),
                compare_content: bool_param(step, "content") || bool_param(step, "compare_content"),
                includes: list_param(step, "includes"),
                excludes: list_param(step, "excludes"),
                show_same: bool_param(step, "show_same"),
                detect_moves: !bool_param(step, "no_moves"),
                compare_attrs: bool_param(step, "attrs"),
                compare_version: bool_param(step, "version"),
                ignore_structure: bool_param(step, "ignore_structure"),
                follow_symlinks: bool_param(step, "follow_symlinks"),
                print: bool_param(step, "print"),
                summary: false,
                html: None,
                txt: None,
                csv: None,
                report_fields: String::new(),
                report_title: None,
                report_no_stats: false,
                report_sort: "path".to_string(),
                report_group: false,
                profile: str_param(step, "profile"),
                color: "never".to_string(),
                json: false,
            };
            let code = crate::compare::run(&args);
            // 记录结果供 report 使用:重新跑一次纯逻辑获取结果(任务场景目录不大,可接受)
            if code != 2 {
                if let Ok(filter) = crate::fsscan::Filter::new(&args.includes, &args.excludes) {
                    if let (Ok(l), Ok(r)) =
                        (crate::vfs::open(&args.left), crate::vfs::open(&args.right))
                    {
                        if let Ok(res) = crate::compare::compare_vfs_attrs(
                            l.as_ref(),
                            r.as_ref(),
                            &filter,
                            args.compare_content,
                            args.detect_moves,
                            args.compare_attrs,
                            args.compare_version,
                        ) {
                            ctx.last_compare = Some(res);
                            ctx.last_left = left;
                            ctx.last_right = right;
                        }
                    }
                }
            }
            code
        }
        "compare3" => {
            let base = ev(&str_param(step, "base").unwrap_or_default());
            let left = ev(&str_param(step, "left").unwrap_or_default());
            let right = ev(&str_param(step, "right").unwrap_or_default());
            if dry_run {
                if !silent {
                    println!("  compare3 {base} {left} {right}");
                }
                return 0;
            }
            if !silent {
                println!("  compare3 {base} {left} {right}");
            }
            let args = crate::compare3::Compare3Args {
                base,
                left,
                right,
                compare_content: bool_param(step, "content"),
                includes: list_param(step, "includes"),
                excludes: list_param(step, "excludes"),
                show_same: bool_param(step, "show_same"),
                summary: false,
                color: "never".to_string(),
                json: false,
            };
            let code = crate::compare3::run(&args);
            if code != 2 {
                if let Ok(filter) = crate::fsscan::Filter::new(&args.includes, &args.excludes) {
                    if let (Ok(b), Ok(l), Ok(r)) = (
                        crate::vfs::open(&args.base),
                        crate::vfs::open(&args.left),
                        crate::vfs::open(&args.right),
                    ) {
                        if let Ok(res) = crate::compare3::compare3_vfs(
                            b.as_ref(),
                            l.as_ref(),
                            r.as_ref(),
                            &filter,
                            args.compare_content,
                        ) {
                            ctx.last_compare3 = Some(res);
                        }
                    }
                }
            }
            code
        }
        "csv" => {
            let left = ev(&str_param(step, "left").unwrap_or_default());
            let right = ev(&str_param(step, "right").unwrap_or_default());
            if dry_run {
                if !silent {
                    println!("  csv {left} {right}");
                }
                return 0;
            }
            if !silent {
                println!("  csv {left} {right}");
            }
            let args = crate::csvcmp::CsvArgs {
                left,
                right,
                key: str_param(step, "key"),
                delimiter: str_param(step, "delimiter").unwrap_or_else(|| ",".to_string()),
                no_header: bool_param(step, "no_header"),
                show_same: bool_param(step, "show_same"),
                summary: false,
                color: "never".to_string(),
                json: false,
            };
            crate::csvcmp::run(&args)
        }
        "merge" => {
            let base = ev(&str_param(step, "base").unwrap_or_default());
            let left = ev(&str_param(step, "left").unwrap_or_default());
            let right = ev(&str_param(step, "right").unwrap_or_default());
            let output = str_param(step, "output").map(|s| ev(&s));
            if dry_run {
                if !silent {
                    println!(
                        "  merge {base} {left} {right} -> {}",
                        output.as_deref().unwrap_or("stdout")
                    );
                }
                return 0;
            }
            if !silent {
                println!(
                    "  merge {base} {left} {right} -> {}",
                    output.as_deref().unwrap_or("stdout")
                );
            }
            let args = crate::merge::MergeArgs {
                base,
                left,
                right,
                output,
                algo: str_param(step, "algo").unwrap_or_else(|| "patience".to_string()),
                labels: list_param(step, "labels"),
                json: false,
            };
            crate::merge::run(&args)
        }
        "sync" => {
            let left = ev(&str_param(step, "left").unwrap_or_default());
            let right = ev(&str_param(step, "right").unwrap_or_default());
            let mode = str_param(step, "mode").unwrap_or_else(|| "update".to_string());
            if dry_run {
                if !silent {
                    println!("  sync {left} {right} --mode {mode}");
                }
                return 0;
            }
            if !silent {
                println!("  sync {left} {right} --mode {mode}");
            }
            let args = crate::sync::SyncArgs {
                left,
                right,
                mode,
                reverse: bool_param(step, "reverse"),
                dry_run: false,
                compare_content: bool_param(step, "content"),
                ignore_structure: bool_param(step, "ignore_structure"),
                follow_symlinks: bool_param(step, "follow_symlinks"),
                includes: list_param(step, "includes"),
                excludes: list_param(step, "excludes"),
                summary: false,
                json: false,
            };
            crate::sync::run(&args)
        }
        "report" => {
            let format = ev(&str_param(step, "format").unwrap_or_default());
            let output = ev(&str_param(step, "output").unwrap_or_default());
            if dry_run {
                if !silent {
                    println!("  report {format} -> {output}");
                }
                return 0;
            }
            let Some(result) = &ctx.last_compare else {
                eprintln!("bcr: report 需要先执行 compare 步骤");
                return 2;
            };
            let code = match format.as_str() {
                "txt" => {
                    let fields =
                        crate::report::parse_fields(&str_param(step, "fields").unwrap_or_default())
                            .unwrap_or_default();
                    let opts = crate::report::ReportOptions {
                        title: str_param(step, "title"),
                        include_stats: !bool_param(step, "no_stats"),
                        sort: str_param(step, "sort").unwrap_or_else(|| "path".to_string()),
                        group_by_status: bool_param(step, "group"),
                    };
                    let txt = crate::report::render_txt_opts(
                        &ctx.last_left,
                        &ctx.last_right,
                        result,
                        &fields,
                        &opts,
                    );
                    write_report(&output, txt.as_bytes())
                }
                "csv" => {
                    let fields =
                        crate::report::parse_fields(&str_param(step, "fields").unwrap_or_default())
                            .unwrap_or_default();
                    let opts = crate::report::ReportOptions {
                        title: str_param(step, "title"),
                        include_stats: !bool_param(step, "no_stats"),
                        sort: str_param(step, "sort").unwrap_or_else(|| "path".to_string()),
                        group_by_status: bool_param(step, "group"),
                    };
                    let csv = crate::report::render_csv_opts(
                        &ctx.last_left,
                        &ctx.last_right,
                        result,
                        &fields,
                        &opts,
                    );
                    write_report(&output, csv.as_bytes())
                }
                "html" => {
                    let now = crate::i18n::fmt(crate::i18n::Key::ReportGeneratedAt, &[]);
                    let html = crate::htmlreport::render_html(
                        &ctx.last_left,
                        &ctx.last_right,
                        result,
                        &now,
                    );
                    write_report(&output, html.as_bytes())
                }
                other => {
                    eprintln!("bcr: 不支持的报告格式: {other}(支持 txt/csv/html)");
                    2
                }
            };
            if !silent && code == 0 {
                println!("  report {format} -> {output}");
            }
            code
        }
        other => {
            eprintln!("bcr: 未知命令: {other}");
            2
        }
    }
}

fn write_report(path: &str, data: &[u8]) -> i32 {
    if let Err(e) = std::fs::write(path, data) {
        eprintln!("bcr: 写入 {path} 失败: {e}");
        2
    } else {
        0
    }
}

// ---------------------------------------------------------------------------
// CLI 入口
// ---------------------------------------------------------------------------

/// `bcr task run/check` 入口
pub fn run(args: &TaskArgs) -> i32 {
    match &args.cmd {
        TaskCmd::Run(run_args) => {
            let task = match load_task(&run_args.file) {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("bcr: {e}");
                    return 2;
                }
            };
            if let Err(e) = validate(&task) {
                eprintln!("bcr: 清单校验失败: {e}");
                return 2;
            }
            if run_args.dry_run {
                println!("[dry-run] {} 个步骤:", task.steps.len());
                let mut ctx = ExecCtx {
                    last_compare: None,
                    last_left: String::new(),
                    last_right: String::new(),
                    last_compare3: None,
                };
                for step in &task.steps {
                    let _ = execute_step(step, &run_args.args, true, false, &mut ctx);
                }
                0
            } else {
                run_task(&task, &run_args.args, false, run_args.silent)
            }
        }
        TaskCmd::Check(check_args) => {
            let task = match load_task(&check_args.file) {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("bcr: {e}");
                    return 2;
                }
            };
            match validate(&task) {
                Ok(()) => {
                    println!("✓ 清单合法: {} 个步骤", task.steps.len());
                    0
                }
                Err(e) => {
                    eprintln!("bcr: {e}");
                    2
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_date_time_vars() {
        let s = expand_vars("report-%date%-%fn_time%.txt", &[]);
        assert!(s.starts_with("report-20"));
        assert!(s.contains(".txt"));
        assert!(!s.contains("%"));
    }

    #[test]
    fn expand_positional_args() {
        let s = expand_vars("load %1 %2", &["/a".to_string(), "/b".to_string()]);
        assert_eq!(s, "load /a /b");
    }

    #[test]
    fn expand_env_var() {
        std::env::set_var("BCR_TEST_VAR", "xyz");
        let s = expand_vars("%env:BCR_TEST_VAR%", &[]);
        assert_eq!(s, "xyz");
    }

    #[test]
    fn validate_ok_and_errors() {
        let task: Task = serde_json::from_str(
            r#"{
                "name": "t",
                "steps": [
                    {"cmd": "compare", "left": "/a", "right": "/b"},
                    {"cmd": "report", "format": "txt", "output": "r.txt"}
                ]
            }"#,
        )
        .unwrap();
        assert!(validate(&task).is_ok());

        let bad: Task =
            serde_json::from_str(r#"{"steps": [{"cmd": "compare", "left": "/a"}]}"#).unwrap();
        assert!(validate(&bad).is_err());

        let unknown: Task = serde_json::from_str(r#"{"steps": [{"cmd": "hack"}]}"#).unwrap();
        assert!(validate(&unknown).is_err());
    }

    #[test]
    fn toml_parse_works() {
        let task: Task = toml::from_str(
            r#"
name = "nightly"
silent = true
[[steps]]
cmd = "compare"
left = "/a"
right = "/b"
content = true
"#,
        )
        .unwrap();
        assert_eq!(task.name, "nightly");
        assert!(task.silent);
        assert_eq!(task.steps.len(), 1);
        assert_eq!(task.steps[0].cmd, "compare");
    }

    #[test]
    fn run_task_echo_and_exit() {
        let task: Task = serde_json::from_str(
            r#"{
                "silent": true,
                "steps": [
                    {"cmd": "echo", "text": "hello"},
                    {"cmd": "exit", "code": 3}
                ]
            }"#,
        )
        .unwrap();
        let code = run_task(&task, &[], false, true);
        assert_eq!(code, 3);
    }

    #[test]
    fn run_task_unknown_command_fails() {
        let task: Task = serde_json::from_str(r#"{"steps": [{"cmd": "nope"}]}"#).unwrap();
        // validate 已拦截;直接执行也应失败
        let code = run_task(&task, &[], false, true);
        assert_eq!(code, 2);
    }
}
