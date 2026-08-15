//! P33 标准菜单栏：对标 Beyond Compare 的
//! `Session | File | Edit | Search | View | Tools | Help` 菜单结构。
//!
//! - 所有入口复用现有 DiffApp 方法（打开会话/弹窗/转发当前标签）
//! - 语言/主题切换移入 View 菜单（BC 观感：设置类操作收进菜单，不再占工具栏）

use eframe::egui::{self, ThemePreference};

use super::{DiffApp, Tab};
use crate::i18n::{self, t, Key as I18nKey};

/// 顶部菜单栏（BC 式 7 个主菜单）
pub fn menu_bar(app: &mut DiffApp, ui: &mut egui::Ui) {
    egui::MenuBar::new().ui(ui, |ui| {
        session_menu(app, ui);
        file_menu(app, ui);
        edit_menu(app, ui);
        search_menu(app, ui);
        view_menu(app, ui);
        tools_menu(app, ui);
        help_menu(app, ui);
    });
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
        // Hex 会话：空文本对比会话（二进制文件自动切 hex 视图）
        if ui.button(t(I18nKey::MenuNewHex)).clicked() {
            ui.close();
            app.open_empty_diff();
        }
        ui.separator();
        // P39-2a：新建标签页 / 新建窗口（BC Session 菜单）
        if ui.button(t(I18nKey::MenuNewTab)).clicked() {
            ui.close();
            app.new_tab_like_current();
        }
        if ui.button(t(I18nKey::MenuNewWindow)).clicked() {
            ui.close();
            super::DiffApp::open_new_window();
        }
        ui.separator();
        // 保存会话：打开会话中心（GUI 内管理已保存会话）
        if ui.button(t(I18nKey::MenuSaveSession)).clicked() {
            ui.close();
            app.show_sessions = true;
        }
        // P39-2a：清除会话（重置当前标签为空会话）
        if ui.button(t(I18nKey::MenuClearSession)).clicked() {
            ui.close();
            app.clear_active_tab();
        }
        ui.separator();
        // P39-2c：报告生成（⌘P）
        if ui.button(t(I18nKey::MenuReport)).clicked() {
            ui.close();
            app.show_report = true;
            app.report_error = None;
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
        ui.separator();
        if ui.button(t(I18nKey::MenuOpenCloud)).clicked() {
            ui.close();
            app.show_cloud = true;
        }
    });
}

/// Edit：撤销/重做（转发当前文本标签）
fn edit_menu(app: &mut DiffApp, ui: &mut egui::Ui) {
    ui.menu_button(t(I18nKey::MenuEdit), |ui| {
        if ui.button(t(I18nKey::MenuUndo)).clicked() {
            ui.close();
            with_diff_tab(app, |tab| tab.undo());
        }
        if ui.button(t(I18nKey::MenuRedo)).clicked() {
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
            ui.separator();
            // P42-1：转换文件（BC Edit>Convert File，DiffTab 分支）
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
        if ui.button(t(I18nKey::MenuFind)).clicked() {
            ui.close();
            with_diff_tab(app, |tab| tab.focus_search());
        }
        // P39-2e：替换…（⇧⌘F）
        if ui.button(t(I18nKey::MenuReplace)).clicked() {
            ui.close();
            with_diff_tab(app, |tab| tab.focus_replace());
        }
        // P39-2a：查找下一 / 上一（⌘G / ⇧⌘G）
        if ui.button(t(I18nKey::MenuFindNext)).clicked() {
            ui.close();
            with_diff_tab(app, |tab| tab.next_match());
        }
        if ui.button(t(I18nKey::MenuFindPrev)).clicked() {
            ui.close();
            with_diff_tab(app, |tab| tab.prev_match());
        }
        // P39-2a：转到行…（⌘L）
        if ui.button(t(I18nKey::MenuGotoLine)).clicked() {
            ui.close();
            with_diff_tab(app, |tab| tab.goto_focus = true);
        }
        ui.separator();
        if ui.button(t(I18nKey::MenuNextDiff)).clicked() {
            ui.close();
            with_diff_tab(app, |tab| tab.next_diff());
        }
        if ui.button(t(I18nKey::MenuPrevDiff)).clicked() {
            ui.close();
            with_diff_tab(app, |tab| tab.prev_diff());
        }
        // P39-2c：差异部分导航（区块级跳转，BC ⇧⌃↓/↑）
        if ui.button(t(I18nKey::MenuNextSection)).clicked() {
            ui.close();
            with_diff_tab(app, |tab| tab.next_diff_section());
        }
        if ui.button(t(I18nKey::MenuPrevSection)).clicked() {
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
        ui.separator();
        if ui.button(t(I18nKey::MenuReload)).clicked() {
            ui.close();
            with_diff_tab(app, |tab| tab.reload());
        }
    });
}

/// View：主题 / 语言 / 设置 / 统计栏 / 缩略图
fn view_menu(app: &mut DiffApp, ui: &mut egui::Ui) {
    ui.menu_button(t(I18nKey::MenuView), |ui| {
        // P39-2a：设置…（⌘,）集中管理对话框
        if ui.button(t(I18nKey::MenuSettings)).clicked() {
            ui.close();
            app.show_settings = true;
        }
        ui.separator();
        // 主题（系统/深色/浅色）
        ui.label(t(I18nKey::Theme));
        let mut pref = app.settings.theme_pref();
        let mut changed = false;
        for (key, p) in [
            (I18nKey::ThemeSystem, ThemePreference::System),
            (I18nKey::ThemeDark, ThemePreference::Dark),
            (I18nKey::ThemeLight, ThemePreference::Light),
        ] {
            if ui.selectable_label(pref == p, t(key)).clicked() {
                pref = p;
                changed = true;
            }
        }
        if changed {
            app.settings.theme = match pref {
                ThemePreference::Dark => "dark".to_string(),
                ThemePreference::Light => "light".to_string(),
                _ => "system".to_string(),
            };
            app.settings.save();
            ui.ctx().set_theme(pref);
        }
        ui.separator();
        // 语言（10 语言列表）
        ui.label(t(I18nKey::Language));
        let mut new_lang = i18n::current();
        let mut lang_changed = false;
        for l in i18n::Lang::ALL {
            if ui
                .selectable_label(new_lang == l, l.native_name())
                .clicked()
            {
                new_lang = l;
                lang_changed = true;
            }
        }
        if lang_changed {
            app.settings.lang = new_lang.code().to_string();
            app.settings.save();
            i18n::set_lang(new_lang);
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
        ui.separator();
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
        if ui.button(t(I18nKey::MenuToggleBookmark)).clicked() {
            ui.close();
            with_diff_tab(app, |tab| tab.toggle_bookmark(0));
        }
        if ui.button(t(I18nKey::MenuGotoBookmark)).clicked() {
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
