//! P33 标准菜单栏：对标 Beyond Compare 的
//! `Session | File | Edit | Search | View | Tools | Help` 菜单结构。
//!
//! - 所有入口复用现有 DiffApp 方法（打开会话/弹窗/转发当前标签）
//! - 语言/主题切换移入 View 菜单（BC 观感：设置类操作收进菜单，不再占工具栏）

use eframe::egui::{self};

use super::{DiffApp, Tab};
use crate::i18n::{t, Key as I18nKey};

/// 顶部菜单栏（BC 式 7 个主菜单）
pub fn menu_bar(app: &mut DiffApp, ui: &mut egui::Ui) {
    egui::MenuBar::new().ui(ui, |ui| {
        session_menu(app, ui);
        file_menu(app, ui);
        edit_menu(app, ui);
        search_menu(app, ui);
        view_menu(app, ui);
        tools_menu(app, ui);
        window_menu(app, ui);
        help_menu(app, ui);
    });
}

/// P51-3：平台快捷键文本（macOS 用 ⌘ 系，Windows/Linux 用 Ctrl 系）
fn sc(mac: &str, win: &str) -> String {
    if cfg!(target_os = "macos") {
        mac.to_string()
    } else {
        win.to_string()
    }
}

/// P51-3：菜单项按钮（带右侧快捷键文本，BC 观感）
fn menu_item(
    ui: &mut egui::Ui,
    label: impl Into<egui::WidgetText>,
    shortcut: String,
) -> egui::Response {
    ui.add(egui::Button::new(label).shortcut_text(shortcut))
}

/// 对当前标签为 DiffTab 时执行操作（菜单转发撤销/重做/跳转等）
fn with_diff_tab(app: &mut DiffApp, f: impl FnOnce(&mut super::difftab::DiffTab)) {
    if let Some(Tab::Diff(t)) = app.tabs.get_mut(app.active) {
        f(t);
    }
}

/// P37-1b：对当前标签为 MergeTab 时执行操作（三路合并导航转发）
fn with_merge_tab(app: &mut DiffApp, f: impl FnOnce(&mut super::mergetab::MergeTab)) {
    if let Some(Tab::Merge(t)) = app.tabs.get_mut(app.active) {
        f(t);
    }
}

/// Session：新建各类会话 + 保存会话
fn session_menu(app: &mut DiffApp, ui: &mut egui::Ui) {
    ui.menu_button(t(I18nKey::MenuSession), |ui| {
        if ui.button(t(I18nKey::MenuNewText)).clicked() {
            ui.close();
            app.open_empty_diff();
        }
        if ui.button(t(I18nKey::MenuNewDir)).clicked() {
            ui.close();
            app.open_empty_dir();
        }
        if ui.button(t(I18nKey::MenuNewMerge)).clicked() {
            ui.close();
            app.open_empty_merge();
        }
        if ui.button(t(I18nKey::MenuNewImage)).clicked() {
            ui.close();
            app.open_empty_image();
        }
        if ui.button(t(I18nKey::MenuNewCsv)).clicked() {
            ui.close();
            app.open_empty_csv();
        }
        // P43-6：新建媒体比较会话（音视频元数据）
        if ui.button(t(I18nKey::SessionMedia)).clicked() {
            ui.close();
            app.open_empty_media();
        }
        // Hex 会话：空文本对比会话（二进制文件自动切 hex 视图）
        if ui.button(t(I18nKey::MenuNewHex)).clicked() {
            ui.close();
            app.open_empty_diff();
        }
        ui.separator();
        // P43-1：导航历史（BC 会话菜单 后退/前进/上一层/比较父文件夹，DirTab 分支）
        if matches!(app.tabs.get(app.active), Some(Tab::Dir(_))) {
            if ui.button(t(I18nKey::MenuBack)).clicked() {
                ui.close();
                if let Some(Tab::Dir(t)) = app.tabs.get_mut(app.active) {
                    t.back();
                }
            }
            if ui.button(t(I18nKey::MenuForward)).clicked() {
                ui.close();
                if let Some(Tab::Dir(t)) = app.tabs.get_mut(app.active) {
                    t.forward();
                }
            }
            if ui.button(t(I18nKey::MenuUpLevel)).clicked() {
                ui.close();
                if let Some(Tab::Dir(t)) = app.tabs.get_mut(app.active) {
                    t.up_level();
                }
            }
            if ui.button(t(I18nKey::MenuCompareParent)).clicked() {
                ui.close();
                if let Some(Tab::Dir(t)) = app.tabs.get_mut(app.active) {
                    t.compare_parent();
                }
            }
        }
        ui.separator();
        // P39-2a：新建标签页 / 新建窗口（BC Session 菜单）
        if menu_item(ui, t(I18nKey::MenuNewTab), sc("⌘T", "Ctrl+T")).clicked() {
            ui.close();
            app.new_tab_like_current();
        }
        if menu_item(ui, t(I18nKey::MenuNewWindow), sc("⌘N", "Ctrl+N")).clicked() {
            ui.close();
            super::DiffApp::open_new_window();
        }
        ui.separator();
        // 保存会话：打开会话中心（GUI 内管理已保存会话）
        if menu_item(ui, t(I18nKey::MenuSaveSession), sc("⌥⌘S", "Ctrl+Alt+S")).clicked() {
            ui.close();
            app.show_sessions = true;
        }
        // P44-4：打开会话（BC Session>打开会话，⌥⌘O；打开会话中心）
        if menu_item(ui, t(I18nKey::MenuOpenSession), sc("⌥⌘O", "Ctrl+Alt+O")).clicked() {
            ui.close();
            app.show_sessions = true;
        }
        // P44-4：重新比较文件（BC Session>重新比较文件，⌘R）
        if menu_item(ui, t(I18nKey::MenuRecompare), sc("⌘R", "Ctrl+R")).clicked() {
            ui.close();
            app.reload_current();
        }
        // P46-5：工作空间（BC 会话菜单 保存工作空间为.../加载工作空间）
        if ui.button(t(I18nKey::MenuSaveWorkspaceAs)).clicked() {
            ui.close();
            if let Some(p) = rfd::FileDialog::new()
                .set_file_name("bcr-workspace.toml")
                .save_file()
            {
                if let Err(e) = app.save_workspace(&p) {
                    app.report_error = Some(e);
                }
            }
        }
        if ui.button(t(I18nKey::MenuLoadWorkspace)).clicked() {
            ui.close();
            if let Some(p) = rfd::FileDialog::new().pick_file() {
                if let Err(e) = app.load_workspace(&p) {
                    app.report_error = Some(e);
                }
            }
        }
        // P44-4：已锁定（BC Session>已锁定；DiffTab 分支，锁定会话防编辑）
        if let Some(Tab::Diff(lock_tab)) = app.tabs.get_mut(app.active) {
            ui.checkbox(&mut lock_tab.locked, crate::i18n::t(I18nKey::MenuLocked));
        }
        // P39-2a：清除会话（重置当前标签为空会话）
        if menu_item(ui, t(I18nKey::MenuClearSession), sc("⌥⌘C", "Ctrl+Alt+C")).clicked() {
            ui.close();
            app.clear_active_tab();
        }
        ui.separator();
        // P39-2c：报告生成（⌘P）
        if menu_item(ui, t(I18nKey::MenuReport), sc("⌘P", "Ctrl+P")).clicked() {
            ui.close();
            app.show_report = true;
            app.report_error = None;
        }
        // P43-5：信息（BC Session>信息，当前标签统计）
        if ui.button(t(I18nKey::MenuInfo)).clicked() {
            ui.close();
            app.show_info = true;
        }
        ui.separator();
        // P39-2e：比较文件使用（视图切换，对标 BC Session>比较文件使用）
        // 用另一视图重新打开当前 DiffTab 的左右文件
        let cur_paths = app.tabs.get(app.active).and_then(|t| match t {
            super::Tab::Diff(d) => match (&d.left, &d.right) {
                (Some(l), Some(r)) => Some((l.path.clone(), r.path.clone())),
                _ => None,
            },
            _ => None,
        });
        if cur_paths.is_some() {
            ui.menu_button(t(I18nKey::MenuCompareUsing), |ui| {
                let targets: [(&str, crate::i18n::Key); 4] = [
                    ("文本对比", crate::i18n::Key::SessionText),
                    ("16进制对比", crate::i18n::Key::MenuNewHex),
                    ("图片对比", crate::i18n::Key::SessionImage),
                    ("表格对比", crate::i18n::Key::SessionCsv),
                ];
                for (label, key) in targets {
                    if ui.button(label).clicked() {
                        ui.close();
                        let (l, r) = cur_paths.clone().unwrap();
                        match key {
                            crate::i18n::Key::SessionText => {
                                app.reopen_as_text(&l, &r);
                            }
                            crate::i18n::Key::MenuNewHex => {
                                app.reopen_as_hex(&l, &r);
                            }
                            crate::i18n::Key::SessionImage => {
                                app.reopen_as_image(&l, &r);
                            }
                            _ => {
                                app.reopen_as_csv(&l, &r);
                            }
                        }
                    }
                }
            });
            // P43-4：合并文件（BC 文本比较 Session>合并文件）
            if ui.button(t(I18nKey::MenuMergeFiles)).clicked() {
                ui.close();
                let (l, r) = cur_paths.clone().unwrap();
                app.reopen_as_merge(&l, &r);
            }
        }
        // P43-4：和输出比较（BC 文件夹合并 Session>和输出比较）
        if matches!(app.tabs.get(app.active), Some(Tab::FolderMerge(_)))
            && ui.button(t(I18nKey::MenuCompareWithOutput)).clicked()
        {
            ui.close();
            app.compare_with_output();
        }
    });
}

