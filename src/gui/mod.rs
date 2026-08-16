//! M5 GUI：多标签并排 Diff / 目录对比 / 三路合并视图。
//!
//! - 多标签页：DiffTab（并排 diff）、DirTab（目录对比）、MergeTab（三路合并）
//! - 虚拟化渲染（ScrollArea::show_rows）支持超大文件
//! - 搜索、差异/冲突跳转、行号跳转快捷键
//! - 主题切换 + 设置持久化（TOML）

mod common;
mod csvtab;
mod difftab;
mod dirtab;
mod foldermergetab;
mod imagetab;
mod mediatab;
mod menubar;
mod mergetab;
mod patchtab;
mod textedit;
mod theme;
#[cfg(test)]
mod ui_snap;
#[cfg(test)]
mod uikit_tests;

use crate::sideview::ViewOptions;
use common::*;
use csvtab::CsvTab;
use difftab::DiffTab;
use dirtab::DirTab;
use eframe::egui::{self, RichText, ThemePreference};
use foldermergetab::FolderMergeTab;
use imagetab::ImageTab;
use mediatab::MediaTab;
use mergetab::MergeTab;
use patchtab::PatchTab;
use std::path::PathBuf;
use textedit::TextEditTab;

/// GUI 子命令参数
#[derive(clap::Args, Debug, Default)]
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

    /// 忽略行尾 CR/LF 差异（CRLF vs LF）
    #[arg(long)]
    pub ignore_crlf: bool,

    /// 忽略匹配正则的行（内容过滤：如版本号/时间戳行，可重复）
    #[arg(long = "ignore-lines")]
    pub ignore_lines: Vec<String>,

    /// P37-1g：文本编辑（单文件，BC `-edit` 等价）
    #[arg(long = "edit")]
    pub edit: Option<String>,

    /// P37-1h：补丁视图（单 .patch/.diff 文件，BC Text Patch）
    #[arg(long = "patch")]
    pub patch: Option<String>,

    /// P37-1i：文件夹合并（BASE LEFT RIGHT 输出目录，BC Folder Merge）
    #[arg(long = "merge-dir", num_args = 4, value_names = ["BASE", "LEFT", "RIGHT", "OUT"])]
    pub merge_dir: Option<Vec<String>>,
}

/// 标签页
#[allow(clippy::large_enum_variant)] // DiffTab 含撤销/忽略状态最大，Box 化收益低
enum Tab {
    Diff(DiffTab),
    Dir(DirTab),
    Merge(MergeTab),
    Image(ImageTab),
    Csv(CsvTab),
    TextEdit(TextEditTab),
    Patch(PatchTab),
    FolderMerge(FolderMergeTab),
    /// P43-6：媒体比较（音视频元数据）
    Media(MediaTab),
}

impl Tab {
    fn title(&self) -> String {
        match self {
            Tab::Diff(t) => t.title(),
            Tab::Dir(t) => t.title(),
            Tab::Merge(t) => t.title(),
            Tab::Image(t) => t.title(),
            Tab::Csv(t) => t.title(),
            Tab::TextEdit(t) => t.title(),
            Tab::Patch(t) => t.title(),
            Tab::FolderMerge(t) => t.title(),
            Tab::Media(t) => t.title(),
        }
    }

