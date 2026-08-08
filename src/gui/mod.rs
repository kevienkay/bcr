//! M5 GUI：多标签并排 Diff / 目录对比 / 三路合并视图。
//!
//! - 多标签页：DiffTab（并排 diff）、DirTab（目录对比）、MergeTab（三路合并）
//! - 虚拟化渲染（ScrollArea::show_rows）支持超大文件
//! - 搜索、差异/冲突跳转、行号跳转快捷键
//! - 主题切换 + 设置持久化（TOML）

mod common;
mod difftab;
mod dirtab;
mod mergetab;

use common::*;
use difftab::DiffTab;
use dirtab::DirTab;
use mergetab::MergeTab;
use crate::sideview::ViewOptions;
use eframe::egui::{self, RichText, ThemePreference};
use std::path::PathBuf;

/// GUI 子命令参数
#[derive(clap::Args, Debug)]
pub struct GuiArgs {
    /// 左侧文件/目录（与 RIGHT 组成并排 diff 或目录对比）
    pub left: Option<String>,

    /// 右侧文件/目录
    pub right: Option<String>,

    /// 三路合并：BASE LEFT RIGHT（三个位置参数）
    #[arg(long = "merge", num_args = 3, value_names = ["BASE", "LEFT", "RIGHT"])]
    pub merge: Option<Vec<String>>,

    /// 忽略所有空白差异
    #[arg(long)]
    pub ignore_whitespace: bool,

    /// 忽略行尾空白差异
    #[arg(long)]
    pub ignore_trailing: bool,

    /// 忽略大小写差异
    #[arg(long)]
    pub ignore_case: bool,
}

/// 标签页
enum Tab {
    Diff(DiffTab),
    Dir(DirTab),
    Merge(MergeTab),
}

impl Tab {
    fn title(&self) -> String {
        match self {
            Tab::Diff(t) => t.title(),
            Tab::Dir(t) => t.title(),
            Tab::Merge(t) => t.title(),
        }
    }
}

/// 持久化设置
#[derive(serde::Serialize, serde::Deserialize, Default)]
struct Settings {
    theme: String, // "system" | "dark" | "light"
    /// 语言代码（"zh"/"en"/...），空 = 跟随 CLI/环境
    #[serde(default)]
    lang: String,
    #[serde(default = "default_true")]
    show_stats: bool,
    #[serde(default)]
    ignore_whitespace: bool,
    #[serde(default)]
    ignore_trailing: bool,
    #[serde(default)]
    ignore_case: bool,
    #[serde(default)]
    window_size: Option<[f32; 2]>,
}

fn default_true() -> bool {
    true
}

impl Settings {
    /// 跨平台配置目录：macOS/Linux 用 $HOME，Windows 用 %USERPROFILE%
    fn path() -> PathBuf {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."));
        home.join(".bcr-gui.toml")
    }

    fn load() -> Self {
        std::fs::read_to_string(Self::path())
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn save(&self) {
        if let Ok(s) = toml::to_string(self) {
            let _ = std::fs::write(Self::path(), s);
        }
    }

    fn theme_pref(&self) -> ThemePreference {
        match self.theme.as_str() {
            "dark" => ThemePreference::Dark,
            "light" => ThemePreference::Light,
            _ => ThemePreference::System,
        }
    }

    /// GUI 语言：设置值 > 环境变量/CLI > 中文
    fn lang(&self) -> crate::i18n::Lang {
        crate::i18n::Lang::parse(&self.lang)
            .or_else(crate::i18n::Lang::from_env)
            .unwrap_or(crate::i18n::Lang::Zh)
    }
}

/// 主应用
struct DiffApp {
    tabs: Vec<Tab>,
    active: usize,
    settings: Settings,
    show_git_help: bool,
}

impl DiffApp {
    fn new(settings: Settings) -> Self {
        DiffApp {
            tabs: Vec::new(),
            active: 0,
            settings,
            show_git_help: false,
        }
    }

    fn add_tab(&mut self, tab: Tab) {
        self.tabs.push(tab);
        self.active = self.tabs.len() - 1;
    }

    fn close_tab(&mut self, idx: usize) {
        if idx >= self.tabs.len() {
            return;
        }
        self.tabs.remove(idx);
        if self.active >= self.tabs.len() {
            self.active = self.tabs.len().saturating_sub(1);
        }
    }