/// File：打开各类对比 / 当前标签打开左侧右侧 / 剪贴板 / 云盘
fn file_menu(app: &mut DiffApp, ui: &mut egui::Ui) {
    ui.menu_button(t(I18nKey::MenuFile), |ui| {
        if ui.button(t(I18nKey::MenuOpenFiles)).clicked() {
            ui.close();
            app.open_diff_files();
        }
        if ui.button(t(I18nKey::MenuOpenDir)).clicked() {
            ui.close();
            app.open_dir_compare();
        }
        if ui.button(t(I18nKey::MenuOpenMerge)).clicked() {
            ui.close();
            app.open_merge();
        }
        ui.separator();
        if ui.button(t(I18nKey::MenuOpenLeft)).clicked() {
            ui.close();
            with_diff_tab(app, |tab| tab.open_left_dialog());
        }
        if ui.button(t(I18nKey::MenuOpenRight)).clicked() {
            ui.close();
            with_diff_tab(app, |tab| tab.open_right_dialog());
        }
        ui.separator();
        if ui.button(t(I18nKey::MenuClipLeft)).clicked() {
            ui.close();
            with_diff_tab(app, |tab| tab.load_clipboard_left());
        }
        if ui.button(t(I18nKey::MenuClipRight)).clicked() {
            ui.close();
            with_diff_tab(app, |tab| tab.load_clipboard_right());
        }
        // P42-2：文本编辑「打开剪贴板」（BC 文本编辑 File 菜单）
        if matches!(app.tabs.get(app.active), Some(Tab::TextEdit(_)))
            && ui.button(t(I18nKey::MenuClipRight)).clicked()
        {
            ui.close();
            if let Some(Tab::TextEdit(t)) = app.tabs.get_mut(app.active) {
                t.open_clipboard();
            }
        }
        ui.separator();
        // P44-4：保存文件为（BC File>保存文件为...，⌘⇧S；TextEdit 另存对话框）
        if matches!(app.tabs.get(app.active), Some(Tab::TextEdit(_)))
            && menu_item(ui, t(I18nKey::MenuSaveFileAs), sc("⌘⇧S", "Ctrl+Shift+S")).clicked()
        {
            ui.close();
            if let Some(Tab::TextEdit(t)) = app.tabs.get_mut(app.active) {
                t.save_as();
            }
        }
        // P44-4：打开方式（BC File>打开方式；DiffTab 左右文件用系统应用打开/在查找器中显示）
        if let Some(Tab::Diff(d)) = app.tabs.get(app.active) {
            let lp = d.left.as_ref().map(|f| f.path.clone());
            let rp = d.right.as_ref().map(|f| f.path.clone());
            if lp.is_some() || rp.is_some() {
                ui.menu_button(t(I18nKey::MenuOpenWith), |ui| {
                    if let Some(p) = &lp {
                        if ui.button(t(I18nKey::MenuOpenWith)).clicked() {
                            ui.close();
                            super::common::open_with_system_app(p);
                        }
                        if ui.button(t(I18nKey::MenuRevealInFinder)).clicked() {
                            ui.close();
                            super::common::reveal_in_file_manager(p);
                        }
                        ui.separator();
                    }
                    if let Some(p) = &rp {
                        if ui.button(t(I18nKey::MenuOpenWith)).clicked() {
                            ui.close();
                            super::common::open_with_system_app(p);
                        }
                        if ui.button(t(I18nKey::MenuRevealInFinder)).clicked() {
                            ui.close();
                            super::common::reveal_in_file_manager(p);
                        }
                    }
                });
            }
        }
        if ui.button(t(I18nKey::MenuOpenCloud)).clicked() {
            ui.close();
            app.show_cloud = true;
        }
    });
}