    /// P53-1：标签类型图标（与主页卡片/空状态同风格，单色 emoji 按类型着色）
    fn icon(&self) -> &'static str {
        match self {
            Tab::Diff(t) => {
                if t.hex.is_some() {
                    "🔢"
                } else {
                    "📄"
                }
            }
            Tab::Dir(_) => "📁",
            Tab::Merge(_) => "🔀",
            Tab::Image(_) => "🖼",
            Tab::Csv(_) => "📊",
            Tab::TextEdit(_) => "✏️",
            Tab::Patch(_) => "🩹",
            Tab::FolderMerge(_) => "🗂",
            Tab::Media(_) => "🎵",
        }
    }

    /// 标签类型图标色（与主页卡片 card_icon_colors 同源）
    fn icon_color(&self) -> egui::Color32 {
        match self {
            Tab::Diff(_) => theme::card_icon_colors()[0],
            Tab::Dir(_) => theme::card_icon_colors()[1],
            Tab::Merge(_) => theme::card_icon_colors()[2],
            Tab::Image(_) => theme::card_icon_colors()[3],
            Tab::Csv(_) => theme::card_icon_colors()[4],
            Tab::TextEdit(_) => theme::card_icon_colors()[0],
            Tab::Patch(_) => theme::card_icon_colors()[5],
            Tab::FolderMerge(_) => theme::card_icon_colors()[2],
            Tab::Media(_) => theme::card_icon_colors()[6],
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
    /// P39-2a：忽略行尾 CR/LF 差异
    #[serde(default)]
    ignore_crlf: bool,
    /// P39-2a：强制编码（空 = 自动检测）
    #[serde(default)]
    encoding: String,
    /// P39-2a：文本大小上限（MB，None = 默认）
    #[serde(default)]
    max_size: Option<u64>,
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

    /// P44-5：导出设置（BC Tools>导出设置...；TOML 复制到用户选择路径）
    fn export_to(&self, path: &std::path::Path) -> Result<(), String> {
        let s = toml::to_string(self).map_err(|e| e.to_string())?;
        std::fs::write(path, s).map_err(|e| e.to_string())
    }

    /// P44-5：导入设置（BC Tools>导入设置...；从用户选择路径读取并保存生效）
    fn import_from(&mut self, path: &std::path::Path) -> Result<(), String> {
        let s = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let loaded: Settings = toml::from_str(&s).map_err(|e| e.to_string())?;
        *self = loaded;
        self.save();
        Ok(())
    }

    /// P44-5：恢复出厂默认（BC Tools>恢复出厂默认...；重置为默认并保存）
    fn reset_defaults(&mut self) {
        *self = Settings::default();
        self.save();
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
    /// 会话中心窗口开关
    show_sessions: bool,
    /// 规则(Profile)管理窗口开关
    show_profiles: bool,
    /// 云盘/远程浏览窗口开关
    show_cloud: bool,
    /// 云盘浏览：左右 URL 输入
    cloud_left: String,
    cloud_right: String,
    /// 云盘浏览：左右根目录条目（子目录/文件相对路径）
    cloud_left_entries: Option<Vec<String>>,
    cloud_right_entries: Option<Vec<String>>,
    /// 云盘浏览错误消息
    cloud_err: Option<String>,
    /// B6 标签拖拽：当前被拖拽的标签索引
    drag_tab: Option<usize>,
    /// P33：外部工具说明弹窗开关
    show_external: bool,
    /// P33：快捷键说明弹窗开关
    show_shortcuts: bool,
    /// P33：关于弹窗开关
    show_about: bool,
    /// P39-2a：设置对话框开关
    show_settings: bool,
    /// P39-2c：报告生成弹窗开关
    show_report: bool,
    /// P39-2c：报告格式（"txt" / "html"）
    report_format: String,
    /// P39-2c：报告错误提示
    report_error: Option<String>,
    /// P39-2c：会话中心「保存当前会话」名称输入
    session_save_name: String,
    /// P42-4：图例弹窗开关
    show_legend: bool,
    /// P42-4：日志面板开关
    show_log: bool,
    /// P42-4：日志条目（最近 N 条操作/错误）
    log: Vec<String>,
    /// P43-5：信息弹窗开关
    show_info: bool,
}

impl DiffApp {
    fn new(settings: Settings) -> Self {
        DiffApp {
            tabs: Vec::new(),
            active: 0,
            settings,
            show_git_help: false,
            show_sessions: false,
            show_profiles: false,
            show_cloud: false,
            cloud_left: String::new(),
            cloud_right: String::new(),
            cloud_left_entries: None,
            cloud_right_entries: None,
            cloud_err: None,
            drag_tab: None,
            show_external: false,
            show_shortcuts: false,
            show_about: false,
            show_settings: false,
            show_report: false,
            report_format: "txt".to_string(),
            report_error: None,
            session_save_name: String::new(),
            show_legend: false,
            show_log: false,
            log: Vec::new(),
            show_info: false,
        }
    }

    fn add_tab(&mut self, tab: Tab) {
        self.tabs.push(tab);
        self.active = self.tabs.len() - 1;
    }

    /// P46-5：保存工作空间（BC 会话>保存工作空间为...）——标签布局 TOML 持久化
    fn save_workspace(&mut self, path: &std::path::Path) -> Result<(), String> {
        #[derive(serde::Serialize)]
        struct WsTab {
            kind: String,
            left: String,
            right: String,
        }
        #[derive(serde::Serialize)]
        struct WsFile {
            tabs: Vec<WsTab>,
        }
        let tabs: Vec<WsTab> = self
            .tabs
            .iter()
            .filter_map(|t| {
                let (l, r) = session_paths(t)?;
                let kind = match t {
                    Tab::Diff(_) => "diff".to_string(),
                    Tab::Dir(_) => "dir".to_string(),
                    Tab::Merge(_) => "merge".to_string(),
                    Tab::Image(_) => "image".to_string(),
                    Tab::Csv(_) => "csv".to_string(),
                    Tab::Media(_) => "media".to_string(),
                    _ => return None,
                };
                Some(WsTab {
                    kind,
                    left: l,
                    right: r,
                })
            })
            .collect();
        if tabs.is_empty() {
            return Err("当前会话无可保存的对比标签".to_string());
        }
        let s = toml::to_string_pretty(&WsFile { tabs }).map_err(|e| e.to_string())?;
        std::fs::write(path, s).map_err(|e| e.to_string())
    }

    /// P46-5：加载工作空间（BC 会话>加载工作空间）——按类型重建标签
    fn load_workspace(&mut self, path: &std::path::Path) -> Result<(), String> {
        #[derive(serde::Deserialize)]
        struct WsTab {
            kind: String,
            left: String,
            right: String,
        }
        #[derive(serde::Deserialize)]
        struct WsFile {
            tabs: Vec<WsTab>,
        }
        let s = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let ws: WsFile = toml::from_str(&s).map_err(|e| e.to_string())?;
        if ws.tabs.is_empty() {
            return Err("工作空间为空".to_string());
        }
        self.tabs.clear();
        self.active = 0;
        for t in ws.tabs {
            let tab = match t.kind.as_str() {
                "diff" => {
                    let mut d = DiffTab::new();
                    d.load_pair(&t.left, &t.right, ViewOptions::default());
                    Tab::Diff(d)
                }
                "dir" => Tab::Dir(DirTab::new(&t.left, &t.right)),
                "merge" => Tab::Merge(MergeTab::new("", &t.left, &t.right)),
                "image" => Tab::Image(ImageTab::new(&t.left, &t.right)),
                "csv" => Tab::Csv(CsvTab::new(&t.left, &t.right)),
                "media" => Tab::Media(MediaTab::new(&t.left, &t.right)),
                _ => continue,
            };
            self.tabs.push(tab);
        }
        self.active = self.tabs.len().saturating_sub(1);
        Ok(())
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

    /// P44-1：选择下一个标签页（⌘] / 窗口菜单，BC Window>选择下一个标签页）
    fn next_tab(&mut self) {
        if self.tabs.is_empty() {
            return;
        }
        self.active = (self.active + 1) % self.tabs.len();
    }

    /// P44-1：选择上一个标签页（⌘[ / 窗口菜单，BC Window>选择上一个标签页）
    fn prev_tab(&mut self) {
        if self.tabs.is_empty() {
            return;
        }
        self.active = (self.active + self.tabs.len() - 1) % self.tabs.len();
    }

    /// P44-1：关闭所有窗口（BC Window>关闭所有窗口）——单窗口应用 = 清空所有标签回主页
    fn close_all_tabs(&mut self) {
        self.tabs.clear();
        self.active = 0;
    }

    /// B6：标签拖拽重排（from → to，保持 active 指向同一标签）
    fn move_tab(&mut self, from: usize, to: usize) {
        if from == to || from >= self.tabs.len() || to > self.tabs.len() {
            return;
        }
        let active_tab = self.active;
        let t = self.tabs.remove(from);
        let insert_at = if to > from { to - 1 } else { to };
        self.tabs.insert(insert_at, t);
        // 修正 active：指向原先激活的标签
        self.active = if active_tab == from {
            insert_at
        } else if active_tab > from && active_tab <= insert_at {
            active_tab - 1
        } else {
            active_tab
        };
    }

    fn new_diff_tab(&mut self) {
        let mut t = DiffTab::new();
        t.opts = ViewOptions {
            ignore_whitespace: self.settings.ignore_whitespace,
            ignore_trailing: self.settings.ignore_trailing,
            ignore_case: self.settings.ignore_case,
            ignore_crlf: self.settings.ignore_crlf,
            ignore_lines: Vec::new(),
        };
        t.show_stats = self.settings.show_stats;
        self.add_tab(Tab::Diff(t));
    }

    /// P39-2a：全局快捷键系统化（对标 BC 菜单快捷键）
    /// ⌘W 关闭标签 / ⌘, 设置 / ⌘T 新建标签 / ⌘N 新建窗口 / ⌥⌘S 保存会话 / ⌥⌘C 清除会话
    fn handle_global_shortcuts(&mut self, ui: &egui::Ui) {
        // B1：Ctrl+W 关闭当前标签（仅在有标签时）
        if !self.tabs.is_empty() && ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::W))
        {
            self.close_tab(self.active);
        }
        let cmd = ui.input(|i| i.modifiers.command);
        if cmd && ui.input(|i| i.key_pressed(egui::Key::Comma)) {
            // ⌘, 设置
            self.show_settings = true;
        }
        if cmd && ui.input(|i| i.key_pressed(egui::Key::T)) {
            // ⌘T 新建标签页（当前会话类型）
            self.new_tab_like_current();
        }
        if cmd && ui.input(|i| i.key_pressed(egui::Key::N)) {
            // ⌘N 新建窗口（新进程 GUI）
            Self::open_new_window();
        }
        if cmd && ui.input(|i| i.key_pressed(egui::Key::P)) {
            // ⌘P 报告生成（当前标签）
            self.show_report = true;
            self.report_error = None;
        }
        if cmd && ui.input(|i| i.modifiers.alt && i.key_pressed(egui::Key::S)) {
            // ⌥⌘S 保存会话（会话中心）
            self.show_sessions = true;
        }
        if cmd && ui.input(|i| i.modifiers.alt && i.key_pressed(egui::Key::C)) {
            // ⌥⌘C 清除会话（重置当前标签为空会话）
            self.clear_active_tab();
        }
        // P44-4：⌥⌘O 打开会话（会话中心；BC Session>打开会话）
        if cmd && ui.input(|i| i.modifiers.alt && i.key_pressed(egui::Key::O)) {
            self.show_sessions = true;
        }
        // P44-4：⌘R 重新比较（BC Session>重新比较文件；转发当前标签 reload/refresh）
        if cmd && ui.input(|i| i.key_pressed(egui::Key::R)) {
            self.reload_current();
        }
        // P44-4：⌘⇧S 保存文件为（BC File>保存文件为...；TextEdit 另存对话框）
        if cmd && ui.input(|i| i.modifiers.shift && i.key_pressed(egui::Key::S)) {
            if let Some(Tab::TextEdit(t)) = self.tabs.get_mut(self.active) {
                t.save_as();
            }
        }
        // P46-5：⇧L 图例（BC 视图>图例，⇧L；输入框聚焦时不触发）
        if ui.input(|i| {
            i.modifiers.shift
                && !i.modifiers.command
                && !i.modifiers.alt
                && !i.modifiers.ctrl
                && i.key_pressed(egui::Key::L)
        }) && !ui.ctx().egui_wants_keyboard_input()
        {
            self.show_legend = !self.show_legend;
        }
    }

    /// P44-4：重新比较当前标签（BC ⌘R）——Diff/Merge/FolderMerge 重载，Dir 重新扫描
    fn reload_current(&mut self) {
        match self.tabs.get_mut(self.active) {
            Some(Tab::Diff(t)) => t.reload(),
            Some(Tab::Merge(t)) => t.reload(),
            Some(Tab::Dir(t)) => t.refresh(),
            Some(Tab::FolderMerge(t)) => t.reload(),
            Some(Tab::Csv(t)) => t.reload(),
            _ => {}
        }
    }

    /// P43-5：信息弹窗（BC Session>信息，当前标签统计；抽为独立方法便于测试）
    fn info_window(&mut self, ui: &mut egui::Ui) {
        if !self.show_info {
            return;
        }
        let mut keep = true;
        crate::gui::common::dialog_window(ui.ctx(), crate::i18n::t(crate::i18n::Key::MenuInfo))
            .collapsible(false)
            .resizable(true)
            .default_size([460.0, 340.0])
            .open(&mut keep)
            .show(ui.ctx(), |ui| {
                let mut rows: Vec<(String, String)> = Vec::new();
                match self.tabs.get(self.active) {
                    Some(Tab::Diff(t)) => {
                        rows.push(("视图".to_string(), "文本对比".to_string()));
                        if let Some(l) = &t.left {
                            rows.push(("左侧文件".to_string(), l.path.clone()));
                            rows.push(("编码".to_string(), l.encoding.name().to_string()));
                            rows.push(("大小".to_string(), format!("{} bytes", l.size)));
                        }
                        if let Some(r) = &t.right {
                            rows.push(("右侧文件".to_string(), r.path.clone()));
                        }
                        rows.push((
                            crate::i18n::t(crate::i18n::Key::LineCount).to_string(),
                            t.rows.len().to_string(),
                        ));
                        rows.push(("差异行".to_string(), t.diff_rows.len().to_string()));
                        rows.push(("相同".to_string(), t.stats.equal.to_string()));
                        rows.push(("仅左".to_string(), t.stats.delete.to_string()));
                        rows.push(("仅右".to_string(), t.stats.insert.to_string()));
                        rows.push(("修改".to_string(), t.stats.replace.to_string()));
                    }
                    Some(Tab::Dir(t)) => {
                        rows.push(("视图".to_string(), "文件夹对比".to_string()));
                        rows.push(("左侧目录".to_string(), t.left.clone()));
                        rows.push(("右侧目录".to_string(), t.right.clone()));
                        if let Some(r) = &t.result {
                            let s = r.stats;
                            rows.push(("条目".to_string(), r.entries.len().to_string()));
                            rows.push(("相同".to_string(), s.same.to_string()));
                            rows.push(("仅左".to_string(), s.left_only.to_string()));
                            rows.push(("仅右".to_string(), s.right_only.to_string()));
                            rows.push(("差异".to_string(), s.differ.to_string()));
                            rows.push(("移动".to_string(), s.moved.to_string()));
                        } else {
                            rows.push(("状态".to_string(), "未比较".to_string()));
                        }
                    }
                    Some(Tab::Merge(t)) => {
                        rows.push(("视图".to_string(), "三路合并".to_string()));
                        rows.push(("BASE".to_string(), t.base_path.clone()));
                        rows.push(("左侧".to_string(), t.left_path.clone()));
                        rows.push(("右侧".to_string(), t.right_path.clone()));
                        rows.push(("冲突".to_string(), t.view.conflicts.to_string()));
                        rows.push((
                            crate::i18n::t(crate::i18n::Key::LineCount).to_string(),
                            t.view.blocks.len().to_string(),
                        ));
                    }
                    Some(Tab::Image(t)) => {
                        rows.push(("视图".to_string(), "图片对比".to_string()));
                        rows.push(("左侧".to_string(), t.left.clone()));
                        rows.push(("右侧".to_string(), t.right.clone()));
                        if let Some(p) = &t.pair {
                            let s = p.stats;
                            rows.push(("帧".to_string(), t.total_frames().to_string()));
                            rows.push(("差异像素".to_string(), s.diff_pixels.to_string()));
                        }
                    }
                    Some(Tab::Csv(t)) => {
                        rows.push(("视图".to_string(), "表格对比".to_string()));
                        rows.push((
                            crate::i18n::t(crate::i18n::Key::LineCount).to_string(),
                            t.row_count().to_string(),
                        ));
                        rows.push(("列数".to_string(), t.col_count().to_string()));
                    }
                    Some(Tab::TextEdit(t)) => {
                        rows.push(("视图".to_string(), "文本编辑".to_string()));
                        rows.push(("标题".to_string(), t.title()));
                        rows.push(("字符".to_string(), t.content.chars().count().to_string()));
                        rows.push((
                            crate::i18n::t(crate::i18n::Key::LineCount).to_string(),
                            t.content.lines().count().to_string(),
                        ));
                    }
                    Some(Tab::Patch(t)) => {
                        rows.push(("视图".to_string(), "补丁".to_string()));
                        rows.push(("标题".to_string(), t.title()));
                    }
                    Some(Tab::FolderMerge(t)) => {
                        rows.push(("视图".to_string(), "文件夹合并".to_string()));
                        rows.push(("输出".to_string(), t.out.clone()));
                        rows.push(("左侧".to_string(), t.left.clone()));
                        rows.push(("右侧".to_string(), t.right.clone()));
                    }
                    Some(Tab::Media(t)) => {
                        rows.push(("视图".to_string(), "媒体比较".to_string()));
                        rows.push(("左侧".to_string(), t.left.clone()));
                        rows.push(("右侧".to_string(), t.right.clone()));
                        rows.push(("差异字段".to_string(), t.diffs.len().to_string()));
                    }
                    None => {
                        rows.push(("状态".to_string(), "无标签".to_string()));
                    }
                }
                egui::Grid::new("info_grid")
                    .num_columns(2)
                    .striped(true)
                    .show(ui, |ui| {
                        for (k, v) in &rows {
                            ui.label(RichText::new(k).strong());
                            ui.monospace(v);
                            ui.end_row();
                        }
                    });
            });
        if !keep {
            self.show_info = false;
        }
    }

    /// P42-4：图例弹窗 + 日志面板（BC View>图例/日志；抽为独立方法便于测试直接调用）
    fn legend_log_windows(&mut self, ui: &mut egui::Ui) {
        // 图例弹窗：差异色/状态徽标含义说明
        if self.show_legend {
            let mut keep = true;
            crate::gui::common::dialog_window(
                ui.ctx(),
                crate::i18n::t(crate::i18n::Key::MenuLegend),
            )
            .collapsible(false)
            .default_size([360.0, 300.0])
            .open(&mut keep)
            .show(ui.ctx(), |ui| {
                ui.label(RichText::new("文本比较").strong());
                for (color, label) in [
                    (
                        theme::diff_delete(ui.visuals().dark_mode),
                        "删除 / 仅左侧（淡红）",
                    ),
                    (
                        theme::diff_insert(ui.visuals().dark_mode),
                        "插入 / 仅右侧（淡绿）",
                    ),
                    (theme::diff_modify(ui.visuals().dark_mode), "修改（淡黄）"),
                    (
                        theme::current_bar(ui.visuals().dark_mode),
                        "当前差异行竖条（蓝）",
                    ),
                ] {
                    ui.horizontal(|ui| {
                        let (rect, _) = ui
                            .allocate_exact_size(egui::Vec2::new(14.0, 14.0), egui::Sense::hover());
                        ui.painter().rect_filled(rect, 2.0, color);
                        ui.label(label);
                    });
                }
                ui.separator();
                ui.label(RichText::new("文件夹比较状态徽标").strong());
                for (letter, color, label) in [
                    ('S', ui.visuals().text_color(), "相同"),
                    ('C', theme::status_differ(), "差异"),
                    ('L', theme::status_orphan(), "仅左侧"),
                    ('R', theme::status_orphan(), "仅右侧"),
                    ('M', theme::status_differ(), "移动/重命名"),
                ] {
                    ui.horizontal(|ui| {
                        let badge_c = ui.cursor().min + egui::vec2(9.0, 10.0);
                        ui.painter()
                            .circle_filled(badge_c, 9.0, color.gamma_multiply(0.25));
                        ui.painter().text(
                            badge_c,
                            egui::Align2::CENTER_CENTER,
                            letter.to_string(),
                            egui::FontId::monospace(12.0),
                            color,
                        );
                        ui.add_space(16.0);
                        ui.label(label);
                    });
                }
            });
            if !keep {
                self.show_legend = false;
            }
        }
        // 日志面板：最近操作/错误记录
        if self.show_log {
            let mut keep = true;
            crate::gui::common::dialog_window(ui.ctx(), crate::i18n::t(crate::i18n::Key::MenuLog))
                .collapsible(false)
                .resizable(true)
                .default_size([520.0, 280.0])
                .open(&mut keep)
                .show(ui.ctx(), |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("{} 条", self.log.len()))
                                .size(12.0)
                                .color(ui.visuals().weak_text_color()),
                        );
                        if ui.button("清空").clicked() {
                            self.log.clear();
                        }
                    });
                    ui.separator();
                    if self.log.is_empty() {
                        ui.label("暂无日志");
                    }
                    egui::ScrollArea::vertical()
                        .max_height(200.0)
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            for line in self.log.iter().rev().take(200) {
                                ui.monospace(line);
                            }
                        });
                });
            if !keep {
                self.show_log = false;
            }
        }
    }

    /// P39-2a：全局快捷键测试用（不含 ⌘N 新进程，避免测试误启新实例）
    #[cfg(test)]
    fn handle_global_shortcuts_safe(&mut self, ui: &egui::Ui) {
        if !self.tabs.is_empty() && ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::W))
        {
            self.close_tab(self.active);
        }
        let cmd = ui.input(|i| i.modifiers.command);
        if cmd && ui.input(|i| i.key_pressed(egui::Key::Comma)) {
            self.show_settings = true;
        }
        if cmd && ui.input(|i| i.key_pressed(egui::Key::T)) {
            self.new_tab_like_current();
        }
        if cmd && ui.input(|i| i.key_pressed(egui::Key::P)) {
            self.show_report = true;
            self.report_error = None;
        }
        if cmd && ui.input(|i| i.modifiers.alt && i.key_pressed(egui::Key::S)) {
            self.show_sessions = true;
        }
        if cmd && ui.input(|i| i.modifiers.alt && i.key_pressed(egui::Key::C)) {
            self.clear_active_tab();
        }
        // P44-1：⌘] / ⌘[ 选择下一/上一标签页（BC Window>选择下一个标签页）
        if !self.tabs.is_empty() && cmd && ui.input(|i| i.key_pressed(egui::Key::CloseBracket)) {
            self.next_tab();
        }
        if !self.tabs.is_empty() && cmd && ui.input(|i| i.key_pressed(egui::Key::OpenBracket)) {
            self.prev_tab();
        }
        // P44-1：⌘⇧W 关闭所有窗口（清空所有标签回主页）
        if !self.tabs.is_empty()
            && cmd
            && ui.input(|i| i.modifiers.shift && i.key_pressed(egui::Key::W))
        {
            self.close_all_tabs();
        }
        // P44-1：⌘M 最小化窗口（BC Window>最小化）
        if cmd && ui.input(|i| i.key_pressed(egui::Key::M)) {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::Minimized(true));
        }
    }

    fn handle_dropped(&mut self, ctx: &egui::Context) {
        let dropped: Vec<std::path::PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .collect()
        });
        if dropped.is_empty() {
            return;
        }
        // 拖放排序：按文件名排序，保证多文件/目录拖入行为可预测
        let mut dropped = dropped;
        dropped.sort_by_key(|p| {
            p.file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default()
        });
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

        // P34：空会话优先填充（拖入内容填充当前空会话对应侧，而非新建）
        if self.fill_empty_session(&dirs, &files) {
            return;
        }

        match (dirs.as_slice(), files.as_slice()) {
            (_, [..]) if files.len() >= 3 => {
                // 多个文件：尝试三路合并（前三个）
                if let Some(merge) = self.try_merge(&files) {
                    self.add_tab(Tab::Merge(merge));
                }
            }
            (_, [a, b]) => {
                // 两个文件：图片/CSV → 专用标签，否则文本对比
                let (a, b) = (a.clone(), b.clone());
                if crate::imgcmp::is_image_file(&a) && crate::imgcmp::is_image_file(&b) {
                    self.add_tab(Tab::Image(ImageTab::new(&a, &b)));
                } else if crate::csvcmp::is_csv_file(&a) && crate::csvcmp::is_csv_file(&b) {
                    self.add_tab(Tab::Csv(CsvTab::new(&a, &b)));
                } else if crate::mediacmp::read_media_info(&a).format.is_some()
                    && crate::mediacmp::read_media_info(&b).format.is_some()
                    && crate::gui::mediatab::is_media_file(&a)
                    && crate::gui::mediatab::is_media_file(&b)
                {
                    self.add_tab(Tab::Media(MediaTab::new(&a, &b)));
                } else {
                    let mut t = DiffTab::new();
                    t.load_pair(&a, &b, ViewOptions::default());
                    self.add_tab(Tab::Diff(t));
                }
            }
            (_, [a]) => {
                // 单个文件：补到当前 diff 标签或新建（图片文件走图片对比）
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

    /// P34：拖入文件/目录时，优先填充当前空会话对应侧（BC 式），成功返回 true
    fn fill_empty_session(&mut self, dirs: &[String], files: &[String]) -> bool {
        let Some(tab) = self.tabs.get_mut(self.active) else {
            return false;
        };
        match tab {
            Tab::Dir(t) if t.left.is_empty() && t.right.is_empty() && !dirs.is_empty() => {
                t.left = dirs[0].clone();
                if dirs.len() >= 2 {
                    t.right = dirs[1].clone();
                }
                t.refresh();
                true
            }
            Tab::Merge(t)
                if t.base_path.is_empty()
                    && t.left_path.is_empty()
                    && t.right_path.is_empty()
                    && !files.is_empty() =>
            {
                let mut i = 0;
                if i < files.len() {
                    t.base_path = files[i].clone();
                    i += 1;
                }
                if i < files.len() {
                    t.left_path = files[i].clone();
                    i += 1;
                }
                if i < files.len() {
                    t.right_path = files[i].clone();
                }
                t.reload();
                true
            }
            Tab::Csv(t) if t.is_empty() && !files.is_empty() => {
                let l = files[0].clone();
                let r = files.get(1).cloned().unwrap_or_default();
                t.load_pair(&l, &r);
                true
            }
            Tab::TextEdit(t) if t.is_empty() && !files.is_empty() => {
                t.open(&files[0]);
                true
            }
            Tab::Patch(t) if t.is_empty() && !files.is_empty() => {
                t.open(&files[0]);
                true
            }
            Tab::Image(t) if t.left.is_empty() && t.right.is_empty() && !files.is_empty() => {
                let l = files[0].clone();
                let r = files.get(1).cloned().unwrap_or_default();
                t.load_pair(&l, &r);
                true
            }
            Tab::Diff(t)
                if t.left.is_none()
                    && t.right.is_none()
                    && !files.is_empty()
                    && files.len() < 3 =>
            {
                let l = files[0].clone();
                let r = files.get(1).cloned().unwrap_or_default();
                t.load_pair(&l, &r, t.opts.clone());
                true
            }
            _ => false,
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
        // 补丁文件：打开补丁视图（BC Text Patch）
        if crate::patchview::is_patch_file(path) {
            self.add_tab(Tab::Patch(PatchTab::new(path)));
            return;
        }
        // 图片：若当前是图片标签且有一侧空 → 填充；否则新建图片标签
        if crate::imgcmp::is_image_file(path) {
            if let Some(Tab::Image(t)) = self.tabs.get_mut(self.active) {
                if t.left.is_empty() || !std::path::Path::new(&t.left).exists() {
                    let r = t.right.clone();
                    t.load_pair(path, &r);
                    return;
                }
                if t.right.is_empty() || !std::path::Path::new(&t.right).exists() {
                    let l = t.left.clone();
                    t.load_pair(&l, path);
                    return;
                }
            }
            let t = ImageTab::new(path, "");
            self.add_tab(Tab::Image(t));
            return;
        }
        // 若当前是 diff 标签且有一侧空 → 填充；否则新建 diff 标签
        if let Some(Tab::Diff(t)) = self.tabs.get_mut(self.active) {
            if t.left.is_none() {
                t.load_left(path, t.opts.clone());
                return;
            }
            if t.right.is_none() {
                t.load_right(path, t.opts.clone());
                return;
            }
        }
        let mut t = DiffTab::new();
        t.load_left(path, ViewOptions::default());
        self.add_tab(Tab::Diff(t));
    }

    fn open_diff_files(&mut self) {
        // 支持多选：选 1 个 → 立即显示左侧（先导入的显示在左边）；
        // 选 2 个 → 双文件逻辑；≥3 个 → 三路合并（与拖拽一致）
        let Some(files) = rfd::FileDialog::new().pick_files() else {
            return;
        };
        let paths: Vec<String> = files
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        match paths.len() {
            0 => {}
            1 => self.drop_single_file(&paths[0]),
            2 => {
                let (l, r) = (&paths[0], &paths[1]);
                // 两侧均为图片 → 图片对比标签
                if crate::imgcmp::is_image_file(l) && crate::imgcmp::is_image_file(r) {
                    self.add_tab(Tab::Image(ImageTab::new(l, r)));
                    return;
                }
                // 两侧均为 CSV/TSV → 表格对比标签（P29）
                if crate::csvcmp::is_csv_file(l) && crate::csvcmp::is_csv_file(r) {
                    self.add_tab(Tab::Csv(CsvTab::new(l, r)));
                    return;
                }
                let mut t = DiffTab::new();
                t.load_pair(l, r, ViewOptions::default());
                self.add_tab(Tab::Diff(t));
            }
            _ => {
                // ≥3 个：三路合并（前三个），与拖拽一致
                if let Some(merge) = self.try_merge(&paths) {
                    self.add_tab(Tab::Merge(merge));
                }
            }
        }
    }

    fn open_dir_compare(&mut self) {
        let Some(l) = pick_dir() else { return };
        let Some(r) = pick_dir() else { return };
        self.add_tab(Tab::Dir(DirTab::new(&l, &r)));
    }

    /// P32-A7：强制图片对比会话（选两个图片文件 → ImageTab）
    #[allow(dead_code)]
    fn open_image_compare(&mut self) {
        let Some(l) = pick_file() else { return };
        let Some(r) = pick_file() else { return };
        self.add_tab(Tab::Image(ImageTab::new(&l, &r)));
    }

    /// P32-A7：强制 CSV 表格对比会话（选两个 CSV/TSV → CsvTab）
    #[allow(dead_code)]
    fn open_csv_compare(&mut self) {
        let Some(l) = pick_file() else { return };
        let Some(r) = pick_file() else { return };
        self.add_tab(Tab::Csv(CsvTab::new(&l, &r)));
    }

    fn open_merge(&mut self) {
        let Some(b) = pick_file() else { return };
        let Some(l) = pick_file() else { return };
        let Some(r) = pick_file() else { return };
        self.add_tab(Tab::Merge(MergeTab::new(&b, &l, &r)));
    }

    // ---- P34：空会话入口（BC 式：点会话类型 → 直接进入空面板，再分别打开/拖拽两侧）----
    /// 空文本对比会话
    fn open_empty_diff(&mut self) {
        self.new_diff_tab();
    }
    /// 空文件夹对比会话
    fn open_empty_dir(&mut self) {
        self.add_tab(Tab::Dir(DirTab::new("", "")));
    }
    /// 空三路合并会话
    fn open_empty_merge(&mut self) {
        self.add_tab(Tab::Merge(MergeTab::new("", "", "")));
    }
    /// 空图片对比会话
    fn open_empty_image(&mut self) {
        self.add_tab(Tab::Image(ImageTab::new("", "")));
    }
    /// 空 CSV 表格对比会话
    fn open_empty_csv(&mut self) {
        self.add_tab(Tab::Csv(CsvTab::new("", "")));
    }

    /// P43-6：新建空媒体比较会话
    fn open_empty_media(&mut self) {
        self.add_tab(Tab::Media(MediaTab::new("", "")));
    }

    /// P39-2a：新建标签页（⌘T）——复制当前会话类型的新空标签
    fn new_tab_like_current(&mut self) {
        match self.tabs.get(self.active) {
            Some(Tab::Diff(_)) => self.open_empty_diff(),
            Some(Tab::Dir(_)) => self.open_empty_dir(),
            Some(Tab::Merge(_)) => self.open_empty_merge(),
            Some(Tab::Image(_)) => self.open_empty_image(),
            Some(Tab::Csv(_)) => self.open_empty_csv(),
            Some(Tab::TextEdit(_)) => self.add_tab(Tab::TextEdit(TextEditTab::new(""))),
            Some(Tab::Patch(_)) => self.add_tab(Tab::Patch(PatchTab::new(""))),
            Some(Tab::FolderMerge(_)) => {
                self.add_tab(Tab::FolderMerge(FolderMergeTab::new("", "", "", "")))
            }
            Some(Tab::Media(_)) => self.add_tab(Tab::Media(MediaTab::new("", ""))),
            None => self.open_empty_diff(),
        }
    }

    /// P39-2a：新建窗口（⌘N）——启动新进程 GUI（对标 BC 新建窗口）
    fn open_new_window() {
        if let Ok(exe) = std::env::current_exe() {
            // 无参数启动 GUI（main.rs 已处理：macOS 无参数一律 GUI，其余平台看 stdin）
            let _ = std::process::Command::new(exe).spawn();
        }
    }

    /// P39-2a：清除会话（⌥⌘C）——把当前标签重置为同类型空会话
    fn clear_active_tab(&mut self) {
        if self.tabs.is_empty() {
            return;
        }
        let idx = self.active;
        match &self.tabs[idx] {
            Tab::Diff(_) => self.tabs[idx] = Tab::Diff(DiffTab::new()),
            Tab::Dir(_) => self.tabs[idx] = Tab::Dir(DirTab::new("", "")),
            Tab::Merge(_) => self.tabs[idx] = Tab::Merge(MergeTab::new("", "", "")),
            Tab::Image(_) => self.tabs[idx] = Tab::Image(ImageTab::new("", "")),
            Tab::Csv(_) => self.tabs[idx] = Tab::Csv(CsvTab::new("", "")),
            Tab::TextEdit(_) => self.tabs[idx] = Tab::TextEdit(TextEditTab::new("")),
            Tab::Patch(_) => self.tabs[idx] = Tab::Patch(PatchTab::new("")),
            Tab::FolderMerge(_) => {
                self.tabs[idx] = Tab::FolderMerge(FolderMergeTab::new("", "", "", ""))
            }
            Tab::Media(_) => self.tabs[idx] = Tab::Media(MediaTab::new("", "")),
        }
    }

    /// P39-2e：比较文件使用（视图切换）——用指定视图重新打开当前文件对
    fn reopen_as_text(&mut self, l: &str, r: &str) {
        let mut t = DiffTab::new();
        t.opts = ViewOptions {
            ignore_whitespace: self.settings.ignore_whitespace,
            ignore_trailing: self.settings.ignore_trailing,
            ignore_case: self.settings.ignore_case,
            ignore_crlf: self.settings.ignore_crlf,
            ignore_lines: Vec::new(),
        };
        t.show_stats = self.settings.show_stats;
        t.load_pair(l, r, t.opts.clone());
        self.add_tab(Tab::Diff(t));
    }

    /// P39-2e：16进制视图（强制 hex 渲染）
    fn reopen_as_hex(&mut self, l: &str, r: &str) {
        self.reopen_as_text(l, r);
        if let Tab::Diff(t) = &mut self.tabs[self.active] {
            t.set_detail_mode(difftab::DiffDetailMode::Hex);
        }
    }

    /// P39-2e：图片视图
    fn reopen_as_image(&mut self, l: &str, r: &str) {
        self.add_tab(Tab::Image(ImageTab::new(l, r)));
    }

    /// P39-2e：表格视图
    fn reopen_as_csv(&mut self, l: &str, r: &str) {
        self.add_tab(Tab::Csv(CsvTab::new(l, r)));
    }

    /// P43-4：合并文件（BC 文本比较 Session>合并文件）——当前左右文件进入三路合并，BASE 留空待填
    fn reopen_as_merge(&mut self, l: &str, r: &str) {
        self.add_tab(Tab::Merge(MergeTab::new("", l, r)));
    }

    /// P43-4：和输出比较（BC 文件夹合并/同步 Session>和输出比较）——输出目录 vs 左侧目录对比
    fn compare_with_output(&mut self) {
        let Some(Tab::FolderMerge(t)) = self.tabs.get(self.active) else {
            return;
        };
        let (out, left) = (t.out.clone(), t.left.clone());
        if out.is_empty() || left.is_empty() {
            return;
        }
        self.add_tab(Tab::Dir(DirTab::new(&out, &left)));
    }

    /// P32-A7：欢迎页 — 大标题 + 会话类型网格卡片（文本/文件夹/三路合并/图片/CSV）
    fn welcome_ui(&mut self, ui: &mut egui::Ui) {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.horizontal(|ui| {
                // ---- 左侧：Sessions 管理面板（BC 主页布局）----
                egui::Frame::group(ui.style())
                    .corner_radius(6.0)
                    .inner_margin(egui::Margin::same(10))
                    .show(ui, |ui| {
                        ui.set_min_width(240.0);
                        ui.set_max_width(300.0);
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new(crate::i18n::t(crate::i18n::Key::MenuSaveSession))
                                    .strong()
                                    .size(14.0),
                            );
                            ui.add_space(4.0);
                            let mut sessions = crate::session::load();
                            if sessions.sessions.is_empty() {
                                ui.label(
                                    RichText::new(
                                        "暂无会话\n用 bcr session save <name> <left> <right> 保存",
                                    )
                                    .size(12.0)
                                    .color(ui.visuals().weak_text_color()),
                                );
                            } else {
                                let mut open_req: Option<(String, String)> = None;
                                let mut delete_req: Option<String> = None;
                                egui::ScrollArea::vertical()
                                    .max_height(320.0)
                                    .auto_shrink([false, true])
                                    .show(ui, |ui| {
                                        for (name, s) in &sessions.sessions {
                                            // P51-4：会话列表项 hover 高亮（Frame 包裹 + 半透明底色）
                                            let label = format!(
                                                "▶ {}\n{}",
                                                name,
                                                if s.left.len() + s.right.len() > 60 {
                                                    format!(
                                                        "{} … {}",
                                                        s.left.chars().take(22).collect::<String>(),
                                                        s.right
                                                            .chars()
                                                            .take(22)
                                                            .collect::<String>()
                                                    )
                                                } else {
                                                    format!("{} ↔ {}", s.left, s.right)
                                                }
                                            );
                                            let row = egui::Frame::new()
                                                .corner_radius(4.0)
                                                .inner_margin(egui::Margin::symmetric(8, 4))
                                                .show(ui, |ui| {
                                                    ui.horizontal(|ui| {
                                                        let resp = ui
                                                            .add(
                                                                egui::Button::new(
                                                                    RichText::new(label).size(12.0),
                                                                )
                                                                .frame(false)
                                                                .wrap_mode(
                                                                    egui::TextWrapMode::Truncate,
                                                                ),
                                                            )
                                                            .on_hover_text(format!(
                                                                "{} ↔ {}",
                                                                s.left, s.right
                                                            ));
                                                        if resp.clicked() {
                                                            open_req = Some((
                                                                s.left.clone(),
                                                                s.right.clone(),
                                                            ));
                                                        }
                                                        // ✕ 删除（hover 红色提示）
                                                        let del = ui
                                                            .add(
                                                                egui::Button::new(
                                                                    RichText::new("✕")
                                                                        .size(11.0)
                                                                        .color(if resp.hovered() {
                                                                            theme::stat_delete(
                                                                                ui.visuals()
                                                                                    .dark_mode,
                                                                            )
                                                                        } else {
                                                                            ui.visuals()
                                                                                .weak_text_color()
                                                                        }),
                                                                )
                                                                .small()
                                                                .frame(false),
                                                            )
                                                            .on_hover_text(crate::i18n::t(
                                                                crate::i18n::Key::DeleteSession,
                                                            ));
                                                        if del.clicked() {
                                                            delete_req = Some(name.clone());
                                                        }
                                                    });
                                                })
                                                .response;
                                            // hover 高亮：整行半透明底色（BC 列表 hover 观感）
                                            if row.hovered() {
                                                ui.painter().rect_filled(
                                                    row.rect,
                                                    4.0,
                                                    theme::bg_match(),
                                                );
                                            }
                                            ui.add_space(2.0);
                                        }
                                    });
                                if let Some((l, r)) = open_req {
                                    self.add_tab(Tab::Dir(DirTab::new(&l, &r)));
                                }
                                if let Some(del) = delete_req {
                                    sessions.sessions.remove(&del);
                                    let _ = crate::session::save_all(&sessions);
                                }
                            }
                        });
                    });
                ui.add_space(12.0);
                // ---- 主区：会话类型大按钮（BC 式竖排/网格 + 拖放提示）----
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new("bcr")
                            .size(34.0)
                            .strong()
                            .color(theme::folder_color(ui.visuals().dark_mode)),
                    );
                    ui.label(
                        RichText::new(crate::i18n::t(crate::i18n::Key::MainHint))
                            .size(13.0)
                            .color(ui.visuals().weak_text_color()),
                    );
                    ui.add_space(4.0);
                    // BC 提示语：将文件夹或文件拖放到会话图标上
                    ui.label(
                        RichText::new("将文件夹或文件拖放到会话图标上")
                            .size(12.0)
                            .color(ui.visuals().weak_text_color()),
                    );
                    ui.add_space(12.0);
                    // 会话类型卡片（7 类，BC 主页语义）
                    let cards: [(&str, crate::i18n::Key, crate::i18n::Key, u32); 7] = [
                        (
                            "📄",
                            crate::i18n::Key::SessionText,
                            crate::i18n::Key::SessionTextDesc,
                            0,
                        ),
                        (
                            "📁",
                            crate::i18n::Key::SessionDir,
                            crate::i18n::Key::SessionDirDesc,
                            1,
                        ),
                        (
                            "🔀",
                            crate::i18n::Key::SessionMerge,
                            crate::i18n::Key::SessionMergeDesc,
                            2,
                        ),
                        (
                            "🖼",
                            crate::i18n::Key::SessionImage,
                            crate::i18n::Key::SessionImageDesc,
                            3,
                        ),
                        (
                            "📊",
                            crate::i18n::Key::SessionCsv,
                            crate::i18n::Key::SessionCsvDesc,
                            4,
                        ),
                        // P33：补齐 BC 会话类型入口
                        (
                            "🔢",
                            crate::i18n::Key::MenuNewHex,
                            crate::i18n::Key::MenuNewHex,
                            5,
                        ),
                        // P43-6：媒体比较（音视频元数据）
                        (
                            "🎵",
                            crate::i18n::Key::SessionMedia,
                            crate::i18n::Key::SessionMediaDesc,
                            6,
                        ),
                    ];
                    let card_w = 170.0;
                    let card_h = 76.0;
                    // P51-4：卡片网格自适应列数（min 170px/列，按可用宽度计算，BC 主页观感）
                    let avail_w = ui.available_width();
                    let cols = ((avail_w + 10.0) / (card_w + 10.0)).floor().max(1.0) as usize;
                    let mut card_click: Option<u32> = None;
                    egui::Grid::new("session-cards")
                        .spacing([10.0, 10.0])
                        .show(ui, |ui| {
                            for (i, (icon, title, desc, id)) in cards.iter().enumerate() {
                                // P53-2：卡片 hover 高亮（浅色/深色主题各自适配）+ 圆角
                                let frame = egui::Frame::group(ui.style())
                                    .corner_radius(8.0)
                                    .inner_margin(egui::Margin::same(10))
                                    .fill(theme::card_bg(ui.visuals().dark_mode));
                                let resp = frame
                                    .show(ui, |ui| {
                                        ui.set_min_size(egui::vec2(card_w, card_h));
                                        ui.vertical(|ui| {
                                            ui.horizontal(|ui| {
                                                // P52-1：彩色图标底片——半透明品牌色圆角块 + 同色符号。
                                                // egui 无彩色 emoji 字形（NotoEmoji 单色），着色后获得品牌色观感。
                                                let chip_c = theme::card_icon_colors()[i.min(6)];
                                                let (chip_rect, _) = ui.allocate_exact_size(
                                                    egui::vec2(32.0, 32.0),
                                                    egui::Sense::hover(),
                                                );
                                                ui.painter().rect_filled(
                                                    chip_rect,
                                                    8.0,
                                                    chip_c.gamma_multiply(0.16),
                                                );
                                                ui.painter().text(
                                                    chip_rect.center(),
                                                    egui::Align2::CENTER_CENTER,
                                                    *icon,
                                                    egui::FontId::proportional(19.0),
                                                    chip_c,
                                                );
                                                ui.add_space(4.0);
                                                ui.label(
                                                    RichText::new(crate::i18n::t(*title))
                                                        .size(14.0)
                                                        .strong(),
                                                );
                                            });
                                            ui.label(
                                                RichText::new(crate::i18n::t(*desc))
                                                    .size(11.0)
                                                    .color(ui.visuals().weak_text_color()),
                                            );
                                        });
                                    })
                                    // Frame::show 的 response 默认 Sense::hover() 不含点击，
                                    // 必须补 interact(Sense::click()) 才能捕获点击（P38-1f 修复）
                                    .response
                                    .interact(egui::Sense::click())
                                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                                // P48-1：hover 上浮动效（0~1 动画插值，平滑过渡）+ 阴影加深
                                let hover_anim = ui
                                    .ctx()
                                    .animate_bool(egui::Id::new(("card_hover", i)), resp.hovered());
                                let lift = 3.0 * hover_anim;
                                let rect = resp.rect.translate(egui::vec2(0.0, -lift));
                                // 阴影：hover 时加深的半透明边框（egui 无原生投影，用叠层模拟）
                                if hover_anim > 0.0 {
                                    ui.painter().rect_filled(
                                        rect.translate(egui::vec2(0.0, 2.0 + lift)),
                                        8.0,
                                        egui::Color32::from_black_alpha((18.0 * hover_anim) as u8),
                                    );
                                }
                                // P39-2d：hover 高亮描边（BC 卡片 hover 反馈）
                                if resp.hovered() {
                                    ui.painter().rect_stroke(
                                        rect,
                                        8.0,
                                        egui::Stroke::new(
                                            1.5,
                                            theme::current_bar(ui.visuals().dark_mode),
                                        ),
                                        egui::StrokeKind::Outside,
                                    );
                                }
                                if resp.clicked() {
                                    card_click = Some(*id);
                                }
                                // P51-4：按可用宽度自适应换行（原固定 4 列）
                                if (i + 1) % cols == 0 {
                                    ui.end_row();
                                }
                            }
                        });
                    match card_click {
                        Some(0) => self.open_empty_diff(),
                        Some(1) => self.open_empty_dir(),
                        Some(2) => self.open_empty_merge(),
                        Some(3) => self.open_empty_image(),
                        Some(4) => self.open_empty_csv(),
                        // Hex：空文本对比会话（二进制自动切 hex）
                        Some(5) => self.open_empty_diff(),
                        // 媒体比较
                        Some(6) => self.open_empty_media(),
                        _ => {}
                    }
                });
            });
        });
    }
}

