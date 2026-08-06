mod diff;
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
}

fn main() {
    let cli = Cli::parse();
    let code = match cli.command {
        Commands::Diff(args) => diff::run(&args),
    };
    std::process::exit(code);
}