/// Edit：撤销/重做（转发当前文本标签）
fn edit_menu(app: &mut DiffApp, ui: &mut egui::Ui) {
    ui.menu_button(t(I18nKey::MenuEdit), |ui| {
        if menu_item(ui, t(I18nKey::MenuUndo), sc("⌘Z", "Ctrl+Z")).clicked() {
            ui.close();
            with_diff_tab(app, |tab| tab.undo());
        }
        if menu_item(ui, t(I18nKey::MenuRedo), sc("⌘Y", "Ctrl+Y")).clicked() {
            ui.close();
            with_diff_tab(app, |tab| tab.redo());
        }
        ui.separator();
        // P40-1：编辑左/右侧（原工具栏低频按钮，收进菜单）
        if ui.button(t(I18nKey::EditLeft)).clicked() {
            ui.close();
            with_diff_tab(app, |tab| tab.start_edit(super::difftab::EditSide::Left));
        }
        if ui.button(t(I18nKey::EditRight)).clicked() {
            ui.close();
            with_diff_tab(app, |tab| tab.start_edit(super::difftab::EditSide::Right));
        }
        ui.separator();
        if ui.button(t(I18nKey::MenuFind)).clicked() {
            ui.close();
            with_diff_tab(app, |tab| tab.focus_search());
        }
        ui.separator();
        // P41-1：展开/折叠全部（DirTab 分支，BC 编辑菜单）
        if matches!(app.tabs.get(app.active), Some(Tab::Dir(_))) {
            if ui.button(t(I18nKey::MenuExpandAll)).clicked() {
                ui.close();
                if let Some(Tab::Dir(t)) = app.tabs.get_mut(app.active) {
                    t.expand_all();
                }
            }
            if ui.button(t(I18nKey::MenuCollapseAll)).clicked() {
                ui.close();
                if let Some(Tab::Dir(t)) = app.tabs.get_mut(app.active) {
                    t.collapse_all();
                }
            }
            ui.separator();
            // P41-3：选择操作（DirTab 分支，BC 编辑菜单「选择较新项/独有项/反向选择」）
            if ui.button(t(I18nKey::MenuSelectAll)).clicked() {
                ui.close();
                if let Some(Tab::Dir(t)) = app.tabs.get_mut(app.active) {
                    t.select_all();
                }
            }
            if ui.button(t(I18nKey::MenuSelectNone)).clicked() {
                ui.close();
                if let Some(Tab::Dir(t)) = app.tabs.get_mut(app.active) {
                    t.select_none();
                }
            }
            if ui.button(t(I18nKey::MenuSelectNewer)).clicked() {
                ui.close();
                if let Some(Tab::Dir(t)) = app.tabs.get_mut(app.active) {
                    t.select_newer();
                }
            }
            if ui.button(t(I18nKey::MenuSelectOrphans)).clicked() {
                ui.close();
                if let Some(Tab::Dir(t)) = app.tabs.get_mut(app.active) {
                    t.select_orphans();
                }
            }
            if ui.button(t(I18nKey::MenuInvertSelection)).clicked() {
                ui.close();
                if let Some(Tab::Dir(t)) = app.tabs.get_mut(app.active) {
                    t.invert_selection();
                }
            }
        }
        // DiffTab 分支：转换文件 + 文本选区操作（BC 编辑菜单）
        if matches!(app.tabs.get(app.active), Some(Tab::Diff(_))) {
            ui.separator();
            // P42-1：转换文件（BC Edit>Convert File）
            ui.menu_button(t(I18nKey::ConvertFile), |ui| {
                let modes = [
                    (t(I18nKey::ConvertTrim), super::textedit::ConvertMode::Trim),
                    (
                        t(I18nKey::ConvertTabs),
                        super::textedit::ConvertMode::TabsToSpaces,
                    ),
                    (
                        t(I18nKey::ConvertCrlf),
                        super::textedit::ConvertMode::ToCrlf,
                    ),
                    (t(I18nKey::ConvertLf), super::textedit::ConvertMode::ToLf),
                ];
                for (label, mode) in modes {
                    if ui.button(label).clicked() {
                        ui.close();
                        if let Some(Tab::Diff(tab)) = app.tabs.get_mut(app.active) {
                            tab.convert_file(mode);
                        }
                    }
                }
            });
            // P43-2：文本选区操作（BC 编辑菜单 选择选择内容/把选择内容和剪贴板比较）
            if ui.button(t(I18nKey::MenuSelectSelection)).clicked() {
                ui.close();
                if let Some(Tab::Diff(tab)) = app.tabs.get_mut(app.active) {
                    tab.select_selection();
                }
            }
            if ui.button(t(I18nKey::MenuSelectionToClipboard)).clicked() {
                ui.close();
                if let Some(Tab::Diff(tab)) = app.tabs.get_mut(app.active) {
                    tab.selection_to_clipboard();
                }
            }
            ui.separator();
            // P44-2：对齐方式/缩进（BC 编辑菜单 对齐方式.../增加缩进/减少缩进）
            if menu_item(ui, t(I18nKey::MenuAlign), sc("⌘A", "Ctrl+A")).clicked() {
                ui.close();
                if let Some(Tab::Diff(tab)) = app.tabs.get_mut(app.active) {
                    tab.align_current();
                }
            }
            if menu_item(ui, t(I18nKey::MenuIndentIncrease), sc("]", "]")).clicked() {
                ui.close();
                if let Some(Tab::Diff(tab)) = app.tabs.get_mut(app.active) {
                    tab.indent_current(1);
                }
            }
            if menu_item(ui, t(I18nKey::MenuIndentDecrease), sc("[", "[")).clicked() {
                ui.close();
                if let Some(Tab::Diff(tab)) = app.tabs.get_mut(app.active) {
                    tab.indent_current(-1);
                }
            }
        }
        // P44-6：CsvTab 分支——排序/修改/插入行（BC 表格比较编辑菜单）
        if matches!(app.tabs.get(app.active), Some(Tab::Csv(_))) {
            ui.separator();
            if ui.button(t(I18nKey::MenuSort)).clicked() {
                ui.close();
                if let Some(Tab::Csv(tab)) = app.tabs.get_mut(app.active) {
                    tab.open_sort_dialog();
                }
            }
            if ui.button(t(I18nKey::CsvEditCell)).clicked() {
                ui.close();
                if let Some(Tab::Csv(tab)) = app.tabs.get_mut(app.active) {
                    tab.open_cell_edit();
                }
            }
            if ui.button(t(I18nKey::CsvInsertRow)).clicked() {
                ui.close();
                if let Some(Tab::Csv(tab)) = app.tabs.get_mut(app.active) {
                    tab.insert_row();
                }
            }
            if ui.button("在后面插入行").clicked() {
                ui.close();
                if let Some(Tab::Csv(tab)) = app.tabs.get_mut(app.active) {
                    tab.insert_row_after();
                }
            }
            if ui.button("在后面插入列").clicked() {
                ui.close();
                if let Some(Tab::Csv(tab)) = app.tabs.get_mut(app.active) {
                    tab.insert_col_after();
                }
            }
            if ui.button(t(I18nKey::CsvDeleteRow)).clicked() {
                ui.close();
                if let Some(Tab::Csv(tab)) = app.tabs.get_mut(app.active) {
                    tab.delete_row();
                }
            }
        }
        // P45-5：PatchTab 分支——选择选择内容（BC 编辑菜单，D）
        if matches!(app.tabs.get(app.active), Some(Tab::Patch(_))) {
            ui.separator();
            if ui.button(t(I18nKey::MenuSelectSelection)).clicked() {
                ui.close();
                if let Some(Tab::Patch(tab)) = app.tabs.get_mut(app.active) {
                    tab.select_selection();
                }
            }
        }
        // P44-3：MergeTab 分支——冲突采用（BC 编辑菜单 冲突子组）
        if matches!(app.tabs.get(app.active), Some(Tab::Merge(_))) {
            ui.separator();
            if ui.button(t(I18nKey::MenuTakeLeft)).clicked() {
                ui.close();
                with_merge_tab(app, |tab| {
                    tab.resolve_current(crate::mergeview::Resolution::Left)
                });
            }
            if ui.button(t(I18nKey::MenuTakeCenter)).clicked() {
                ui.close();
                with_merge_tab(app, |tab| {
                    tab.resolve_current(crate::mergeview::Resolution::Base)
                });
            }
            if ui.button(t(I18nKey::MenuTakeRight)).clicked() {
                ui.close();
                with_merge_tab(app, |tab| {
                    tab.resolve_current(crate::mergeview::Resolution::Right)
                });
            }
            ui.separator();
            if ui.button(t(I18nKey::MenuTakeLeftThenRight)).clicked() {
                ui.close();
                with_merge_tab(app, |tab| {
                    tab.resolve_current(crate::mergeview::Resolution::LeftThenRight)
                });
            }
            if ui.button(t(I18nKey::MenuTakeRightThenLeft)).clicked() {
                ui.close();
                with_merge_tab(app, |tab| {
                    tab.resolve_current(crate::mergeview::Resolution::RightThenLeft)
                });
            }
            ui.separator();
            // P45-1：行级采用（BC 编辑菜单 采用左边的行/中心行/右边行）
            if ui.button(t(I18nKey::MenuTakeLeftLine)).clicked() {
                ui.close();
                with_merge_tab(app, |tab| tab.take_line(crate::mergeview::Resolution::Left));
            }
            if ui.button(t(I18nKey::MenuTakeCenterLine)).clicked() {
                ui.close();
                with_merge_tab(app, |tab| tab.take_line(crate::mergeview::Resolution::Base));
            }
            if ui.button(t(I18nKey::MenuTakeRightLine)).clicked() {
                ui.close();
                with_merge_tab(app, |tab| {
                    tab.take_line(crate::mergeview::Resolution::Right)
                });
            }
        }
    });
}