/// 逗号分隔的 glob 输入 → Vec（与 DirTab 的过滤输入一致）
fn split_globs_ui(s: &str) -> Vec<String> {
    s.split(',')
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

/// 扫描云盘/远程根目录，返回顶层条目（目录/文件相对路径），失败时返回 None
fn scan_cloud_root(url: &str) -> Option<Vec<String>> {
    let v = match crate::vfs::open(url.trim()) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("bcr: 打开 {url} 失败: {e}");
            return None;
        }
    };
    let filter = crate::fsscan::Filter::new(&[], &[]).expect("empty filter cannot fail");
    let map = match v.scan(&filter) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("bcr: 扫描 {url} 失败: {e}");
            return None;
        }
    };
    // 顶层条目：取每个 key 的第一段，去重排序
    let mut top: Vec<String> = map
        .keys()
        .filter_map(|k| k.split('/').next())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();
    top.sort();
    top.dedup();
    Some(top)
}

/// 拼接云 URL 与相对路径：URL 以 / 结尾则直接拼接，否则补一个 /
fn join_cloud_url(base: &str, rel: &str) -> String {
    let base = base.trim().trim_end_matches('/');
    format!("{base}/{rel}")
}

fn pick_file() -> Option<String> {
    let p = rfd::FileDialog::new().pick_file()?;
    Some(p.to_string_lossy().into_owned())
}