    fn new_diff_tab(&mut self) {
        let mut t = DiffTab::new();
        t.opts = ViewOptions {
            ignore_whitespace: self.settings.ignore_whitespace,
            ignore_trailing: self.settings.ignore_trailing,
            ignore_case: self.settings.ignore_case,
        };
        t.show_stats = self.settings.show_stats;
        self.add_tab(Tab::Diff(t));
    }

    fn handle_dropped(&mut self, ctx: &egui::Context) {
        let dropped: Vec<std::path::PathBuf> = ctx
            .input(|i| {
                i.raw
                    .dropped_files
                    .iter()
                    .map(|f| f.path().to_path_buf())
                    .collect()
            });
        if dropped.is_empty() {
            return;
        }
        // 目录拖入 → 目录对比（两目录）或加载
        let dirs: Vec<String> = dropped
            .iter()
            .filter(|p| p.is_dir())
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        let files: Vec<String> = dropped
            .iter()
            .filter(|p| p.is_file())
            .map(|p| p.to_string_lossy().into_owned())
            .collect();

        match (dirs.as_slice(), files.as_slice()) {
            (_, [..]) if files.len() >= 3 => {
                // 多个文件：尝试三路合并（前三个）
                if let Some(merge) = self.try_merge(&files) {
                    self.add_tab(Tab::Merge(merge));
                }
            }
            (_, [a]) => {
                // 单个文件：补到当前 diff 标签或新建
                self.drop_single_file(a);
            }
            (d1, []) if d1.len() >= 2 => {
                self.add_tab(Tab::Dir(DirTab::new(&d1[0], &d1[1])));
            }
            ([d], []) => {
                // 单个目录：打开目录选择第二侧
                if let Some(r) = pick_dir() {
                    self.add_tab(Tab::Dir(DirTab::new(d, &r)));
                }
            }
            _ => {}
        }
    }

    fn try_merge(&self, files: &[String]) -> Option<MergeTab> {
        if files.len() >= 3 {
            Some(MergeTab::new(&files[0], &files[1], &files[2]))
        } else {
            None
        }
    }

    fn drop_single_file(&mut self, path: &str) {
        // 若当前是 diff 标签且有一侧空 → 填充；否则新建 diff 标签
        if let Some(Tab::Diff(t)) = self.tabs.get_mut(self.active) {
            if t.left.is_none() {
                t.load_left(path, t.opts);
                return;
            }
            if t.right.is_none() {
                t.load_right(path, t.opts);
                return;
            }
        }
        let mut t = DiffTab::new();
        t.load_left(path, ViewOptions::default());
        self.add_tab(Tab::Diff(t));
    }

    fn open_diff_files(&mut self) {
        let Some(l) = pick_file() else { return };
        let Some(r) = pick_file() else { return };
        let mut t = DiffTab::new();
        t.load_pair(&l, &r, ViewOptions::default());
        self.add_tab(Tab::Diff(t));
    }

    fn open_dir_compare(&mut self) {
        let Some(l) = pick_dir() else { return };
        let Some(r) = pick_dir() else { return };
        self.add_tab(Tab::Dir(DirTab::new(&l, &r)));
    }

    fn open_merge(&mut self) {
        let Some(b) = pick_file() else { return };
        let Some(l) = pick_file() else { return };
        let Some(r) = pick_file() else { return };
        self.add_tab(Tab::Merge(MergeTab::new(&b, &l, &r)));
    }
}

fn pick_file() -> Option<String> {
    let p = rfd::FileDialog::new().pick_file()?;
    Some(p.to_string_lossy().into_owned())
}

fn pick_dir() -> Option<String> {
    let p = rfd::FileDialog::new().pick_folder()?;
    Some(p.to_string_lossy().into_owned())
}

impl eframe::App for DiffApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // 记录窗口大小用于退出时持久化
        if let Some(rect) = ui.ctx().input(|i| i.viewport().inner_rect) {
            self.settings.window_size = Some([rect.width(), rect.height()]);
        }

        self.handle_dropped(ui.ctx());