/// Search：查找 / 下一差异 / 上一差异（转发当前标签）
///
/// P37-1b：DiffTab 转发差异导航；MergeTab 转发差异/冲突/采用导航（BC Text Merge 搜索菜单）
fn search_menu(app: &mut DiffApp, ui: &mut egui::Ui) {
    ui.menu_button(t(I18nKey::MenuSearch), |ui| {
        // MergeTab：三路合并导航（BC Text Merge 搜索菜单）
        if matches!(app.tabs.get(app.active), Some(Tab::Merge(_))) {
            if ui.button(t(I18nKey::ClearConflictNext)).clicked() {
                ui.close();
                with_merge_tab(app, |tab| tab.clear_conflict_next());
            }
            ui.separator();
            if ui.button(t(I18nKey::NextConflict)).clicked() {
                ui.close();
                with_merge_tab(app, |tab| tab.next_conflict());
            }
            if ui.button(t(I18nKey::PrevConflict)).clicked() {
                ui.close();
                with_merge_tab(app, |tab| tab.prev_conflict());
            }
            ui.separator();
            if ui.button(t(I18nKey::NextDiff)).clicked() {
                ui.close();
                with_merge_tab(app, |tab| tab.next_diff());
            }
            if ui.button(t(I18nKey::PrevDiff)).clicked() {
                ui.close();
                with_merge_tab(app, |tab| tab.prev_diff());
            }
            ui.separator();
            if ui.button(t(I18nKey::NextLeftTaken)).clicked() {
                ui.close();
                with_merge_tab(app, |tab| tab.next_taken_left());
            }
            if ui.button(t(I18nKey::PrevLeftTaken)).clicked() {
                ui.close();
                with_merge_tab(app, |tab| tab.prev_taken_left());
            }
            if ui.button(t(I18nKey::NextRightTaken)).clicked() {
                ui.close();
                with_merge_tab(app, |tab| tab.next_taken_right());
            }
            if ui.button(t(I18nKey::PrevRightTaken)).clicked() {
                ui.close();
                with_merge_tab(app, |tab| tab.prev_taken_right());
            }
            return;
        }
        // DiffTab：查找 / 差异导航 / 编辑导航 / 重载
        if menu_item(ui, t(I18nKey::MenuFind), sc("⌘F", "Ctrl+F")).clicked() {
            ui.close();
            with_diff_tab(app, |tab| tab.focus_search());
        }
        // P44-2：使用选择内容进行查找（BC 搜索>使用选择内容进行查找，⌘E）
        if matches!(app.tabs.get(app.active), Some(Tab::Diff(_)))
            && menu_item(ui, t(I18nKey::MenuFindSelection), sc("⌘E", "Ctrl+E")).clicked()
        {
            ui.close();
            with_diff_tab(app, |tab| tab.find_selection());
        }
        // P45-5：文本编辑 使用选择内容进行查找（BC 搜索>使用选择内容进行查找，⌘E）
        if matches!(app.tabs.get(app.active), Some(Tab::TextEdit(_)))
            && menu_item(ui, t(I18nKey::MenuFindSelection), sc("⌘E", "Ctrl+E")).clicked()
        {
            ui.close();
            if let Some(Tab::TextEdit(t)) = app.tabs.get_mut(app.active) {
                t.find_selection();
            }
        }
        // P39-2e：替换…（⇧⌘F）
        if menu_item(ui, t(I18nKey::MenuReplace), sc("⇧⌘F", "Ctrl+Shift+F")).clicked() {
            ui.close();
            with_diff_tab(app, |tab| tab.focus_replace());
        }
        // P39-2a：查找下一 / 上一（⌘G / ⇧⌘G）
        if menu_item(ui, t(I18nKey::MenuFindNext), sc("⌘G", "Ctrl+G")).clicked() {
            ui.close();
            with_diff_tab(app, |tab| tab.next_match());
        }
        if menu_item(ui, t(I18nKey::MenuFindPrev), sc("⇧⌘G", "Ctrl+Shift+G")).clicked() {
            ui.close();
            with_diff_tab(app, |tab| tab.prev_match());
        }
        // P39-2a：转到行…（⌘L）
        if menu_item(ui, t(I18nKey::MenuGotoLine), sc("⌘L", "Ctrl+L")).clicked() {
            ui.close();
            with_diff_tab(app, |tab| tab.goto_focus = true);
        }
        ui.separator();
        if menu_item(ui, t(I18nKey::MenuNextDiff), sc("F6", "F6")).clicked() {
            ui.close();
            with_diff_tab(app, |tab| tab.next_diff());
        }
        if menu_item(ui, t(I18nKey::MenuPrevDiff), sc("F7", "F7")).clicked() {
            ui.close();
            with_diff_tab(app, |tab| tab.prev_diff());
        }
        // P39-2c：差异部分导航（区块级跳转，BC ⇧⌃↓/↑）
        if menu_item(ui, t(I18nKey::MenuNextSection), sc("⇧⌃↓", "Ctrl+Shift+↓")).clicked() {
            ui.close();
            with_diff_tab(app, |tab| tab.next_diff_section());
        }
        if menu_item(ui, t(I18nKey::MenuPrevSection), sc("⇧⌃↑", "Ctrl+Shift+↑")).clicked() {
            ui.close();
            with_diff_tab(app, |tab| tab.prev_diff_section());
        }
        // P38-1d：编辑导航（BC Next/Previous Edit）
        if ui.button(t(I18nKey::MenuNextEdit)).clicked() {
            ui.close();
            with_diff_tab(app, |tab| tab.next_edit());
        }
        if ui.button(t(I18nKey::MenuPrevEdit)).clicked() {
            ui.close();
            with_diff_tab(app, |tab| tab.prev_edit());
        }
        // P46-2：PatchTab 差异导航（BC 文本补丁搜索菜单 下一个/上一个差异、差异部分）
        if matches!(app.tabs.get(app.active), Some(Tab::Patch(_))) {
            ui.separator();
            if ui.button(t(I18nKey::MenuNextDiff)).clicked() {
                ui.close();
                if let Some(Tab::Patch(tab)) = app.tabs.get_mut(app.active) {
                    tab.next_diff();
                }
            }
            if ui.button(t(I18nKey::MenuPrevDiff)).clicked() {
                ui.close();
                if let Some(Tab::Patch(tab)) = app.tabs.get_mut(app.active) {
                    tab.prev_diff();
                }
            }
            if ui.button(t(I18nKey::MenuNextSection)).clicked() {
                ui.close();
                if let Some(Tab::Patch(tab)) = app.tabs.get_mut(app.active) {
                    tab.next_diff_section();
                }
            }
            if ui.button(t(I18nKey::MenuPrevSection)).clicked() {
                ui.close();
                if let Some(Tab::Patch(tab)) = app.tabs.get_mut(app.active) {
                    tab.prev_diff_section();
                }
            }
        }
        // P43-3：替换导航（BC 搜索菜单 下一个/上一个替换）
        if ui.button(t(I18nKey::MenuNextReplace)).clicked() {
            ui.close();
            with_diff_tab(app, |tab| tab.next_replace());
        }
        if ui.button(t(I18nKey::MenuPrevReplace)).clicked() {
            ui.close();
            with_diff_tab(app, |tab| tab.prev_replace());
        }
        ui.separator();
        if menu_item(ui, t(I18nKey::MenuReload), sc("F5", "F5")).clicked() {
            ui.close();
            with_diff_tab(app, |tab| tab.reload());
        }
        // P43-3：差异文件导航（DirTab 分支，BC 搜索菜单 下一个/上一个差异文件）
        if matches!(app.tabs.get(app.active), Some(Tab::Dir(_))) {
            // P44-7：查找文件名（⌘F，BC 文件夹比较搜索菜单）
            if menu_item(ui, t(I18nKey::MenuFindFileName), sc("⌘F", "Ctrl+F")).clicked() {
                ui.close();
                if let Some(Tab::Dir(t)) = app.tabs.get_mut(app.active) {
                    t.show_filter_panel = true;
                }
            }
            if ui.button(t(I18nKey::MenuNextDiffFile)).clicked() {
                ui.close();
                if let Some(Tab::Dir(t)) = app.tabs.get_mut(app.active) {
                    t.next_diff_file();
                }
            }
            if ui.button(t(I18nKey::MenuPrevDiffFile)).clicked() {
                ui.close();
                if let Some(Tab::Dir(t)) = app.tabs.get_mut(app.active) {
                    t.prev_diff_file();
                }
            }
        }
        // P44-7：在多个文件中查找（⌘⇧F，BC 文本编辑搜索菜单；已有 P37-1n 弹窗补入口）
        if matches!(app.tabs.get(app.active), Some(Tab::TextEdit(_)))
            && menu_item(ui, t(I18nKey::MenuFindInFiles), sc("⌘⇧F", "Ctrl+Shift+F")).clicked()
        {
            ui.close();
            if let Some(Tab::TextEdit(t)) = app.tabs.get_mut(app.active) {
                t.open_find_in_files();
            }
        }
    });
}