/// P39-2a：把设置中的编码 / 大小上限写入环境变量（对标 CLI --encoding/--max-size）
fn apply_settings_env(s: &Settings) {
    if s.encoding.is_empty() {
        std::env::remove_var("BCR_ENCODING");
    } else {
        std::env::set_var("BCR_ENCODING", &s.encoding);
    }
    match s.max_size {
        Some(mb) => std::env::set_var("BCR_MAX_SIZE", mb.to_string()),
        None => std::env::remove_var("BCR_MAX_SIZE"),
    }
}

fn pick_dir() -> Option<String> {
    let p = rfd::FileDialog::new().pick_folder()?;
    Some(p.to_string_lossy().into_owned())
}

/// P39-2c：从标签提取左右路径（会话保存用）
fn session_paths(t: &Tab) -> Option<(String, String)> {
    match t {
        Tab::Dir(d) => Some((d.left.clone(), d.right.clone())),
        Tab::Diff(d) => match (&d.left, &d.right) {
            (Some(l), Some(r)) => Some((l.path.clone(), r.path.clone())),
            _ => None,
        },
        _ => None,
    }
}

/// P39-2c：DiffTab 文本报告预览（统计 + 差异行摘要）
fn diff_report_preview(t: &crate::gui::difftab::DiffTab) -> String {
    let mut out = String::new();
    let s = &t.stats;
    out.push_str("bcr 文本对比报告\n");
    out.push_str(&format!(
        "统计: {} 相同, {} 仅左侧, {} 仅右侧, {} 修改\n",
        s.equal, s.delete, s.insert, s.replace
    ));
    out.push_str("----------------------------------------\n");
    for (i, row) in t.rows.iter().enumerate() {
        let tag = match row.tag {
            crate::sideview::RowTag::Equal => "=",
            crate::sideview::RowTag::Delete => "<",
            crate::sideview::RowTag::Insert => ">",
            crate::sideview::RowTag::Replace => "<",
        };
        let l = row.left.as_ref().map(|c| c.text.as_str()).unwrap_or("");
        let r = row.right.as_ref().map(|c| c.text.as_str()).unwrap_or("");
        if row.tag != crate::sideview::RowTag::Equal {
            out.push_str(&format!("{} {:>4} | {}\n", tag, i + 1, l));
            if row.tag == crate::sideview::RowTag::Replace {
                out.push_str(&format!("> {:>4} | {}\n", i + 1, r));
            }
        }
    }
    out
}