        // 顶部菜单栏
        egui::Panel::top("menu").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui.button(crate::i18n::t(crate::i18n::Key::MenuOpenFiles)).clicked() {
                    self.open_diff_files();
                }
                if ui.button(crate::i18n::t(crate::i18n::Key::MenuOpenDir)).clicked() {
                    self.open_dir_compare();
                }
                if ui.button(crate::i18n::t(crate::i18n::Key::MenuOpenMerge)).clicked() {
                    self.open_merge();
                }
                ui.separator();
                if ui.button(crate::i18n::t(crate::i18n::Key::MenuGit)).clicked() {
                    self.show_git_help = !self.show_git_help;
                }
                ui.separator();

                // 标签栏
                let mut close: Option<usize> = None;
                let mut activate: Option<usize> = None;
                for i in 0..self.tabs.len() {
                    let selected = i == self.active;
                    let resp = ui.selectable_label(selected, self.tabs[i].title());
                    if resp.clicked() {
                        activate = Some(i);
                    }
                    // 关闭按钮
                    let close_resp = ui
                        .small_button("✕")
                        .on_hover_text(crate::i18n::t(crate::i18n::Key::CloseTab));
                    if close_resp.clicked() {
                        close = Some(i);
                    }
                }
                if let Some(i) = activate {
                    self.active = i;
                }
                if let Some(i) = close {
                    self.close_tab(i);
                }
                if ui
                    .button("+")
                    .on_hover_text(crate::i18n::t(crate::i18n::Key::NewDiffTab))
                    .clicked()
                {
                    self.new_diff_tab();
                }

                ui.separator();
                // 语言切换
                let mut lang_changed = false;
                let mut new_lang = crate::i18n::current();
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::t(crate::i18n::Key::Language));
                    for l in crate::i18n::Lang::ALL {
                        if ui.selectable_label(new_lang == l, l.native_name()).clicked() {
                            new_lang = l;
                            lang_changed = true;
                        }
                    }
                });
                if lang_changed {
                    self.settings.lang = new_lang.code().to_string();
                    self.settings.save();
                    crate::i18n::set_lang(new_lang);
                }
                ui.separator();
                // 主题切换
                let mut pref = self.settings.theme_pref();
                let mut changed = false;
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::t(crate::i18n::Key::Theme));
                    for (key, p) in [
                        (crate::i18n::Key::ThemeSystem, ThemePreference::System),
                        (crate::i18n::Key::ThemeDark, ThemePreference::Dark),
                        (crate::i18n::Key::ThemeLight, ThemePreference::Light),
                    ] {
                        if ui.selectable_label(pref == p, crate::i18n::t(key)).clicked() {
                            pref = p;
                            changed = true;
                        }
                    }
                });
                if changed {
                    self.settings.theme = match pref {
                        ThemePreference::Dark => "dark".to_string(),
                        ThemePreference::Light => "light".to_string(),
                        _ => "system".to_string(),
                    };
                    self.settings.save();
                    ui.ctx().set_theme(pref);
                }
            });
        });

        // Git 配置帮助弹窗
        if self.show_git_help {
            let lines = [
                "[difftool \"bcr\"]",
                "\tcmd = bcr diff \"$LOCAL\" \"$REMOTE\" -L \"$LOCAL\" -L \"$REMOTE\"",
                "",
                "[mergetool \"bcr\"]",
                "\tcmd = bcr merge \"$BASE\" \"$LOCAL\" \"$REMOTE\" -o \"$MERGED\"",
            ];
            let config = lines.join("\n");
            egui::Window::new(crate::i18n::t(crate::i18n::Key::GitTitle))
                .collapsible(false)
                .default_size([560.0, 340.0])
                .show(ui.ctx(), |ui| {
                    ui.label(crate::i18n::t(crate::i18n::Key::GitDesc));
                    ui.add_space(4.0);
                    egui::ScrollArea::vertical()
                        .max_height(160.0)
                        .show(ui, |ui| {
                            for l in &lines {
                                ui.monospace(*l);
                            }
                        });
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        if ui.button(crate::i18n::t(crate::i18n::Key::GitCopy)).clicked() {
                            ui.ctx().copy_text(config.clone());
                            self.show_git_help = false;
                        }
                        if ui.button(crate::i18n::t(crate::i18n::Key::Close)).clicked() {
                            self.show_git_help = false;
                        }
                    });
                    ui.separator();
                    ui.label(crate::i18n::t(crate::i18n::Key::GitUsage));
                    ui.monospace("git difftool --tool=bcr");
                    ui.monospace("git mergetool --tool=bcr");
                    ui.label(crate::i18n::t(crate::i18n::Key::GitExit));
                });
        }

        // 当前标签内容
        if self.tabs.is_empty() {
            egui::CentralPanel::default().show(ui, |ui| {
                ui.centered_and_justified(|ui| {
                    ui.label(
                        RichText::new(crate::i18n::t(crate::i18n::Key::MainHint))
                            .size(18.0)
                            .color(ui.visuals().weak_text_color()),
                    );
                });
            });
            return;
        }

        // 处理各标签可能发出的请求
        let mut open_diff_req: Option<String> = None;
        {
            let active = self.active;
            match &mut self.tabs[active] {
                Tab::Diff(t) => t.ui(ui),
                Tab::Dir(t) => {
                    t.ui(ui);
                    open_diff_req = t.open_diff.take();
                }
                Tab::Merge(t) => t.ui(ui),
            }
        }

        if let Some(rel) = open_diff_req {
            // 目录对比双击 → 打开该文件的并排 diff（用 Path::join 保证跨平台分隔符）
            if let Some(Tab::Dir(dir_tab)) = self.tabs.get(self.active) {
                let (l, r) = (dir_tab.left.clone(), dir_tab.right.clone());
                let mut t = DiffTab::new();
                let l = std::path::Path::new(&l).join(&rel);
                let r = std::path::Path::new(&r).join(&rel);
                t.load_pair(
                    &l.to_string_lossy(),
                    &r.to_string_lossy(),
                    ViewOptions::default(),
                );
                self.add_tab(Tab::Diff(t));
            }
        }
    }

    fn on_exit(&mut self) {
        self.settings.save();
    }
}