/// View：设置 / 统计栏 / 缩略图（主题与语言切换已移入设置对话框，对标 BC 设置集中管理）
fn view_menu(app: &mut DiffApp, ui: &mut egui::Ui) {
    ui.menu_button(t(I18nKey::MenuView), |ui| {
        // P39-2a：设置…（⌘,）集中管理对话框
        if menu_item(ui, t(I18nKey::MenuSettings), sc("⌘,", "Ctrl+,")).clicked() {
            ui.close();
            app.show_settings = true;
        }
        ui.separator();
        // P39-2e：忽略不重要差异（空白/行尾/大小写一键切换，对标 BC View>Ignore Minor）
        let minor = app.tabs.get(app.active).and_then(|t| match t {
            super::Tab::Diff(tab) => Some(
                tab.opts.ignore_whitespace
                    && tab.opts.ignore_trailing
                    && tab.opts.ignore_case
                    && tab.opts.ignore_crlf,
            ),
            _ => None,
        });
        if let Some(m) = minor {
            let mut mm = m;
            if ui.checkbox(&mut mm, t(I18nKey::IgnoreMinor)).changed() {
                if let super::Tab::Diff(tab) = &mut app.tabs[app.active] {
                    tab.opts.ignore_whitespace = mm;
                    tab.opts.ignore_trailing = mm;
                    tab.opts.ignore_case = mm;
                    tab.opts.ignore_crlf = mm;
                    tab.recompute();
                }
            }
            // P40-1：单项忽略选项（原工具栏 checkbox，收进 View 菜单）
            if let super::Tab::Diff(tab) = &mut app.tabs[app.active] {
                let mut iw = tab.opts.ignore_whitespace;
                let mut it = tab.opts.ignore_trailing;
                let mut ic = tab.opts.ignore_case;
                let mut icr = tab.opts.ignore_crlf;
                let mut changed = false;
                if ui.checkbox(&mut iw, t(I18nKey::IgnoreWs)).changed() {
                    tab.opts.ignore_whitespace = iw;
                    changed = true;
                }
                if ui.checkbox(&mut it, t(I18nKey::IgnoreTrailing)).changed() {
                    tab.opts.ignore_trailing = it;
                    changed = true;
                }
                if ui.checkbox(&mut ic, t(I18nKey::IgnoreCase)).changed() {
                    tab.opts.ignore_case = ic;
                    changed = true;
                }
                if ui
                    .checkbox(&mut icr, t(I18nKey::SettingsIgnoreCrlf))
                    .changed()
                {
                    tab.opts.ignore_crlf = icr;
                    changed = true;
                }
                if changed {
                    tab.recompute();
                }
            }
            ui.separator();
            // P40-1：显示选项（原工具栏 checkbox，收进 View 菜单）
            if let super::Tab::Diff(tab) = &mut app.tabs[app.active] {
                let mut wrap = tab.wrap;
                let mut ws = tab.show_whitespace;
                if ui.checkbox(&mut wrap, t(I18nKey::WordWrap)).changed() {
                    tab.wrap = wrap;
                }
                if ui.checkbox(&mut ws, t(I18nKey::VisibleWs)).changed() {
                    tab.show_whitespace = ws;
                }
                // P42-3：字符列标尺
                let mut ruler = tab.show_ruler;
                if ui.checkbox(&mut ruler, t(I18nKey::ShowRuler)).changed() {
                    tab.show_ruler = ruler;
                }
            }
            // P40-1：hex 显示选项（原工具栏 ComboBox，收进 View 菜单）
            if let super::Tab::Diff(tab) = &mut app.tabs[app.active] {
                if let Some(h) = tab.hex.as_mut() {
                    ui.separator();
                    ui.checkbox(&mut h.show_addr, t(I18nKey::HexShowAddr));
                    let cur_addr_hex = h.addr_hex;
                    egui::ComboBox::from_id_salt("view_hex_addr_fmt")
                        .selected_text(if cur_addr_hex {
                            t(I18nKey::HexAddrHex)
                        } else {
                            t(I18nKey::HexAddrDec)
                        })
                        .show_ui(ui, |ui| {
                            if ui
                                .selectable_label(cur_addr_hex, t(I18nKey::HexAddrHex))
                                .clicked()
                            {
                                h.addr_hex = true;
                            }
                            if ui
                                .selectable_label(!cur_addr_hex, t(I18nKey::HexAddrDec))
                                .clicked()
                            {
                                h.addr_hex = false;
                            }
                        });
                    use crate::hexview::HexValueMode;
                    let cur = h.value_mode;
                    let label = match cur {
                        HexValueMode::Raw => t(I18nKey::HexValRaw),
                        HexValueMode::LittleEndian => t(I18nKey::HexValLittle),
                        HexValueMode::BigEndian => t(I18nKey::HexValBig),
                    };
                    egui::ComboBox::from_id_salt("view_hex_val_fmt")
                        .selected_text(label)
                        .show_ui(ui, |ui| {
                            for (mode, k) in [
                                (HexValueMode::Raw, I18nKey::HexValRaw),
                                (HexValueMode::LittleEndian, I18nKey::HexValLittle),
                                (HexValueMode::BigEndian, I18nKey::HexValBig),
                            ] {
                                if ui.selectable_label(cur == mode, t(k)).clicked() {
                                    h.value_mode = mode;
                                }
                            }
                        });
                    // P46-3：hex 视图过滤（BC 16进制 显示全部/差异/相同，1/2/3）
                    let cur_f = tab.hex_filter;
                    ui.label(t(I18nKey::MenuFilter));
                    for (f, k) in [
                        (super::difftab::HexViewFilter::All, I18nKey::HexFilterAll),
                        (super::difftab::HexViewFilter::Diff, I18nKey::HexFilterDiff),
                        (super::difftab::HexViewFilter::Same, I18nKey::HexFilterSame),
                    ] {
                        if ui.selectable_label(cur_f == f, t(k)).clicked() {
                            tab.hex_filter = f;
                        }
                    }
                    // P46-3：hex 布局（BC 16进制 边并排/上-下）
                    let cur_l = tab.hex_layout;
                    for (l, k) in [
                        (
                            super::difftab::HexViewLayout::SideBySide,
                            I18nKey::HexLayoutSideBySide,
                        ),
                        (
                            super::difftab::HexViewLayout::TopBottom,
                            I18nKey::HexLayoutTopBottom,
                        ),
                    ] {
                        if ui.selectable_label(cur_l == l, t(k)).clicked() {
                            tab.hex_layout = l;
                        }
                    }
                }
            }
        }
        // 统计栏 / 缩略图（当前文本标签）
        if ui.button(t(I18nKey::MenuStats)).clicked() {
            ui.close();
            with_diff_tab(app, |tab| tab.show_stats = !tab.show_stats);
        }
        if ui.button(t(I18nKey::MenuThumb)).clicked() {
            ui.close();
            with_diff_tab(app, |tab| tab.show_overview = !tab.show_overview);
        }
        // P46-1：TextEdit 视图开关（BC 文本编辑视图菜单 行号/自动换行/文件信息）
        if matches!(app.tabs.get(app.active), Some(Tab::TextEdit(_))) {
            let mut show_ln = app
                .tabs
                .get(app.active)
                .and_then(|t| match t {
                    Tab::TextEdit(tab) => Some(tab.show_line_numbers),
                    _ => None,
                })
                .unwrap_or(true);
            if ui
                .checkbox(&mut show_ln, t(I18nKey::MenuLineNumbers))
                .changed()
            {
                if let Tab::TextEdit(tab) = &mut app.tabs[app.active] {
                    tab.show_line_numbers = show_ln;
                }
            }
            let mut show_wrap = app
                .tabs
                .get(app.active)
                .and_then(|t| match t {
                    Tab::TextEdit(tab) => Some(tab.show_wrap),
                    _ => None,
                })
                .unwrap_or(false);
            if ui.checkbox(&mut show_wrap, t(I18nKey::WordWrap)).changed() {
                if let Tab::TextEdit(tab) = &mut app.tabs[app.active] {
                    tab.show_wrap = show_wrap;
                }
            }
            let mut show_fi = app
                .tabs
                .get(app.active)
                .and_then(|t| match t {
                    Tab::TextEdit(tab) => Some(tab.show_file_info),
                    _ => None,
                })
                .unwrap_or(true);
            if ui
                .checkbox(&mut show_fi, t(I18nKey::MenuFileInfo))
                .changed()
            {
                if let Tab::TextEdit(tab) = &mut app.tabs[app.active] {
                    tab.show_file_info = show_fi;
                }
            }
        }
        // P45-4：图片比较补齐（BC View 菜单 重置差异偏移/比较元数据，ImageTab 分支）
        if matches!(app.tabs.get(app.active), Some(Tab::Image(_))) {
            if ui.button(t(I18nKey::ImgResetOffset)).clicked() {
                ui.close();
                if let Some(Tab::Image(t)) = app.tabs.get_mut(app.active) {
                    t.reset_diff_offset();
                }
            }
            if ui.button(t(I18nKey::ImgCompareMeta)).clicked() {
                ui.close();
                if let Some(Tab::Image(t)) = app.tabs.get_mut(app.active) {
                    t.compare_meta();
                }
            }
        }
        // P44-6：行号 / 语法加亮开关（BC 视图菜单；DiffTab 分支）
        if matches!(app.tabs.get(app.active), Some(Tab::Diff(_))) {
            let show_ln = app
                .tabs
                .get(app.active)
                .and_then(|t| match t {
                    Tab::Diff(tab) => Some(tab.show_line_numbers),
                    _ => None,
                })
                .unwrap_or(true);
            if ui
                .checkbox(&mut show_ln.clone(), t(I18nKey::MenuLineNumbers))
                .changed()
            {
                if let Tab::Diff(tab) = &mut app.tabs[app.active] {
                    tab.show_line_numbers = show_ln;
                }
            }
            let show_syn = app
                .tabs
                .get(app.active)
                .and_then(|t| match t {
                    Tab::Diff(tab) => Some(tab.show_syntax),
                    _ => None,
                })
                .unwrap_or(true);
            if ui
                .checkbox(&mut show_syn.clone(), t(I18nKey::MenuSyntaxHighlight))
                .changed()
            {
                if let Tab::Diff(tab) = &mut app.tabs[app.active] {
                    tab.show_syntax = show_syn;
                }
            }
        }
        ui.separator();
        // P42-4：图例 / 日志 / 工具栏开关（BC 视图菜单）
        if menu_item(ui, t(I18nKey::MenuLegend), sc("⇧L", "Shift+L")).clicked() {
            ui.close();
            app.show_legend = !app.show_legend;
        }
        if ui.button(t(I18nKey::MenuLog)).clicked() {
            ui.close();
            app.show_log = !app.show_log;
        }
        let mut tb = crate::gui::common::SHOW_TOOLBAR.load(std::sync::atomic::Ordering::Relaxed);
        if ui.checkbox(&mut tb, t(I18nKey::MenuToolbar)).changed() {
            crate::gui::common::SHOW_TOOLBAR.store(tb, std::sync::atomic::Ordering::Relaxed);
        }
        ui.separator();
        // P45-2：文件夹合并视图过滤（BC View 菜单 显示全部/更改/冲突/左变/右变/可合并/未变化，1-7）
        if matches!(app.tabs.get(app.active), Some(Tab::FolderMerge(_))) {
            ui.label(t(I18nKey::MenuFilter));
            let cur = app
                .tabs
                .get(app.active)
                .and_then(|t| match t {
                    Tab::FolderMerge(tab) => Some(tab.view_filter),
                    _ => None,
                })
                .unwrap_or(super::foldermergetab::MergeFilter::All);
            for (f, key) in [
                (
                    super::foldermergetab::MergeFilter::All,
                    I18nKey::MergeFilterAll,
                ),
                (
                    super::foldermergetab::MergeFilter::Changed,
                    I18nKey::MergeFilterChanged,
                ),
                (
                    super::foldermergetab::MergeFilter::Conflict,
                    I18nKey::MergeFilterConflict,
                ),
                (
                    super::foldermergetab::MergeFilter::LeftChanged,
                    I18nKey::MergeFilterLeftChanged,
                ),
                (
                    super::foldermergetab::MergeFilter::RightChanged,
                    I18nKey::MergeFilterRightChanged,
                ),
                (
                    super::foldermergetab::MergeFilter::Mergeable,
                    I18nKey::MergeFilterMergeable,
                ),
                (
                    super::foldermergetab::MergeFilter::Unchanged,
                    I18nKey::MergeFilterUnchanged,
                ),
            ] {
                if ui.selectable_label(cur == f, t(key)).clicked() {
                    if let Tab::FolderMerge(tab) = &mut app.tabs[app.active] {
                        tab.view_filter = f;
                    }
                    ui.close();
                }
            }
            ui.separator();
        }
        // P45-3：文件夹比较视图过滤扩展（BC View 菜单 显示独有/不独有/差异但无独有/组合项）
        if matches!(app.tabs.get(app.active), Some(Tab::Dir(_))) {
            ui.label(t(I18nKey::MenuFilter));
            let cur = app
                .tabs
                .get(app.active)
                .and_then(|t| match t {
                    Tab::Dir(tab) => Some(tab.view_filter),
                    _ => None,
                })
                .unwrap_or(super::dirtab::ViewFilter::All);
            for (f, key) in [
                (super::dirtab::ViewFilter::All, I18nKey::ShowAll),
                (super::dirtab::ViewFilter::Diff, I18nKey::OnlyDiff),
                (super::dirtab::ViewFilter::Same, I18nKey::ShowSame),
                (
                    super::dirtab::ViewFilter::Orphans,
                    I18nKey::DirFilterOrphans,
                ),
                (
                    super::dirtab::ViewFilter::NonOrphans,
                    I18nKey::DirFilterNonOrphans,
                ),
                (
                    super::dirtab::ViewFilter::DiffNoOrphans,
                    I18nKey::DirFilterDiffNoOrphans,
                ),
                (super::dirtab::ViewFilter::LeftNewer, I18nKey::ViewLeftNewer),
                (
                    super::dirtab::ViewFilter::RightNewer,
                    I18nKey::ViewRightNewer,
                ),
                (
                    super::dirtab::ViewFilter::LeftNewerOrOrphan,
                    I18nKey::DirFilterLeftNewerOrOrphan,
                ),
                (
                    super::dirtab::ViewFilter::RightNewerOrOrphan,
                    I18nKey::DirFilterRightNewerOrOrphan,
                ),
            ] {
                if ui.selectable_label(cur == f, t(key)).clicked() {
                    if let Tab::Dir(tab) = &mut app.tabs[app.active] {
                        tab.view_filter = f;
                        tab.rebuild_tree();
                    }
                    ui.close();
                }
            }
            // P46-4：结构选项（BC 文件夹比较视图菜单 总是显示文件夹/仅比较文件）
            let mut show_dirs = app
                .tabs
                .get(app.active)
                .and_then(|t| match t {
                    Tab::Dir(tab) => Some(tab.show_all_dirs),
                    _ => None,
                })
                .unwrap_or(true);
            if ui
                .checkbox(&mut show_dirs, t(I18nKey::DirShowAllDirs))
                .changed()
            {
                if let Tab::Dir(tab) = &mut app.tabs[app.active] {
                    tab.show_all_dirs = show_dirs;
                    tab.rebuild_tree();
                }
            }
            let mut only_files = app
                .tabs
                .get(app.active)
                .and_then(|t| match t {
                    Tab::Dir(tab) => Some(tab.only_files),
                    _ => None,
                })
                .unwrap_or(false);
            if ui
                .checkbox(&mut only_files, t(I18nKey::DirOnlyFiles))
                .changed()
            {
                if let Tab::Dir(tab) = &mut app.tabs[app.active] {
                    tab.only_files = only_files;
                    tab.rebuild_tree();
                }
            }
            ui.separator();
        }
        // P39-2d：细节三模式（BC 视图菜单「细节」）
        ui.label(t(I18nKey::MenuDetail));
        let cur_detail = app.tabs.get(app.active).and_then(|t| match t {
            super::Tab::Diff(tab) => Some(tab.detail_mode),
            _ => None,
        });
        if let Some(cur) = cur_detail {
            for (mode, key) in [
                (super::difftab::DiffDetailMode::Text, I18nKey::DetailText),
                (super::difftab::DiffDetailMode::Hex, I18nKey::DetailHex),
                (super::difftab::DiffDetailMode::Align, I18nKey::DetailAlign),
            ] {
                if ui.selectable_label(cur == mode, t(key)).clicked() {
                    if let super::Tab::Diff(tab) = &mut app.tabs[app.active] {
                        tab.set_detail_mode(mode);
                    }
                    ui.close();
                }
            }
        }
        ui.separator();
        // P39-2d：布局（BC 视图菜单「布局」）
        ui.label(t(I18nKey::MenuLayout));
        let cur_layout = app.tabs.get(app.active).and_then(|t| match t {
            super::Tab::Diff(tab) => Some(tab.layout),
            _ => None,
        });
        if let Some(cur) = cur_layout {
            for (layout, key) in [
                (
                    super::difftab::DiffLayout::SideBySide,
                    I18nKey::LayoutSideBySide,
                ),
                (
                    super::difftab::DiffLayout::TopBottom,
                    I18nKey::LayoutTopBottom,
                ),
                (super::difftab::DiffLayout::Web, I18nKey::LayoutWeb),
            ] {
                if ui.selectable_label(cur == layout, t(key)).clicked() {
                    if let super::Tab::Diff(tab) = &mut app.tabs[app.active] {
                        tab.set_layout(layout);
                    }
                    ui.close();
                }
            }
        }
        ui.separator();
        // P39-2d：书签（BC 书签 0-9，⌘⌥⌃0-9 切换 / ⌘0-9 跳转）
        ui.label(t(I18nKey::MenuBookmark));
        if menu_item(
            ui,
            t(I18nKey::MenuToggleBookmark),
            sc("⌘⌥⌃0-9", "Ctrl+Alt+Shift+0-9"),
        )
        .clicked()
        {
            ui.close();
            with_diff_tab(app, |tab| tab.toggle_bookmark(0));
        }
        if menu_item(ui, t(I18nKey::MenuGotoBookmark), sc("⌘0-9", "Ctrl+0-9")).clicked() {
            ui.close();
            with_diff_tab(app, |tab| tab.goto_bookmark(0));
        }
        if ui.button(t(I18nKey::MenuClearBookmarks)).clicked() {
            ui.close();
            with_diff_tab(app, |tab| tab.clear_bookmarks());
        }
    });
}

