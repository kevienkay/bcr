//! P58：跨平台「原生顶部菜单栏」（基于 muda）。
//!
//! - **macOS**：`Menu::init_for_nsapp()` 将菜单设为 NSApp 主菜单（顶部系统菜单栏）。
//! - **Windows**：`Menu::init_for_hwnd(hwnd)`，HWND 取自 eframe `CreationContext::winit_window()`。
//! - **Linux**：muda Linux 后端为 GTK，需 GTK 窗口；eframe/winit 窗口非 GTK，故 Linux 无原生菜单栏，
//!   保留窗口内菜单栏（见 mod.rs 的 `menu` 面板）。此模块在 Linux 为 no-op。
//!
//! 菜单项点击经 muda 的 `MenuEvent::receiver()`（内建通道）回传；egui 每帧轮询 `drain()`
//! 取回命令，由 `DiffApp::run_menu_cmd` 分派到对应动作（与窗口内菜单栏同源逻辑，避免重复实现）。

/// 原生菜单项代表的动作（由 egui 每帧轮询事件还原，再分派到 DiffApp 方法）。
///
/// Linux 无原生菜单（no-op），枚举与解析函数仅被 macOS/Windows 的 `drain()` 使用，
/// 故在 Linux 目标上放行 dead_code（测试仍覆盖映射逻辑）。
#[cfg_attr(target_os = "linux", allow(dead_code))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuCmd {
    NewText,
    NewDir,
    NewImage,
    NewCsv,
    NewMerge,
    NewMedia,
    OpenLeft,
    OpenRight,
    Refresh,
    Undo,
    Redo,
    CopyRight,
    CopyLeft,
    NextDiff,
    PrevDiff,
    ToggleSidebar,
    CycleTheme,
    Settings,
    Shortcuts,
    About,
    Quit,
    NextTab,
    PrevTab,
    Minimize,
    CloseAllTabs,
    LayoutSideBySide,
    LayoutTopBottom,
    LayoutWeb,
    DetailText,
    DetailHex,
    DetailAlign,
    ExportSettings,
    ImportSettings,
    ResetDefaults,
    OpenFiles,
    OpenDirCompare,
    OpenMerge,
    CompareWithOutput,
    NewTabLike,
    NextDiffFile,
    PrevDiffFile,
    CompareParent,
    CollapseAll,
    ExpandAll,
    RebuildTree,
    NextConflict,
    PrevConflict,
    NextDiffSection,
    PrevDiffSection,
    NextEdit,
    PrevEdit,
    FocusSearch,
    FocusReplace,
    SaveAs,
    NextTakenLeft,
    NextTakenRight,
    LoadClipboardLeft,
    LoadClipboardRight,
    GotoBookmark,
    ToggleBookmark,
    ClearBookmarks,
    RotateCw,
    RotateCcw,
    FlipHorizontal,
    FlipVertical,
    ImageResetDiff,
    ImageCompareMeta,
    CsvSort,
    CsvInsertRow,
    CsvDeleteRow,
    DiffCopyLine,
    DiffIndent,
    DiffRecompute,
    MergeTakeLine,
    MergeResolve,
    DirUpLevel,
    DirBack,
    DirForward,
    TextConvertTrim,
    TextConvertTabs,
    TextOpenClipboard,
    TextFindInFiles,
}