/// 加载系统中文字体作为 fallback（egui 默认字体不含 CJK，三端中文 UI 都需要）。
/// 按平台探测常见中文字体路径，找到第一个存在的加载。
fn install_cjk_fonts(ctx: &egui::Context) {
    let candidates: &[&str] = if cfg!(target_os = "windows") {
        &[
            "C:\\Windows\\Fonts\\msyh.ttc", // 微软雅黑
            "C:\\Windows\\Fonts\\simhei.ttf", // 黑体
            "C:\\Windows\\Fonts\\simsun.ttc", // 宋体
        ]
    } else if cfg!(target_os = "macos") {
        &[
            "/System/Library/Fonts/PingFang.ttc",
            "/System/Library/Fonts/STHeiti Light.ttc",
            "/System/Library/Fonts/Hiragino Sans GB.ttc",
            "/Library/Fonts/Arial Unicode.ttf",
        ]
    } else {
        &[
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
            "/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc",
            "/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf",
        ]
    };
    let Some(path) = candidates.iter().find(|p| std::path::Path::new(p).exists()) else {
        return;
    };
    let Ok(bytes) = std::fs::read(path) else {
        return;
    };
    let mut fonts = egui::FontDefinitions::default();
    fonts
        .font_data
        .insert("cjk".to_owned(), egui::FontData::from_owned(bytes).into());
    // 追加到比例字体与等宽字体末尾作为 fallback，保留默认拉丁字体
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts.families.entry(family).or_default().push("cjk".to_owned());
    }
    ctx.set_fonts(fonts);
}

