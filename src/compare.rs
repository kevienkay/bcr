use crate::fsscan::{FileMeta, Filter};
use crate::i18n::{fmt, Key};
use crate::vfs::{LocalVfs, Vfs};
use clap::Args;
use std::io::{self, IsTerminal};
use std::path::Path;

const RED: &str = "\x1b[31m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const BLUE: &str = "\x1b[34m";
const RESET: &str = "\x1b[0m";

/// compare 子命令参数
#[derive(Args, Debug, Clone)]
pub struct CompareArgs {
    /// 左侧目录
    pub left: String,

    /// 右侧目录
    pub right: String,

    /// 深度比较：对大小相同的文件对做 blake3 哈希比对（默认仅比较大小+修改时间）
    #[arg(long)]
    pub compare_content: bool,

    /// 包含过滤（glob，可重复），仅比较匹配的文件
    #[arg(long = "include")]
    pub includes: Vec<String>,

    /// 排除过滤（glob，可重复），跳过匹配的文件/目录
    #[arg(long = "exclude")]
    pub excludes: Vec<String>,

    /// 同时显示相同的文件
    #[arg(long)]
    pub show_same: bool,

    /// 检测重命名/移动（默认开启；关闭可避免误判）
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    pub detect_moves: bool,

    /// 比较文件属性（Unix 权限位/符号链接目标；默认仅比较大小+时间+内容）
    #[arg(long)]
    pub compare_attrs: bool,

    /// 输出统计信息
    #[arg(long)]
    pub summary: bool,

    /// 导出 HTML 对比报告到指定文件（自包含，浏览器可直接打开）
    #[arg(long = "html")]
    pub html: Option<String>,

    /// 导出文本对比报告到指定文件（统计 + 差异条目表）
    #[arg(long = "txt")]
    pub txt: Option<String>,

    /// 导出 CSV 对比报告到指定文件（机器可读，每行一个条目）
    #[arg(long = "csv")]
    pub csv: Option<String>,

    /// 复用已保存的规则 Profile（过滤/忽略/编码等，可叠加本命令显式参数）
    #[arg(long)]
    pub profile: Option<String>,

    /// 颜色输出：auto | always | never
    #[arg(long, default_value = "auto", value_parser = ["auto", "always", "never"])]
    pub color: String,
}

/// 合并 Profile 到命令参数：Profile 提供默认值，命令显式参数优先。
fn merge_profile(args: &CompareArgs) -> CompareArgs {
    let Some(name) = &args.profile else {
        return args.clone();
    };
    let Ok(p) = crate::profile::get(name) else {
        eprintln!(
            "bcr: {}",
            crate::i18n::fmt(crate::i18n::Key::ProfileNotFound, &[name])
        );
        std::process::exit(2);
    };
    let mut out = args.clone();
    if out.includes.is_empty() {
        out.includes = p.includes;
    }
    if out.excludes.is_empty() {
        out.excludes = p.excludes;
    }
    if !out.compare_content && p.compare_content {
        out.compare_content = true;
    }
    if out.detect_moves && !p.detect_moves {
        out.detect_moves = false;
    }
    if let Some(enc) = p.encoding {
        // 仅当用户未显式指定编码时应用（--encoding 是全局参数，已写入 BCR_ENCODING）
        if std::env::var("BCR_ENCODING")
            .map(|v| v.is_empty())
            .unwrap_or(true)
        {
            unsafe { std::env::set_var("BCR_ENCODING", enc) };
        }
    }
    out
}

/// 文件比较状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileStatus {
    Same,
    LeftOnly,
    RightOnly,
    Differ,
    /// 检测为移动/重命名（内容相同，仅路径不同）
    Moved,
}

impl FileStatus {
    pub fn letter(self) -> char {
        match self {
            FileStatus::Same => 'S',
            FileStatus::LeftOnly => 'L',
            FileStatus::RightOnly => 'R',
            FileStatus::Differ => 'C',
            FileStatus::Moved => 'M',
        }
    }
}

/// 单个文件的比较条目
#[derive(Debug, Clone)]
pub struct FileEntry {
    pub rel: String,
    pub status: FileStatus,
    pub left: Option<FileMeta>,
    pub right: Option<FileMeta>,
    /// 移动/重命名的目标相对路径（仅 status == Moved 时有值）
    pub moved_to: Option<String>,
    /// 内容一致但属性不同（权限/符号链接，仅 --compare-attrs 时可能为 true）
    pub attrs_differ: bool,
}

