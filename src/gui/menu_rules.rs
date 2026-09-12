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

/// P65：标准编辑动作（剪切/复制/粘贴/删除/全选）当前是否有可作用的目标。
///
/// 设计稿规则（`menus.html` 画板②）：只读比较会话
/// （Diff / Dir / Csv / Image / Media / Patch）这 5 项一律置灰；
/// 可编辑会话（TextEdit / Merge）可用。本实现中：
/// - **文本编辑会话**：内容区是 `egui::TextEdit`，有真实选区 → 可用
///   （语法高亮预览模式为只读渲染，此时置灰）；
/// - **文本合并会话**：左/右栏是「对齐后的绘制行」、输出栏按设计稿只读，
///   暂无文本选区模型 → 仍置灰（缺口记录在 CHANGELOG P65）。
pub fn clipboard_ops_enabled(app: &DiffApp) -> bool {
    match app.tabs.get(app.active) {
        Some(Tab::TextEdit(t)) => t.is_editable(),
        _ => false,
    }
}

// P65：窗口菜单「移动标签页到新窗口 / 合并所有窗口」的可用性。
//
// 设计稿规则：两者都在 `tabs.len() <= 1` 时置灰。此外：
// - 「移动标签页到新窗口」要求当前标签有可重建的会话表示
//   （文本编辑/补丁是未保存的内存态 → 置灰）；
// - 「合并所有窗口」要求存在心跳有效的对端窗口（多进程模型，见 gui::windows）。

/// 是否有可移动到新窗口的当前标签
pub fn move_tab_enabled(app: &DiffApp) -> bool {
    app.tabs.len() > 1 && app.movable_session().is_some()
}

/// 是否存在可合并的对端窗口
pub fn merge_windows_enabled(app: &DiffApp) -> bool {
    app.tabs.len() > 1 && app.peer_windows() > 0
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
        assert!(menu_flags(&app).0, "多标签：多标签项可用");
    }

    #[test]
    fn image_offset_zero_disables_reset() {
        let mut app = DiffApp::new(super::super::Settings::default());
        app.add_tab(Tab::Image(super::super::imagetab::ImageTab::new("", "")));
        assert!(!image_offset_nonzero(&app), "偏移为 0 时应置灰");
    }

    // ---- P65：标准编辑动作可用性（只读比较会话置灰）----

    #[test]
    fn clipboard_ops_disabled_for_readonly_compare_sessions() {
        for tab in [
            Tab::Diff(super::super::difftab::DiffTab::new()),
            Tab::Dir(super::super::dirtab::DirTab::new("", "")),
            Tab::Csv(super::super::csvtab::CsvTab::new("", "")),
            Tab::Image(super::super::imagetab::ImageTab::new("", "")),
            Tab::Media(super::super::mediatab::MediaTab::new("", "")),
            Tab::Patch(super::super::patchtab::PatchTab::new("")),
        ] {
            let mut app = DiffApp::new(super::super::Settings::default());
            app.add_tab(tab);
            assert!(
                !clipboard_ops_enabled(&app),
                "只读比较会话：剪切/复制/粘贴/删除/全选 应置灰"
            );
        }
        let app = DiffApp::new(super::super::Settings::default());
        assert!(!clipboard_ops_enabled(&app), "主页（无标签）应置灰");
    }

    #[test]
    fn clipboard_ops_enabled_for_text_edit_session() {
        let mut app = DiffApp::new(super::super::Settings::default());
        app.add_tab(Tab::TextEdit(super::super::textedit::TextEditTab::new("")));
        assert!(clipboard_ops_enabled(&app), "文本编辑会话：编辑项应可用");
    }

    // ---- P65：窗口菜单多窗口项（设计稿：单标签置灰）----

    #[test]
    fn window_items_need_more_than_one_tab() {
        let mut app = DiffApp::new(super::super::Settings::default());
        app.add_tab(Tab::Dir(super::super::dirtab::DirTab::new("/a", "/b")));
        // 单标签：两项都置灰（设计稿 §grayedRules 窗口菜单）
        assert!(!move_tab_enabled(&app), "单标签：移动标签页应置灰");
        assert!(!merge_windows_enabled(&app), "单标签：合并所有窗口应置灰");
        // 多标签 + 可重建会话：移动可用（合并仍需真实对端窗口）
        app.add_tab(Tab::Dir(super::super::dirtab::DirTab::new("/c", "/d")));
        assert!(move_tab_enabled(&app), "多标签且会话可重建：移动应可用");
        assert!(
            !merge_windows_enabled(&app),
            "无对端窗口时：合并所有窗口保持置灰"
        );
    }

    #[test]
    fn move_tab_disabled_for_unsaved_editor_tabs() {
        let mut app = DiffApp::new(super::super::Settings::default());
        app.add_tab(Tab::TextEdit(super::super::textedit::TextEditTab::new("")));
        app.add_tab(Tab::TextEdit(super::super::textedit::TextEditTab::new("")));
        app.active = 1;
        assert!(
            !move_tab_enabled(&app),
            "文本编辑会话是未保存内存态：不能搬到新窗口"
        );
    }
}
