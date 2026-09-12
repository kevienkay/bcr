//! P65（BC 5.2.5 设计稿 · 编辑菜单）：标准编辑动作的纯逻辑 + 剪贴板 IO。
//!
//! 设计稿 `design/BC菜单和状态栏/screens/menus.html` 画板②的「编辑」菜单含
//! **剪切 ⌘X / 复制 ⌘C / 粘贴 ⌘V / 删除 / 全选 ⌘A**，置灰规则见
//! `交接/design-tokens.json` 的 `menus.grayedRules`：
//! 只读比较会话（Diff/Dir/Csv/Image/Media/Patch）一律置灰，可编辑会话可用。
//!
//! 本模块只做「文本 + 选区 → 新文本 + 新光标」的纯计算与剪贴板读写，
//! 不依赖 egui 控件；由 `textedit.rs`（文本编辑会话）把结果接到
//! `egui::TextEdit` 的选区状态上。这样菜单动作可以脱离输入事件单测。

use crate::i18n::Key as I18nKey;

/// 标准编辑动作（BC 编辑菜单四＋一项）
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditOp {
    /// 剪切 ⌘X：复制选区到剪贴板并删除
    Cut,
    /// 复制 ⌘C
    Copy,
    /// 粘贴 ⌘V
    Paste,
    /// 删除（无快捷键，BC 用 Delete 键）
    Delete,
    /// 全选 ⌘A
    SelectAll,
}

impl EditOp {
    /// 全部动作（菜单渲染与测试遍历用）
    pub const ALL: [EditOp; 5] = [
        EditOp::Cut,
        EditOp::Copy,
        EditOp::Paste,
        EditOp::Delete,
        EditOp::SelectAll,
    ];

    /// 菜单文案 i18n key
    pub fn i18n(self) -> I18nKey {
        match self {
            EditOp::Cut => I18nKey::MenuCut,
            EditOp::Copy => I18nKey::MenuCopy,
            EditOp::Paste => I18nKey::MenuPaste,
            EditOp::Delete => I18nKey::MenuDelete,
            EditOp::SelectAll => I18nKey::MenuSelectAll,
        }
    }

    /// 平台快捷键文本（macOS ⌘ 系 / Windows·Linux Ctrl 系）
    ///
    /// 原生菜单（macOS/Windows）用 muda 的真实 Accelerator，不需要字符串形式；
    /// 仅 Linux 窗口内菜单栏使用，故那两个平台放行 dead_code。
    #[cfg_attr(any(target_os = "macos", target_os = "windows"), allow(dead_code))]
    pub fn shortcut(self) -> String {
        let (mac, win) = match self {
            EditOp::Cut => ("⌘X", "Ctrl+X"),
            EditOp::Copy => ("⌘C", "Ctrl+C"),
            EditOp::Paste => ("⌘V", "Ctrl+V"),
            // BC 的「删除」不显示快捷键（Delete 键随焦点控件生效）
            EditOp::Delete => ("", ""),
            EditOp::SelectAll => ("⌘A", "Ctrl+A"),
        };
        if cfg!(target_os = "macos") {
            mac.to_string()
        } else {
            win.to_string()
        }
    }

    /// 原生菜单项 id（muda 事件回传用；与 `MenuCmd` 解析表一一对应）
    ///
    /// Linux 无原生菜单（`native_menu` 的平台块被 cfg 掉），此方法仅测试调用，
    /// 故按本仓惯例在 Linux 目标上放行 dead_code（与 `menu_state_plan` 同处理）。
    #[cfg_attr(target_os = "linux", allow(dead_code))]
    pub fn cmd_id(self) -> &'static str {
        match self {
            EditOp::Cut => "edit_cut",
            EditOp::Copy => "edit_copy",
            EditOp::Paste => "edit_paste",
            EditOp::Delete => "edit_delete",
            EditOp::SelectAll => "edit_select_all",
        }
    }
}

/// char 索引数（非字节数）
pub fn char_len(s: &str) -> usize {
    s.chars().count()
}

/// 选区按 char 数钳位到 `[0, len]`，并保证 `start <= end`
pub fn clamp_range(s: &str, range: (usize, usize)) -> (usize, usize) {
    let n = char_len(s);
    let a = range.0.min(n);
    let b = range.1.min(n);
    (a.min(b), a.max(b))
}

