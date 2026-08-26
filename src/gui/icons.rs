//! 矢量图标（Phosphor Icons，vendored 字体，零版本耦合）。
//!
//! egui 内置控件没有现成的彩色图标字体；此前用文字 emoji（📁✕▶▾...）当图标，
//! 观感简陋且基线错乱。这里把 Phosphor Regular 字体内嵌进二进制，并注册为默认
//! `Proportional` 家族的兜底字体；图标字体只含 PUA 码点，普通文本不会命中。
//!
//! 字体与字形常量取自 egui-phosphor 生态（MIT，作者 Romet Tagobert），
//! 字形码点位于 Unicode Private Use Area。
//! 若后续精简二进制体积，可用 fonttools 子集化到仅保留用到的码点。

use eframe::egui::{self, FontFamily, FontId};

/// 图标字体文件名（相对 assets/icons/）
static PHOSPHOR_TTF: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/icons/Phosphor-Regular.ttf"
));

/// 图标字体注册名
pub const FAMILY: &str = "phosphor";

/// 把图标字体注册进 FontDefinitions（与 CJK/等宽合并后一次 set_fonts）。
/// 仅在启动时调用一次。
///
/// 用默认 `Proportional` 家族兜底（而非独立命名家族）：即使某个测试 Harness
/// 未调用字体安装，也不会因 `FontFamily::Name` 未绑定而 panic——未注册时
/// 图标仅退化为无字形块，绝不崩溃。
pub fn add_to_fonts(fonts: &mut egui::FontDefinitions) {
    fonts.font_data.insert(
        FAMILY.to_string(),
        egui::FontData::from_static(PHOSPHOR_TTF).into(),
    );
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .push(FAMILY.to_string());
}

/// 图标字体的 FontId（调用方用 `RichText::new(glyph).font(icons::font(size))`）。
/// 使用默认比例家族，避免依赖"phosphor"命名家族已注册。
pub fn font(size: f32) -> FontId {
    FontId::proportional(size)
}

/// 语义化图标（方便在 widget/工具栏里一一对应，避免散落码点）。
/// 是一个增长的图标目录，未用到的变体允许保留以备后续 tab/工具栏接入。
#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Icon {
    // 打开/对比
    OpenLeft,
    OpenRight,
    Compare,
    // 导航
    Prev,
    Next,
    Search,
    // 工具
    Refresh,
    Save,
    Swap,
    // 路径控件
    Folder,
    History,
    Clear,
    Close,
    // 设置/杂项
    Home,
    Settings,
    Plug,
    Plus,
    // 视图
    Eye,
    EyeSlash,
    // 语义
    Equal,
    Differ,
    Copy,
    Align,
    // 标签类型
    Diff,
    Text,
    Dir,
    Merge,
    Image,
    Csv,
    Hex,
    Patch,
    Media,
}

impl Icon {
    pub fn glyph(self) -> char {
        use Icon::*;
        match self {
            OpenLeft => '\u{E058}',  // arrow-left
            OpenRight => '\u{E06C}', // arrow-right
            Compare => '\u{E862}',   // brackets-angle
            Prev => '\u{E13C}',      // caret-up
            Next => '\u{E136}',      // caret-down
            Search => '\u{E30C}',    // magnifying-glass
            Refresh => '\u{E094}',   // arrows-clockwise
            Save => '\u{E248}',      // floppy-disk
            Swap => '\u{E83C}',      // swap
            Folder => '\u{E24A}',    // folder
            History => '\u{E19A}',   // clock
            Clear => '\u{E4F6}',     // x
            Close => '\u{E4F6}',     // x
            Home => '\u{E2C2}',      // house
            Settings => '\u{E272}',  // gear-six
            Plug => '\u{E946}',      // plug
            Plus => '\u{E3D4}',      // plus
            Eye => '\u{E220}',       // eye
            EyeSlash => '\u{E224}',  // eye-slash
            Equal => '\u{E182}',     // check
            Differ => '\u{E2F4}',    // list-dashes
            Copy => '\u{E1CA}',      // copy
            Align => '\u{E506}',     // align-bottom
            Diff => '\u{E23A}',      // file-text
            Text => '\u{E48A}',      // text-t
            Dir => '\u{E24A}',       // folder
            Merge => '\u{E278}',     // git-branch
            Image => '\u{E2CA}',     // image
            Csv => '\u{E476}',       // table
            Hex => '\u{E2A2}',       // hash
            Patch => '\u{EAE8}',     // terminal-window
            Media => '\u{E340}',     // music-notes
        }
    }
}