impl eframe::App for DiffApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // 记录窗口大小用于退出时持久化
        if let Some(rect) = ui.ctx().input(|i| i.viewport().inner_rect) {
            self.settings.window_size = Some([rect.width(), rect.height()]);
        }

        self.handle_dropped(ui.ctx());

        // B1 + P39-2a：全局快捷键（关闭标签 / 设置 / 新建标签 / 新建窗口 / 会话）
        self.handle_global_shortcuts(ui);

        // 顶部菜单栏（P33：BC 式标准菜单，替代扁平按钮排）
        egui::Panel::top("menu").show(ui, |ui| {
            menubar::menu_bar(self, ui);
        });

        // 标签栏（P33：菜单栏之下独立一行，BC 观感）
        egui::Panel::top("tabbar").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                // 标签栏
                let mut close: Option<usize> = None;
                let mut activate: Option<usize> = None;
                // B6：标签拖拽重排（拖拽标签到另一标签上 → 交换）
                let mut drag_req: Option<(usize, usize)> = None;
                for i in 0..self.tabs.len() {
                    let selected = i == self.active;
                    // P48-2：标签用 Frame 包裹——选中标签高亮底色+圆角（BC 观感），普通标签弱色
                    let tab_bg = if selected {
                        theme::tab_selected_bg(ui.visuals().dark_mode)
                    } else {
                        egui::Color32::TRANSPARENT
                    };
                    let text = if selected {
                        RichText::new(self.tabs[i].title()).strong()
                    } else {
                        RichText::new(self.tabs[i].title())
                    };
                    // P53-1：标签前加类型图标（单色 emoji 按类型着色，与主页卡片呼应）
                    let icon = self.tabs[i].icon();
                    let icon_c = self.tabs[i].icon_color();
                    let resp = egui::Frame::new()
                        .fill(tab_bg)
                        .corner_radius(6.0)
                        .inner_margin(egui::Margin::symmetric(8, 4))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(icon).size(14.0).color(icon_c));
                                // 选中态视觉由 selectable_label 提供，点击交给外层 Frame interact
                                let _ = ui.selectable_label(selected, text);
                            });
                        })
                        .response
                        .interact(egui::Sense::click())
                        .on_hover_cursor(egui::CursorIcon::PointingHand);
                    // B6：拖拽检测（拖拽标签标题区域）
                    if resp.drag_started() {
                        self.drag_tab = Some(i);
                    }
                    if let Some(from) = self.drag_tab {
                        if resp.dragged() && from != i {
                            drag_req = Some((from, i));
                        }
                        if resp.drag_stopped() {
                            self.drag_tab = None;
                        }
                    }
                    if resp.clicked() {
                        activate = Some(i);
                    }
                    // 关闭按钮（hover 变色；当前标签 hover 时红色提示）
                    let close_color = if resp.hovered() {
                        theme::diff_delete(ui.visuals().dark_mode)
                    } else if selected {
                        ui.visuals().strong_text_color()
                    } else {
                        ui.visuals().weak_text_color()
                    };
                    let close_resp = ui
                        .add(
                            egui::Button::new(RichText::new("✕").color(close_color))
                                .small()
                                .corner_radius(eframe::egui::CornerRadius::same(
                                    theme::CORNER as u8,
                                )),
                        )
                        .on_hover_text(crate::i18n::t(crate::i18n::Key::CloseTab));
                    if close_resp.clicked() {
                        close = Some(i);
                    }
                }
                if let Some((from, to)) = drag_req {
                    self.move_tab(from, to);
                    self.drag_tab = Some(if to > from { to - 1 } else { to });
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
            crate::gui::common::dialog_window(ui.ctx(), crate::i18n::t(crate::i18n::Key::GitTitle))
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
                        if ui
                            .button(crate::i18n::t(crate::i18n::Key::GitCopy))
                            .clicked()
                        {
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

        // P33：快捷键说明弹窗（Help > Shortcuts）
        if self.show_shortcuts {
            let mut keep = true;
            crate::gui::common::dialog_window(
                ui.ctx(),
                crate::i18n::t(crate::i18n::Key::MenuShortcuts),
            )
            .collapsible(false)
            .default_size([420.0, 380.0])
            .open(&mut keep)
            .show(ui.ctx(), |ui| {
                let rows = [
                    ("F5", "重新加载 / 刷新"),
                    ("F6", "下一差异"),
                    ("F7", "上一差异"),
                    ("F2", "重命名（目录对比）"),
                    ("⌘,", "设置"),
                    ("⌘T", "新建标签页"),
                    ("⌘N", "新建窗口"),
                    ("⌘L", "转到行"),
                    ("⌘G / ⇧⌘G", "查找下一 / 上一"),
                    ("⌥⌘S", "保存会话"),
                    ("⌥⌘C", "清除会话"),
                    ("1 / 2 / 3", "显示全部 / 差异 / 相同"),
                    ("⌘F", "查找"),
                    ("⌘Z", "撤销"),
                    ("⌘Y", "重做"),
                    ("⌘W", "关闭当前标签"),
                    ("Enter", "打开选中文件对比（目录）"),
                    ("双击行", "内联编辑"),
                ];
                egui::Grid::new("shortcut_grid")
                    .num_columns(2)
                    .striped(true)
                    .show(ui, |ui| {
                        for (k, v) in rows {
                            ui.monospace(k);
                            ui.label(v);
                            ui.end_row();
                        }
                    });
            });
            if !keep {
                self.show_shortcuts = false;
            }
        }

        // P33：关于弹窗（Help > About）
        if self.show_about {
            let mut keep = true;
            crate::gui::common::dialog_window(
                ui.ctx(),
                crate::i18n::t(crate::i18n::Key::MenuAbout),
            )
            .collapsible(false)
            .default_size([400.0, 260.0])
            .open(&mut keep)
            .show(ui.ctx(), |ui| {
                ui.label(RichText::new("bcr").size(26.0).strong());
                ui.label(format!(
                    "v{} — Beyond Compare 风格文件对比工具",
                    env!("CARGO_PKG_VERSION")
                ));
                ui.separator();
                ui.label("Rust + egui 实现");
                ui.label("文本 / 文件夹 / 三路合并 / 图片 / CSV / Hex 对比");
                ui.add_space(6.0);
                ui.label("GitHub: github.com/kevienkay/bcr");
            });
            if !keep {
                self.show_about = false;
            }
        }

        // P39-2a：设置对话框（⌘,）——忽略选项 / 编码 / 大小上限集中管理
        if self.show_settings {
            let mut keep = true;
            let mut close_req = false;
            crate::gui::common::dialog_window(
                ui.ctx(),
                crate::i18n::t(crate::i18n::Key::SettingsTitle),
            )
            .collapsible(false)
            .default_size([460.0, 440.0])
            .open(&mut keep)
            .show(ui.ctx(), |ui| {
                ui.label(
                    RichText::new(crate::i18n::t(crate::i18n::Key::SettingsIgnoreWs)).strong(),
                );
                ui.checkbox(
                    &mut self.settings.ignore_whitespace,
                    crate::i18n::t(crate::i18n::Key::SettingsIgnoreWs),
                );
                ui.checkbox(
                    &mut self.settings.ignore_trailing,
                    crate::i18n::t(crate::i18n::Key::SettingsIgnoreTrail),
                );
                ui.checkbox(
                    &mut self.settings.ignore_case,
                    crate::i18n::t(crate::i18n::Key::SettingsIgnoreCase),
                );
                ui.checkbox(
                    &mut self.settings.ignore_crlf,
                    crate::i18n::t(crate::i18n::Key::SettingsIgnoreCrlf),
                );
                ui.separator();
                // 外观：主题（系统/深色/浅色）——从 View 菜单移入设置（BC 设置集中管理）
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::t(crate::i18n::Key::Theme));
                    let mut pref = self.settings.theme_pref();
                    for (key, p) in [
                        (crate::i18n::Key::ThemeSystem, ThemePreference::System),
                        (crate::i18n::Key::ThemeDark, ThemePreference::Dark),
                        (crate::i18n::Key::ThemeLight, ThemePreference::Light),
                    ] {
                        if ui
                            .selectable_label(pref == p, crate::i18n::t(key))
                            .clicked()
                        {
                            pref = p;
                            self.settings.theme = match p {
                                ThemePreference::Dark => "dark".to_string(),
                                ThemePreference::Light => "light".to_string(),
                                _ => "system".to_string(),
                            };
                            self.settings.save();
                            ui.ctx().set_theme(p);
                        }
                    }
                });
                // 外观：语言（10 语言）——从 View 菜单移入设置
                ui.horizontal_wrapped(|ui| {
                    ui.label(crate::i18n::t(crate::i18n::Key::Language));
                    let mut new_lang = crate::i18n::current();
                    let mut lang_changed = false;
                    for l in crate::i18n::Lang::ALL {
                        if ui
                            .selectable_label(new_lang == l, l.native_name())
                            .clicked()
                        {
                            new_lang = l;
                            lang_changed = true;
                        }
                    }
                    if lang_changed {
                        self.settings.lang = new_lang.code().to_string();
                        self.settings.save();
                        crate::i18n::set_lang(new_lang);
                    }
                });
                ui.separator();
                // 编码（空 = 自动检测）
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::t(crate::i18n::Key::SettingsEncoding));
                    let encodings = [
                        "",
                        "utf-8",
                        "utf-16le",
                        "utf-16be",
                        "utf-32le",
                        "utf-32be",
                        "gbk",
                        "big5",
                        "shift_jis",
                    ];
                    let mut sel = self.settings.encoding.clone();
                    egui::ComboBox::from_id_salt("settings_encoding")
                        .selected_text(if sel.is_empty() {
                            "auto".to_string()
                        } else {
                            sel.clone()
                        })
                        .show_ui(ui, |ui| {
                            for e in encodings {
                                let label = if e.is_empty() {
                                    "auto".to_string()
                                } else {
                                    e.to_string()
                                };
                                if ui.selectable_label(sel == e, label).clicked() {
                                    sel = e.to_string();
                                }
                            }
                        });
                    self.settings.encoding = sel;
                });
                // 大小上限（MB，0 = 默认）
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::t(crate::i18n::Key::SettingsMaxSize));
                    let mut mb = self.settings.max_size.unwrap_or(0);
                    if ui
                        .add(egui::DragValue::new(&mut mb).range(0..=65536))
                        .changed()
                    {
                        self.settings.max_size = if mb == 0 { None } else { Some(mb) };
                    }
                    ui.label("MB");
                });
                ui.separator();
                ui.horizontal(|ui| {
                    let save = ui
                        .button(crate::i18n::t(crate::i18n::Key::MenuSaveSession))
                        .on_hover_text("保存设置到 ~/.bcr-gui.toml")
                        .clicked();
                    if save {
                        // 持久化 + 写入环境变量（编码 / 大小上限对标 CLI 行为）
                        apply_settings_env(&self.settings);
                        self.settings.save();
                        close_req = true;
                    }
                    if ui.button(crate::i18n::t(crate::i18n::Key::Close)).clicked() {
                        close_req = true;
                    }
                });
            });
            if close_req || !keep {
                self.show_settings = false;
            }
        }

        // P39-2c：报告生成弹窗（⌘P）——当前标签生成文本/HTML 报告
        if self.show_report {
            let mut keep = true;
            let mut save_req = false;
            crate::gui::common::dialog_window(
                ui.ctx(),
                crate::i18n::t(crate::i18n::Key::MenuReport),
            )
            .collapsible(false)
            .default_size([460.0, 340.0])
            .open(&mut keep)
            .show(ui.ctx(), |ui| {
                // 格式选择
                ui.horizontal(|ui| {
                    ui.label("格式:");
                    for (fmt, label) in [("txt", "文本 TXT"), ("html", "HTML")] {
                        if ui
                            .selectable_label(self.report_format == fmt, label)
                            .clicked()
                        {
                            self.report_format = fmt.to_string();
                        }
                    }
                });
                ui.separator();
                // 报告预览（当前标签）
                let preview = match self.tabs.get(self.active) {
                    Some(Tab::Dir(t)) => t
                        .result
                        .as_ref()
                        .map(|r| crate::report::render_txt(&t.left, &t.right, r)),
                    Some(Tab::Diff(t)) => Some(diff_report_preview(t)),
                    _ => None,
                };
                match preview {
                    Some(p) => {
                        egui::ScrollArea::vertical()
                            .max_height(160.0)
                            .show(ui, |ui| {
                                ui.monospace(p);
                            });
                        ui.separator();
                        if ui.button("💾 保存报告…").clicked() {
                            save_req = true;
                        }
                    }
                    None => {
                        ui.label("当前标签暂不支持报告（需目录对比结果或文本对比）");
                    }
                }
                if let Some(err) = &self.report_error {
                    ui.label(RichText::new(err).color(theme::error_color()).size(12.0));
                }
            });
            if save_req {
                self.save_current_report();
            }
            if !keep {
                self.show_report = false;
                self.report_error = None;
            }
        }

        // P42-4：图例弹窗 + 日志面板（BC View>图例/日志）
        self.legend_log_windows(ui);
        // P43-5：信息弹窗（BC Session>信息）
        self.info_window(ui);

        // P33：外部工具说明弹窗（Tools > 外部工具）
        if self.show_external {
            let mut keep = true;
            crate::gui::common::dialog_window(
                ui.ctx(),
                crate::i18n::t(crate::i18n::Key::MenuExternal),
            )
            .collapsible(false)
            .default_size([520.0, 300.0])
            .open(&mut keep)
            .show(ui.ctx(), |ui| {
                ui.label("通过 ~/.bcr-external.toml 把扩展名映射到第三方对比工具：");
                ui.monospace("[external]");
                ui.monospace(".docx = \"wps\"");
                ui.monospace(".pdf  = \"open\"");
                ui.add_space(6.0);
                ui.label("CLI 用法：bcr diff a.docx b.docx --external");
                ui.label("GUI 目录对比：右键文件 → 外部工具对比");
            });
            if !keep {
                self.show_external = false;
            }
        }

        // 会话中心弹窗：列出已保存会话，点击打开目录对比（收藏优先 + 最近使用排序）
        if self.show_sessions {
            let mut sessions = crate::session::load();
            let mut keep = true;
            let mut open_req: Option<(String, String)> = None;
            let mut delete_req: Option<String> = None;
            let mut fav_req: Option<String> = None;
            // 排序：收藏优先，其次最近使用
            let mut order: Vec<(String, bool, u64)> = sessions
                .sessions
                .iter()
                .map(|(n, s)| (n.clone(), s.favorite, s.last_used.unwrap_or(0)))
                .collect();
            order.sort_by(|a, b| b.1.cmp(&a.1).then(b.2.cmp(&a.2)));
            crate::gui::common::dialog_window(ui.ctx(), "会话中心")
                .collapsible(false)
                .default_size([560.0, 420.0])
                .open(&mut keep)
                .show(ui.ctx(), |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!(
                                "{} 个会话（{}）",
                                sessions.sessions.len(),
                                crate::session::sessions_path().display()
                            ))
                            .size(12.0)
                            .color(ui.visuals().weak_text_color()),
                        );
                    });
                    // P39-2c：保存当前会话（GUI 入口，替代命令行 bcr session save）
                    ui.horizontal(|ui| {
                        ui.label(crate::i18n::t(crate::i18n::Key::SessionName));
                        let resp = ui.add(
                            egui::TextEdit::singleline(&mut self.session_save_name)
                                .hint_text("my-session")
                                .desired_width(160.0),
                        );
                        let save_clicked = ui
                            .button(crate::i18n::t(crate::i18n::Key::SessionSaveCurrent))
                            .on_hover_text("保存当前标签（目录或文本对比）的左右路径为会话")
                            .clicked()
                            || (resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)));
                        if save_clicked {
                            let name = self.session_save_name.trim().to_string();
                            let cur = self.tabs.get(self.active).and_then(session_paths);
                            match (name.as_str(), cur) {
                                ("", _) => {
                                    self.report_error = Some("请输入会话名".to_string());
                                }
                                (_, Some((l, r))) => {
                                    let mut all = crate::session::load();
                                    all.sessions.insert(
                                        name,
                                        crate::session::Session {
                                            left: l,
                                            right: r,
                                            compare_content: false,
                                            detect_moves: true,
                                            includes: Vec::new(),
                                            excludes: Vec::new(),
                                            favorite: false,
                                            last_used: None,
                                        },
                                    );
                                    let _ = crate::session::save_all(&all);
                                    // 下一帧重新 load() 自动刷新列表
                                }
                                (_, None) => {
                                    self.report_error = Some("当前标签无可用路径".to_string());
                                }
                            }
                        }
                    });
                    if let Some(err) = &self.report_error {
                        ui.label(RichText::new(err).color(theme::error_color()).size(12.0));
                    }
                    ui.separator();
                    if sessions.sessions.is_empty() {
                        ui.label("暂无会话，可在上方输入名称保存当前对比");
                    }
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for (name, fav, _last) in &order {
                            let Some(s) = sessions.sessions.get(name) else {
                                continue;
                            };
                            ui.horizontal(|ui| {
                                // 收藏星标
                                let star = if *fav { "★" } else { "☆" };
                                if ui
                                    .small_button(star)
                                    .on_hover_text("收藏/取消收藏")
                                    .clicked()
                                {
                                    fav_req = Some(name.clone());
                                }
                                let opts = format!(
                                    "{}{}",
                                    if s.compare_content { " [hash]" } else { "" },
                                    if s.includes.is_empty() {
                                        String::new()
                                    } else {
                                        format!(" [inc:{}]", s.includes.join(","))
                                    }
                                );
                                if ui
                                    .button(format!("▶ {}", name))
                                    .on_hover_text(format!("{} ↔ {}", s.left, s.right))
                                    .clicked()
                                {
                                    open_req = Some((s.left.clone(), s.right.clone()));
                                }
                                ui.label(
                                    RichText::new(format!("{} ↔ {}{}", s.left, s.right, opts))
                                        .size(12.0),
                                );
                                if ui
                                    .small_button("✕")
                                    .on_hover_text(crate::i18n::t(crate::i18n::Key::DeleteSession))
                                    .clicked()
                                {
                                    delete_req = Some(name.clone());
                                }
                            });
                        }
                    });
                });
            // 收藏切换
            if let Some(name) = fav_req {
                if let Some(s) = sessions.sessions.get_mut(&name) {
                    s.favorite = !s.favorite;
                }
                let _ = crate::session::save_all(&sessions);
                self.show_sessions = false;
                self.show_sessions = true;
            }
            if let Some((l, r)) = open_req {
                // 记录最近使用时间
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                for s in sessions.sessions.values_mut() {
                    if s.left == l && s.right == r {
                        s.last_used = Some(now);
                    }
                }
                let _ = crate::session::save_all(&sessions);
                self.add_tab(Tab::Dir(DirTab::new(&l, &r)));
            }
            if let Some(name) = delete_req {
                let mut all = crate::session::load();
                all.sessions.remove(&name);
                let _ = crate::session::save_all(&all);
                // 关闭重开窗口以刷新
                self.show_sessions = false;
                self.show_sessions = true;
            }
            if !keep {
                self.show_sessions = false;
            }
        }

        // 规则(Profile)管理弹窗：列出/编辑/保存/应用/删除命名规则集
        if self.show_profiles {
            let mut keep = true;
            let mut apply_req: Option<(String, String, String)> = None; // (name, includes, excludes)
            let mut delete_req: Option<String> = None;
            let mut selected_name: Option<String> = None;
            let profiles = crate::profile::load();
            crate::gui::common::dialog_window(ui.ctx(), "比较规则 (Profile)")
                .collapsible(false)
                .resizable(true)
                .default_size([560.0, 440.0])
                .open(&mut keep)
                .show(ui.ctx(), |ui| {
                    ui.label(
                        RichText::new(format!(
                            "{} 个规则集（{}）",
                            profiles.profiles.len(),
                            crate::profile::profiles_path().display()
                        ))
                        .size(12.0)
                        .color(ui.visuals().weak_text_color()),
                    );
                    ui.separator();
                    // 左侧：规则列表；右侧：选中规则的编辑表单
                    ui.horizontal_top(|ui| {
                        // 列表
                        ui.group(|ui| {
                            ui.set_width(190.0);
                            ui.label("规则集");
                            egui::ScrollArea::vertical()
                                .max_height(300.0)
                                .show(ui, |ui| {
                                    for (name, p) in &profiles.profiles {
                                        let mut parts: Vec<&str> = Vec::new();
                                        if !p.includes.is_empty() {
                                            parts.push("inc");
                                        }
                                        if !p.excludes.is_empty() {
                                            parts.push("exc");
                                        }
                                        if p.ignore_whitespace {
                                            parts.push("ws");
                                        }
                                        if p.ignore_trailing {
                                            parts.push("trail");
                                        }
                                        if p.ignore_case {
                                            parts.push("case");
                                        }
                                        if p.compare_content {
                                            parts.push("hash");
                                        }
                                        let flags = if parts.is_empty() {
                                            String::new()
                                        } else {
                                            format!("  [{:?}]", parts)
                                        };
                                        if ui
                                            .selectable_label(
                                                selected_name.as_deref() == Some(name.as_str()),
                                                format!("{}{}", name, flags),
                                            )
                                            .clicked()
                                        {
                                            selected_name = Some(name.clone());
                                        }
                                    }
                                });
                        });
                        // 编辑表单（选中规则）
                        ui.group(|ui| {
                            ui.label("编辑");
                            if let Some(name) = &selected_name {
                                if let Some(p) = profiles.profiles.get(name) {
                                    let mut inc = p.includes.join(",");
                                    let mut exc = p.excludes.join(",");
                                    let mut iw = p.ignore_whitespace;
                                    let mut it = p.ignore_trailing;
                                    let mut ic = p.ignore_case;
                                    let mut cc = p.compare_content;
                                    let mut dm = p.detect_moves;
                                    let mut enc = p.encoding.clone().unwrap_or_default();
                                    ui.horizontal(|ui| {
                                        ui.label("include");
                                        ui.add(
                                            egui::TextEdit::singleline(&mut inc)
                                                .desired_width(180.0),
                                        );
                                    });
                                    ui.horizontal(|ui| {
                                        ui.label("exclude");
                                        ui.add(
                                            egui::TextEdit::singleline(&mut exc)
                                                .desired_width(180.0),
                                        );
                                    });
                                    ui.checkbox(&mut iw, "忽略空白");
                                    ui.checkbox(&mut it, "忽略行尾空白");
                                    ui.checkbox(&mut ic, "忽略大小写");
                                    ui.checkbox(&mut cc, "内容哈希");
                                    ui.checkbox(&mut dm, "移动检测");
                                    ui.horizontal(|ui| {
                                        ui.label("编码");
                                        ui.add(
                                            egui::TextEdit::singleline(&mut enc)
                                                .desired_width(120.0)
                                                .hint_text("auto"),
                                        );
                                    });
                                    ui.separator();
                                    ui.horizontal(|ui| {
                                        if ui.button("保存修改").clicked() {
                                            let mut all = crate::profile::load();
                                            if let Some(slot) = all.profiles.get_mut(name) {
                                                slot.includes = split_globs_ui(&inc);
                                                slot.excludes = split_globs_ui(&exc);
                                                slot.ignore_whitespace = iw;
                                                slot.ignore_trailing = it;
                                                slot.ignore_case = ic;
                                                slot.compare_content = cc;
                                                slot.detect_moves = dm;
                                                slot.encoding = if enc.trim().is_empty() {
                                                    None
                                                } else {
                                                    Some(enc.trim().to_string())
                                                };
                                            }
                                            let _ = crate::profile::save_all(&all);
                                        }
                                        if ui.button("应用").clicked() {
                                            apply_req = Some((name.clone(), inc, exc));
                                        }
                                        if ui.button("删除").clicked() {
                                            delete_req = Some(name.clone());
                                        }
                                    });
                                }
                            } else {
                                ui.label("左侧选择一个规则集，或用命令行 bcr profile save 新建");
                            }
                        });
                    });
                });
            if let Some(name) = delete_req {
                let mut all = crate::profile::load();
                all.profiles.remove(&name);
                let _ = crate::profile::save_all(&all);
                self.show_profiles = false;
                self.show_profiles = true;
            }
            if let Some((name, inc, exc)) = apply_req {
                // 应用规则到当前目录对比标签（若有）
                let mut applied = false;
                if let Some(Tab::Dir(t)) = self.tabs.get_mut(self.active) {
                    t.includes = inc;
                    t.excludes = exc;
                    t.refresh();
                    applied = true;
                }
                if !applied {
                    // 无活动目录对比：提示
                    let _ = name;
                }
            }
            if !keep {
                self.show_profiles = false;
            }
        }

        // 云盘/远程浏览弹窗：输入 webdav:// s3:// onedrive:// dropbox:// sftp:// ftp:// 等 URL，
        // 扫描根目录列出条目，选择后打开目录对比
        if self.show_cloud {
            let mut keep = true;
            let mut open_req: Option<(String, String)> = None;
            let mut close_req = false;
            let mut scan_left = false;
            let mut scan_right = false;
            crate::gui::common::dialog_window(ui.ctx(), "☁ 云盘/远程目录")
                .collapsible(false)
                .resizable(true)
                .default_size([620.0, 460.0])
                .open(&mut keep)
                .show(ui.ctx(), |ui| {
                    ui.label(
                        RichText::new(
                            "支持 webdav:// webdavs:// s3:// onedrive:// dropbox:// sftp:// ftp:// 前缀；\n本地路径也可直接输入",
                        )
                        .size(12.0)
                        .color(ui.visuals().weak_text_color()),
                    );
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label("左侧");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.cloud_left)
                                .hint_text("webdav://... 或 /本地/路径")
                                .desired_width(340.0),
                        );
                        if ui.button("扫描").clicked() {
                            scan_left = true;
                        }
                    });
                    if let Some(entries) = &self.cloud_left_entries {
                        egui::ScrollArea::vertical()
                            .max_height(140.0)
                            .show(ui, |ui| {
                                for rel in entries {
                                    if ui.selectable_label(false, rel).clicked() {
                                        self.cloud_left = join_cloud_url(&self.cloud_left, rel);
                                        scan_left = true;
                                    }
                                }
                            });
                    }
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label("右侧");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.cloud_right)
                                .hint_text("webdav://... 或 /本地/路径")
                                .desired_width(340.0),
                        );
                        if ui.button("扫描").clicked() {
                            scan_right = true;
                        }
                    });
                    if let Some(entries) = &self.cloud_right_entries {
                        egui::ScrollArea::vertical()
                            .max_height(140.0)
                            .show(ui, |ui| {
                                for rel in entries {
                                    if ui.selectable_label(false, rel).clicked() {
                                        self.cloud_right = join_cloud_url(&self.cloud_right, rel);
                                        scan_right = true;
                                    }
                                }
                            });
                    }
                    if let Some(err) = &self.cloud_err {
                        ui.colored_label(theme::error_color(), err);
                    }
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("打开目录对比").clicked() {
                            let l = self.cloud_left.trim().to_string();
                            let r = self.cloud_right.trim().to_string();
                            if !l.is_empty() && !r.is_empty() {
                                open_req = Some((l, r));
                            }
                        }
                        if ui.button(crate::i18n::t(crate::i18n::Key::Close)).clicked() {
                            close_req = true;
                        }
                    });
                });
            if scan_left {
                self.cloud_left_entries = scan_cloud_root(&self.cloud_left);
                self.cloud_err = None;
            }
            if scan_right {
                self.cloud_right_entries = scan_cloud_root(&self.cloud_right);
                self.cloud_err = None;
            }
            if let Some((l, r)) = open_req {
                self.add_tab(Tab::Dir(DirTab::new(&l, &r)));
                self.show_cloud = false;
            }
            if close_req || !keep {
                self.show_cloud = false;
            }
        }

        // 底部全局状态栏（P31，对标 BC 状态栏）——面板须在 CentralPanel 之前声明：
        // egui 的 CentralPanel 会占满全部剩余空间，其后声明的 bottom 面板拿不到高度（实测不渲染）。
        if !self.tabs.is_empty() {
            self.status_bar(ui);
        }

        // 当前标签内容
        if self.tabs.is_empty() {
            self.welcome_ui(ui);
            return;
        }

        // 处理各标签可能发出的请求
        let mut open_diff_req: Option<String> = None;
        let mut open_pair_req: Option<(String, String)> = None;
        {
            let active = self.active;
            match &mut self.tabs[active] {
                Tab::Diff(t) => t.ui(ui),
                Tab::Dir(t) => {
                    t.ui(ui);
                    open_diff_req = t.open_diff.take();
                    open_pair_req = t.open_pair.take();
                }
                Tab::Merge(t) => t.ui(ui),
                Tab::Image(t) => t.ui(ui),
                Tab::Csv(t) => t.ui(ui),
                Tab::TextEdit(t) => t.ui(ui),
                Tab::Patch(t) => t.ui(ui),
                Tab::FolderMerge(t) => t.ui(ui),
                Tab::Media(t) => t.ui(ui),
            }
        }


        if let Some((l_rel, r_rel)) = open_pair_req {
            // 手动对齐：左右相对路径配对打开并排 diff（可不同文件名）
            if let Some(Tab::Dir(dir_tab)) = self.tabs.get(self.active) {
                let (l_root, r_root) = (dir_tab.left.clone(), dir_tab.right.clone());
                let l = std::path::Path::new(&l_root).join(&l_rel);
                let r = std::path::Path::new(&r_root).join(&r_rel);
                let ls = l.to_string_lossy();
                let rs = r.to_string_lossy();
                if crate::imgcmp::is_image_file(&ls) && crate::imgcmp::is_image_file(&rs) {
                    self.add_tab(Tab::Image(ImageTab::new(&ls, &rs)));
                } else if crate::csvcmp::is_csv_file(&ls) && crate::csvcmp::is_csv_file(&rs) {
                    self.add_tab(Tab::Csv(CsvTab::new(&ls, &rs)));
                } else {
                    let mut t = DiffTab::new();
                    t.load_pair(&ls, &rs, ViewOptions::default());
                    self.add_tab(Tab::Diff(t));
                }
                return;
            }
        }

        if let Some(rel) = open_diff_req {
            // 目录对比双击 → 打开该文件的并排 diff（用 Path::join 保证跨平台分隔符）
            if let Some(Tab::Dir(dir_tab)) = self.tabs.get(self.active) {
                let (l, r) = (dir_tab.left.clone(), dir_tab.right.clone());
                let l = std::path::Path::new(&l).join(&rel);
                let r = std::path::Path::new(&r).join(&rel);
                // 图片文件 → 图片对比标签
                let ls = l.to_string_lossy();
                let rs = r.to_string_lossy();
                if crate::imgcmp::is_image_file(&ls) && crate::imgcmp::is_image_file(&rs) {
                    self.add_tab(Tab::Image(ImageTab::new(&ls, &rs)));
                    return;
                }
                // CSV 表格 → 表格对比标签（P29）
                if crate::csvcmp::is_csv_file(&ls) && crate::csvcmp::is_csv_file(&rs) {
                    self.add_tab(Tab::Csv(CsvTab::new(&ls, &rs)));
                    return;
                }
                let mut t = DiffTab::new();
                t.load_pair(&ls, &rs, ViewOptions::default());
                self.add_tab(Tab::Diff(t));
            }
        }
    }

    fn on_exit(&mut self) {
        self.settings.save();
    }
}

