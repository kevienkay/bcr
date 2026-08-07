mod compare;
mod diff;
mod fsscan;
mod gui;
mod i18n;
mod merge;
mod mergeview;
mod render;
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

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 对比两个文本文件（unified 格式输出，支持行内高亮）
    Diff(diff::DiffArgs),

    /// 对比两个目录树（快速元数据比较或深度内容比较）
    Compare(compare::CompareArgs),

    /// 三路合并（Base + Left + Right），冲突输出标记块
    Merge(merge::MergeArgs),

    /// 目录同步（update/mirror/two-way，支持 dry-run 预览）
    Sync(sync::SyncArgs),

    /// GUI 并排 Diff 视图（egui）
    Gui(gui::GuiArgs),
}

fn main() {
    let cli = Cli::parse();
    // 初始化语言：--lang 优先，其次 BCR_LANG/系统 LANG，最后中文
    let lang = cli
        .lang
        .as_deref()
        .and_then(i18n::Lang::parse)
        .or_else(i18n::Lang::from_env)
        .unwrap_or(i18n::Lang::Zh);
    i18n::set_lang(lang);
    let code = match cli.command {
        Commands::Diff(args) => diff::run(&args),
        Commands::Compare(args) => compare::run(&args),
        Commands::Merge(args) => merge::run(&args),
        Commands::Sync(args) => sync::run(&args),
        Commands::Gui(args) => gui::run(&args),
    };
    std::process::exit(code);
}
