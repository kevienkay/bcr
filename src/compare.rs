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

    /// 忽略文件夹结构（A7）：跨目录层级按文件名对齐比较（BC 的 Ignore folder structure）
    #[arg(long)]
    pub ignore_structure: bool,

    /// 符号链接跟随（B4）：扫描时跟随链接读取目标元数据（默认记录链接自身）
    #[arg(long)]
    pub follow_symlinks: bool,

    /// 比较文件属性（Unix 权限位/符号链接目标；默认仅比较大小+时间+内容）
    #[arg(long)]
    pub compare_attrs: bool,

    /// 比较文件版本号（从 FileVersion/ProductVersion 字段提取；含版本号的文件对按版本比较，否则回退快速模式）
    #[arg(long)]
    pub compare_version: bool,

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

    /// 报告字段（逗号分隔：status,path,size,mtime,moved；默认全部，作用于 --txt/--csv）
    #[arg(long = "report-fields", default_value = "")]
    pub report_fields: String,

    /// 报告自定义标题（默认 "bcr compare: L ↔ R"）
    #[arg(long = "report-title")]
    pub report_title: Option<String>,

    /// 报告不含统计行（默认包含，作用于 --txt/--csv）
    #[arg(long = "report-no-stats")]
    pub report_no_stats: bool,

    /// 报告排序：path（默认，按路径）| status（按状态字母序）| size（按差异大小降序）
    #[arg(long = "report-sort", default_value = "path", value_parser = ["path", "status", "size"])]
    pub report_sort: String,

    /// 按状态分组输出（仅文本报告）
    #[arg(long = "report-group")]
    pub report_group: bool,

    /// 以 JSON 契约输出结果（schema: compare.v1，供脚本/CI 消费）
    #[arg(long)]
    pub json: bool,

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
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
        false,
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
    compare_vfs_attrs(
        left,
        right,
        filter,
        compare_content,
        enable_moves,
        false,
        false,
    )
}

/// 带属性比较的虚拟后端对比
#[allow(clippy::too_many_arguments)]
pub fn compare_vfs_attrs(
    left: &dyn Vfs,
    right: &dyn Vfs,
    filter: &Filter,
    compare_content: bool,
    enable_moves: bool,
    compare_attrs: bool,
    compare_version: bool,
) -> io::Result<CompareResult> {
    compare_vfs_attrs_is(
        left,
        right,
        filter,
        compare_content,
        enable_moves,
        compare_attrs,
        compare_version,
        false,
    )
}

