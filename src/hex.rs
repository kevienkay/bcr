//! `bcr hex` 子命令：两个文件的十六进制对比（二进制友好）。
//!
//! 读取原始字节（不经过文本解码），按 16 字节对齐输出并排对比。
//! 退出码：0=无差异，1=有差异，2=错误（与 diff 一致）。

use crate::hexview::{build_hex_rows, render_hex};
use clap::Args;
use std::io::{self, IsTerminal};

/// hex 子命令参数
#[derive(Args, Debug)]
pub struct HexArgs {
    /// 左侧文件
    pub left: String,

    /// 右侧文件
    pub right: String,

    /// 颜色输出：auto | always | never
    #[arg(long, default_value = "auto", value_parser = ["auto", "always", "never"])]
    pub color: String,

    /// 显示所有行（默认只显示差异行）
    #[arg(long)]
    pub show_same: bool,
}

/// 运行 hex 子命令，返回进程退出码（0=无差异，1=有差异，2=错误）
pub fn run(args: &HexArgs) -> i32 {
    let left = match std::fs::read(&args.left) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("bcr: {}", crate::i18n::fmt(crate::i18n::Key::CannotRead, &[&args.left, &e.to_string()]));
            return 2;
        }
    };
    let right = match std::fs::read(&args.right) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("bcr: {}", crate::i18n::fmt(crate::i18n::Key::CannotRead, &[&args.right, &e.to_string()]));
            return 2;
        }
    };

    let color = match args.color.as_str() {
        "always" => true,
        "never" => false,
        _ => io::stdout().is_terminal(),
    };
    let rows = build_hex_rows(&left, &right);
    render_hex(&rows, color, args.show_same);
    if rows.iter().any(|r| r.diff) {
        1
    } else {
        0
    }
}