/// 由菜单项 id（字符串）还原命令；未知返回 None（将来新增向后兼容）。
#[cfg_attr(target_os = "linux", allow(dead_code))]
pub fn cmd_from_id(id: &str) -> Option<MenuCmd> {
    Some(match id {
        "new_text" => MenuCmd::NewText,
        "new_dir" => MenuCmd::NewDir,
        "new_image" => MenuCmd::NewImage,
        "new_csv" => MenuCmd::NewCsv,
        "new_merge" => MenuCmd::NewMerge,
        "new_media" => MenuCmd::NewMedia,
        "open_left" => MenuCmd::OpenLeft,
        "open_right" => MenuCmd::OpenRight,
        "refresh" => MenuCmd::Refresh,
        "undo" => MenuCmd::Undo,
        "redo" => MenuCmd::Redo,
        "copy_right" => MenuCmd::CopyRight,
        "copy_left" => MenuCmd::CopyLeft,
        "next_diff" => MenuCmd::NextDiff,
        "prev_diff" => MenuCmd::PrevDiff,
        "toggle_sidebar" => MenuCmd::ToggleSidebar,
        "cycle_theme" => MenuCmd::CycleTheme,
        "settings" => MenuCmd::Settings,
        "shortcuts" => MenuCmd::Shortcuts,
        "about" => MenuCmd::About,
        "quit" => MenuCmd::Quit,
        "next_tab" => MenuCmd::NextTab,
        "prev_tab" => MenuCmd::PrevTab,
        "minimize" => MenuCmd::Minimize,
        "close_all" => MenuCmd::CloseAllTabs,
        "layout_side" => MenuCmd::LayoutSideBySide,
        "layout_top" => MenuCmd::LayoutTopBottom,
        "layout_web" => MenuCmd::LayoutWeb,
        "detail_text" => MenuCmd::DetailText,
        "detail_hex" => MenuCmd::DetailHex,
        "detail_align" => MenuCmd::DetailAlign,
        "export_settings" => MenuCmd::ExportSettings,
        "import_settings" => MenuCmd::ImportSettings,
        "reset_defaults" => MenuCmd::ResetDefaults,
        "open_files" => MenuCmd::OpenFiles,
        "open_dir_compare" => MenuCmd::OpenDirCompare,
        "open_merge" => MenuCmd::OpenMerge,
        "compare_with_output" => MenuCmd::CompareWithOutput,
        "new_tab_like" => MenuCmd::NewTabLike,
        "next_diff_file" => MenuCmd::NextDiffFile,
        "prev_diff_file" => MenuCmd::PrevDiffFile,
        "compare_parent" => MenuCmd::CompareParent,
        "collapse_all" => MenuCmd::CollapseAll,
        "expand_all" => MenuCmd::ExpandAll,
        "rebuild_tree" => MenuCmd::RebuildTree,
        "next_conflict" => MenuCmd::NextConflict,
        "prev_conflict" => MenuCmd::PrevConflict,
        "next_diff_section" => MenuCmd::NextDiffSection,
        "prev_diff_section" => MenuCmd::PrevDiffSection,
        "next_edit" => MenuCmd::NextEdit,
        "prev_edit" => MenuCmd::PrevEdit,
        "focus_search" => MenuCmd::FocusSearch,
        "focus_replace" => MenuCmd::FocusReplace,
        "save_as" => MenuCmd::SaveAs,
        "next_taken_left" => MenuCmd::NextTakenLeft,
        "next_taken_right" => MenuCmd::NextTakenRight,
        "clip_left" => MenuCmd::LoadClipboardLeft,
        "clip_right" => MenuCmd::LoadClipboardRight,
        "goto_bookmark" => MenuCmd::GotoBookmark,
        "toggle_bookmark" => MenuCmd::ToggleBookmark,
        "clear_bookmarks" => MenuCmd::ClearBookmarks,
        "rotate_cw" => MenuCmd::RotateCw,
        "rotate_ccw" => MenuCmd::RotateCcw,
        "flip_h" => MenuCmd::FlipHorizontal,
        "flip_v" => MenuCmd::FlipVertical,
        "image_reset_diff" => MenuCmd::ImageResetDiff,
        "image_compare_meta" => MenuCmd::ImageCompareMeta,
        "csv_sort" => MenuCmd::CsvSort,
        "csv_insert_row" => MenuCmd::CsvInsertRow,
        "csv_delete_row" => MenuCmd::CsvDeleteRow,
        "diff_copy_line" => MenuCmd::DiffCopyLine,
        "diff_indent" => MenuCmd::DiffIndent,
        "diff_recompute" => MenuCmd::DiffRecompute,
        "merge_take_line" => MenuCmd::MergeTakeLine,
        "merge_resolve" => MenuCmd::MergeResolve,
        "dir_up" => MenuCmd::DirUpLevel,
        "dir_back" => MenuCmd::DirBack,
        "dir_forward" => MenuCmd::DirForward,
        "text_trim" => MenuCmd::TextConvertTrim,
        "text_tabs" => MenuCmd::TextConvertTabs,
        "text_clipboard" => MenuCmd::TextOpenClipboard,
        "text_find_files" => MenuCmd::TextFindInFiles,
        _ => return None,
    })
}

