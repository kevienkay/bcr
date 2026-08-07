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
}

impl Settings {
    fn path() -> PathBuf {
        let mut p = std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."));
        p.push(".bcr-gui.toml");
        p
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
}

/// 主应用
struct DiffApp {
    tabs: Vec<Tab>,
    active: usize,
    settings: Settings,
}

impl DiffApp {
    fn new(settings: Settings) -> Self {
        DiffApp {
            tabs: Vec::new(),
            active: 0,
            settings,
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
            ignore_whitespace: false,
            ignore_trailing: false,
            ignore_case: false,
        };
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
        self.handle_dropped(ui.ctx());

        // 顶部菜单栏
        egui::Panel::top("menu").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui.button("📁 打开文件对比…").clicked() {
                    self.open_diff_files();
                }
                if ui.button("📂 目录对比…").clicked() {
                    self.open_dir_compare();
                }
                if ui.button("🔀 三路合并…").clicked() {
                    self.open_merge();
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
                        .on_hover_text("关闭标签页");
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
                if ui.button("+").on_hover_text("新建并排 Diff 标签").clicked() {
                    self.new_diff_tab();
                }

                ui.separator();
                // 主题切换
                let mut pref = self.settings.theme_pref();
                let mut changed = false;
                ui.horizontal(|ui| {
                    ui.label("主题:");
                    for (label, p) in [
                        ("系统", ThemePreference::System),
                        ("深色", ThemePreference::Dark),
                        ("浅色", ThemePreference::Light),
                    ] {
                        if ui.selectable_label(pref == p, label).clicked() {
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

        // 当前标签内容
        if self.tabs.is_empty() {
            egui::CentralPanel::default().show(ui, |ui| {
                ui.centered_and_justified(|ui| {
                    ui.label(
                        RichText::new("bcr GUI — 并排 Diff / 目录对比 / 三路合并\n\n打开文件对比，或将文件/目录拖入窗口")
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
            // 目录对比双击 → 打开该文件的并排 diff
            if let Some(Tab::Dir(dir_tab)) = self.tabs.get(self.active) {
                let (l, r) = (dir_tab.left.clone(), dir_tab.right.clone());
                let mut t = DiffTab::new();
                t.load_pair(
                    &format!("{}/{}", l.trim_end_matches('/'), rel),
                    &format!("{}/{}", r.trim_end_matches('/'), rel),
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

/// 运行 GUI 事件循环，返回进程退出码
pub fn run(args: &GuiArgs) -> i32 {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1360.0, 860.0])
            .with_title("bcr — 对比工具"),
        ..Default::default()
    };

    let settings = Settings::load();
    let mut app = DiffApp::new(settings);
    let opts = ViewOptions {
        ignore_whitespace: args.ignore_whitespace,
        ignore_trailing: args.ignore_trailing,
        ignore_case: args.ignore_case,
    };

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
                    t.load_pair(l, r, opts);
                    app.add_tab(Tab::Diff(t));
                }
            }
            (Some(l), None) => {
                let mut t = DiffTab::new();
                t.load_left(l, opts);
                app.add_tab(Tab::Diff(t));
            }
            (None, Some(r)) => {
                let mut t = DiffTab::new();
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
            for theme in [egui::Theme::Dark, egui::Theme::Light] {
                cc.egui_ctx.style_mut_of(theme, |style| {
                    style
                        .text_styles
                        .insert(egui::TextStyle::Monospace, egui::FontId::monospace(FONT_SIZE));
                });
            }
            Ok(Box::new(app))
        }),
    ) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("bcr: GUI 启动失败: {e}");
            2
        }
    }
}