/// 比较统计
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CompareStats {
    pub same: usize,
    pub left_only: usize,
    pub right_only: usize,
    pub differ: usize,
    /// 检测到的移动/重命名对数
    pub moved: usize,
}

impl CompareStats {
    pub fn has_differences(self) -> bool {
        self.left_only + self.right_only + self.differ + self.moved > 0
    }
}

/// 目录比较结果（CLI 与 GUI 共用）
#[derive(Debug, Default, Clone)]
pub struct CompareResult {
    /// 排序后的条目（BTreeMap 顺序）
    pub entries: Vec<FileEntry>,
    pub stats: CompareStats,
    /// 比较过程中的非致命警告（如读取失败被跳过）
    pub warnings: Vec<String>,
}

/// 对比两个目录树，返回结构化结果。
///
/// 行为与 CLI 一致：快速模式比较大小+mtime；compare_content 时对大小相同的
/// 文件对做哈希比对。读取失败的文件记入 warnings 并跳过。
pub fn compare_dirs(
    left_dir: &Path,
    right_dir: &Path,
    filter: &Filter,
    compare_content: bool,
    enable_moves: bool,
) -> io::Result<CompareResult> {
    compare_dirs_attrs(
        left_dir,
        right_dir,
        filter,
        compare_content,
        enable_moves,
        false,
    )
}

/// 带属性比较的目录对比（compare_attrs=true 时权限/符号链接差异计入 Differ）
pub fn compare_dirs_attrs(
    left_dir: &Path,
    right_dir: &Path,
    filter: &Filter,
    compare_content: bool,
    enable_moves: bool,
    compare_attrs: bool,
) -> io::Result<CompareResult> {
    let left = LocalVfs::new(left_dir)?;
    let right = LocalVfs::new(right_dir)?;
    compare_vfs_attrs(
        &left,
        &right,
        filter,
        compare_content,
        enable_moves,
        compare_attrs,
    )
}

/// 对比两个虚拟文件系统后端，返回结构化结果（CLI/GUI/远程共用）。
#[allow(dead_code)] // 保留为公共 API，供外部以默认属性比较复用
pub fn compare_vfs(
    left: &dyn Vfs,
    right: &dyn Vfs,
    filter: &Filter,
    compare_content: bool,
    enable_moves: bool,
) -> io::Result<CompareResult> {
    compare_vfs_attrs(left, right, filter, compare_content, enable_moves, false)
}

/// 带属性比较的虚拟后端对比
pub fn compare_vfs_attrs(
    left: &dyn Vfs,
    right: &dyn Vfs,
    filter: &Filter,
    compare_content: bool,
    enable_moves: bool,
    compare_attrs: bool,
) -> io::Result<CompareResult> {
    let left_map = left.scan(filter)?;
    let right_map = right.scan(filter)?;

    // 合并 key 集合（已排序，保证输出顺序稳定）
    let mut keys: Vec<&String> = Vec::with_capacity(left_map.len() + right_map.len());
    for k in left_map.keys() {
        keys.push(k);
    }
    for k in right_map.keys() {
        if !left_map.contains_key(k) {
            keys.push(k);
        }
    }
    keys.sort();

    let mut result = CompareResult::default();
    for key in keys {
        match (left_map.get(key), right_map.get(key)) {
            (Some(l), Some(r)) => {
                let content_same = if l.size != r.size {
                    false
                } else if compare_content {
                    match crate::vfs::content_equal_vfs(left, right, key) {
                        Ok(eq) => eq,
                        Err(e) => {
                            result.warnings.push(format!("读取 {key} 失败: {e}"));
                            continue;
                        }
                    }
                } else {
                    l.mtime == r.mtime
                };
                // 属性比较：内容一致但权限/符号链接不同 → 计为 Differ（--compare-attrs）
                let attrs_differ = compare_attrs && attrs_diff(l, r);
                let same = content_same && !attrs_differ;
                if same {
                    result.stats.same += 1;
                    result.entries.push(FileEntry {
                        rel: key.clone(),
                        status: FileStatus::Same,
                        left: Some(l.clone()),
                        right: Some(r.clone()),
                        moved_to: None,
                        attrs_differ: false,
                    });
                } else {
                    result.stats.differ += 1;
                    result.entries.push(FileEntry {
                        rel: key.clone(),
                        status: FileStatus::Differ,
                        left: Some(l.clone()),
                        right: Some(r.clone()),
                        moved_to: None,
                        attrs_differ,
                    });
                }
            }
            (Some(l), None) => {
                result.stats.left_only += 1;
                result.entries.push(FileEntry {
                    rel: key.clone(),
                    status: FileStatus::LeftOnly,
                    left: Some(l.clone()),
                    right: None,
                    moved_to: None,
                    attrs_differ: false,
                });
            }
            (None, Some(r)) => {
                result.stats.right_only += 1;
                result.entries.push(FileEntry {
                    rel: key.clone(),
                    status: FileStatus::RightOnly,
                    left: None,
                    right: Some(r.clone()),
                    moved_to: None,
                    attrs_differ: false,
                });
            }
            (None, None) => unreachable!(),
        }
    }

    // 重命名/移动检测：把内容相同的 仅左侧+仅右侧 对合并为 Moved
    if enable_moves {
        detect_moves(&mut result, left, right);
    }
    Ok(result)
}