/// 运行 GUI 事件循环，返回进程退出码
pub fn run(args: &GuiArgs) -> i32 {
    let settings = Settings::load();
    let gui_lang = settings.lang();
    crate::i18n::set_lang(gui_lang);
    let win = settings.window_size.unwrap_or([1360.0, 860.0]);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([win[0], win[1]])
            .with_title(crate::i18n::t(crate::i18n::Key::WinTitle)),
        ..Default::default()
    };

    let theme_pref = settings.theme_pref();
    let mut app = DiffApp::new(settings);
    // CLI 参数优先；未指定时应用持久化忽略选项
    let mut opts = ViewOptions {
        ignore_whitespace: args.ignore_whitespace || app.settings.ignore_whitespace,
        ignore_trailing: args.ignore_trailing || app.settings.ignore_trailing,
        ignore_case: args.ignore_case || app.settings.ignore_case,
    };
    // CLI 显式参数优先于持久化值
    if args.ignore_whitespace {
        opts.ignore_whitespace = true;
    }
    if args.ignore_trailing {
        opts.ignore_trailing = true;
    }
    if args.ignore_case {
        opts.ignore_case = true;
    }
    let show_stats = app.settings.show_stats;

    // CLI 参数初始化标签
    if let Some(m) = &args.merge {
        if m.len() == 3 {
            app.add_tab(Tab::Merge(MergeTab::new(&m[0], &m[1], &m[2])));
        }
    } else {
        match (&args.left, &args.right) {
            (Some(l), Some(r)) => {
                let lp = std::path::Path::new(l);
                let rp = std::path::Path::new(r);
                if lp.is_dir() && rp.is_dir() {
                    app.add_tab(Tab::Dir(DirTab::new(l, r)));
                } else {
                    let mut t = DiffTab::new();
                    t.show_stats = show_stats;
                    t.load_pair(l, r, opts);
                    app.add_tab(Tab::Diff(t));
                }
            }
            (Some(l), None) => {
                let mut t = DiffTab::new();
                t.show_stats = show_stats;
                t.load_left(l, opts);
                app.add_tab(Tab::Diff(t));
            }
            (None, Some(r)) => {
                let mut t = DiffTab::new();
                t.show_stats = show_stats;
                t.load_right(r, opts);
                app.add_tab(Tab::Diff(t));
            }
            (None, None) => {}
        }
    }

    match eframe::run_native(
        "bcr",
        options,
        Box::new(move |cc| {
            install_cjk_fonts(&cc.egui_ctx);
            for theme in [egui::Theme::Dark, egui::Theme::Light] {
                cc.egui_ctx.style_mut_of(theme, |style| {
                    style
                        .text_styles
                        .insert(egui::TextStyle::Monospace, egui::FontId::monospace(FONT_SIZE));
                });
            }
            cc.egui_ctx.set_theme(theme_pref);
            Ok(Box::new(app))
        }),
    ) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("bcr: {}", crate::i18n::fmt(crate::i18n::Key::GuiFail, &[&e.to_string()]));
            2
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::difftab::{EditSide, EditState};
    use crate::mergeview::{render_merged, Resolution};
    use crate::sideview::RowTag;
    use std::fs;
    use tempfile::tempdir;

    fn write(dir: &std::path::Path, name: &str, content: &str) -> String {
        let p = dir.join(name);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&p, content).unwrap();
        p.to_str().unwrap().to_string()
    }

    // ---- DiffTab：加载/跳转/搜索/编辑状态 ----------------

    #[test]
    fn difftab_load_and_jump() {
        let d = tempdir().unwrap();
        // 大文件：跳转才会产生实际滚动偏移
        let mut big_l = String::new();
        let mut big_r = String::new();
        for i in 0..50 {
            big_l.push_str(&format!("l{i}\n"));
            big_r.push_str(&format!("r{i}\n"));
        }
        let l = write(d.path(), "l.txt", &big_l);
        let r = write(d.path(), "r.txt", &big_r);
        let mut t = DiffTab::new();
        t.load_pair(&l, &r, ViewOptions::default());
        assert_eq!(t.rows.len(), 50);
        assert!(!t.diff_rows.is_empty());
        // 差异跳转会定位到差异行
        t.next_diff();
        assert!(t.diff_pos.is_some());
        t.next_diff();
        t.prev_diff();
        // 行号跳转（第 40 行）产生滚动偏移
        t.jump_to_row(40);
        assert!(t.scroll.y > 0.0);
    }

    #[test]
    fn difftab_search_highlights_matches() {
        let d = tempdir().unwrap();
        let l = write(d.path(), "l.txt", "foo\nbar\nfoo\nbaz\n");
        let r = write(d.path(), "r.txt", "foo\nqux\nfoo\nbaz\n");
        let mut t = DiffTab::new();
        t.load_pair(&l, &r, ViewOptions::default());
        t.search.query = "foo".to_string();
        t.update_search();
        assert_eq!(t.search.matches.len(), 2);
        t.next_match();
        assert_eq!(t.search.current, Some(0));
        t.next_match();
        assert_eq!(t.search.current, Some(1));
        t.next_match(); // 循环回 0
        assert_eq!(t.search.current, Some(0));
        t.prev_match();
        assert_eq!(t.search.current, Some(1));
    }

    #[test]
    fn difftab_edit_state_and_save() {
        let d = tempdir().unwrap();
        let l = write(d.path(), "l.txt", "a\nb\n");
        let r = write(d.path(), "r.txt", "a\nb\n");
        let mut t = DiffTab::new();
        t.load_pair(&l, &r, ViewOptions::default());
        // 打开编辑左侧
        t.editing = Some(EditState {
            side: EditSide::Left,
            path: l.clone(),
            content: "a\nEDITED\n".to_string(),
        });
        assert!(t.editing.is_some());
        // 保存（模拟 Ctrl+S 分支，这里直接调用保存逻辑：写文件 + 重新加载）
        let (path, side) = t.editing.as_ref().map(|e| (e.path.clone(), e.side)).unwrap();
        fs::write(&path, "a\nEDITED\n").unwrap();
        match side {
            EditSide::Left => t.load_left(&path, t.opts),
            EditSide::Right => t.load_right(&path, t.opts),
        }
        t.editing = None;
        assert!(t.editing.is_none());
        // 重新加载后 diff 应检测到差异
        assert!(t.rows.iter().any(|r| r.tag != RowTag::Equal));
    }

    #[test]
    fn difftab_renders_headless() {
        let d = tempdir().unwrap();
        let l = write(d.path(), "l.txt", "a\nb\nc\n");
        let r = write(d.path(), "r.txt", "a\nX\nc\n");
        let mut t = DiffTab::new();
        t.load_pair(&l, &r, ViewOptions::default());
        egui::__run_test_ui(|ui| {
            t.ui(ui);
        });
    }

    #[test]
    fn difftab_empty_renders_headless() {
        let mut t = DiffTab::new();
        egui::__run_test_ui(|ui| {
            t.ui(ui);
        });
    }

    // ---- DirTab：树构建/折叠/键盘导航 ----------------

    #[test]
    fn dirtab_tree_and_navigation() {
        let d1 = tempdir().unwrap();
        let d2 = tempdir().unwrap();
        // 统一 mtime：保证只有内容差异被检出
        let fixed = filetime::FileTime::from_unix_time(1_700_000_000, 0);
        for (dir, name, content) in [
            (d1.path(), "same.txt", "x"),
            (d1.path(), "sub/a.txt", "a"),
            (d1.path(), "sub/deep/b.txt", "b"),
            (d1.path(), "only_l.txt", "y"),
            (d2.path(), "same.txt", "x"),
            (d2.path(), "sub/a.txt", "a"),
            (d2.path(), "sub/deep/b.txt", "b"),
        ] {
            let p = dir.join(name);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(&p, content).unwrap();
            filetime::set_file_mtime(&p, fixed).unwrap();
        }
        let mut t = DirTab::new(d1.path().to_str().unwrap(), d2.path().to_str().unwrap());
        t.refresh();
        assert!(t.result.is_some());
        // only_diff 默认 → 只有 only_l.txt 是差异文件
        assert!(!t.flat.is_empty());
        assert!(t.flat.iter().any(|r| r.name == "only_l.txt"));
        assert!(!t.flat.iter().any(|r| r.name == "same.txt"));
        assert!(!t.flat.iter().any(|r| r.name == "a.txt"));
        // 键盘选择 + 回车 → 打开请求
        t.selected = t.flat.iter().position(|r| !r.is_dir);
        t.open_selected();
        assert!(t.open_diff.is_some());
        assert_eq!(t.open_diff.as_deref(), Some("only_l.txt"));
    }

    #[test]
    fn dirtab_collapse_hides_children() {
        let d1 = tempdir().unwrap();
        let d2 = tempdir().unwrap();
        write(d1.path(), "sub/a.txt", "a");
        write(d1.path(), "sub/b.txt", "b");
        write(d2.path(), "sub/a.txt", "a");
        write(d2.path(), "sub/b.txt", "b");
        let mut t = DirTab::new(d1.path().to_str().unwrap(), d2.path().to_str().unwrap());
        t.only_diff = false;
        t.show_same = true;
        t.refresh();
        let dir_idx = t.flat.iter().position(|r| r.is_dir).unwrap();
        let dir_path = t.flat[dir_idx].path.clone();
        let before = t.flat.len();
        assert!(before > 1);
        t.toggle_dir(&dir_path);
        let after = t.flat.len();
        assert!(after < before);
        t.toggle_dir(&dir_path);
        assert_eq!(t.flat.len(), before);
    }

    #[test]
    fn dirtab_renders_headless() {
        let d1 = tempdir().unwrap();
        let d2 = tempdir().unwrap();
        write(d1.path(), "a.txt", "x");
        write(d1.path(), "sub/b.txt", "b");
        write(d2.path(), "a.txt", "x");
        write(d2.path(), "sub/b.txt", "b");
        let mut t = DirTab::new(d1.path().to_str().unwrap(), d2.path().to_str().unwrap());
        t.only_diff = false;
        t.show_same = true;
        egui::__run_test_ui(|ui| {
            t.ui(ui);
        });
        assert!(t.result.is_some());
    }

    // ---- MergeTab：冲突导航/解决/预览 ----------------

    #[test]
    fn mergetab_conflict_navigation_and_resolution() {
        let d = tempdir().unwrap();
        let b = write(d.path(), "base.txt", "l1\nX\nl3\nl4\nY\nl6\n");
        let l = write(d.path(), "left.txt", "l1\nL\nl3\nl4\nL2\nl6\n");
        let r = write(d.path(), "right.txt", "l1\nR\nl3\nl4\nR2\nl6\n");
        let mut t = MergeTab::new(&b, &l, &r);
        assert_eq!(t.view.conflicts, 2);
        assert_eq!(t.view.conflict_block_indices.len(), 2);
        // 冲突导航
        t.next_conflict();
        assert_eq!(t.conflict_idx, Some(0));
        t.next_conflict();
        assert_eq!(t.conflict_idx, Some(1));
        t.next_conflict(); // 循环
        assert_eq!(t.conflict_idx, Some(0));
        t.prev_conflict();
        assert_eq!(t.conflict_idx, Some(1));
        // 解决当前冲突（取右侧）
        t.resolve_current(Resolution::Right);
        let bi = t.current_conflict_block().unwrap();
        assert_eq!(t.view.blocks[bi].resolution, Resolution::Right);
        // 预览输出：第一个冲突已解决，第二个未解决
        let (lines, unresolved) = render_merged(&t.view, "LEFT", "RIGHT");
        assert_eq!(unresolved, 1);
        assert!(lines.iter().any(|l| l == "R2"));
    }

    #[test]
    fn mergetab_renders_headless() {
        let d = tempdir().unwrap();
        let b = write(d.path(), "base.txt", "l1\nX\nl3\n");
        let l = write(d.path(), "left.txt", "l1\nL\nl3\n");
        let r = write(d.path(), "right.txt", "l1\nR\nl3\n");
        let mut t = MergeTab::new(&b, &l, &r);
        egui::__run_test_ui(|ui| {
            t.ui(ui);
        });
    }

    // ---- 设置持久化 ----------------

    #[test]
    fn settings_roundtrip() {
        let mut s = Settings::default();
        s.theme = "dark".to_string();
        s.show_stats = false;
        s.ignore_whitespace = true;
        s.window_size = Some([1000.0, 700.0]);
        let toml_str = toml::to_string(&s).unwrap();
        let back: Settings = toml::from_str(&toml_str).unwrap();
        assert_eq!(back.theme, "dark");
        assert!(!back.show_stats);
        assert!(back.ignore_whitespace);
        assert_eq!(back.window_size, Some([1000.0, 700.0]));
        // 旧配置兼容：缺失字段用默认值
        let old: Settings = toml::from_str("theme = \"light\"\n").unwrap();
        assert!(old.show_stats);
        assert!(!old.ignore_whitespace);
        assert_eq!(old.theme_pref(), ThemePreference::Light);
    }

    #[test]
    fn app_tab_lifecycle() {
        let mut app = DiffApp::new(Settings::default());
        assert!(app.tabs.is_empty());
        app.new_diff_tab();
        assert_eq!(app.tabs.len(), 1);
        assert_eq!(app.active, 0);
        app.close_tab(0);
        assert!(app.tabs.is_empty());
        assert_eq!(app.active, 0);
    }
}