/// P39-2c：保存当前标签报告到用户选择的文件
impl DiffApp {
    fn status_bar(&self, ui: &mut egui::Ui) {
        // 底部全局状态栏（P31，对标 BC 状态栏：当前标签统计汇总；B3 补路径/行列数/选中项数）
        egui::Panel::bottom("status_bar").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add_space(6.0);
                if let Some(tab) = self.tabs.get(self.active) {
                    match tab {
                        Tab::Diff(t) => {
                            let s = &t.stats;
                            // B3：当前标签路径 + 行数
                            let paths = match (&t.left, &t.right) {
                                (Some(l), Some(r)) => format!(
                                    "{} ↔ {}",
                                    std::path::Path::new(&l.path)
                                        .file_name()
                                        .map(|s| s.to_string_lossy().into_owned())
                                        .unwrap_or_else(|| l.path.clone()),
                                    std::path::Path::new(&r.path)
                                        .file_name()
                                        .map(|s| s.to_string_lossy().into_owned())
                                        .unwrap_or_else(|| r.path.clone()),
                                ),
                                (Some(l), None) => std::path::Path::new(&l.path)
                                    .file_name()
                                    .map(|s| s.to_string_lossy().into_owned())
                                    .unwrap_or_else(|| l.path.clone()),
                                (None, Some(r)) => std::path::Path::new(&r.path)
                                    .file_name()
                                    .map(|s| s.to_string_lossy().into_owned())
                                    .unwrap_or_else(|| r.path.clone()),
                                (None, None) => String::new(),
                            };
                            // P47-3：路径用弱色（BC 状态栏分区观感），统计用彩色（同/删/增/改）
                            ui.label(RichText::new(paths).weak());
                            ui.separator();
                            ui.label(RichText::new(format!("行 {}", t.rows.len())).weak());
                            ui.separator();
                            let (sc, sd, si, sr) = (
                                crate::i18n::t(crate::i18n::Key::StatSame),
                                crate::i18n::t(crate::i18n::Key::StatDelete),
                                crate::i18n::t(crate::i18n::Key::StatInsert),
                                crate::i18n::t(crate::i18n::Key::StatReplace),
                            );
                            ui.label(
                                RichText::new(format!("{} {}", sc, s.equal))
                                    .color(theme::stat_same(ui.visuals().dark_mode)),
                            );
                            ui.label(
                                RichText::new(format!("{} {}", sd, s.delete))
                                    .color(theme::stat_delete(ui.visuals().dark_mode)),
                            );
                            ui.label(
                                RichText::new(format!("{} {}", si, s.insert))
                                    .color(theme::stat_insert(ui.visuals().dark_mode)),
                            );
                            ui.label(
                                RichText::new(format!("{} {}", sr, s.replace))
                                    .color(theme::stat_modify(ui.visuals().dark_mode)),
                            );
                            // P47-3：右侧编码/大小弱色（右对齐）
                            if let Some(l) = &t.left {
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.label(
                                            RichText::new(format!(
                                                "{} · {}B",
                                                l.encoding.name(),
                                                l.size
                                            ))
                                            .weak(),
                                        );
                                    },
                                );
                            }
                        }
                        Tab::Dir(t) => {
                            let dark = ui.visuals().dark_mode;
                            // 左：路径弱色（BC 分区观感）
                            let paths = format!(
                                "{} ↔ {}",
                                std::path::Path::new(&t.left)
                                    .file_name()
                                    .map(|s| s.to_string_lossy().into_owned())
                                    .unwrap_or_else(|| t.left.clone()),
                                std::path::Path::new(&t.right)
                                    .file_name()
                                    .map(|s| s.to_string_lossy().into_owned())
                                    .unwrap_or_else(|| t.right.clone()),
                            );
                            ui.label(RichText::new(paths).weak());
                            ui.separator();
                            if let Some(r) = &t.result {
                                let s = &r.stats;
                                // 彩色统计（同绿/仅左红/仅右绿/修改黄）
                                ui.label(
                                    RichText::new(format!(
                                        "{} {}",
                                        crate::i18n::t(crate::i18n::Key::StatSame),
                                        s.same
                                    ))
                                    .color(theme::stat_same(dark)),
                                );
                                ui.label(
                                    RichText::new(format!(
                                        "{} {}",
                                        crate::i18n::t(crate::i18n::Key::StatDelete),
                                        s.left_only
                                    ))
                                    .color(theme::stat_delete(dark)),
                                );
                                ui.label(
                                    RichText::new(format!(
                                        "{} {}",
                                        crate::i18n::t(crate::i18n::Key::StatInsert),
                                        s.right_only
                                    ))
                                    .color(theme::stat_insert(dark)),
                                );
                                ui.label(
                                    RichText::new(format!(
                                        "{} {}",
                                        crate::i18n::t(crate::i18n::Key::StatReplace),
                                        s.differ
                                    ))
                                    .color(theme::stat_modify(dark)),
                                );
                                // 右对齐：选中项数
                                let sel = t.selected.map(|i| i + 1).unwrap_or(0);
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.label(
                                            RichText::new(format!(
                                                "{} {}/{}",
                                                crate::i18n::t(crate::i18n::Key::StatsSelected),
                                                sel,
                                                t.flat.len()
                                            ))
                                            .weak(),
                                        );
                                    },
                                );
                            } else {
                                ui.label(crate::i18n::t(crate::i18n::Key::Hint));
                            }
                        }
                        Tab::Csv(t) => {
                            let dark = ui.visuals().dark_mode;
                            let s = t.stats();
                            // 左：路径弱色
                            let bn = |p: &str| {
                                std::path::Path::new(p)
                                    .file_name()
                                    .map(|x| x.to_string_lossy().into_owned())
                                    .unwrap_or_else(|| p.to_string())
                            };
                            ui.label(
                                RichText::new(format!("{} ↔ {}", bn(&t.left), bn(&t.right))).weak(),
                            );
                            ui.separator();
                            ui.label(
                                RichText::new(format!("{} × {}", t.row_count(), t.col_count()))
                                    .weak(),
                            );
                            ui.separator();
                            // 彩色统计
                            ui.label(
                                RichText::new(format!(
                                    "{} {}",
                                    crate::i18n::t(crate::i18n::Key::StatSame),
                                    s.same
                                ))
                                .color(theme::stat_same(dark)),
                            );
                            ui.label(
                                RichText::new(format!(
                                    "{} {}",
                                    crate::i18n::t(crate::i18n::Key::StatDelete),
                                    s.left_only
                                ))
                                .color(theme::stat_delete(dark)),
                            );
                            ui.label(
                                RichText::new(format!(
                                    "{} {}",
                                    crate::i18n::t(crate::i18n::Key::StatInsert),
                                    s.right_only
                                ))
                                .color(theme::stat_insert(dark)),
                            );
                            ui.label(
                                RichText::new(format!(
                                    "{} {}",
                                    crate::i18n::t(crate::i18n::Key::StatReplace),
                                    s.modified
                                ))
                                .color(theme::stat_modify(dark)),
                            );
                        }
                        Tab::Merge(t) => {
                            let dark = ui.visuals().dark_mode;
                            // 左：路径弱色
                            let bn = |p: &str| {
                                std::path::Path::new(p)
                                    .file_name()
                                    .map(|x| x.to_string_lossy().into_owned())
                                    .unwrap_or_else(|| p.to_string())
                            };
                            ui.label(
                                RichText::new(format!(
                                    "{} ↔ {} ↔ {}",
                                    bn(&t.base_path),
                                    bn(&t.left_path),
                                    bn(&t.right_path)
                                ))
                                .weak(),
                            );
                            ui.separator();
                            // 冲突数（有冲突黄 / 无冲突绿）
                            let (label, color) = if t.view.conflicts > 0 {
                                (
                                    format!(
                                        "{} {}",
                                        crate::i18n::t(crate::i18n::Key::ConflictsCount),
                                        t.view.conflicts
                                    ),
                                    theme::conflict_color(dark),
                                )
                            } else {
                                (
                                    crate::i18n::t(crate::i18n::Key::MergeAllResolved).to_string(),
                                    theme::resolved_color(dark),
                                )
                            };
                            ui.label(RichText::new(label).color(color));
                        }
                        Tab::Image(t) => {
                            let dark = ui.visuals().dark_mode;
                            let bn = |p: &str| {
                                std::path::Path::new(p)
                                    .file_name()
                                    .map(|x| x.to_string_lossy().into_owned())
                                    .unwrap_or_else(|| p.to_string())
                            };
                            ui.label(
                                RichText::new(format!("{} ↔ {}", bn(&t.left), bn(&t.right))).weak(),
                            );
                            ui.separator();
                            let diff_frames = t.frame_diffs.iter().filter(|d| **d).count();
                            // 有差异红 / 无差异绿
                            let (label, color) = if diff_frames > 0 {
                                (
                                    format!(
                                        "{} {}/{}",
                                        crate::i18n::t(crate::i18n::Key::StatReplace),
                                        diff_frames,
                                        t.frame_diffs.len()
                                    ),
                                    theme::img_diff(dark),
                                )
                            } else {
                                (
                                    format!(
                                        "{} {}/{}",
                                        crate::i18n::t(crate::i18n::Key::StatSame),
                                        diff_frames,
                                        t.frame_diffs.len()
                                    ),
                                    theme::img_same(dark),
                                )
                            };
                            ui.label(RichText::new(label).color(color));
                        }
                        Tab::TextEdit(t) => {
                            // 左：路径弱色 + 行/字符数
                            let bn = std::path::Path::new(&t.path)
                                .file_name()
                                .map(|x| x.to_string_lossy().into_owned())
                                .unwrap_or_else(|| t.path.clone());
                            ui.label(RichText::new(bn).weak());
                            ui.separator();
                            ui.label(
                                RichText::new(format!("{} | {}", t.line_count(), t.char_count()))
                                    .weak(),
                            );
                        }
                        Tab::Patch(t) => {
                            let dark = ui.visuals().dark_mode;
                            if let Some(p) = &t.parsed {
                                // 左：路径弱色 + 新增绿/删除红
                                let bn = std::path::Path::new(&t.path)
                                    .file_name()
                                    .map(|x| x.to_string_lossy().into_owned())
                                    .unwrap_or_else(|| t.path.clone());
                                ui.label(RichText::new(bn).weak());
                                ui.separator();
                                ui.label(
                                    RichText::new(format!(
                                        "{} {}",
                                        crate::i18n::t(crate::i18n::Key::PatchAdded),
                                        p.added
                                    ))
                                    .color(theme::stat_insert(dark)),
                                );
                                ui.label(
                                    RichText::new(format!(
                                        "{} {}",
                                        crate::i18n::t(crate::i18n::Key::PatchRemoved),
                                        p.removed
                                    ))
                                    .color(theme::stat_delete(dark)),
                                );
                            } else {
                                ui.label(crate::i18n::t(crate::i18n::Key::PatchTitle));
                            }
                        }
                        Tab::FolderMerge(t) => {
                            let dark = ui.visuals().dark_mode;
                            let s = t.stats;
                            // 左：路径弱色 + 彩色统计（复制蓝/合并黄/冲突红）
                            let bn = |p: &str| {
                                std::path::Path::new(p)
                                    .file_name()
                                    .map(|x| x.to_string_lossy().into_owned())
                                    .unwrap_or_else(|| p.to_string())
                            };
                            ui.label(
                                RichText::new(format!("{} ↔ {}", bn(&t.left), bn(&t.right))).weak(),
                            );
                            ui.separator();
                            ui.label(
                                RichText::new(format!(
                                    "{} {}",
                                    crate::i18n::t(crate::i18n::Key::StatInsert),
                                    s.copied
                                ))
                                .color(theme::plan_copy(dark)),
                            );
                            ui.label(
                                RichText::new(format!(
                                    "{} {}",
                                    crate::i18n::t(crate::i18n::Key::StatReplace),
                                    s.merged
                                ))
                                .color(theme::plan_merge(dark)),
                            );
                            if s.conflicts > 0 {
                                ui.label(
                                    RichText::new(format!(
                                        "{} {}",
                                        crate::i18n::t(crate::i18n::Key::ConflictsCount),
                                        s.conflicts
                                    ))
                                    .color(theme::img_diff(dark)),
                                );
                            }
                        }
                        Tab::Media(t) => {
                            let dark = ui.visuals().dark_mode;
                            let bn = |p: &str| {
                                std::path::Path::new(p)
                                    .file_name()
                                    .map(|s| s.to_string_lossy().into_owned())
                                    .unwrap_or_else(|| p.to_string())
                            };
                            ui.label(
                                RichText::new(format!("{} ↔ {}", bn(&t.left), bn(&t.right))).weak(),
                            );
                            ui.separator();
                            // 差异字段数（有差异红/无差异绿）
                            let (label, color) = if t.diffs.is_empty() {
                                (
                                    crate::i18n::t(crate::i18n::Key::StatSame).to_string(),
                                    theme::img_same(dark),
                                )
                            } else {
                                (
                                    format!(
                                        "{} {}",
                                        crate::i18n::t(crate::i18n::Key::StatReplace),
                                        t.diffs.len()
                                    ),
                                    theme::img_diff(dark),
                                )
                            };
                            ui.label(RichText::new(label).color(color));
                        }
                    }
                } else {
                    ui.label(crate::i18n::t(crate::i18n::Key::Hint));
                }
            });
        });
    }

    fn save_current_report(&mut self) {
        self.report_error = None;
        let (content, default_name) = match self.tabs.get(self.active) {
            Some(Tab::Dir(t)) => match &t.result {
                Some(r) => {
                    if self.report_format == "html" {
                        let html = crate::htmlreport::render_html(
                            &t.left,
                            &t.right,
                            r,
                            &crate::i18n::fmt(crate::i18n::Key::ReportGeneratedAt, &[]),
                        );
                        (html, "compare.html".to_string())
                    } else {
                        (
                            crate::report::render_txt(&t.left, &t.right, r),
                            "compare.txt".to_string(),
                        )
                    }
                }
                None => {
                    self.report_error = Some("目录对比尚未完成，请先刷新".to_string());
                    return;
                }
            },
            Some(Tab::Diff(t)) => (diff_report_preview(t), "diff.txt".to_string()),
            _ => {
                self.report_error = Some("当前标签暂不支持报告".to_string());
                return;
            }
        };
        let Some(path) = rfd::FileDialog::new()
            .set_file_name(&default_name)
            .save_file()
        else {
            return;
        };
        match std::fs::write(&path, content) {
            Ok(()) => {
                self.report_error = Some(crate::i18n::fmt(
                    crate::i18n::Key::ReportSaved,
                    &[&path.display().to_string()],
                ));
            }
            Err(e) => {
                self.report_error = Some(format!("保存报告失败: {}", e));
            }
        }
    }
}