/// 重命名/移动检测：仅左侧与仅右侧中，尺寸相同且内容哈希一致的文件对
/// 判定为移动，合并为 Moved 条目（始终读内容，避免 size+mtime 巧合误判）。
fn detect_moves(result: &mut CompareResult, left: &dyn Vfs, right: &dyn Vfs) {
    let left_idx: Vec<usize> = result
        .entries
        .iter()
        .enumerate()
        .filter(|(_, e)| e.status == FileStatus::LeftOnly)
        .map(|(i, _)| i)
        .collect();
    let right_idx: Vec<usize> = result
        .entries
        .iter()
        .enumerate()
        .filter(|(_, e)| e.status == FileStatus::RightOnly)
        .map(|(i, _)| i)
        .collect();
    if left_idx.is_empty() || right_idx.is_empty() {
        return;
    }

    // 按尺寸分组右侧独有文件，避免全量两两比较
    let mut by_size: std::collections::BTreeMap<u64, Vec<usize>> = Default::default();
    for &ri in &right_idx {
        if let Some(r) = &result.entries[ri].right {
            by_size.entry(r.size).or_default().push(ri);
        }
    }

    let mut used = vec![false; result.entries.len()];
    let mut matched_left: Vec<usize> = Vec::new();
    let mut matched_right: Vec<usize> = Vec::new();

    for &li in &left_idx {
        let l = match &result.entries[li].left {
            Some(m) => m.clone(),
            None => continue,
        };
        let Some(cands) = by_size.get(&l.size) else {
            continue;
        };
        for &ri in cands {
            if used[ri] {
                continue;
            }
            // 始终用内容哈希确认移动（与 BC Detect Moves 一致）：
            // 仅对“仅左侧/仅右侧”候选对读内容，避免 size+mtime 巧合误判；
            // 走 Vfs::hash 流式计算，超大文件也不占内存
            let same = match (
                left.hash(&result.entries[li].rel),
                right.hash(&result.entries[ri].rel),
            ) {
                (Ok(lh), Ok(rh)) => lh == rh,
                _ => false,
            };
            if same {
                used[ri] = true;
                matched_left.push(li);
                matched_right.push(ri);
                break;
            }
        }
    }

    let n = matched_left.len();
    if n == 0 {
        return;
    }
    // 目标路径在 retain 前克隆（retain 会改动 entries）
    let targets: Vec<String> = matched_right
        .iter()
        .map(|&ri| result.entries[ri].rel.clone())
        .collect();
    let right_meta: Vec<Option<FileMeta>> = matched_right
        .iter()
        .map(|&ri| result.entries[ri].right.clone())
        .collect();
    for (k, &li) in matched_left.iter().enumerate() {
        let e = &mut result.entries[li];
        e.status = FileStatus::Moved;
        e.moved_to = Some(targets[k].clone());
        e.right = right_meta[k].clone();
    }
    // 移除已匹配的右侧独有条目
    let remove: std::collections::HashSet<usize> = matched_right.iter().copied().collect();
    let mut i = 0usize;
    result.entries.retain(|_| {
        let keep = !remove.contains(&i);
        i += 1;
        keep
    });
    result.stats.moved += n;
    result.stats.left_only -= n;
    result.stats.right_only -= n;
}