/// Tools：Git 配置 / 会话中心 / 规则 / 云盘 / 外部工具
fn tools_menu(app: &mut DiffApp, ui: &mut egui::Ui) {
    ui.menu_button(t(I18nKey::MenuTools), |ui| {
        if ui.button(t(I18nKey::MenuGit)).clicked() {
            ui.close();
            app.show_git_help = true;
        }
        if ui.button(t(I18nKey::MenuSaveSession)).clicked() {
            ui.close();
            app.show_sessions = true;
        }
        if ui.button(t(I18nKey::MenuExternal)).clicked() {
            ui.close();
            app.show_external = true;
        }
        ui.separator();
        // P39-2e：保存快照（保存当前标签为命名会话，对标 BC Tools>保存快照）
        if ui.button(t(I18nKey::MenuSnapshot)).clicked() {
            ui.close();
            app.show_sessions = true;
        }
        if ui.button("💬 会话中心").clicked() {
            ui.close();
            app.show_sessions = true;
        }
        if ui.button("⚙ 规则").clicked() {
            ui.close();
            app.show_profiles = true;
        }
        if ui.button(t(I18nKey::MenuOpenCloud)).clicked() {
            ui.close();
            app.show_cloud = true;
        }
        ui.separator();
        // P44-5：导出/导入设置 + 恢复出厂默认（BC Tools 菜单）
        if ui.button(t(I18nKey::MenuExportSettings)).clicked() {
            ui.close();
            if let Some(p) = rfd::FileDialog::new()
                .set_file_name("bcr-settings.toml")
                .save_file()
            {
                if let Err(e) = app.settings.export_to(&p) {
                    app.report_error = Some(e);
                }
            }
        }
        if ui.button(t(I18nKey::MenuImportSettings)).clicked() {
            ui.close();
            if let Some(p) = rfd::FileDialog::new().pick_file() {
                if let Err(e) = app.settings.import_from(&p) {
                    app.report_error = Some(e);
                }
            }
        }
        if ui.button(t(I18nKey::MenuResetDefaults)).clicked() {
            ui.close();
            app.settings.reset_defaults();
        }
        ui.separator();
        // P44-5：编辑文本文件 / 查看补丁（BC Tools 菜单入口，对应 TextEdit/PatchTab 视图）
        if ui.button(t(I18nKey::MenuEditTextFile)).clicked() {
            ui.close();
            app.add_tab(Tab::TextEdit(super::textedit::TextEditTab::new("")));
        }
        if ui.button(t(I18nKey::MenuViewPatch)).clicked() {
            ui.close();
            app.add_tab(Tab::Patch(super::patchtab::PatchTab::new("")));
        }
    });
}