/// 带属性比较 + 忽略文件夹结构（A7）
#[allow(clippy::too_many_arguments)]
pub fn compare_vfs_attrs_is(
    left: &dyn Vfs,
    right: &dyn Vfs,
    filter: &Filter,
    compare_content: bool,
    enable_moves: bool,
    compare_attrs: bool,
    compare_version: bool,
    ignore_structure: bool,
) -> io::Result<CompareResult> {
    let left_map = left.scan(filter)?;
    let right_map = right.scan(filter)?;

    // 快照缓存：仅本地目录启用（远程/压缩包每次全量扫描）
    let cache_key =
        if !crate::vfs::is_remote(&left.describe()) && !crate::vfs::is_remote(&right.describe()) {
            let opts = format!(
                "cc={} moves={} attrs={} cv={} is={}",
                compare_content, enable_moves, compare_attrs, compare_version, ignore_structure
            );
            Some(crate::cache::key_for(
                &left.describe(),
                &right.describe(),
                &filter.includes,
                &filter.excludes,
                &opts,
            ))
        } else {
            None
        };
    let left_snap = cache_key
        .as_ref()
        .map(|_| crate::cache::snapshot_of(&left_map));
    let right_snap = cache_key
        .as_ref()
        .map(|_| crate::cache::snapshot_of(&right_map));
    if let (Some(k), Some(ls), Some(rs)) = (&cache_key, &left_snap, &right_snap) {
        if let Some(cached) = crate::cache::lookup(k, ls, rs) {
            return Ok(cached);
        }
    }

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

    // A7 忽略文件夹结构：按文件名（basename）对齐配对
    let alignment: Vec<(Option<String>, Option<String>)> = if ignore_structure {
        align_by_basename(&left_map, &right_map)
    } else {
        keys.iter()
            .map(|k| {
                (
                    left_map.contains_key(*k).then(|| (*k).clone()),
                    right_map.contains_key(*k).then(|| (*k).clone()),
                )
            })
            .collect()
    };

    let mut result = CompareResult::default();
    for (l_rel, r_rel) in alignment {
        match (l_rel, r_rel) {
            (Some(l_rel), Some(r_rel)) => {
                let l = &left_map[&l_rel];
                let r = &right_map[&r_rel];
                let content_same = if ignore_structure {
                    // A7 忽略结构：两侧路径不同，内容/版本比较必须按各自 rel 读取
                    if l.size != r.size {
                        false
                    } else if compare_content || compare_version {
                        // 内容哈希（版本模式在忽略结构时退化到内容比较，路径不同无法复用 version_equal_vfs）
                        match (left.hash(&l_rel), right.hash(&r_rel)) {
                            (Ok(lh), Ok(rh)) => lh == rh,
                            _ => {
                                result.warnings.push(format!("读取 {l_rel}/{r_rel} 失败"));
                                continue;
                            }
                        }
                    } else {
                        l.mtime == r.mtime
                    }
                } else if compare_version {
                    // 版本模式优先：提取两侧版本号；任一侧无版本号 → 回退快速模式(size+mtime)
                    match version_equal_vfs(left, right, &l_rel) {
                        Ok(Some(eq)) => eq,
                        Ok(None) => l.size == r.size && l.mtime == r.mtime,
                        Err(e) => {
                            result.warnings.push(format!("读取 {l_rel} 失败: {e}"));
                            continue;
                        }
                    }
                } else if l.size != r.size {
                    false
                } else if compare_content {
                    match crate::vfs::content_equal_vfs(left, right, &l_rel) {
                        Ok(eq) => eq,
                        Err(e) => {
                            result.warnings.push(format!("读取 {l_rel} 失败: {e}"));
                            continue;
                        }
                    }
                } else {
                    l.mtime == r.mtime
                };
                // 属性比较：内容一致但权限/符号链接不同 → 计为 Differ（--compare-attrs）
                let attrs_differ = compare_attrs && attrs_diff(l, r);
                let same = content_same && !attrs_differ;
                let rel = if ignore_structure {
                    // 忽略结构时条目用左侧真实路径；同名但路径不同时右侧并入
                    if l_rel == r_rel {
                        l_rel.clone()
                    } else {
                        format!("{l_rel} ⇄ {r_rel}")
                    }
                } else {
                    l_rel.clone()
                };
                if same {
                    result.stats.same += 1;
                    result.entries.push(FileEntry {
                        rel,
                        status: FileStatus::Same,
                        left: Some(l.clone()),
                        right: Some(r.clone()),
                        moved_to: None,
                        attrs_differ: false,
                    });
                } else {
                    result.stats.differ += 1;
                    result.entries.push(FileEntry {
                        rel,
                        status: FileStatus::Differ,
                        left: Some(l.clone()),
                        right: Some(r.clone()),
                        moved_to: None,
                        attrs_differ,
                    });
                }
            }
            (Some(l_rel), None) => {
                let l = &left_map[&l_rel];
                result.stats.left_only += 1;
                result.entries.push(FileEntry {
                    rel: l_rel.clone(),
                    status: FileStatus::LeftOnly,
                    left: Some(l.clone()),
                    right: None,
                    moved_to: None,
                    attrs_differ: false,
                });
            }
            (None, Some(r_rel)) => {
                let r = &right_map[&r_rel];
                result.stats.right_only += 1;
                result.entries.push(FileEntry {
                    rel: r_rel.clone(),
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

    // 写缓存（本地目录）
    if let (Some(k), Some(ls), Some(rs)) = (&cache_key, left_snap, right_snap) {
        crate::cache::insert(k, ls, rs, result.clone());
    }
    Ok(result)
}

/// 忽略文件夹结构（A7）：两侧文件按 basename（文件名）对齐配对。
/// 同名文件跨目录层级配对比较；两侧各自独有（无同名）的文件单独列出一侧。
/// 返回 (左侧真实 rel 或 None, 右侧真实 rel 或 None) 列表，按左侧路径排序。
fn align_by_basename(
    left_map: &std::collections::BTreeMap<String, FileMeta>,
    right_map: &std::collections::BTreeMap<String, FileMeta>,
) -> Vec<(Option<String>, Option<String>)> {
    use std::collections::BTreeMap as Map;

    // basename -> rel 列表（一个 basename 可能在多级目录出现多次）
    let mut l_names: Map<String, Vec<&String>> = Map::new();
    for k in left_map.keys() {
        let name = k.rsplit('/').next().unwrap_or(k);
        l_names.entry(name.to_string()).or_default().push(k);
    }
    let mut r_names: Map<String, Vec<&String>> = Map::new();
    for k in right_map.keys() {
        let name = k.rsplit('/').next().unwrap_or(k);
        r_names.entry(name.to_string()).or_default().push(k);
    }

    let mut names: Vec<&String> = l_names.keys().chain(r_names.keys()).collect();
    names.sort();
    names.dedup();

    let mut out: Vec<(Option<String>, Option<String>)> = Vec::new();
    for name in names {
        let ls = l_names.get(name);
        let rs = r_names.get(name);
        match (ls, rs) {
            (Some(l), Some(r)) => {
                // 同名多实例：逐个配对（左侧第 i 个 ↔ 右侧第 i 个），多余归单侧
                let n = l.len().min(r.len());
                for i in 0..n {
                    out.push((Some((*l[i]).clone()), Some((*r[i]).clone())));
                }
                for k in &l[n..] {
                    out.push((Some((*k).clone()), None));
                }
                for k in &r[n..] {
                    out.push((None, Some((*k).clone())));
                }
            }
            (Some(l), None) => {
                for k in l {
                    out.push((Some((*k).clone()), None));
                }
            }
            (None, Some(r)) => {
                for k in r {
                    out.push((None, Some((*k).clone())));
                }
            }
            (None, None) => {}
        }
    }
    out
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
        Ok(f) => f.set_follow_symlinks(args.follow_symlinks),
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

    let result = if args.ignore_structure {
        compare_vfs_attrs_is(
            left.as_ref(),
            right.as_ref(),
            &filter,
            args.compare_content,
            args.detect_moves,
            args.compare_attrs,
            args.compare_version,
            true,
        )
    } else {
        compare_vfs_attrs(
            left.as_ref(),
            right.as_ref(),
            &filter,
            args.compare_content,
            args.detect_moves,
            args.compare_attrs,
            args.compare_version,
        )
    };
    let result = match result {
        Ok(r) => r,
        Err(e) => {
            eprintln!("bcr: {}", fmt(Key::ScanFailed, &[&e.to_string()]));
            return 2;
        }
    };

    for w in &result.warnings {
        eprintln!("bcr: {w}");
    }

    // JSON 契约输出：stdout 只输出 JSON，人类可读错误走 stderr
    if args.json {
        let v = crate::jsonout::compare_json(&args.left, &args.right, &result, args.show_same);
        println!(
            "{}",
            serde_json::to_string(&v).unwrap_or_else(|_| "{}".into())
        );
        return if result.stats.has_differences() { 1 } else { 0 };
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
        let fields = match crate::report::parse_fields(&args.report_fields) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("bcr: {}", e);
                return 2;
            }
        };
        let opts = crate::report::ReportOptions {
            title: args.report_title.clone(),
            include_stats: !args.report_no_stats,
            sort: args.report_sort.clone(),
            group_by_status: args.report_group,
        };
        let txt = crate::report::render_txt_opts(&args.left, &args.right, &result, &fields, &opts);
        if let Err(e) = std::fs::write(txt_path, txt) {
            eprintln!(
                "bcr: {}",
                fmt(Key::WriteFailed, &[txt_path, &e.to_string()])
            );
            return 2;
        }
    }
    if let Some(csv_path) = &args.csv {
        let fields = match crate::report::parse_fields(&args.report_fields) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("bcr: {}", e);
                return 2;
            }
        };
        let opts = crate::report::ReportOptions {
            title: args.report_title.clone(),
            include_stats: !args.report_no_stats,
            sort: args.report_sort.clone(),
            group_by_status: false,
        };
        let csv = crate::report::render_csv_opts(&args.left, &args.right, &result, &fields, &opts);
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

/// 版本比较：提取两侧文件版本号并比较。
/// Ok(Some(true)) = 两侧版本相同；Ok(Some(false)) = 不同；Ok(None) = 任一侧无版本号。
fn version_equal_vfs(left: &dyn Vfs, right: &dyn Vfs, rel: &str) -> io::Result<Option<bool>> {
    let lv = crate::version::extract_version(&left.read(rel)?);
    let rv = crate::version::extract_version(&right.read(rel)?);
    match (lv, rv) {
        (Some(a), Some(b)) => Ok(Some(
            crate::version::compare_versions(&a, &b) == std::cmp::Ordering::Equal,
        )),
        _ => Ok(None),
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
            compare_version: false,
            ignore_structure: false,
            follow_symlinks: false,
            json: false,
            summary: false,
            html: None,
            txt: None,
            csv: None,
            report_fields: String::new(),
            report_title: None,
            report_no_stats: false,
            report_sort: "path".to_string(),
            report_group: false,
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
    fn compare_version_same_version_same_file() {
        // 内容不同但版本号相同（mtime 也不同）→ --compare-version 视为相同
        let d1 = tempdir().unwrap();
        let d2 = tempdir().unwrap();
        let v = b"FileVersion\x00, 1.2.3.4\x00";
        let mut a = v.to_vec();
        let mut b = v.to_vec();
        a.extend_from_slice(b"AAA");
        b.extend_from_slice(b"BBBB"); // 长度不同 → 快速模式 Differ，但版本号相同
        let fixed = filetime::FileTime::from_unix_time(1_700_000_000, 0);
        let set = |d: &tempfile::TempDir, data: &[u8]| {
            let p = d.path().join("app.dll");
            std::fs::write(&p, data).unwrap();
            filetime::set_file_mtime(&p, fixed).unwrap();
        };
        set(&d1, &a);
        set(&d2, &b);
        // 快速模式：大小不同 → Differ
        let r = compare_dirs(d1.path(), d2.path(), &empty_filter(), false, true).unwrap();
        assert_eq!(r.stats.differ, 1);
        // 版本模式：版本号相同 → Same
        let r = compare_vfs_attrs(
            &LocalVfs::new(d1.path()).unwrap(),
            &LocalVfs::new(d2.path()).unwrap(),
            &empty_filter(),
            false,
            true,
            false,
            true,
        )
        .unwrap();
        assert_eq!(r.stats.same, 1);
        assert_eq!(r.stats.differ, 0);
    }

    #[test]
    fn compare_version_different_version_diffs() {
        let d1 = tempdir().unwrap();
        let d2 = tempdir().unwrap();
        let fixed = filetime::FileTime::from_unix_time(1_700_000_000, 0);
        let set = |d: &tempfile::TempDir, ver: &str| {
            let p = d.path().join("app.dll");
            std::fs::write(&p, format!("FileVersion\x00, {ver}\x00")).unwrap();
            filetime::set_file_mtime(&p, fixed).unwrap();
        };
        set(&d1, "1.2.3.4");
        set(&d2, "1.2.4.0");
        let r = compare_vfs_attrs(
            &LocalVfs::new(d1.path()).unwrap(),
            &LocalVfs::new(d2.path()).unwrap(),
            &empty_filter(),
            false,
            true,
            false,
            true,
        )
        .unwrap();
        assert_eq!(r.stats.differ, 1);
        assert_eq!(r.stats.same, 0);
    }

    #[test]
    fn compare_version_no_version_falls_back_mtime() {
        // 无版本号的文件对：回退 mtime 比较
        let d1 = tempdir().unwrap();
        let d2 = tempdir().unwrap();
        let fixed = filetime::FileTime::from_unix_time(1_700_000_000, 0);
        let p1 = d1.path().join("x.bin");
        let p2 = d2.path().join("x.bin");
        std::fs::write(&p1, b"data").unwrap();
        std::fs::write(&p2, b"data").unwrap();
        filetime::set_file_mtime(&p1, fixed).unwrap();
        filetime::set_file_mtime(&p2, fixed).unwrap();
        let r = compare_vfs_attrs(
            &LocalVfs::new(d1.path()).unwrap(),
            &LocalVfs::new(d2.path()).unwrap(),
            &empty_filter(),
            false,
            true,
            false,
            true,
        )
        .unwrap();
        assert_eq!(r.stats.same, 1);
    }

    #[test]
    fn json_output_contract_shape() {
        // compare --json 的契约形状：schema/ok/result.stats/entries
        let d1 = tempdir().unwrap();
        let d2 = tempdir().unwrap();
        make_tree(d1.path(), &[("a.txt", "aaa"), ("same.txt", "same")]);
        make_tree(d2.path(), &[("a.txt", "bbb"), ("same.txt", "same")]);
        let result = compare_dirs(d1.path(), d2.path(), &empty_filter(), true, true).unwrap();
        let v = crate::jsonout::compare_json(
            d1.path().to_str().unwrap(),
            d2.path().to_str().unwrap(),
            &result,
            false,
        );
        assert_eq!(v["schema"], "compare.v1");
        assert_eq!(v["ok"], true);
        assert_eq!(v["command"], "compare");
        assert_eq!(v["error"], serde_json::Value::Null);
        let entries = v["result"]["entries"].as_array().unwrap();
        // 默认不含 same 条目
        assert!(entries.iter().all(|e| e["status"] != "same"));
        // 每个条目含契约字段
        for e in entries {
            assert!(e["rel"].is_string());
            assert!(e["status"].is_string());
            assert!(e["moved_to"].is_null() || e["moved_to"].is_string());
        }
        // mtime 为 ISO-8601 字符串
        let a = entries.iter().find(|e| e["rel"] == "a.txt").unwrap();
        let m = a["left"]["mtime"].as_str().unwrap();
        assert!(m.ends_with('Z') && m.contains('T'));
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

    // ---- A7 忽略文件夹结构 ----

    #[test]
    fn align_by_basename_pairs_across_dirs() {
        let mut l: std::collections::BTreeMap<String, FileMeta> = Default::default();
        let mut r: std::collections::BTreeMap<String, FileMeta> = Default::default();
        l.insert(
            "a/x.txt".into(),
            FileMeta {
                size: 1,
                mtime: std::time::UNIX_EPOCH,
                mode: None,
                symlink: None,
            },
        );
        l.insert(
            "only.txt".into(),
            FileMeta {
                size: 1,
                mtime: std::time::UNIX_EPOCH,
                mode: None,
                symlink: None,
            },
        );
        r.insert(
            "sub/x.txt".into(),
            FileMeta {
                size: 1,
                mtime: std::time::UNIX_EPOCH,
                mode: None,
                symlink: None,
            },
        );
        r.insert(
            "sub/only2.txt".into(),
            FileMeta {
                size: 1,
                mtime: std::time::UNIX_EPOCH,
                mode: None,
                symlink: None,
            },
        );
        let pairs = align_by_basename(&l, &r);
        // x.txt 配对；only.txt 仅左侧；only2.txt 仅右侧
        assert!(pairs.contains(&(Some("a/x.txt".into()), Some("sub/x.txt".into()))));
        assert!(pairs.contains(&(Some("only.txt".into()), None)));
        assert!(pairs.contains(&(None, Some("sub/only2.txt".into()))));
    }

    #[test]
    fn ignore_structure_matches_same_basename() {
        let d1 = tempdir().unwrap();
        let d2 = tempdir().unwrap();
        fs::create_dir_all(d1.path().join("a")).unwrap();
        fs::create_dir_all(d2.path().join("b")).unwrap();
        fs::write(d1.path().join("a/readme.md"), "same").unwrap();
        fs::write(d2.path().join("b/readme.md"), "same").unwrap();
        fs::write(d1.path().join("a/extra.txt"), "L").unwrap();
        let f = Filter::new(&[], &[]).unwrap();
        let r = compare_vfs_attrs_is(
            &LocalVfs::new(d1.path()).unwrap(),
            &LocalVfs::new(d2.path()).unwrap(),
            &f,
            true,
            true,
            false,
            false,
            true,
        )
        .unwrap();
        // 忽略结构后 readme.md 应配对为 Same；extra.txt 仅左侧
        assert!(r
            .entries
            .iter()
            .any(|e| e.status == FileStatus::Same && e.rel.contains("readme")));
        assert!(r
            .entries
            .iter()
            .any(|e| e.status == FileStatus::LeftOnly && e.rel.contains("extra")));
        assert_eq!(r.stats.same, 1);
        assert_eq!(r.stats.left_only, 1);
    }
}