/// 运行 compare 子命令，返回进程退出码（0=无差异，1=有差异，2=错误）
/// 运行 compare 子命令，返回进程退出码（0=无差异，1=有差异，2=错误）
pub fn run(args: &CompareArgs) -> i32 {
    // Profile 规则合并：profile 提供过滤/忽略/编码等默认值，命令显式参数覆盖
    let merged = merge_profile(args);
    let args = &merged;
    // 本地路径需要是目录；zip:// 与 sftp:// 交给 vfs::open 处理
    if !crate::vfs::is_remote(&args.left) && !Path::new(&args.left).is_dir() {
        eprintln!("bcr: {}", fmt(Key::NotDir, &[&args.left]));
        return 2;
    }
    if !crate::vfs::is_remote(&args.right) && !Path::new(&args.right).is_dir() {
        eprintln!("bcr: {}", fmt(Key::NotDir, &[&args.right]));
        return 2;
    }

    let filter = match Filter::new(&args.includes, &args.excludes) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("bcr: {}", fmt(Key::FilterError, &[&e.to_string()]));
            return 2;
        }
    };

    let left = match crate::vfs::open(&args.left) {
        Ok(v) => v,
        Err(e) => {
            eprintln!(
                "bcr: {}",
                fmt(Key::OpenFailed, &[&args.left, &e.to_string()])
            );
            return 2;
        }
    };
    let right = match crate::vfs::open(&args.right) {
        Ok(v) => v,
        Err(e) => {
            eprintln!(
                "bcr: {}",
                fmt(Key::OpenFailed, &[&args.right, &e.to_string()])
            );
            return 2;
        }
    };

    let result = match compare_vfs_attrs(
        left.as_ref(),
        right.as_ref(),
        &filter,
        args.compare_content,
        args.detect_moves,
        args.compare_attrs,
    ) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("bcr: {}", fmt(Key::ScanFailed, &[&e.to_string()]));
            return 2;
        }
    };

    for w in &result.warnings {
        eprintln!("bcr: {w}");
    }

    let color = match args.color.as_str() {
        "always" => true,
        "never" => false,
        _ => io::stdout().is_terminal(),
    };

    for entry in &result.entries {
        if entry.status == FileStatus::Same && !args.show_same {
            continue;
        }
        if entry.status == FileStatus::Moved {
            let target = entry.moved_to.as_deref().unwrap_or("");
            if color {
                println!("{BLUE}[M]{RESET} {} -> {}", entry.rel, target);
            } else {
                println!("[M] {} -> {}", entry.rel, target);
            }
            continue;
        }
        emit(entry.status.letter(), &entry.rel, color);
        if entry.attrs_differ {
            eprintln!("  ↳ 属性不同（权限/符号链接）");
        }
    }

    if args.summary {
        let s = result.stats;
        println!(
            "{}",
            fmt(
                Key::SummaryCompare,
                &[
                    &s.same.to_string(),
                    &s.left_only.to_string(),
                    &s.right_only.to_string(),
                    &s.differ.to_string(),
                ]
            )
        );
        if s.moved > 0 {
            println!("{}", fmt(Key::SummaryMoved, &[&s.moved.to_string()]));
        }
    }

    if let Some(html_path) = &args.html {
        let now = crate::i18n::fmt(Key::ReportGeneratedAt, &[]);
        let html = crate::htmlreport::render_html(&args.left, &args.right, &result, &now);
        if let Err(e) = std::fs::write(html_path, html) {
            eprintln!(
                "bcr: {}",
                fmt(Key::WriteFailed, &[html_path, &e.to_string()])
            );
            return 2;
        }
    }
    if let Some(txt_path) = &args.txt {
        let txt = crate::report::render_txt(&args.left, &args.right, &result);
        if let Err(e) = std::fs::write(txt_path, txt) {
            eprintln!(
                "bcr: {}",
                fmt(Key::WriteFailed, &[txt_path, &e.to_string()])
            );
            return 2;
        }
    }
    if let Some(csv_path) = &args.csv {
        let csv = crate::report::render_csv(&args.left, &args.right, &result);
        if let Err(e) = std::fs::write(csv_path, csv) {
            eprintln!(
                "bcr: {}",
                fmt(Key::WriteFailed, &[csv_path, &e.to_string()])
            );
            return 2;
        }
    }

    if result.stats.has_differences() {
        1
    } else {
        0
    }
}

