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

/// Session：新建各类会话 + 保存会话
fn session_menu(app: &mut DiffApp, ui: &mut egui::Ui) {
    ui.menu_button(t(I18nKey::MenuSession), |ui| {
        if ui.button(t(I18nKey::MenuNewText)).clicked() {
            ui.close();
            app.open_diff_files();
        }
        if ui.button(t(I18nKey::MenuNewDir)).clicked() {
            ui.close();
            app.open_dir_compare();
        }
        if ui.button(t(I18nKey::MenuNewMerge)).clicked() {
            ui.close();
            app.open_merge();
        }
        if ui.button(t(I18nKey::MenuNewImage)).clicked() {
            ui.close();
            app.open_image_compare();
        }
        if ui.button(t(I18nKey::MenuNewCsv)).clicked() {
            ui.close();
            app.open_csv_compare();
        }
        // Hex 会话：打开文件对比（二进制文件自动切 hex 视图）
        if ui.button(t(I18nKey::MenuNewHex)).clicked() {
            ui.close();
            app.open_diff_files();
        }
        ui.separator();
        // 保存会话：打开会话中心（GUI 内管理已保存会话）
        if ui.button(t(I18nKey::MenuSaveSession)).clicked() {
            ui.close();
            app.show_sessions = true;
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
        if ui.button(t(I18nKey::MenuFind)).clicked() {
            ui.close();
            with_diff_tab(app, |tab| tab.focus_search());
        }
    });
}

/// Search：查找 / 下一差异 / 上一差异（转发当前标签）
fn search_menu(app: &mut DiffApp, ui: &mut egui::Ui) {
    ui.menu_button(t(I18nKey::MenuSearch), |ui| {
        if ui.button(t(I18nKey::MenuFind)).clicked() {
            ui.close();
            with_diff_tab(app, |tab| tab.focus_search());
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
        ui.separator();
        if ui.button(t(I18nKey::MenuReload)).clicked() {
            ui.close();
            with_diff_tab(app, |tab| tab.reload());
        }
    });
}

/// View：主题 / 语言 / 统计栏 / 缩略图
fn view_menu(app: &mut DiffApp, ui: &mut egui::Ui) {
    ui.menu_button(t(I18nKey::MenuView), |ui| {
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
            if ui.selectable_label(new_lang == l, l.native_name()).clicked() {
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
        // 统计栏 / 缩略图（当前文本标签）
        if ui.button(t(I18nKey::MenuStats)).clicked() {
            ui.close();
            with_diff_tab(app, |tab| tab.show_stats = !tab.show_stats);
        }
        if ui.button(t(I18nKey::MenuThumb)).clicked() {
            ui.close();
            with_diff_tab(app, |tab| tab.show_overview = !tab.show_overview);
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
