mod cache;
mod compare;
mod compare3;
mod csvcmp;
mod diff;
mod encoding;
mod external;
mod fsscan;
mod gui;
mod hex;
mod hexview;
mod highlight;
mod htmlreport;
mod i18n;
mod imgcmp;
mod jsonout;
mod mediacmp;
mod merge;
mod merge3;
mod mergeview;
mod mp3tag;
mod patchview;
mod profile;
mod render;
mod report;
mod session;
mod sideview;
mod sync;
mod systemtime_secs;
mod task;
mod version;
mod vfs;

use clap::{Parser, Subcommand};
use std::io::IsTerminal;

#[derive(Parser)]
#[command(
    name = "bcr",
    version,
    about = "Beyond Compare 风格的文件对比工具（Rust 实现）",
    arg_required_else_help = true
)]
struct Cli {
    /// 语言：zh/en/de/ja/ko/es/pt/ar/ru/fr（默认取 BCR_LANG 或系统 LANG）
    #[arg(long, global = true)]
    lang: Option<String>,

    /// 编码：utf-8/utf-16le/utf-16be/gbk/big5/shift_jis 等（默认自动检测；可用 BCR_ENCODING）
    #[arg(long, global = true)]
    encoding: Option<String>,

    /// 文本文件大小上限（MB，默认 64；超过按文本加载报错，可用 BCR_MAX_SIZE）
    #[arg(long, global = true)]
    max_size: Option<u64>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 对比两个文本文件（unified 格式输出，支持行内高亮）
    Diff(diff::DiffArgs),

    /// 十六进制对比两个文件（二进制友好，按字节输出差异）
    Hex(hex::HexArgs),

    /// 图片对比（逐像素差异 + 统计，PNG/JPEG/GIF/WebP/BMP）
    Imgcmp(imgcmp::ImgcmpArgs),

    /// MP3 标签对比（ID3v1/v2，字段级差异）
    Mp3tag(mp3tag::Mp3tagArgs),

    /// 媒体元数据对比（WAV/MP3/FLAC 容器头解析，字段级差异）
    Media(mediacmp::MediaArgs),

    /// 对比两个目录树（快速元数据比较或深度内容比较）
    Compare(compare::CompareArgs),

    /// 三路文件夹对比（BASE + LEFT + RIGHT）
    Compare3(compare3::Compare3Args),

    /// CSV/表格对比（按主键对齐，逐列 diff）
    Csv(csvcmp::CsvArgs),

    /// 三路合并（Base + Left + Right），冲突输出标记块
    Merge(merge::MergeArgs),

    /// 三路文件夹合并（BASE + LEFT + RIGHT → 输出目录，文本自动合并）
    Merge3(merge3::Merge3Args),

    /// 目录同步（update/mirror/two-way，支持 dry-run 预览）
    Sync(sync::SyncArgs),

    /// 会话管理（save/list/run/delete，持久化比较配置）
    Session(session::SessionArgs),

    /// 比较规则 Profile（save/list/delete，可复用规则集）
    Profile(profile::ProfileArgs),

    /// GUI 并排 Diff 视图（egui）
    Gui(gui::GuiArgs),

    /// 执行/校验任务清单（纯数据 JSON/TOML 步骤列表）
    Task(task::TaskArgs),
}

/// 平台初始化：Windows 控制台切换到 UTF-8 代码页，保证中文输出不乱码。
#[cfg(windows)]
fn init_platform() {
    // SetConsoleOutputCP(65001) / SetConsoleCP(65001)
    #[link(name = "Kernel32")]
    extern "system" {
        fn SetConsoleOutputCP(wCodePageID: u32) -> i32;
        fn SetConsoleCP(wCodePageID: u32) -> i32;
    }
    unsafe {
        SetConsoleOutputCP(65001);
        SetConsoleCP(65001);
    }
}

#[cfg(not(windows))]
fn init_platform() {}

/// Windows：判断是否由「双击」启动（而非从终端运行）。
/// 双击时系统新建控制台，进程列表只有自己（≤1）；从 cmd/powershell 启动时列表含 shell（≥2）。
/// 双击场景同时隐藏系统新建的控制台黑框（避免一闪而过的窗口残留）。
#[cfg(windows)]
fn console_procs() -> u32 {
    #[link(name = "Kernel32")]
    extern "system" {
        fn GetConsoleProcessList(lpdwProcessList: *mut u32, dwProcessCount: u32) -> u32;
        fn FreeConsole() -> i32;
    }
    let mut list = [0u32; 4];
    let count = unsafe { GetConsoleProcessList(list.as_mut_ptr(), list.len() as u32) };
    if count <= 1 {
        unsafe {
            FreeConsole();
        }
    }
    count
}

