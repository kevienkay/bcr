mod compare;
mod compare3;
mod csvcmp;
mod diff;
mod encoding;
mod fsscan;
mod gui;
mod hex;
mod hexview;
mod highlight;
mod htmlreport;
mod i18n;
mod imgcmp;
mod merge;
mod mergeview;
mod profile;
mod render;
mod session;
mod sideview;
mod sync;
mod vfs;

use clap::{Parser, Subcommand};

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

    /// 对比两个目录树（快速元数据比较或深度内容比较）
    Compare(compare::CompareArgs),

    /// 三路文件夹对比（BASE + LEFT + RIGHT）
    Compare3(compare3::Compare3Args),

    /// CSV/表格对比（按主键对齐，逐列 diff）
    Csv(csvcmp::CsvArgs),

    /// 三路合并（Base + Left + Right），冲突输出标记块
    Merge(merge::MergeArgs),

    /// 目录同步（update/mirror/two-way，支持 dry-run 预览）
    Sync(sync::SyncArgs),

    /// 会话管理（save/list/run/delete，持久化比较配置）
    Session(session::SessionArgs),

    /// 比较规则 Profile（save/list/delete，可复用规则集）
    Profile(profile::ProfileArgs),

    /// GUI 并排 Diff 视图（egui）
    Gui(gui::GuiArgs),
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

fn main() {
    init_platform();
    let cli = Cli::parse();
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
        Commands::Compare(args) => compare::run(&args),
        Commands::Compare3(args) => compare3::run(&args),
        Commands::Csv(args) => csvcmp::run(&args),
        Commands::Merge(args) => merge::run(&args),
        Commands::Sync(args) => sync::run(&args),
        Commands::Session(args) => session::run(&args),
        Commands::Profile(args) => profile::run(&args),
        Commands::Gui(args) => gui::run(&args),
    };
    std::process::exit(code);
}