/// 属性差异判定：权限位（仅 Unix 有）或符号链接目标不同即视为属性差异。
/// 任一侧缺失属性信息（后端不支持）视为相同，避免远程/压缩包误报。
fn attrs_diff(l: &FileMeta, r: &FileMeta) -> bool {
    let mode_diff = match (l.mode, r.mode) {
        (Some(a), Some(b)) => a != b,
        _ => false,
    };
    let link_diff = l.symlink != r.symlink;
    mode_diff || link_diff
}

fn emit(status: char, rel: &str, color: bool) {
    if color {
        let c = match status {
            'L' => RED,
            'R' => BLUE,
            'C' => YELLOW,
            _ => GREEN,
        };
        println!("{c}[{status}]{RESET} {rel}");
    } else {
        println!("[{status}] {rel}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn args(left: &str, right: &str) -> CompareArgs {
        CompareArgs {
            left: left.into(),
            right: right.into(),
            compare_content: false,
            includes: vec![],
            excludes: vec![],
            show_same: false,
            detect_moves: true,
            compare_attrs: false,
            summary: false,
            html: None,
            txt: None,
            csv: None,
            profile: None,
            color: "never".into(),
        }
    }

    /// 构建目录树：entries 为 (相对路径, 内容)，统一写入固定 mtime 保证快速模式可比
    fn make_tree(dir: &std::path::Path, entries: &[(&str, &str)]) {
        let fixed = filetime::FileTime::from_unix_time(1_700_000_000, 0);
        for (rel, content) in entries {
            let p = dir.join(rel);
            if let Some(parent) = p.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(p, content).unwrap();
            filetime::set_file_mtime(dir.join(rel), fixed).unwrap();
        }
    }

    fn empty_filter() -> Filter {
        Filter::new(&[], &[]).unwrap()
    }

    #[test]
    fn run_identical_dirs_exit_zero() {
        let d1 = tempdir().unwrap();
        let d2 = tempdir().unwrap();
        make_tree(d1.path(), &[("a.txt", "x"), ("sub/b.txt", "y")]);
        make_tree(d2.path(), &[("a.txt", "x"), ("sub/b.txt", "y")]);
        let a = args(d1.path().to_str().unwrap(), d2.path().to_str().unwrap());
        assert_eq!(run(&a), 0);
    }

    #[test]
    fn run_different_dirs_exit_one() {
        let d1 = tempdir().unwrap();
        let d2 = tempdir().unwrap();
        make_tree(d1.path(), &[("only_l.txt", "x")]);
        make_tree(d2.path(), &[("only_r.txt", "x")]);
        let a = args(d1.path().to_str().unwrap(), d2.path().to_str().unwrap());
        assert_eq!(run(&a), 1);
    }

    #[test]
    fn run_missing_dir_exit_two() {
        let d = tempdir().unwrap();
        let a = args(d.path().to_str().unwrap(), "/nonexistent/bcr-dir");
        assert_eq!(run(&a), 2);
    }

    #[test]
    fn run_quick_mode_mtime_diff_counts_as_differ() {
        let d1 = tempdir().unwrap();
        let d2 = tempdir().unwrap();
        make_tree(d1.path(), &[("f.txt", "same")]);
        make_tree(d2.path(), &[("f.txt", "same")]);
        // 内容相同但 mtime 不同 → 快速模式判为不同
        let old = filetime::FileTime::from_unix_time(1_600_000_000, 0);
        filetime::set_file_mtime(d1.path().join("f.txt"), old).unwrap();
        let new = filetime::FileTime::from_unix_time(1_700_000_000, 0);
        filetime::set_file_mtime(d2.path().join("f.txt"), new).unwrap();
        let a = args(d1.path().to_str().unwrap(), d2.path().to_str().unwrap());
        assert_eq!(run(&a), 1);
    }

    #[test]
    fn run_content_mode_ignores_mtime() {
        let d1 = tempdir().unwrap();
        let d2 = tempdir().unwrap();
        make_tree(d1.path(), &[("f.txt", "same")]);
        make_tree(d2.path(), &[("f.txt", "same")]);
        let old = filetime::FileTime::from_unix_time(1_600_000_000, 0);
        filetime::set_file_mtime(d1.path().join("f.txt"), old).unwrap();
        let new = filetime::FileTime::from_unix_time(1_700_000_000, 0);
        filetime::set_file_mtime(d2.path().join("f.txt"), new).unwrap();
        let mut a = args(d1.path().to_str().unwrap(), d2.path().to_str().unwrap());
        a.compare_content = true;
        assert_eq!(run(&a), 0);
    }

    #[test]
    fn run_content_mode_detects_different_content() {
        let d1 = tempdir().unwrap();
        let d2 = tempdir().unwrap();
        make_tree(d1.path(), &[("f.txt", "aaa")]);
        make_tree(d2.path(), &[("f.txt", "bbb")]);
        let t = filetime::FileTime::from_unix_time(1_700_000_000, 0);
        filetime::set_file_mtime(d1.path().join("f.txt"), t).unwrap();
        filetime::set_file_mtime(d2.path().join("f.txt"), t).unwrap();
        let mut a = args(d1.path().to_str().unwrap(), d2.path().to_str().unwrap());
        a.compare_content = true;
        assert_eq!(run(&a), 1);
    }

    #[test]
    fn run_include_filter_limits_comparison() {
        let d1 = tempdir().unwrap();
        let d2 = tempdir().unwrap();
        make_tree(d1.path(), &[("keep.txt", "x"), ("drop.txt", "y")]);
        make_tree(d2.path(), &[("keep.txt", "x")]);
        // 不带 include：drop.txt 仅左侧 → 有差异
        let a = args(d1.path().to_str().unwrap(), d2.path().to_str().unwrap());
        assert_eq!(run(&a), 1);
        // 带 include：drop.txt 被过滤 → 无差异
        let mut b = args(d1.path().to_str().unwrap(), d2.path().to_str().unwrap());
        b.includes = vec!["keep.txt".into()];
        assert_eq!(run(&b), 0);
    }

    #[test]
    fn run_exclude_filter_ignores_files() {
        let d1 = tempdir().unwrap();
        let d2 = tempdir().unwrap();
        make_tree(d1.path(), &[("keep.txt", "x"), ("skip.txt", "y")]);
        make_tree(d2.path(), &[("keep.txt", "x")]);
        let mut a = args(d1.path().to_str().unwrap(), d2.path().to_str().unwrap());
        a.excludes = vec!["skip.txt".into()];
        assert_eq!(run(&a), 0);
    }

    #[test]
    fn emit_formats_plain_status() {
        // 不校验 stdout，只保证不 panic，且颜色路径逻辑可用
        emit('L', "a.txt", false);
        emit('R', "a.txt", false);
        emit('C', "a.txt", false);
        emit('S', "a.txt", false);
    }

    // ---- compare_dirs 结构化 API ----

    #[test]
    fn compare_dirs_reports_statuses_and_stats() {
        let d1 = tempdir().unwrap();
        let d2 = tempdir().unwrap();
        make_tree(
            d1.path(),
            &[("same.txt", "x"), ("diff.txt", "v1"), ("only_l.txt", "a")],
        );
        make_tree(
            d2.path(),
            &[("same.txt", "x"), ("diff.txt", "v22"), ("only_r.txt", "b")],
        );
        let r = compare_dirs(d1.path(), d2.path(), &empty_filter(), false, true).unwrap();
        let by_rel: std::collections::BTreeMap<&str, FileStatus> = r
            .entries
            .iter()
            .map(|e| (e.rel.as_str(), e.status))
            .collect();
        assert_eq!(by_rel["same.txt"], FileStatus::Same);
        assert_eq!(by_rel["diff.txt"], FileStatus::Differ);
        assert_eq!(by_rel["only_l.txt"], FileStatus::LeftOnly);
        assert_eq!(by_rel["only_r.txt"], FileStatus::RightOnly);
        assert_eq!(r.stats.same, 1);
        assert_eq!(r.stats.differ, 1);
        assert_eq!(r.stats.left_only, 1);
        assert_eq!(r.stats.right_only, 1);
        assert!(r.stats.has_differences());
        // 顺序稳定（BTreeMap 排序）
        let rels: Vec<&str> = r.entries.iter().map(|e| e.rel.as_str()).collect();
        assert_eq!(
            rels,
            vec!["diff.txt", "only_l.txt", "only_r.txt", "same.txt"]
        );
    }

    #[test]
    fn compare_dirs_identical_dirs_no_differences() {
        let d = tempdir().unwrap();
        make_tree(d.path(), &[("a.txt", "x"), ("sub/b.txt", "y")]);
        let r = compare_dirs(d.path(), d.path(), &empty_filter(), false, true).unwrap();
        assert!(!r.stats.has_differences());
        assert_eq!(r.stats.same, 2);
        assert!(r.warnings.is_empty());
    }

    #[test]
    fn compare_dirs_entries_carry_metadata() {
        let d1 = tempdir().unwrap();
        let d2 = tempdir().unwrap();
        make_tree(d1.path(), &[("f.txt", "12345")]);
        make_tree(d2.path(), &[("f.txt", "12345")]);
        let r = compare_dirs(d1.path(), d2.path(), &empty_filter(), false, true).unwrap();
        let e = &r.entries[0];
        assert_eq!(e.left.as_ref().unwrap().size, 5);
        assert_eq!(e.right.as_ref().unwrap().size, 5);
    }

    // ---- 移动/重命名检测 ----

    #[test]
    fn detect_rename_same_content() {
        let d1 = tempdir().unwrap();
        let d2 = tempdir().unwrap();
        make_tree(d1.path(), &[("old.txt", "same-content")]);
        make_tree(d2.path(), &[("new.txt", "same-content")]);
        let r = compare_dirs(d1.path(), d2.path(), &empty_filter(), false, true).unwrap();
        let by_rel: std::collections::BTreeMap<&str, &FileEntry> =
            r.entries.iter().map(|e| (e.rel.as_str(), e)).collect();
        assert_eq!(by_rel["old.txt"].status, FileStatus::Moved);
        assert_eq!(by_rel["old.txt"].moved_to.as_deref(), Some("new.txt"));
        assert_eq!(r.stats.moved, 1);
        assert_eq!(r.stats.left_only, 0);
        assert_eq!(r.stats.right_only, 0);
        assert!(r.stats.has_differences());
    }

    #[test]
    fn detect_move_across_subdirs() {
        let d1 = tempdir().unwrap();
        let d2 = tempdir().unwrap();
        make_tree(d1.path(), &[("src/a.rs", "fn main() {}")]);
        make_tree(d2.path(), &[("lib/b.rs", "fn main() {}")]);
        let r = compare_dirs(d1.path(), d2.path(), &empty_filter(), false, true).unwrap();
        let e = &r.entries[0];
        assert_eq!(e.status, FileStatus::Moved);
        assert_eq!(e.moved_to.as_deref(), Some("lib/b.rs"));
    }

    #[test]
    fn no_move_when_content_differs_same_size() {
        let d1 = tempdir().unwrap();
        let d2 = tempdir().unwrap();
        make_tree(d1.path(), &[("a.txt", "aaaa")]);
        make_tree(d2.path(), &[("b.txt", "bbbb")]);
        let r = compare_dirs(d1.path(), d2.path(), &empty_filter(), false, true).unwrap();
        let by_rel: std::collections::BTreeMap<&str, FileStatus> = r
            .entries
            .iter()
            .map(|e| (e.rel.as_str(), e.status))
            .collect();
        assert_eq!(by_rel["a.txt"], FileStatus::LeftOnly);
        assert_eq!(by_rel["b.txt"], FileStatus::RightOnly);
        assert_eq!(r.stats.moved, 0);
    }

    #[test]
    fn no_move_when_disabled() {
        let d1 = tempdir().unwrap();
        let d2 = tempdir().unwrap();
        make_tree(d1.path(), &[("old.txt", "same-content")]);
        make_tree(d2.path(), &[("new.txt", "same-content")]);
        let r = compare_dirs(d1.path(), d2.path(), &empty_filter(), false, false).unwrap();
        let by_rel: std::collections::BTreeMap<&str, FileStatus> = r
            .entries
            .iter()
            .map(|e| (e.rel.as_str(), e.status))
            .collect();
        assert_eq!(by_rel["old.txt"], FileStatus::LeftOnly);
        assert_eq!(by_rel["new.txt"], FileStatus::RightOnly);
        assert_eq!(r.stats.moved, 0);
    }

    #[test]
    fn move_matching_is_pairwise_unique() {
        // 两个左侧独有、两个右侧独有，内容互不相同 → 不误配
        let d1 = tempdir().unwrap();
        let d2 = tempdir().unwrap();
        make_tree(
            d1.path(),
            &[("l1.txt", "content-one"), ("l2.txt", "content-two")],
        );
        make_tree(
            d2.path(),
            &[("r1.txt", "content-one"), ("r2.txt", "content-two")],
        );
        let r = compare_dirs(d1.path(), d2.path(), &empty_filter(), false, true).unwrap();
        assert_eq!(r.stats.moved, 2);
        assert_eq!(r.stats.left_only, 0);
        assert_eq!(r.stats.right_only, 0);
        // 每个目标只匹配一次
        let targets: Vec<&str> = r
            .entries
            .iter()
            .filter(|e| e.status == FileStatus::Moved)
            .filter_map(|e| e.moved_to.as_deref())
            .collect();
        let mut sorted = targets.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(targets.len(), sorted.len());
    }

    #[test]
    fn move_quick_mode_ignores_mtime() {
        // 快速模式（compare_content=false）也按内容判定移动，mtime 不同不影响
        let d1 = tempdir().unwrap();
        let d2 = tempdir().unwrap();
        make_tree(d1.path(), &[("old.txt", "same-content")]);
        make_tree(d2.path(), &[("new.txt", "same-content")]);
        let old = filetime::FileTime::from_unix_time(1_600_000_000, 0);
        filetime::set_file_mtime(d1.path().join("old.txt"), old).unwrap();
        let new = filetime::FileTime::from_unix_time(1_700_000_000, 0);
        filetime::set_file_mtime(d2.path().join("new.txt"), new).unwrap();
        let r = compare_dirs(d1.path(), d2.path(), &empty_filter(), false, true).unwrap();
        assert_eq!(r.stats.moved, 1);
        let e = &r.entries[0];
        assert_eq!(e.status, FileStatus::Moved);
    }
}

#[cfg(test)]
mod attrs_tests {
    #[cfg(unix)]
    use super::*;
    #[cfg(unix)]
    use std::fs;
    #[cfg(unix)]
    use tempfile::tempdir;

    #[cfg(unix)]
    fn empty_filter() -> Filter {
        Filter::new(&[], &[]).unwrap()
    }

    #[test]
    fn compare_attrs_detects_mode_diff() {
        // Unix 下验证权限差异；非 Unix 平台跳过
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let d1 = tempdir().unwrap();
            let d2 = tempdir().unwrap();
            let fixed = filetime::FileTime::from_unix_time(1_700_000_000, 0);
            for (dir, mode) in [(d1.path(), 0o644), (d2.path(), 0o600)] {
                let p = dir.join("f.txt");
                fs::write(&p, "same-content").unwrap();
                fs::set_permissions(&p, fs::Permissions::from_mode(mode)).unwrap();
                filetime::set_file_mtime(&p, fixed).unwrap();
            }
            // 内容相同但权限不同：不开 --compare-attrs 判 Same
            let r1 = compare_dirs(d1.path(), d2.path(), &empty_filter(), true, true).unwrap();
            let e1 = r1.entries.iter().find(|e| e.rel == "f.txt").unwrap();
            assert_eq!(e1.status, FileStatus::Same);
            // 开 --compare-attrs 判 Differ + attrs_differ
            let r2 = compare_dirs_attrs(d1.path(), d2.path(), &empty_filter(), true, true, true)
                .unwrap();
            let e2 = r2.entries.iter().find(|e| e.rel == "f.txt").unwrap();
            assert_eq!(e2.status, FileStatus::Differ);
            assert!(e2.attrs_differ);
        }
    }

    #[test]
    fn compare_attrs_detects_symlink() {
        #[cfg(unix)]
        {
            let d1 = tempdir().unwrap();
            let d2 = tempdir().unwrap();
            let fixed = filetime::FileTime::from_unix_time(1_700_000_000, 0);
            // 左侧: 普通文件; 右侧: 符号链接 f.txt -> real.txt（real.txt 内容与左侧相同）
            let lp = d1.path().join("f.txt");
            fs::write(&lp, "data").unwrap();
            filetime::set_file_mtime(&lp, fixed).unwrap();
            fs::write(d2.path().join("real.txt"), "data").unwrap();
            filetime::set_file_mtime(d2.path().join("real.txt"), fixed).unwrap();
            let rp = d2.path().join("f.txt");
            std::os::unix::fs::symlink("real.txt", &rp).unwrap();
            filetime::set_file_mtime(&rp, fixed).unwrap();
            // 内容一致(符号链接读目标)但链接属性不同 → attrs_differ
            let r = compare_dirs_attrs(d1.path(), d2.path(), &empty_filter(), true, true, true)
                .unwrap();
            let e = r.entries.iter().find(|e| e.rel == "f.txt").unwrap();
            assert_eq!(e.status, FileStatus::Differ);
            assert!(e.attrs_differ);
        }
    }
}
