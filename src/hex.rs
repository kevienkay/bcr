//! `bcr hex` 子命令：两个文件的十六进制对比（二进制友好）。
//!
//! 流式分块读取（每块 64KB），逐块构建对比行并渲染，内存 O(64KB)，
//! 支持超大二进制文件。退出码：0=无差异，1=有差异，2=错误。

use crate::hexview::{build_hex_rows, render_hex};
use clap::Args;
use std::fs::File;
use std::io::{self, BufReader, IsTerminal, Read};

/// 每块读取字节数（16 字节对齐）
const CHUNK: usize = 65536;

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
    let mut lf = match File::open(&args.left) {
        Ok(f) => BufReader::new(f),
        Err(e) => {
            eprintln!(
                "bcr: {}",
                crate::i18n::fmt(crate::i18n::Key::CannotRead, &[&args.left, &e.to_string()])
            );
            return 2;
        }
    };
    let mut rf = match File::open(&args.right) {
        Ok(f) => BufReader::new(f),
        Err(e) => {
            eprintln!(
                "bcr: {}",
                crate::i18n::fmt(crate::i18n::Key::CannotRead, &[&args.right, &e.to_string()])
            );
            return 2;
        }
    };

    let color = match args.color.as_str() {
        "always" => true,
        "never" => false,
        _ => io::stdout().is_terminal(),
    };

    let mut lbuf = vec![0u8; CHUNK];
    let mut rbuf = vec![0u8; CHUNK];
    let mut base = 0usize;
    let mut any_diff = false;
    loop {
        let nl = fill(&mut lf, &mut lbuf);
        let nr = fill(&mut rf, &mut rbuf);
        if nl == 0 && nr == 0 {
            break;
        }
        let mut rows = build_hex_rows(&lbuf[..nl], &rbuf[..nr]);
        for r in &mut rows {
            r.offset += base;
        }
        any_diff |= rows.iter().any(|r| r.diff);
        render_hex(&rows, color, args.show_same);
        base += CHUNK;
        if nl < CHUNK && nr < CHUNK {
            break; // 两侧都已读到 EOF
        }
    }

    if any_diff {
        1
    } else {
        0
    }
}

/// 尽力填满 buf（或读到 EOF），返回实际读到的字节数
fn fill(r: &mut impl Read, buf: &mut [u8]) -> usize {
    let mut got = 0usize;
    while got < buf.len() {
        match r.read(&mut buf[got..]) {
            Ok(0) => break,
            Ok(n) => got += n,
            Err(_) => break,
        }
    }
    got
}