// ---- macOS / Windows：muda 实现 ----
#[cfg(any(target_os = "macos", target_os = "windows"))]
mod plat {
    use super::*;
    use muda::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};

    fn submenu(id: &str, key: crate::i18n::Key) -> Submenu {
        Submenu::with_id(id, crate::i18n::t(key), true)
    }

    fn item(id: &str, key: crate::i18n::Key) -> MenuItem {
        MenuItem::with_id(id, crate::i18n::t(key), true, None)
    }

    /// 无对应 i18n 键时用固定标签（原生菜单构建一次，语言切换不刷新系统菜单）。
    fn fixed(id: &str, label: &str) -> MenuItem {
        MenuItem::with_id(id, label, true, None)
    }

    /// 构建 bcr 菜单（顶级菜单 → 子菜单 → 菜单项；项 id 即命令字符串）。
    #[allow(unused_must_use)] // muda append 返回 Result，构建期无需逐个处理
    fn build_menu() -> Menu {
        let menu = Menu::new();
        // ---- 会话 ----
        {
            let m = submenu("session", crate::i18n::Key::MenuSession);
            m.append(&item("new_text", crate::i18n::Key::MenuNewText));
            m.append(&item("new_image", crate::i18n::Key::MenuNewImage));
            m.append(&item("new_csv", crate::i18n::Key::MenuNewCsv));
            m.append(&item("new_media", crate::i18n::Key::SessionMedia));
            m.append(&PredefinedMenuItem::separator());
            m.append(&fixed("new_tab_like", "新建类似标签"));
            m.append(&PredefinedMenuItem::separator());
            menu.append(&m);
        }
        // ---- 文件 ----
        {
            let m = submenu("file", crate::i18n::Key::MenuFile);
            m.append(&fixed("open_files", "打开文件…"));
            m.append(&fixed("open_dir_compare", "文件夹对比…"));
            m.append(&fixed("open_merge", "三路合并…"));
            m.append(&fixed("compare_with_output", "与输出比较"));
            m.append(&PredefinedMenuItem::separator());
            m.append(&item("open_left", crate::i18n::Key::MenuOpenLeft));
            m.append(&item("open_right", crate::i18n::Key::MenuOpenRight));
            m.append(&PredefinedMenuItem::separator());
            m.append(&item("refresh", crate::i18n::Key::Refresh));
            m.append(&PredefinedMenuItem::separator());
            m.append(&fixed("save_as", "另存为…"));
            m.append(&PredefinedMenuItem::separator());
            m.append(&fixed("clip_left", "剪贴板 → 左侧"));
            m.append(&fixed("clip_right", "剪贴板 → 右侧"));
            m.append(&PredefinedMenuItem::separator());
            m.append(&fixed("quit", "退出 bcr"));
            menu.append(&m);
        }
        // ---- 编辑 ----
        {
            let m = submenu("edit", crate::i18n::Key::MenuEdit);
            m.append(&item("undo", crate::i18n::Key::MenuUndo));
            m.append(&item("redo", crate::i18n::Key::MenuRedo));
            m.append(&PredefinedMenuItem::separator());
            m.append(&item("copy_right", crate::i18n::Key::CopyToRight));
            m.append(&item("copy_left", crate::i18n::Key::CopyToLeft));
            m.append(&PredefinedMenuItem::separator());
            m.append(&item("next_diff", crate::i18n::Key::NextDiff));
            m.append(&item("prev_diff", crate::i18n::Key::PrevDiff));
            m.append(&PredefinedMenuItem::separator());
            m.append(&fixed("next_diff_section", "下一差异区段"));
            m.append(&fixed("prev_diff_section", "上一差异区段"));
            m.append(&fixed("next_edit", "下一编辑点"));
            m.append(&fixed("prev_edit", "上一编辑点"));
            m.append(&fixed("next_conflict", "下一冲突"));
            m.append(&fixed("prev_conflict", "上一冲突"));
            m.append(&fixed("next_taken_left", "下一已取左"));
            m.append(&fixed("next_taken_right", "下一已取右"));
            menu.append(&m);
        }
        // ---- 搜索 ----
        {
            let m = submenu("search", crate::i18n::Key::MenuSearch);
            m.append(&item("next_diff", crate::i18n::Key::MenuFindNext));
            m.append(&item("prev_diff", crate::i18n::Key::MenuFindPrev));
            m.append(&PredefinedMenuItem::separator());
            m.append(&fixed("focus_search", "聚焦搜索"));
            m.append(&fixed("focus_replace", "聚焦替换"));
            m.append(&PredefinedMenuItem::separator());
            m.append(&item("next_diff_file", crate::i18n::Key::MenuNextDiffFile));
            m.append(&item("prev_diff_file", crate::i18n::Key::MenuPrevDiffFile));
            m.append(&item("compare_parent", crate::i18n::Key::MenuCompareParent));
            menu.append(&m);
        }
        // ---- 视图 ----
        {
            let m = submenu("view", crate::i18n::Key::MenuView);
            m.append(&fixed("toggle_sidebar", "切换侧栏"));
            m.append(&item("cycle_theme", crate::i18n::Key::Theme));
            m.append(&PredefinedMenuItem::separator());
            // 布局（DiffTab）
            m.append(&item("layout_side", crate::i18n::Key::LayoutSideBySide));
            m.append(&item("layout_top", crate::i18n::Key::LayoutTopBottom));
            m.append(&item("layout_web", crate::i18n::Key::LayoutWeb));
            m.append(&PredefinedMenuItem::separator());
            // 细节（DiffTab）
            m.append(&item("detail_text", crate::i18n::Key::DetailText));
            m.append(&item("detail_hex", crate::i18n::Key::DetailHex));
            m.append(&item("detail_align", crate::i18n::Key::DetailAlign));
            m.append(&PredefinedMenuItem::separator());
            // 目录折叠/展开/重建
            m.append(&fixed("collapse_all", "全部折叠"));
            m.append(&fixed("expand_all", "全部展开"));
            m.append(&fixed("rebuild_tree", "重建目录树"));
            m.append(&PredefinedMenuItem::separator());
            // 书签
            m.append(&fixed("toggle_bookmark", "切换书签"));
            m.append(&fixed("goto_bookmark", "跳转书签 0"));
            m.append(&fixed("clear_bookmarks", "清除书签"));
            m.append(&PredefinedMenuItem::separator());
            // 图片
            m.append(&fixed("rotate_cw", "图片顺时针旋转"));
            m.append(&fixed("rotate_ccw", "图片逆时针旋转"));
            m.append(&fixed("flip_h", "图片水平翻转"));
            m.append(&fixed("flip_v", "图片垂直翻转"));
            m.append(&fixed("image_reset_diff", "重置图片差异偏移"));
            m.append(&fixed("image_compare_meta", "图片元数据对比"));
            menu.append(&m);
        }
        // ---- 工具 ----
        {
            let m = submenu("tools", crate::i18n::Key::MenuTools);
            m.append(&item(
                "export_settings",
                crate::i18n::Key::MenuExportSettings,
            ));
            m.append(&item(
                "import_settings",
                crate::i18n::Key::MenuImportSettings,
            ));
            m.append(&PredefinedMenuItem::separator());
            m.append(&item("reset_defaults", crate::i18n::Key::MenuResetDefaults));
            m.append(&PredefinedMenuItem::separator());
            // 目录导航
            m.append(&fixed("dir_up", "上一级"));
            m.append(&fixed("dir_back", "后退"));
            m.append(&fixed("dir_forward", "前进"));
            m.append(&PredefinedMenuItem::separator());
            // 差异/合并
            m.append(&fixed("diff_copy_line", "复制当前行"));
            m.append(&fixed("diff_indent", "调整缩进"));
            m.append(&fixed("diff_recompute", "重新计算"));
            m.append(&fixed("merge_take_line", "取当前行"));
            m.append(&fixed("merge_resolve", "解决当前冲突"));
            m.append(&PredefinedMenuItem::separator());
            // 表格/文本
            m.append(&fixed("csv_sort", "表格排序…"));
            m.append(&fixed("csv_insert_row", "插入行"));
            m.append(&fixed("csv_delete_row", "删除行"));
            m.append(&fixed("text_trim", "文本去行尾空白"));
            m.append(&fixed("text_tabs", "文本 Tab→空格"));
            m.append(&fixed("text_clipboard", "打开剪贴板文本"));
            m.append(&fixed("text_find_files", "在文件中查找"));
            menu.append(&m);
        }
        // ---- 窗口 ----
        {
            let m = submenu("window", crate::i18n::Key::MenuWindow);
            m.append(&item("next_tab", crate::i18n::Key::MenuNextTab));
            m.append(&item("prev_tab", crate::i18n::Key::MenuPrevTab));
            m.append(&PredefinedMenuItem::separator());
            m.append(&item("minimize", crate::i18n::Key::MenuMinimize));
            m.append(&item("close_all", crate::i18n::Key::MenuCloseAllWindows));
            menu.append(&m);
        }
        // ---- 帮助 ----
        {
            let m = submenu("help", crate::i18n::Key::MenuHelp);
            m.append(&item("shortcuts", crate::i18n::Key::MenuShortcuts));
            m.append(&item("settings", crate::i18n::Key::MenuSettings));
            m.append(&PredefinedMenuItem::separator());
            m.append(&item("about", crate::i18n::Key::MenuAbout));
            menu.append(&m);
        }
        menu
    }

    /// 在 eframe 主线程安装原生菜单。macOS 直接设为 NSApp 主菜单；Windows 需窗口句柄。
    pub fn install(cc: &eframe::CreationContext) {
        let _ = cc; // macOS 不需要窗口句柄（init_for_nsapp）；Windows 用 cc 取 HWND
        let menu = build_menu();
        #[cfg(target_os = "macos")]
        {
            menu.init_for_nsapp();
        }
        #[cfg(target_os = "windows")]
        {
            use raw_window_handle::{HasWindowHandle, RawWindowHandle};
            if let Some(w) = cc.winit_window() {
                let hwnd = match w.window_handle().map(|h| h.as_raw()) {
                    Ok(RawWindowHandle::Win32(h)) => h.hwnd as isize,
                    _ => return,
                };
                let _ = unsafe { menu.init_for_hwnd(hwnd) };
            }
        }
    }

    /// 每帧取走全部菜单点击事件并还原为命令。
    pub fn drain() -> Vec<MenuCmd> {
        let mut out = Vec::new();
        while let Ok(ev) = MenuEvent::receiver().try_recv() {
            if let Some(cmd) = super::cmd_from_id(ev.id().as_ref()) {
                out.push(cmd);
            }
        }
        out
    }
}

