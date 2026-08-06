mod compare;
mod diff;
mod merge;
mod render;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "bcr",
    version,
    about = "Beyond Compare 风格的文件对比工具（Rust 实现）",
    arg_required_else_help = true
)]
struct Cli {
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
}

fn main() {
    let cli = Cli::parse();
    let code = match cli.command {
        Commands::Diff(args) => diff::run(&args),
        Commands::Compare(args) => compare::run(&args),
        Commands::Merge(args) => merge::run(&args),
    };
    std::process::exit(code);
}