/// 加载系统字体：等宽优先（JetBrains Mono / 系统等宽）+ CJK fallback（egui 默认字体不含 CJK）。
/// 按平台探测常见字体路径，全部存在的都加载：
/// - Monospace 族首位插入等宽字体（行号/代码/hex 观感提升）
/// - Proportional/Monospace 末尾追加 CJK fallback（中/日/韩/阿拉伯，保证 10 语言可显示）
fn install_cjk_fonts(ctx: &egui::Context) {
    // 等宽字体候选（按优先级，找到即作为 Monospace 首选）
    let mono_candidates: &[&str] = if cfg!(target_os = "windows") {
        &[
            "C:\\Windows\\Fonts\\JetBrainsMono-Regular.ttf",
            "C:\\Windows\\Fonts\\Consolas.ttf",
            "C:\\Windows\\Fonts\\cour.ttf",
        ]
    } else if cfg!(target_os = "macos") {
        &[
            "/Library/Fonts/JetBrainsMono-Regular.ttf",
            "/System/Library/Fonts/SFNSMono.ttf",
            "/System/Library/Fonts/Menlo.ttc",
        ]
    } else {
        &[
            "/usr/share/fonts/truetype/jetbrains/JetBrainsMono-Regular.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationMono-Regular.ttf",
        ]
    };
    // CJK fallback 候选（每平台按优先级）
    let cjk_candidates: &[&str] = if cfg!(target_os = "windows") {
        &[
            "C:\\Windows\\Fonts\\msyh.ttc",     // 微软雅黑（中日韩）
            "C:\\Windows\\Fonts\\simhei.ttf",   // 黑体
            "C:\\Windows\\Fonts\\simsun.ttc",   // 宋体
            "C:\\Windows\\Fonts\\malgun.ttf",   // Malgun Gothic（韩文）
            "C:\\Windows\\Fonts\\seguiemj.ttf", // Segoe UI（阿拉伯文）
        ]
    } else if cfg!(target_os = "macos") {
        &[
            "/System/Library/Fonts/PingFang.ttc", // 中日韩
            "/System/Library/Fonts/STHeiti Light.ttc",
            "/System/Library/Fonts/Hiragino Sans GB.ttc",
            "/System/Library/Fonts/AppleSDGothicNeo.ttc", // 韩文
            "/System/Library/Fonts/GeezaPro.ttc",         // 阿拉伯文
            "/System/Library/Fonts/SFArabic.ttf",         // 阿拉伯文（SF Arabic）
            "/Library/Fonts/Arial Unicode.ttf",           // 全字符集保底
        ]
    } else {
        &[
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
            "/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc",
            "/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf",
            "/usr/share/fonts/opentype/noto/NotoSansKR-Regular.otf", // 韩文（若有）
            "/usr/share/fonts/truetype/noto/NotoSansArabic-Regular.ttf", // 阿拉伯（若有）
        ]
    };
    let mut fonts = egui::FontDefinitions::default();
    let mut loaded_any = false;
    // 1. 等宽字体：Monospace 族首位
    for (i, p) in mono_candidates.iter().enumerate() {
        if std::path::Path::new(p).exists() {
            if let Ok(bytes) = std::fs::read(p) {
                let name = format!("mono_{i}");
                fonts
                    .font_data
                    .insert(name.clone(), egui::FontData::from_owned(bytes).into());
                fonts
                    .families
                    .entry(egui::FontFamily::Monospace)
                    .or_default()
                    .insert(0, name.clone());
                loaded_any = true;
            }
        }
    }
    // 2. CJK fallback：追加到比例字体与等宽字体末尾（保留默认拉丁字体）
    for (i, p) in cjk_candidates.iter().enumerate() {
        if std::path::Path::new(p).exists() {
            if let Ok(bytes) = std::fs::read(p) {
                let name = format!("fallback_{i}");
                fonts
                    .font_data
                    .insert(name.clone(), egui::FontData::from_owned(bytes).into());
                for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                    fonts.families.entry(family).or_default().push(name.clone());
                }
                loaded_any = true;
            }
        }
    }
    if loaded_any {
        ctx.set_fonts(fonts);
    }
}