#[cfg(not(windows))]
fn console_procs() -> u32 {
    2
}

/// 判断无参数时应否启动 GUI（纯逻辑，便于测试）：
/// stdin 非终端（管道/重定向）或 Windows 双击（控制台进程列表 ≤1）→ GUI。
fn should_launch_gui(stdin_is_terminal: bool, console_procs: u32) -> bool {
    !stdin_is_terminal || console_procs <= 1
}

fn main() {
    init_platform();
    // Windows 双击 exe（无参数 + 独立控制台）或 stdin 非终端（管道/重定向）→ 自动启动 GUI；
    // 终端里无参数仍打印帮助退出（保持 CLI 行为）。
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e)
            if matches!(
                e.kind(),
                clap::error::ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
                    | clap::error::ErrorKind::MissingSubcommand
            ) =>
        {
            // macOS：无参数一律启动 GUI（对标 BC：bcomp 无参数打开 GUI；
            // Finder 双击裸二进制会用 Terminal 打开，stdin 是终端，无法与手动运行区分）
            let launch_gui = cfg!(target_os = "macos")
                || should_launch_gui(std::io::stdin().is_terminal(), console_procs());
            if launch_gui {
                Cli {
                    lang: None,
                    encoding: None,
                    max_size: None,
                    command: Commands::Gui(gui::GuiArgs::default()),
                }
            } else {
                e.exit();
            }
        }
        Err(e) => e.exit(),
    };
    // 初始化语言：--lang 优先，其次 BCR_LANG/系统 LANG，最后中文
    let lang = cli
        .lang
        .as_deref()
        .and_then(i18n::Lang::parse)
        .or_else(i18n::Lang::from_env)
        .unwrap_or(i18n::Lang::Zh);
    i18n::set_lang(lang);
    // 编码覆盖：--encoding 优先写入 BCR_ENCODING，供 encoding 模块统一读取
    if let Some(enc) = &cli.encoding {
        unsafe { std::env::set_var("BCR_ENCODING", enc) };
    }
    // 大小上限：--max-size 优先写入 BCR_MAX_SIZE（MB）
    if let Some(mb) = cli.max_size {
        unsafe { std::env::set_var("BCR_MAX_SIZE", mb.to_string()) };
    }
    let code = match cli.command {
        Commands::Diff(args) => diff::run(&args),
        Commands::Hex(args) => hex::run(&args),
        Commands::Imgcmp(args) => imgcmp::run(&args),
        Commands::Mp3tag(args) => mp3tag::run(&args),
        Commands::Media(args) => mediacmp::run(&args),
        Commands::Compare(args) => compare::run(&args),
        Commands::Compare3(args) => compare3::run(&args),
        Commands::Csv(args) => csvcmp::run(&args),
        Commands::Merge(args) => merge::run(&args),
        Commands::Merge3(args) => merge3::run(&args),
        Commands::Sync(args) => sync::run(&args),
        Commands::Session(args) => session::run(&args),
        Commands::Profile(args) => profile::run(&args),
        Commands::Gui(args) => gui::run(&args),
        Commands::Task(args) => task::run(&args),
    };
    std::process::exit(code);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gui_launch_logic_covers_all_cases() {
        // 终端 + 控制台进程 ≥2（cmd/powershell 里运行）→ 打印帮助
        assert!(!should_launch_gui(true, 2));
        assert!(!should_launch_gui(true, 5));
        // 终端 + 控制台进程 ≤1（Windows 双击，新建控制台只有自己）→ GUI
        assert!(should_launch_gui(true, 1));
        assert!(should_launch_gui(true, 0));
        // stdin 非终端（管道/重定向/CI 模拟）→ GUI
        assert!(should_launch_gui(false, 2));
        assert!(should_launch_gui(false, 1));
        assert!(should_launch_gui(false, 0));
    }

    #[test]
    fn console_procs_platform_safe() {
        // 非 Windows 恒为 2；Windows 上返回真实值（不 panic）
        let _ = console_procs();
    }
}
