//! P1（BC 5.2.5 设计稿）：菜单可用性规则 —— 跨平台共享的纯函数。
//!
//! 设计来源：`design/BC菜单和状态栏/screens/menus.html` +
//! `design/BC菜单和状态栏/交接/design-tokens.json` 的 `menus.grayedRules`。
//!
//! 这些判定不依赖 egui 控件，也不依赖具体菜单实现，因此同时被：
//! - `menubar.rs`（Linux 窗口内菜单栏）
//! - `native_menu.rs`（macOS/Windows 的 muda 原生菜单）
//! 复用，保证两端置灰规则一致。

use super::{DiffApp, Tab};

/// 返回 `(多标签项可用, 编辑项可用)`：
/// - 会话 / 窗口菜单的多标签项（关闭标签页 / 切换标签页 / 移动标签页 / 合并窗口）
///   在 `tabs.len() <= 1` 时置灰；
/// - 编辑菜单项仅在**可编辑会话**（`Tab::Merge` / `Tab::TextEdit`）可用；
///   只读比较会话（Diff / Dir / Csv / Image / Media / Patch）与无标签（主页）一律置灰。
pub fn menu_flags(app: &DiffApp) -> (bool, bool) {
    let multi_tab = app.tabs.len() > 1;
    let edit_enabled = matches!(
        app.tabs.get(app.active),
        Some(Tab::Merge(_)) | Some(Tab::TextEdit(_))
    );
    (multi_tab, edit_enabled)
}

/// 图片比较「重置差异偏移」是否可用（偏移为 0 时置灰）。
/// 非图片会话该项不显示，返回值无意义。
pub fn image_offset_nonzero(app: &DiffApp) -> bool {
    match app.tabs.get(app.active) {
        Some(Tab::Image(t)) => t.scroll.x != 0.0 || t.scroll.y != 0.0,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_flags_multi_tab_requires_more_than_one() {
        let mut app = DiffApp::new(super::super::Settings::default());
        assert_eq!(menu_flags(&app), (false, false), "无标签：均置灰");
        app.add_tab(Tab::Diff(super::super::difftab::DiffTab::new()));
        assert_eq!(menu_flags(&app), (false, false), "单标签：多标签项置灰");
        app.add_tab(Tab::Diff(super::super::difftab::DiffTab::new()));
        assert_eq!(menu_flags(&app).0, true, "多标签：多标签项可用");
    }

    #[test]
    fn image_offset_zero_disables_reset() {
        let mut app = DiffApp::new(super::super::Settings::default());
        app.add_tab(Tab::Image(super::super::imagetab::ImageTab::new("", "")));
        assert!(!image_offset_nonzero(&app), "偏移为 0 时应置灰");
    }
}