/// 运行 GUI 事件循环，返回进程退出码
pub fn run(args: &GuiArgs) -> i32 {
    let settings = Settings::load();
    // P39-2a：启动时把持久化的编码 / 大小上限写入环境变量（对标 CLI 行为）
    apply_settings_env(&settings);
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
        ignore_crlf: args.ignore_crlf,
        ignore_lines: args.ignore_lines.clone(),
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
    } else if let Some(f) = &args.edit {
        // P37-1g：文本编辑（BC -edit 单文件）
        app.add_tab(Tab::TextEdit(TextEditTab::new(f)));
    } else if let Some(f) = &args.patch {
        // P37-1h：补丁视图（BC Text Patch 单文件）
        app.add_tab(Tab::Patch(PatchTab::new(f)));
    } else if let Some(m) = &args.merge_dir {
        // P37-1i：文件夹合并（BASE LEFT RIGHT OUT）
        if m.len() == 4 {
            app.add_tab(Tab::FolderMerge(FolderMergeTab::new(
                &m[0], &m[1], &m[2], &m[3],
            )));
        }
    } else {
        match (&args.left, &args.right) {
            (Some(l), Some(r)) => {
                let lp = std::path::Path::new(l);
                let rp = std::path::Path::new(r);
                if lp.is_dir() && rp.is_dir() {
                    app.add_tab(Tab::Dir(DirTab::new(l, r)));
                } else if crate::imgcmp::is_image_file(l) && crate::imgcmp::is_image_file(r) {
                    app.add_tab(Tab::Image(ImageTab::new(l, r)));
                } else if crate::csvcmp::is_csv_file(l) && crate::csvcmp::is_csv_file(r) {
                    app.add_tab(Tab::Csv(CsvTab::new(l, r)));
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
            theme::apply(&cc.egui_ctx);
            for theme in [egui::Theme::Dark, egui::Theme::Light] {
                cc.egui_ctx.style_mut_of(theme, |style| {
                    style.text_styles.insert(
                        egui::TextStyle::Monospace,
                        egui::FontId::monospace(FONT_SIZE),
                    );
                });
            }
            cc.egui_ctx.set_theme(theme_pref);
            Ok(Box::new(app))
        }),
    ) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!(
                "bcr: {}",
                crate::i18n::fmt(crate::i18n::Key::GuiFail, &[&e.to_string()])
            );
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
    use crate::sync::SyncOp;
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
        let (path, side) = t
            .editing
            .as_ref()
            .map(|e| (e.path.clone(), e.side))
            .unwrap();
        fs::write(&path, "a\nEDITED\n").unwrap();
        match side {
            EditSide::Left => t.load_left(&path, t.opts.clone()),
            EditSide::Right => t.load_right(&path, t.opts.clone()),
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
        t.refresh_sync();
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
        // 必须显式改为 All：默认 Diff 过滤会把两侧相同文件滤掉 → flat 为空 → unwrap 失败
        // （macOS 上 mtime 巧合导致偶发通过，Windows 稳定复现）
        t.view_filter = dirtab::ViewFilter::All;
        t.refresh_sync();
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
        t.refresh_sync();
        egui::__run_test_ui(|ui| {
            t.ui(ui);
        });
        assert!(t.result.is_some());
    }

    // ---- DirTab：同步面板 ----------------

    #[test]
    fn dirtab_sync_plan_and_execute() {
        let d1 = tempdir().unwrap();
        let d2 = tempdir().unwrap();
        // 统一 mtime 避免快速模式误判
        let fixed = filetime::FileTime::from_unix_time(1_700_000_000, 0);
        let w = |dir: &std::path::Path, name: &str, content: &str| {
            let p = dir.join(name);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(&p, content).unwrap();
            filetime::set_file_mtime(&p, fixed).unwrap();
        };
        w(d1.path(), "new.txt", "hello");
        w(d1.path(), "same.txt", "same");
        w(d2.path(), "same.txt", "same");
        w(d2.path(), "only.txt", "dst-only");
        let mut t = DirTab::new(d1.path().to_str().unwrap(), d2.path().to_str().unwrap());
        t.refresh_sync();
        // update 模式：复制 new.txt 到右侧，保留 only.txt
        t.gen_sync_plan();
        let plan = t.sync_plan.as_ref().unwrap();
        assert!(plan
            .iter()
            .any(|op| matches!(op, SyncOp::Copy { rel, from_src: true, .. } if rel == "new.txt")));
        assert!(plan
            .iter()
            .any(|op| matches!(op, SyncOp::Skip { rel, .. } if rel == "only.txt")));
        // 执行勾选
        t.run_sync_checked_sync();
        assert_eq!(
            fs::read_to_string(d2.path().join("new.txt")).unwrap(),
            "hello"
        );
        assert!(d2.path().join("only.txt").exists());
        // 再生成计划：new.txt 已一致，无可执行项（only.txt 仍为 Skip 标记）
        t.gen_sync_plan();
        let plan2 = t.sync_plan.as_ref().unwrap();
        assert!(!plan2.iter().any(|op| {
            matches!(
                op,
                SyncOp::Copy { .. }
                    | SyncOp::Delete { .. }
                    | SyncOp::Rename { .. }
                    | SyncOp::RmDir { .. }
            )
        }));
        // 勾选集合为空（无可执行项）
        assert!(t.sync_checked.is_empty());
    }

    #[test]
    fn dirtab_single_op_copy_and_delete() {
        let d1 = tempdir().unwrap();
        let d2 = tempdir().unwrap();
        let fixed = filetime::FileTime::from_unix_time(1_700_000_000, 0);
        let w = |dir: &std::path::Path, name: &str, content: &str| {
            let p = dir.join(name);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(&p, content).unwrap();
            filetime::set_file_mtime(&p, fixed).unwrap();
        };
        w(d1.path(), "a.txt", "A");
        w(d2.path(), "b.txt", "B");
        let mut t = DirTab::new(d1.path().to_str().unwrap(), d2.path().to_str().unwrap());
        t.refresh_sync();
        // 复制左侧 a.txt → 右侧
        t.run_single_op(SyncOp::Copy {
            rel: "a.txt".to_string(),
            src_rel: None,
            from_src: true,
        });
        assert_eq!(fs::read_to_string(d2.path().join("a.txt")).unwrap(), "A");
        // 删除右侧 b.txt
        t.run_single_op(SyncOp::Delete {
            rel: "b.txt".to_string(),
        });
        assert!(!d2.path().join("b.txt").exists());
    }

    #[test]
    fn dirtab_batch_ops() {
        let d1 = tempdir().unwrap();
        let d2 = tempdir().unwrap();
        let fixed = filetime::FileTime::from_unix_time(1_700_000_000, 0);
        let w = |dir: &std::path::Path, name: &str, content: &str| {
            let p = dir.join(name);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(&p, content).unwrap();
            filetime::set_file_mtime(&p, fixed).unwrap();
        };
        w(d1.path(), "diff.txt", "L");
        w(d1.path(), "only_l.txt", "L-only");
        w(d2.path(), "diff.txt", "R");
        w(d2.path(), "only_r.txt", "R-only");
        let mut t = DirTab::new(d1.path().to_str().unwrap(), d2.path().to_str().unwrap());
        // 内容级对比：同 size 同 mtime 的 diff.txt 才能被识别为差异
        t.compare_content = true;
        t.refresh_sync();
        // 批量复制 → 右：diff.txt 与 only_l.txt 复制到右侧
        t.run_batch_copy_to_right();
        assert_eq!(fs::read_to_string(d2.path().join("diff.txt")).unwrap(), "L");
        assert_eq!(
            fs::read_to_string(d2.path().join("only_l.txt")).unwrap(),
            "L-only"
        );
        // 批量删除右侧：diff.txt（已同内容）与 only_r.txt
        t.run_batch_delete_right();
        assert!(!d2.path().join("only_r.txt").exists());
        // diff.txt 此时两侧一致，不再被删除（删除仅作用于差异/仅右侧）
        assert!(d2.path().join("diff.txt").exists());
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

    // ---- ImageTab：图片对比标签 ----------------

    /// 生成纯色 PNG 并写入临时目录，返回路径
    fn write_png(dir: &std::path::Path, name: &str, rgba: [u8; 4]) -> String {
        let img = image::RgbaImage::from_pixel(4, 4, image::Rgba(rgba));
        let p = dir.join(name);
        image::DynamicImage::ImageRgba8(img)
            .write_to(
                &mut std::io::BufWriter::new(std::fs::File::create(&p).unwrap()),
                image::ImageFormat::Png,
            )
            .unwrap();
        p.to_str().unwrap().to_string()
    }

    #[test]
    fn imagetab_load_and_stats() {
        let d = tempdir().unwrap();
        let a = write_png(d.path(), "a.png", [10, 20, 30, 255]);
        let b = write_png(d.path(), "b.png", [10, 20, 30, 255]);
        let c = write_png(d.path(), "c.png", [10, 20, 99, 255]);
        // 相同图 → 无差异
        let mut t = ImageTab::new(&a, &b);
        assert!(t.error.is_none());
        assert!(!t.pair.as_ref().unwrap().stats.has_differences());
        // 不同图 → 有差异
        t.load_pair(&a, &c);
        assert!(t.pair.as_ref().unwrap().stats.has_differences());
        // 空右侧 → 不加载不报错
        t.load_pair(&a, "");
        assert!(t.pair.is_none());
        assert!(t.error.is_none());
    }

    #[test]
    fn imagetab_renders_headless() {
        let d = tempdir().unwrap();
        let a = write_png(d.path(), "a.png", [1, 2, 3, 255]);
        let b = write_png(d.path(), "b.png", [1, 2, 4, 255]);
        let mut t = ImageTab::new(&a, &b);
        t.show_overlay = true;
        egui::__run_test_ui(|ui| {
            t.ui(ui);
        });
        // 空标签页也不 panic
        let mut t2 = ImageTab::new("", "");
        egui::__run_test_ui(|ui| {
            t2.ui(ui);
        });
    }

    #[test]
    fn imagetab_diff_frame_navigation() {
        // 构造 3 帧 GIF：帧 0 相同、帧 1 有差异、帧 2 相同
        let solid = |w: u32, h: u32, rgba: [u8; 4]| -> image::RgbaImage {
            image::RgbaImage::from_pixel(w, h, image::Rgba(rgba))
        };
        let make_gif = |frames: &[image::RgbaImage]| -> Vec<u8> {
            let mut buf = std::io::Cursor::new(Vec::new());
            {
                let mut enc = image::codecs::gif::GifEncoder::new(&mut buf);
                for f in frames {
                    enc.encode_frame(image::Frame::new(f.clone())).unwrap();
                }
            } // drop enc，释放对 buf 的借用
            buf.into_inner()
        };
        let d = tempdir().unwrap();
        // 左侧：3 帧 [black, black, black]
        let black = solid(4, 4, [0, 0, 0, 255]);
        let white = solid(4, 4, [255, 255, 255, 255]);
        let lgif = make_gif(&[black.clone(), black.clone(), black.clone()]);
        // 右侧：3 帧 [black, white, black] → 仅帧 1 有差异
        let rgif = make_gif(&[black.clone(), white, black]);
        let lp = d.path().join("l.gif");
        let rp = d.path().join("r.gif");
        std::fs::write(&lp, &lgif).unwrap();
        std::fs::write(&rp, &rgif).unwrap();
        let mut t = ImageTab::new(lp.to_str().unwrap(), rp.to_str().unwrap());
        assert_eq!(t.total_frames(), 3);
        assert_eq!(t.frame_diffs, vec![false, true, false]);
        // 从帧 0 找下一个差异帧 → 帧 1
        t.next_diff_frame();
        assert_eq!(t.frame_idx, 1);
        // 再下一个 → 循环回帧 1（只有它是差异帧）
        t.next_diff_frame();
        assert_eq!(t.frame_idx, 1);
        // 上一个差异帧从帧 1 出发 → 还是帧 1
        t.prev_diff_frame();
        assert_eq!(t.frame_idx, 1);
    }

    #[test]
    fn imagetab_locate_diff_zooms_to_bounds() {
        let d = tempdir().unwrap();
        // 大图：右下角小块差异 → locate_diff 应放大并设置滚动偏移
        let mut a = image::RgbaImage::from_pixel(200, 200, image::Rgba([0, 0, 0, 255]));
        let b = image::RgbaImage::from_pixel(200, 200, image::Rgba([0, 0, 0, 255]));
        for y in 150..160 {
            for x in 150..160 {
                a.put_pixel(x, y, image::Rgba([255, 255, 255, 255]));
            }
        }
        let save = |dir: &std::path::Path, name: &str, img: &image::RgbaImage| -> String {
            let p = dir.join(name);
            image::DynamicImage::ImageRgba8(img.clone())
                .write_to(
                    &mut std::io::BufWriter::new(std::fs::File::create(&p).unwrap()),
                    image::ImageFormat::Png,
                )
                .unwrap();
            p.to_str().unwrap().to_string()
        };
        let pa = save(d.path(), "a.png", &a);
        let pb = save(d.path(), "b.png", &b);
        let mut t = ImageTab::new(&pa, &pb);
        assert!(t.pair.as_ref().unwrap().stats.bounds.is_some());
        let before_zoom = t.zoom;
        t.locate_diff(egui::vec2(800.0, 600.0));
        // 定位后应放大（包围盒 10x10 → zoom > 1）并滚动到右下区域
        assert!(t.zoom > before_zoom);
        assert!(t.zoom > 1.0);
        assert!(t.scroll.x > 0.0 && t.scroll.y > 0.0);
        assert!(!t.fit);
    }

    // ---- 设置持久化 ----------------

    #[test]
    fn settings_roundtrip() {
        let s = Settings {
            theme: "dark".to_string(),
            show_stats: false,
            ignore_whitespace: true,
            window_size: Some([1000.0, 700.0]),
            ..Settings::default()
        };
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

    #[test]
    fn app_move_tab_reorders_and_keeps_active() {
        let mut app = DiffApp::new(Settings::default());
        // 三个标签，激活中间的
        app.new_diff_tab();
        app.new_diff_tab();
        app.new_diff_tab();
        assert_eq!(app.tabs.len(), 3);
        app.active = 1;
        // 把 0 拖到 2 → [1, 2, 0]
        app.move_tab(0, 2);
        assert_eq!(app.tabs.len(), 3);
        // active 原为 1（拖拽源之后），应仍指向原标签
        // move_tab(0→2)：remove 0 → [t1,t2]，insert at 1 → [t1,t2,t0]
        // active 1 > from 0 且 <= insert_at 1 → 1-1=0，指向原 t1（现在位置 0）
        assert_eq!(app.active, 0, "拖拽后 active 应指向原标签");
        // 再把 0 拖到 0（原地）不改变
        let before = app.tabs.len();
        app.move_tab(0, 0);
        assert_eq!(app.tabs.len(), before);
        // 越界忽略
        app.move_tab(5, 0);
        assert_eq!(app.tabs.len(), before);
    }

    #[test]
    fn join_cloud_url_keeps_single_slash() {
        assert_eq!(
            join_cloud_url("webdav://host/share", "docs"),
            "webdav://host/share/docs"
        );
        assert_eq!(
            join_cloud_url("webdav://host/share/", "docs"),
            "webdav://host/share/docs"
        );
        assert_eq!(join_cloud_url("/tmp/base", "sub"), "/tmp/base/sub");
        assert_eq!(join_cloud_url("  s3://bucket  ", "key"), "s3://bucket/key");
    }

    #[test]
    fn scan_cloud_root_lists_top_level_entries() {
        let dir = tempdir().unwrap();
        let d = dir.path();
        fs::create_dir_all(d.join("docs")).unwrap();
        fs::create_dir_all(d.join("src/sub")).unwrap();
        fs::write(d.join("readme.md"), "hi").unwrap();
        fs::write(d.join("docs/a.md"), "a").unwrap();
        fs::write(d.join("src/main.rs"), "fn").unwrap();
        let entries = scan_cloud_root(d.to_str().unwrap()).unwrap();
        assert_eq!(entries, vec!["docs", "readme.md", "src"]);
    }

    #[test]
    fn scan_cloud_root_nonexistent_returns_none() {
        assert!(scan_cloud_root("/nonexistent/definitely/not/here").is_none());
    }

    // ---- P29 CSV 表格路由 ----------------

    #[test]
    fn is_csv_file_detects_extensions() {
        assert!(crate::csvcmp::is_csv_file("a.CSV"));
        assert!(crate::csvcmp::is_csv_file("b.tsv"));
        assert!(crate::csvcmp::is_csv_file("c.tab"));
        assert!(!crate::csvcmp::is_csv_file("d.txt"));
        assert!(!crate::csvcmp::is_csv_file("e.csv.bak"));
    }
}