/// P44-1：窗口菜单（BC Window>选择下一/上一标签页/最小化/关闭所有窗口）
fn window_menu(app: &mut DiffApp, ui: &mut egui::Ui) {
    ui.menu_button(t(I18nKey::MenuWindow), |ui| {
        if app.tabs.is_empty() {
            ui.add_enabled(false, egui::Button::new(t(I18nKey::MenuNextTab)));
            ui.add_enabled(false, egui::Button::new(t(I18nKey::MenuPrevTab)));
            ui.add_enabled(false, egui::Button::new(t(I18nKey::MenuCloseAllWindows)));
            return;
        }
        if menu_item(ui, t(I18nKey::MenuNextTab), sc("⌘]", "Ctrl+]")).clicked() {
            ui.close();
            app.next_tab();
        }
        if menu_item(ui, t(I18nKey::MenuPrevTab), sc("⌘[", "Ctrl+[")).clicked() {
            ui.close();
            app.prev_tab();
        }
        ui.separator();
        if menu_item(ui, t(I18nKey::MenuMinimize), sc("⌘M", "Ctrl+M")).clicked() {
            ui.close();
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::Minimized(true));
        }
        if menu_item(
            ui,
            t(I18nKey::MenuCloseAllWindows),
            sc("⌘⇧W", "Ctrl+Shift+W"),
        )
        .clicked()
        {
            ui.close();
            app.close_all_tabs();
        }
    });
}

/// Help：快捷键 / 关于
fn help_menu(app: &mut DiffApp, ui: &mut egui::Ui) {
    ui.menu_button(t(I18nKey::MenuHelp), |ui| {
        if ui.button(t(I18nKey::MenuShortcuts)).clicked() {
            ui.close();
            app.show_shortcuts = true;
        }
        if ui.button(t(I18nKey::MenuAbout)).clicked() {
            ui.close();
            app.show_about = true;
        }
    });
}