// ---- Linux：no-op（保留窗口内菜单栏）----
#[cfg(target_os = "linux")]
mod plat {
    use super::*;
    pub fn install(_cc: &eframe::CreationContext) {}
    pub fn drain() -> Vec<MenuCmd> {
        Vec::new()
    }
}

pub use crate::gui::native_menu::plat::{drain, install};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cmd_from_id_maps_all_known_ids() {
        for (id, expect) in [
            ("new_text", MenuCmd::NewText),
            ("new_dir", MenuCmd::NewDir),
            ("new_image", MenuCmd::NewImage),
            ("new_csv", MenuCmd::NewCsv),
            ("new_merge", MenuCmd::NewMerge),
            ("new_media", MenuCmd::NewMedia),
            ("open_left", MenuCmd::OpenLeft),
            ("open_right", MenuCmd::OpenRight),
            ("refresh", MenuCmd::Refresh),
            ("undo", MenuCmd::Undo),
            ("redo", MenuCmd::Redo),
            ("copy_right", MenuCmd::CopyRight),
            ("copy_left", MenuCmd::CopyLeft),
            ("next_diff", MenuCmd::NextDiff),
            ("prev_diff", MenuCmd::PrevDiff),
            ("toggle_sidebar", MenuCmd::ToggleSidebar),
            ("cycle_theme", MenuCmd::CycleTheme),
            ("settings", MenuCmd::Settings),
            ("shortcuts", MenuCmd::Shortcuts),
            ("about", MenuCmd::About),
            ("quit", MenuCmd::Quit),
        ] {
            assert_eq!(
                cmd_from_id(id),
                Some(expect),
                "id `{id}` 应映射为 {:?}",
                expect
            );
        }
    }

    #[test]
    fn cmd_from_id_unknown_returns_none() {
        assert_eq!(cmd_from_id("nonexistent"), None);
        assert_eq!(cmd_from_id(""), None);
    }
}