/// 选区文本（越界钳位；空选区返回空串）
pub fn selection_text(s: &str, range: (usize, usize)) -> String {
    let (a, b) = clamp_range(s, range);
    if a == b {
        return String::new();
    }
    s.chars().skip(a).take(b - a).collect()
}

/// 用 `replacement` 替换 char 选区，返回新文本与插入点（char 索引）
pub fn splice(s: &str, range: (usize, usize), replacement: &str) -> (String, usize) {
    let (a, b) = clamp_range(s, range);
    let mut out = String::with_capacity(s.len() + replacement.len());
    out.extend(s.chars().take(a));
    out.push_str(replacement);
    out.extend(s.chars().skip(b));
    (out, a + char_len(replacement))
}

/// 全选区间；空文本无内容可选（返回 None）
pub fn all_range(s: &str) -> Option<(usize, usize)> {
    let n = char_len(s);
    if n == 0 {
        None
    } else {
        Some((0, n))
    }
}

/// 读系统剪贴板文本（arboard；headless/不可用时返回 None，不 panic）
pub fn read_clipboard() -> Option<String> {
    let mut cb = arboard::Clipboard::new().ok()?;
    cb.get_text().ok()
}

/// 写系统剪贴板文本，返回是否成功（headless/不可用返回 false）
pub fn write_clipboard(text: &str) -> bool {
    match arboard::Clipboard::new() {
        Ok(mut cb) => cb.set_text(text.to_string()).is_ok(),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_range_handles_overflow_and_reversed() {
        let s = "中文abc"; // 5 chars
        assert_eq!(clamp_range(s, (2, 4)), (2, 4));
        assert_eq!(clamp_range(s, (4, 2)), (2, 4), "反序应归一");
        assert_eq!(clamp_range(s, (3, 99)), (3, 5), "越界应钳位");
        assert_eq!(clamp_range("", (1, 2)), (0, 0));
    }

    #[test]
    fn selection_text_uses_char_indices_not_bytes() {
        let s = "中文abc";
        assert_eq!(selection_text(s, (0, 2)), "中文");
        assert_eq!(selection_text(s, (2, 5)), "abc");
        assert_eq!(selection_text(s, (2, 2)), "");
    }

    #[test]
    fn splice_replaces_and_reports_caret() {
        let (out, caret) = splice("abcdef", (1, 3), "XY");
        assert_eq!(out, "aXYdef");
        assert_eq!(caret, 3);
        // 插入（空选区）
        let (out, caret) = splice("ab", (1, 1), "中");
        assert_eq!(out, "a中b");
        assert_eq!(caret, 2);
        // 删除（替换为空串）
        let (out, caret) = splice("abcdef", (2, 5), "");
        assert_eq!(out, "abf");
        assert_eq!(caret, 2);
    }

    #[test]
    fn all_range_none_for_empty_text() {
        assert_eq!(all_range(""), None);
        assert_eq!(all_range("x"), Some((0, 1)));
    }

    #[test]
    fn edit_op_metadata_matches_design_menu() {
        assert_eq!(EditOp::Cut.i18n(), I18nKey::MenuCut);
        assert_eq!(EditOp::Copy.i18n(), I18nKey::MenuCopy);
        assert_eq!(EditOp::Paste.i18n(), I18nKey::MenuPaste);
        assert_eq!(EditOp::Delete.i18n(), I18nKey::MenuDelete);
        assert_eq!(EditOp::SelectAll.i18n(), I18nKey::MenuSelectAll);
        // 设计稿：剪切/复制/粘贴/全选 显快捷键，删除不显
        assert!(!EditOp::Cut.shortcut().is_empty());
        assert!(EditOp::Delete.shortcut().is_empty());
        assert_eq!(EditOp::ALL.len(), 5);
        assert_eq!(EditOp::Cut.cmd_id(), "edit_cut");
        assert_eq!(EditOp::SelectAll.cmd_id(), "edit_select_all");
        // 菜单渲染顺序（设计稿画板②）：剪切 / 复制 / 粘贴 / 删除 … 全选
        assert_eq!(
            EditOp::ALL.map(|o| o.cmd_id()),
            [
                "edit_cut",
                "edit_copy",
                "edit_paste",
                "edit_delete",
                "edit_select_all"
            ]
        );
    }
}
