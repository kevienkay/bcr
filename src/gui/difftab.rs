//! 并排 Diff 标签页：虚拟化渲染、行内高亮、搜索、差异/行号跳转。

use super::common::*;
use crate::i18n::{fmt, t, Key as I18nKey};
use crate::sideview::{build_rows, RowTag, SideRow, Stats, ViewOptions};
use eframe::egui::{self, Color32, Key, Pos2, Rect, Vec2};

/// P35-A3：文本对比视图过滤（BC Show All/Differences/Same/Context）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffViewFilter {
    All,
    Diff,
    Same,
    Context,
}

/// P39-2d：细节三模式（BC 视图菜单「细节」）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DiffDetailMode {
    /// 文本细节（默认行渲染）
    #[default]
    Text,
    /// 16进制细节（字节网格）
    Hex,
    /// 对齐方式细节（手动对齐行标记）
    Align,
}

/// P39-2d：布局（BC 视图菜单「布局」）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DiffLayout {
    /// 边并排（默认）
    #[default]
    SideBySide,
    /// 上-下
    TopBottom,
    /// 网页（单栏）
    Web,
}

/// 加载的文件
#[derive(Clone)]
pub struct LoadedFile {
    pub path: String,
    pub content: String,
    /// 解码信息（编码 + BOM），保存时按原编码回写
    pub encoding: crate::encoding::EncodingKind,
    pub had_bom: bool,
    /// 按路径解析的语法（无匹配 = None，纯文本）
    pub syntax: Option<&'static syntect::parsing::SyntaxReference>,
    /// P33：文件大小（详情行显示）
    pub size: u64,
    /// P33：修改时间可读串（详情行显示）
    pub mtime: String,
}

/// 搜索状态
#[derive(Default)]
pub struct SearchState {
    pub query: String,
    /// 替换为文本（A4）
    pub replace: String,
    /// 匹配行索引
    pub matches: Vec<usize>,
    /// 当前匹配在 matches 中的位置
    pub current: Option<usize>,
    /// 请求聚焦搜索框
    pub focus: bool,
    /// P39-2e：请求聚焦替换框
    pub replace_focus: bool,
}

/// P46-3：hex 视图过滤（BC 16进制 视图菜单 显示全部/差异/相同，1/2/3）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HexViewFilter {
    #[default]
    All,
    Diff,
    Same,
}

/// P46-3：hex 布局（BC 16进制 视图菜单 边并排/上-下）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HexViewLayout {
    #[default]
    SideBySide,
    TopBottom,
}

/// 二进制文件的十六进制对比数据（任一文件检测为二进制时启用）
#[derive(Clone)]
pub struct HexTabData {
    pub left: String,
    pub right: String,
    pub rows: Vec<crate::hexview::HexRow>,
    /// 左侧原始字节（编辑保存用）
    pub left_bytes: Vec<u8>,
    /// 右侧原始字节（编辑保存用）
    pub right_bytes: Vec<u8>,
    /// P37-1d：偏移列用 hex 还是 dec
    pub addr_hex: bool,
    /// P37-1d：字节值显示模式（逐字节 / 小尾 / 大端）
    pub value_mode: crate::hexview::HexValueMode,
    /// P37-1d：是否显示字节地址列
    pub show_addr: bool,
}

/// hex 编辑状态：编辑某侧某行的字节
pub struct HexEditState {
    pub side: EditSide,
    /// 行索引
    pub row: usize,
    /// 编辑缓冲区（十六进制字符串，如 "01 0a ff"）
    pub buf: String,
}

/// 行内编辑状态
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EditSide {
    Left,
    Right,
}

pub struct DiffTab {
    pub left: Option<LoadedFile>,
    pub right: Option<LoadedFile>,
    pub rows: Vec<SideRow>,
    pub stats: Stats,
    pub opts: ViewOptions,
    pub error: Option<String>,
    pub show_stats: bool,
    /// 滚动偏移（受控滚动，支持跳转）
    pub scroll: Vec2,
    /// 差异行索引（tag != Equal），供跳转
    pub diff_rows: Vec<usize>,
    pub diff_pos: Option<usize>,
    /// P32-A5：差异块（连续差异行的起止），供折叠
    pub diff_blocks: Vec<(usize, usize)>,
    /// P32-A5：已折叠的差异块（按块索引）
    pub collapsed_blocks: std::collections::HashSet<usize>,
    /// P32-B5：已忽略的行（原始行索引），导航/统计排除
    pub ignored_rows: std::collections::HashSet<usize>,
    /// P38-1a：隔离的差异块范围（rows 行索引，None = 未隔离）
    pub isolated: Option<(usize, usize)>,
    /// P38-1b：手动对齐对（左侧行号 1-based, 右侧行号 1-based）
    pub manual_aligns: Vec<(usize, usize)>,
    /// P38-1b：对齐模式（源侧 + 源行号，等待点击目标行）
    pub align_pick: Option<(EditSide, usize)>,
    /// P38-1d：已编辑行锚点（left_no, right_no），recompute 后重映射为行索引
    pub edited_anchors: Vec<(Option<usize>, Option<usize>)>,
    /// P38-1d：当前编辑导航位置（edited_rows 索引）
    pub edit_pos: Option<usize>,
    pub search: SearchState,
    /// 待跳转行号（1-based）
    pub goto_line: Option<usize>,
    pub goto_focus: bool,
    /// 编辑状态（编辑左侧/右侧内容）
    pub editing: Option<EditState>,
    /// P32-A2：行内直接编辑状态（双击行进入）
    pub inline_edit: Option<InlineEditState>,
    /// P56-1：有未保存/未重载的内容修改（BC 标题栏 `*` 星号标记）
    pub dirty: bool,
    /// P32-A6：撤销栈（编辑/替换前的文件内容）
    pub undo_stack: Vec<EditSnapshot>,
    /// P32-A6：重做栈
    pub redo_stack: Vec<EditSnapshot>,
    /// 二进制 hex 对比模式（Some 时优先于文本行渲染）
    pub hex: Option<HexTabData>,
    /// hex 编辑状态
    pub hex_edit: Option<HexEditState>,
    /// B4：hex 差异导航位置（hex 差异行索引）
    pub hex_diff_pos: Option<usize>,
    /// P46-3：hex 视图过滤（显示全部/差异/相同，BC 1/2/3）
    pub hex_filter: HexViewFilter,
    /// P46-3：hex 布局（边并排/上-下，BC 布局）
    pub hex_layout: HexViewLayout,
    /// A8 自动换行（word wrap，BC5 特性）
    pub wrap: bool,
    /// A11 缩略图总览（右侧迷你差异地图，点击跳转）
    pub show_overview: bool,
    /// P33：长行横向滚动偏移（两栏固定半屏，超长行栏内左右滑动查看）
    pub h_scroll: f32,
    /// P35-A4：显示空白符（空格→·、制表符→→）
    pub show_whitespace: bool,
    /// P42-3：字符列标尺
    pub show_ruler: bool,
    /// P43-2：文本选区（rows 行索引范围 [start, end]，T6 选择选择内容/剪贴板比较）
    pub selection: Option<(usize, usize)>,
    /// P43-3：替换导航位置（search.matches 索引）
    #[allow(dead_code)] // P43-3 批次使用
    pub replace_nav: Option<usize>,
    /// P35-A3：视图过滤（All/Diff/Same/Context）
    pub view_filter: DiffViewFilter,
    /// P35-A3：Context 模式上下文行数
    pub context_lines: usize,
    /// P39-2d：细节三模式（文本/16进制/对齐方式）
    pub detail_mode: DiffDetailMode,
    /// P39-2d：布局（边并排/上-下/网页）
    pub layout: DiffLayout,
    /// P39-2d：书签（编号 0-9 → 渲染行索引）
    pub bookmarks: std::collections::HashMap<u8, usize>,
    /// P44-4：会话锁定（BC Session>已锁定，锁定时禁止编辑操作）
    pub locked: bool,
    /// P44-6：行号显示开关（BC View>行号）
    pub show_line_numbers: bool,
    /// P44-6：语法加亮开关（BC View>语法加亮）
    pub show_syntax: bool,
}

/// 编辑窗口状态
pub struct EditState {
    pub side: EditSide,
    pub path: String,
    pub content: String,
}

/// P32-A2：行内直接编辑状态（双击差异行/内容行进入）
#[derive(Clone)]
pub struct InlineEditState {
    pub side: EditSide,
    /// 行索引（对齐后的显示行索引）
    pub row: usize,
    /// 编辑缓冲区
    pub buf: String,
}

/// P32-A6：编辑快照（撤销/重做用）
#[derive(Clone)]
pub struct EditSnapshot {
    pub side: EditSide,
    pub path: String,
    pub before: String,
    pub after: String,
}

impl DiffTab {
    pub fn new() -> Self {
        DiffTab {
            left: None,
            right: None,
            rows: Vec::new(),
            stats: Stats::default(),
            opts: ViewOptions::default(),
            error: None,
            show_stats: true,
            scroll: Vec2::ZERO,
            diff_rows: Vec::new(),
            diff_pos: None,
            diff_blocks: Vec::new(),
            collapsed_blocks: std::collections::HashSet::new(),
            ignored_rows: std::collections::HashSet::new(),
            isolated: None,
            manual_aligns: Vec::new(),
            align_pick: None,
            edited_anchors: Vec::new(),
            edit_pos: None,
            search: SearchState::default(),
            goto_line: None,
            goto_focus: false,
            editing: None,
            inline_edit: None,
            dirty: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            hex: None,
            hex_edit: None,
            hex_diff_pos: None,
            hex_filter: HexViewFilter::All,
            hex_layout: HexViewLayout::SideBySide,
            wrap: false,
            show_overview: true,
            h_scroll: 0.0,
            show_whitespace: false,
            show_ruler: false,
            selection: None,
            replace_nav: None,
            view_filter: DiffViewFilter::All,
            context_lines: 3,
            detail_mode: DiffDetailMode::Text,
            layout: DiffLayout::SideBySide,
            bookmarks: std::collections::HashMap::new(),
            locked: false,
            show_line_numbers: true,
            show_syntax: true,
        }
    }

    pub fn title(&self) -> String {
        // P56-1：有未保存修改 → 标题前缀 `*`（BC 未保存标记）
        let star = if self.dirty { "* " } else { "" };
        if let Some(h) = &self.hex {
            return format!(
                "{}{}: {} ↔ {}",
                star,
                t(I18nKey::HexTitle),
                basename(&h.left),
                basename(&h.right)
            );
        }
        match (&self.left, &self.right) {
            (Some(l), Some(r)) => format!(
                "{}{}: {} ↔ {}",
                star,
                t(I18nKey::DiffTitle),
                basename(&l.path),
                basename(&r.path)
            ),
            (Some(l), None) => format!("{}: {}", t(I18nKey::DiffTitle), basename(&l.path)),
            (None, Some(r)) => format!("{}: {}", t(I18nKey::DiffTitle), basename(&r.path)),
            (None, None) => t(I18nKey::DiffTitle).to_string(),
        }
    }

    pub fn load_pair(&mut self, l: &str, r: &str, opts: ViewOptions) {
        // P56-1：新内容加载 → 清除未保存标记
        self.dirty = false;
        // 拖入单文件/空路径守卫：只有一侧时转单侧加载（BC 语义：先导入的显示在左边）
        if l.is_empty() && r.is_empty() {
            self.opts = opts;
            self.left = None;
            self.right = None;
            self.rows.clear();
            self.hex = None;
            self.error = None;
            return;
        }
        if r.is_empty() {
            self.load_left(l, opts);
            return;
        }
        if l.is_empty() {
            self.load_right(r, opts);
            return;
        }
        self.opts = opts;
        // 任一文件为二进制 → 切换 hex 对比视图
        if is_binary_file(l) || is_binary_file(r) {
            let rows = match (std::fs::read(l), std::fs::read(r)) {
                (Ok(lb), Ok(rb)) => crate::hexview::build_hex_rows(&lb, &rb),
                (Err(e), _) => {
                    self.error = Some(fmt(I18nKey::CannotRead, &[l, &e.to_string()]));
                    return;
                }
                (_, Err(e)) => {
                    self.error = Some(fmt(I18nKey::CannotRead, &[r, &e.to_string()]));
                    return;
                }
            };
            self.hex = Some(HexTabData {
                left: l.to_string(),
                right: r.to_string(),
                rows,
                left_bytes: std::fs::read(l).unwrap_or_default(),
                right_bytes: std::fs::read(r).unwrap_or_default(),
                addr_hex: true,
                value_mode: crate::hexview::HexValueMode::Raw,
                show_addr: true,
            });
            self.left = None;
            self.right = None;
            self.rows.clear();
            self.error = None;
            return;
        }
        match (crate::encoding::read_text(l), crate::encoding::read_text(r)) {
            (Ok(lf), Ok(rf)) => {
                if lf.is_binary {
                    self.error = Some(fmt(I18nKey::BinaryFile, &[l]));
                    return;
                }
                if rf.is_binary {
                    self.error = Some(fmt(I18nKey::BinaryFile, &[r]));
                    return;
                }
                self.left = Some(LoadedFile {
                    path: l.to_string(),
                    content: lf.text,
                    encoding: lf.encoding,
                    had_bom: lf.had_bom,
                    syntax: crate::highlight::syntax_for(l),
                    size: file_size(l),
                    mtime: file_mtime_str(l),
                });
                self.right = Some(LoadedFile {
                    path: r.to_string(),
                    content: rf.text,
                    encoding: rf.encoding,
                    had_bom: rf.had_bom,
                    syntax: crate::highlight::syntax_for(r),
                    size: file_size(r),
                    mtime: file_mtime_str(r),
                });
                self.recompute();
                self.error = None;
            }
            (Err(e), _) => self.error = Some(fmt(I18nKey::CannotRead, &[l, &e.to_string()])),
            (_, Err(e)) => self.error = Some(fmt(I18nKey::CannotRead, &[r, &e.to_string()])),
        }
    }

    pub fn load_left(&mut self, path: &str, opts: ViewOptions) {
        self.dirty = false;
        self.opts = opts;
        match crate::encoding::read_text(path) {
            Ok(tf) => {
                if tf.is_binary {
                    self.error = Some(fmt(I18nKey::BinaryFile, &[path]));
                    return;
                }
                self.left = Some(LoadedFile {
                    path: path.to_string(),
                    content: tf.text,
                    encoding: tf.encoding,
                    had_bom: tf.had_bom,
                    syntax: crate::highlight::syntax_for(path),
                    size: file_size(path),
                    mtime: file_mtime_str(path),
                });
                self.recompute();
                self.error = None;
            }
            Err(e) => self.error = Some(fmt(I18nKey::CannotRead, &[path, &e.to_string()])),
        }
    }

    pub fn load_right(&mut self, path: &str, opts: ViewOptions) {
        self.dirty = false;
        self.opts = opts;
        match crate::encoding::read_text(path) {
            Ok(tf) => {
                if tf.is_binary {
                    self.error = Some(fmt(I18nKey::BinaryFile, &[path]));
                    return;
                }
                self.right = Some(LoadedFile {
                    path: path.to_string(),
                    content: tf.text,
                    encoding: tf.encoding,
                    had_bom: tf.had_bom,
                    syntax: crate::highlight::syntax_for(path),
                    size: file_size(path),
                    mtime: file_mtime_str(path),
                });
                self.recompute();
                self.error = None;
            }
            Err(e) => self.error = Some(fmt(I18nKey::CannotRead, &[path, &e.to_string()])),
        }
    }

    pub fn recompute(&mut self) {
        let (l, r) = match (&self.left, &self.right) {
            (Some(l), Some(r)) => (l.content.as_str(), r.content.as_str()),
            // P50：单侧导入时仍构建 rows（BC 语义：先导入的显示在左边，右侧留空）
            (Some(l), None) => (l.content.as_str(), ""),
            (None, Some(r)) => ("", r.content.as_str()),
            (None, None) => {
                self.rows.clear();
                self.stats = Stats::default();
                self.diff_rows.clear();
                self.diff_pos = None;
                return;
            }
        };
        let (mut rows, stats) = match build_rows(l, r, self.opts.clone()) {
            Ok(v) => v,
            Err(e) => {
                self.error = Some(e);
                return;
            }
        };
        // P38-1b：应用手动对齐（强制左侧/右侧行配对）
        Self::apply_manual_aligns(&mut rows, &self.manual_aligns);
        // P32-B5：忽略行从差异行/统计中排除（会话级）
        let mut diff_rows: Vec<usize> = Vec::new();
        let mut ignored_delete = 0usize;
        let mut ignored_insert = 0usize;
        let mut ignored_replace = 0usize;
        for (i, row) in rows.iter().enumerate() {
            if row.tag == RowTag::Equal {
                continue;
            }
            if self.ignored_rows.contains(&i) {
                match row.tag {
                    RowTag::Delete => ignored_delete += 1,
                    RowTag::Insert => ignored_insert += 1,
                    RowTag::Replace => ignored_replace += 1,
                    RowTag::Equal => {}
                }
                continue;
            }
            diff_rows.push(i);
        }
        // P32-A5：差异块 = 连续未忽略差异行分组（起止索引）
        let mut diff_blocks: Vec<(usize, usize)> = Vec::new();
        for &i in &diff_rows {
            match diff_blocks.last_mut() {
                Some((_, end)) if *end + 1 == i => *end = i,
                _ => diff_blocks.push((i, i)),
            }
        }
        // 清理失效的折叠块索引
        self.collapsed_blocks.retain(|&b| b < diff_blocks.len());
        self.diff_rows = diff_rows;
        self.diff_blocks = diff_blocks;
        self.diff_pos = None;
        self.rows = rows;
        // 统计：扣除忽略的差异行
        self.stats = Stats {
            delete: stats.delete - ignored_delete,
            insert: stats.insert - ignored_insert,
            replace: stats.replace - ignored_replace,
            equal: stats.equal,
        };
        self.update_search();
    }

    /// P38-1b：应用手动对齐对（left_no, right_no 1-based）。
    /// 定位两行，若未并排则移除并插入 Replace 行（左=源行内容, 右=目标行内容）。
    fn apply_manual_aligns(rows: &mut Vec<SideRow>, aligns: &[(usize, usize)]) {
        for &(ln, rn) in aligns {
            let l_idx = rows.iter().position(|r| r.left_no == Some(ln));
            let r_idx = rows.iter().position(|r| r.right_no == Some(rn));
            let (Some(li), Some(ri)) = (l_idx, r_idx) else {
                continue;
            };
            if li == ri {
                // 已并排（可能已是 Replace 对），跳过
                continue;
            }
            // 取两行内容，移除，在较前位置插入 Replace 行
            let l_cell = rows[li].left.clone();
            let r_cell = rows[ri].right.clone();
            let pos = li.min(ri);
            rows.remove(li.max(ri));
            rows.remove(li.min(ri));
            rows.insert(
                pos,
                SideRow {
                    left: l_cell,
                    right: r_cell,
                    tag: RowTag::Replace,
                    left_no: Some(ln),
                    right_no: Some(rn),
                },
            );
        }
    }

    /// P38-1b：开始对齐（源侧 + 源行号，进入等待点击目标行模式）
    pub fn start_align(&mut self, side: EditSide, row_no: usize) {
        self.align_pick = Some((side, row_no));
    }

    /// P38-1b：完成对齐——记录手动对齐对并重算（`target_row_no` 为目标侧行号）
    pub fn finish_align(&mut self, target_row_no: usize) -> bool {
        let Some((side, src_no)) = self.align_pick.take() else {
            return false;
        };
        let (ln, rn) = match side {
            EditSide::Left => (src_no, target_row_no),
            EditSide::Right => (target_row_no, src_no),
        };
        // 去重后加入
        if !self.manual_aligns.contains(&(ln, rn)) {
            self.manual_aligns.push((ln, rn));
        }
        self.recompute();
        true
    }

    /// P38-1b：清除全部手动对齐
    pub fn clear_aligns(&mut self) {
        self.manual_aligns.clear();
        self.recompute();
    }

    // ---- A4 文本替换 ----

    /// 替换当前匹配（左侧与右侧各一次），按原编码回写文件。
    /// 返回是否发生了替换。
    pub fn replace_current(&mut self) -> bool {
        let Some(k) = self.search.current else {
            return false;
        };
        let Some(&row_idx) = self.search.matches.get(k) else {
            return false;
        };
        let q = self.search.query.clone();
        let rep = self.search.replace.clone();
        if q.is_empty() {
            return false;
        }
        let row = &self.rows[row_idx];
        let mut changed = false;
        if let Some(no) = row.left_no {
            if let Some(l) = &mut self.left {
                if replace_line(&mut l.content, no, &q, &rep) {
                    changed = true;
                }
            }
        }
        if let Some(no) = row.right_no {
            if let Some(r) = &mut self.right {
                if replace_line(&mut r.content, no, &q, &rep) {
                    changed = true;
                }
            }
        }
        if changed {
            self.dirty = true;
            self.finish_replace();
        }
        changed
    }

    /// 全部替换（两侧全文），按原编码回写文件。返回是否发生了替换。
    pub fn replace_all(&mut self) -> bool {
        let q = self.search.query.clone();
        let rep = self.search.replace.clone();
        if q.is_empty() {
            return false;
        }
        let mut changed = false;
        if let Some(l) = &mut self.left {
            if l.content.contains(&q) {
                l.content = l.content.replace(&q, &rep);
                changed = true;
            }
        }
        if let Some(r) = &mut self.right {
            if r.content.contains(&q) {
                r.content = r.content.replace(&q, &rep);
                changed = true;
            }
        }
        if changed {
            self.dirty = true;
            self.finish_replace();
        }
        changed
    }

    /// 替换后统一收尾：按原编码回写两侧文件 → 重算 diff → 提示
    fn finish_replace(&mut self) {
        let mut errs: Vec<String> = Vec::new();
        for side in [EditSide::Left, EditSide::Right] {
            let opt = match side {
                EditSide::Left => self
                    .left
                    .as_ref()
                    .map(|f| (f.path.clone(), f.encoding, f.had_bom, f.content.clone())),
                EditSide::Right => self
                    .right
                    .as_ref()
                    .map(|f| (f.path.clone(), f.encoding, f.had_bom, f.content.clone())),
            };
            let Some((path, enc, bom, content)) = opt else {
                continue;
            };
            // 保存前自动备份（A2）
            let _ = std::fs::copy(&path, format!("{path}.bak"));
            let bytes = crate::encoding::encode_back(
                &crate::encoding::TextFile {
                    text: String::new(),
                    encoding: enc,
                    had_bom: bom,
                    is_binary: false,
                },
                &content,
            );
            if let Err(e) = std::fs::write(&path, bytes) {
                errs.push(format!("{path}: {e}"));
            }
        }
        if !errs.is_empty() {
            self.error = Some(format!("替换写回失败: {}", errs.join("; ")));
        }
        self.recompute();
        self.error = Some(fmt(I18nKey::Saved, &["替换已写回"]));
    }

    /// P42-1：转换文件（BC Convert File），作用于两侧。
    /// mode：Trim 行尾空白 / Tabs→空格 / 行尾 CRLF↔LF。
    /// 与替换同模式：.bak 备份 + 编码回写 + 撤销快照 + 重算 diff。
    pub fn convert_file(&mut self, mode: crate::gui::textedit::ConvertMode) {
        let mut changed_any = false;
        let mut snapshots = Vec::new();
        for side in [EditSide::Left, EditSide::Right] {
            let opt = match side {
                EditSide::Left => self
                    .left
                    .as_ref()
                    .map(|f| (f.path.clone(), f.encoding, f.had_bom, f.content.clone())),
                EditSide::Right => self
                    .right
                    .as_ref()
                    .map(|f| (f.path.clone(), f.encoding, f.had_bom, f.content.clone())),
            };
            let Some((path, enc, bom, content)) = opt else {
                continue;
            };
            let next = crate::gui::textedit::convert_content(&content, mode);
            if next == content {
                continue;
            }
            changed_any = true;
            snapshots.push(EditSnapshot {
                side,
                path: path.clone(),
                before: content.clone(),
                after: next.clone(),
            });
            // 保存前自动备份（A2）
            let _ = std::fs::copy(&path, format!("{path}.bak"));
            let bytes = crate::encoding::encode_back(
                &crate::encoding::TextFile {
                    text: String::new(),
                    encoding: enc,
                    had_bom: bom,
                    is_binary: false,
                },
                &next,
            );
            if let Err(e) = std::fs::write(&path, bytes) {
                self.error = Some(format!("转换写回失败: {path}: {e}"));
                return;
            }
        }
        if !changed_any {
            self.error = Some(fmt(I18nKey::Saved, &["无内容变化，跳过转换"]));
            return;
        }
        // 入撤销栈（批量快照）
        for snap in snapshots {
            self.undo_stack.push(snap);
        }
        self.redo_stack.clear();
        // 重载两侧内容
        let (l, r) = (
            self.left.as_ref().map(|f| f.path.clone()),
            self.right.as_ref().map(|f| f.path.clone()),
        );
        if let (Some(l), Some(r)) = (l, r) {
            self.load_pair(&l, &r, self.opts.clone());
        }
        self.error = Some(fmt(I18nKey::Saved, &["转换已写回两侧"]));
    }

    pub fn reload(&mut self) {
        let opts = self.opts.clone();
        let l = self.left.as_ref().map(|f| f.path.clone());
        let r = self.right.as_ref().map(|f| f.path.clone());
        match (l, r) {
            (Some(l), Some(r)) => self.load_pair(&l, &r, opts),
            (Some(l), None) => self.load_left(&l, opts),
            (None, Some(r)) => self.load_right(&r, opts),
            (None, None) => {}
        }
    }

    // ---- P33 菜单栏转发：打开左/右对话框、剪贴板加载、聚焦搜索 ----
    /// 打开左侧文件（文件对话框）
    pub fn open_left_dialog(&mut self) {
        if let Some(p) = super::pick_file() {
            self.load_left(&p, self.opts.clone());
        }
    }

    /// 打开右侧文件（文件对话框）
    pub fn open_right_dialog(&mut self) {
        if let Some(p) = super::pick_file() {
            self.load_right(&p, self.opts.clone());
        }
    }

    /// 剪贴板 → 左侧（读系统剪贴板文本 → 临时文件 → 加载）
    pub fn load_clipboard_left(&mut self) {
        match read_clipboard_text() {
            Some(txt) => {
                if let Some(p) = write_clipboard_temp(&txt) {
                    self.load_left(&p, self.opts.clone());
                } else {
                    self.error = Some("写入剪贴板临时文件失败".to_string());
                }
            }
            None => self.error = Some(t(I18nKey::ClipboardUnavailable).to_string()),
        }
    }

    /// 剪贴板 → 右侧
    pub fn load_clipboard_right(&mut self) {
        match read_clipboard_text() {
            Some(txt) => {
                if let Some(p) = write_clipboard_temp(&txt) {
                    self.load_right(&p, self.opts.clone());
                } else {
                    self.error = Some("写入剪贴板临时文件失败".to_string());
                }
            }
            None => self.error = Some(t(I18nKey::ClipboardUnavailable).to_string()),
        }
    }

    /// P43-2：选择选择内容(D)——把当前差异块（diff_pos 所在块）选为选区
    pub fn select_selection(&mut self) {
        // 优先：当前差异行所在块；否则第一个差异块
        let cur_row = self
            .diff_pos
            .and_then(|p| self.diff_rows.get(p))
            .copied()
            .unwrap_or(0);
        let block = self
            .diff_blocks
            .iter()
            .find(|&&(s, e)| s <= cur_row && cur_row <= e)
            .copied()
            .or_else(|| self.diff_blocks.first().copied());
        match block {
            Some((s, e)) => self.selection = Some((s, e)),
            None => self.selection = None,
        }
    }

    /// P43-2：把选择内容和剪贴板比较（选区文本 → 剪贴板 → 右侧对比）
    pub fn selection_to_clipboard(&mut self) {
        let Some((s, e)) = self.selection else {
            return;
        };
        let mut text = String::new();
        for (i, row) in self.rows.iter().enumerate() {
            if i < s || i > e {
                continue;
            }
            if let Some(c) = &row.left {
                text.push_str(&c.text);
                text.push('\n');
            }
        }
        if text.is_empty() {
            return;
        }
        // 写入临时文件并加载为右侧（复用剪贴板对比路径）
        if let Some(path) = write_clipboard_temp(&text) {
            self.load_right(&path, self.opts.clone());
        }
    }

    /// P43-2：当前选区文本（供测试）
    #[cfg(test)]
    pub(crate) fn selection_text(&self) -> String {
        let Some((s, e)) = self.selection else {
            return String::new();
        };
        self.rows[s..=e.min(self.rows.len().saturating_sub(1))]
            .iter()
            .filter_map(|r| r.left.as_ref().map(|c| c.text.as_str()))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// P44-2：当前差异块起始行（cur_row 定位；无当前位置取第一块）
    fn current_block_start(&self) -> Option<usize> {
        let cur_row = self
            .diff_pos
            .and_then(|p| self.diff_rows.get(p))
            .copied()
            .unwrap_or(0);
        self.diff_blocks
            .iter()
            .find(|&&(s, e)| s <= cur_row && cur_row <= e)
            .map(|&(s, _e)| s)
            .or_else(|| self.diff_blocks.first().map(|&(s, _e)| s))
    }

    /// P44-2：⌘A 对齐方式——当前差异块左侧行与右侧当前行对齐（BC 编辑>对齐方式...）
    pub fn align_current(&mut self) {
        let Some(s) = self.current_block_start() else {
            return;
        };
        if let Some(r) = self.rows.get(s) {
            if let Some(ln) = r.left_no {
                self.start_align(EditSide::Left, ln);
            }
        }
    }

    /// P44-2：]/[ 缩进——当前差异块整体增加/减少缩进（BC 编辑>增加缩进/减少缩进）
    pub fn indent_current(&mut self, delta: isize) {
        let Some(s) = self.current_block_start() else {
            return;
        };
        self.indent_block(s, delta);
    }

    /// P44-2：⌘E 使用选择内容进行查找（BC 搜索>使用选择内容进行查找）
    pub fn find_selection(&mut self) {
        let Some((s, e)) = self.selection else {
            return;
        };
        let text = self.rows[s..=e.min(self.rows.len().saturating_sub(1))]
            .iter()
            .filter_map(|r| r.left.as_ref().map(|c| c.text.as_str()))
            .collect::<Vec<_>>()
            .join("\n");
        if text.is_empty() {
            return;
        }
        self.search.query = text;
        self.update_search();
        self.search.focus = true;
    }

    /// 聚焦搜索框（菜单 Search>Find）
    pub fn focus_search(&mut self) {
        self.search.focus = true;
    }

    /// P40-1：打开编辑窗口（菜单 Edit>编辑左侧/右侧）
    pub fn start_edit(&mut self, side: EditSide) {
        let path = match side {
            EditSide::Left => self.left.as_ref().map(|f| f.path.clone()),
            EditSide::Right => self.right.as_ref().map(|f| f.path.clone()),
        };
        let Some(path) = path else { return };
        let content = match side {
            EditSide::Left => self.left.as_ref().map(|f| f.content.clone()),
            EditSide::Right => self.right.as_ref().map(|f| f.content.clone()),
        }
        .unwrap_or_default();
        self.editing = Some(EditState {
            side,
            path,
            content,
        });
    }

    /// P39-2e：聚焦替换框（菜单 Search>Replace，⇧⌘F）
    pub fn focus_replace(&mut self) {
        self.search.replace_focus = true;
    }

    // ---- P32-A2/A6：行内编辑提交 + 撤销/重做 ----

    /// 提交行内编辑：修改对应侧文件对应行 → 按原编码写回 → 重新加载 → 入撤销栈
    pub fn commit_inline_edit(&mut self) {
        let Some(ie) = self.inline_edit.take() else {
            return;
        };
        let Some(row) = self.rows.get(ie.row) else {
            return;
        };
        // 取该侧文件信息与行号
        let (path, line_no, enc, bom) = match ie.side {
            EditSide::Left => match (&self.left, row.left_no) {
                (Some(f), Some(no)) => (f.path.clone(), no, f.encoding, f.had_bom),
                _ => return,
            },
            EditSide::Right => match (&self.right, row.right_no) {
                (Some(f), Some(no)) => (f.path.clone(), no, f.encoding, f.had_bom),
                _ => return,
            },
        };
        // 读取当前文件原文，替换目标行
        let Ok(orig) = std::fs::read_to_string(&path) else {
            return;
        };
        let mut lines: Vec<&str> = orig.split('\n').collect();
        let idx = line_no.saturating_sub(1);
        if idx >= lines.len() {
            return;
        }
        let before_line = lines[idx].to_string();
        let after_line = ie.buf.clone();
        if before_line == after_line {
            return;
        }
        lines[idx] = &after_line;
        let new_content = lines.join("\n");
        // 按原编码写回（保留 BOM，自动备份）
        let _ = std::fs::copy(&path, format!("{path}.bak"));
        let bytes = crate::encoding::encode_back(
            &crate::encoding::TextFile {
                text: String::new(),
                encoding: enc,
                had_bom: bom,
                is_binary: false,
            },
            &new_content,
        );
        if let Err(e) = std::fs::write(&path, bytes) {
            self.error = Some(fmt(I18nKey::SaveFailed, &[&e.to_string()]));
            return;
        }
        // 入撤销栈（重做栈清空）
        self.undo_stack.push(EditSnapshot {
            side: ie.side,
            path: path.clone(),
            before: orig,
            after: new_content,
        });
        self.redo_stack.clear();
        // 重新加载对应侧并重算
        match ie.side {
            EditSide::Left => self.load_left(&path, self.opts.clone()),
            EditSide::Right => self.load_right(&path, self.opts.clone()),
        }
        self.error = Some(fmt(I18nKey::Saved, &[&path]));
    }

    /// 撤销：恢复快照 before 内容到文件并重新加载
    pub fn undo(&mut self) {
        let Some(snap) = self.undo_stack.pop() else {
            return;
        };
        let before = snap.before.clone();
        let path = snap.path.clone();
        let (enc, bom) = match snap.side {
            EditSide::Left => self
                .left
                .as_ref()
                .map(|f| (f.encoding, f.had_bom))
                .unwrap_or((crate::encoding::EncodingKind::Utf8, false)),
            EditSide::Right => self
                .right
                .as_ref()
                .map(|f| (f.encoding, f.had_bom))
                .unwrap_or((crate::encoding::EncodingKind::Utf8, false)),
        };
        let _ = std::fs::copy(&path, format!("{path}.bak"));
        let bytes = crate::encoding::encode_back(
            &crate::encoding::TextFile {
                text: String::new(),
                encoding: enc,
                had_bom: bom,
                is_binary: false,
            },
            &before,
        );
        if let Err(e) = std::fs::write(&path, bytes) {
            self.error = Some(fmt(I18nKey::SaveFailed, &[&e.to_string()]));
            self.undo_stack.push(snap);
            return;
        }
        match snap.side {
            EditSide::Left => self.load_left(&path, self.opts.clone()),
            EditSide::Right => self.load_right(&path, self.opts.clone()),
        }
        self.redo_stack.push(snap);
        self.error = Some(fmt(I18nKey::Saved, &["已撤销"]));
    }

    /// 重做：恢复快照 after 内容到文件并重新加载
    pub fn redo(&mut self) {
        let Some(snap) = self.redo_stack.pop() else {
            return;
        };
        let after = snap.after.clone();
        let path = snap.path.clone();
        let (enc, bom) = match snap.side {
            EditSide::Left => self
                .left
                .as_ref()
                .map(|f| (f.encoding, f.had_bom))
                .unwrap_or((crate::encoding::EncodingKind::Utf8, false)),
            EditSide::Right => self
                .right
                .as_ref()
                .map(|f| (f.encoding, f.had_bom))
                .unwrap_or((crate::encoding::EncodingKind::Utf8, false)),
        };
        let _ = std::fs::copy(&path, format!("{path}.bak"));
        let bytes = crate::encoding::encode_back(
            &crate::encoding::TextFile {
                text: String::new(),
                encoding: enc,
                had_bom: bom,
                is_binary: false,
            },
            &after,
        );
        if let Err(e) = std::fs::write(&path, bytes) {
            self.error = Some(fmt(I18nKey::SaveFailed, &[&e.to_string()]));
            self.redo_stack.push(snap);
            return;
        }
        match snap.side {
            EditSide::Left => self.load_left(&path, self.opts.clone()),
            EditSide::Right => self.load_right(&path, self.opts.clone()),
        }
        self.undo_stack.push(snap);
        self.error = Some(fmt(I18nKey::Saved, &["已重做"]));
    }

    /// P39-2d：切换书签（当前可见顶部行绑定编号 0-9，已存在则取消）
    pub fn toggle_bookmark(&mut self, no: u8) {
        if no > 9 {
            return;
        }
        let top = (self.scroll.y / ROW_H).max(0.0) as usize;
        if self.bookmarks.get(&no) == Some(&top) {
            self.bookmarks.remove(&no);
        } else {
            self.bookmarks.insert(no, top);
        }
    }

    /// P39-2d：转到书签（0-9）
    pub fn goto_bookmark(&mut self, no: u8) {
        if let Some(&row) = self.bookmarks.get(&no) {
            self.scroll.y = row as f32 * ROW_H;
            // 同步 diff_pos：书签行若是差异行则高亮
            if let Some(p) = self.diff_rows.iter().position(|&r| r == row) {
                self.diff_pos = Some(p);
            }
        }
    }

    /// P39-2d：清除全部书签
    pub fn clear_bookmarks(&mut self) {
        self.bookmarks.clear();
    }

    /// P39-2d：细节三模式切换（文本/16进制/对齐方式）
    /// 16进制细节：对文本文件也构建字节网格（复用 hex 数据）
    pub fn set_detail_mode(&mut self, mode: DiffDetailMode) {
        self.detail_mode = mode;
        if mode == DiffDetailMode::Hex && self.hex.is_none() {
            // 文本文件强制 hex：读取两侧字节构建 hex rows
            let l = self.left.as_ref().map(|f| f.path.clone());
            let r = self.right.as_ref().map(|f| f.path.clone());
            if let (Some(lp), Some(rp)) = (&l, &r) {
                let rows = match (std::fs::read(lp), std::fs::read(rp)) {
                    (Ok(lb), Ok(rb)) => Some(crate::hexview::build_hex_rows(&lb, &rb)),
                    _ => None,
                };
                if let Some(rows) = rows {
                    self.hex = Some(HexTabData {
                        left: lp.clone(),
                        right: rp.clone(),
                        rows,
                        left_bytes: std::fs::read(lp).unwrap_or_default(),
                        right_bytes: std::fs::read(rp).unwrap_or_default(),
                        addr_hex: true,
                        value_mode: crate::hexview::HexValueMode::Raw,
                        show_addr: true,
                    });
                }
            }
        }
    }

    /// P39-2d：布局切换
    pub fn set_layout(&mut self, layout: DiffLayout) {
        self.layout = layout;
    }

    /// P39-2d：当前布局的行高（上-下布局每数据行占 2 行）
    pub(crate) fn row_h(&self) -> f32 {
        match self.layout {
            DiffLayout::TopBottom => ROW_H * 2.0,
            DiffLayout::SideBySide | DiffLayout::Web => ROW_H,
        }
    }

    /// P39-2d：书签（测试用）
    #[cfg(test)]
    pub(crate) fn bookmarks(&self) -> &std::collections::HashMap<u8, usize> {
        &self.bookmarks
    }

    // ---- P35-A1：复制差异块到另一侧（BC Copy to Other Side）----

    /// 把当前差异块的内容复制到目标侧（覆盖目标侧该块），入撤销栈。
    /// `target` 是被覆盖的一侧：Right = 左侧→右侧，Left = 右侧→左侧。
    pub fn copy_block_to(&mut self, target: EditSide) -> bool {
        // 定位当前差异块（diff_pos → diff_rows → 所在块）
        let Some(pos) = self.diff_pos else {
            return false;
        };
        let Some(&cur_row) = self.diff_rows.get(pos) else {
            return false;
        };
        self.copy_block_at(cur_row, target)
    }

    /// 复制指定行所在的差异块到目标侧（`row` 为 self.rows 的行索引）。
    /// `target` 是被覆盖的一侧：Right = 左侧→右侧，Left = 右侧→左侧。
    pub fn copy_block_at(&mut self, row: usize, target: EditSide) -> bool {
        let Some(&(s, e)) = self
            .diff_blocks
            .iter()
            .find(|&&(s, e)| s <= row && row <= e)
        else {
            return false;
        };
        let src_is_left = target == EditSide::Right;
        // 目标侧文件信息（路径 + 编码 + BOM + 原文）
        let (dst_path, dst_enc, dst_bom, dst_orig) = match target {
            EditSide::Right => match &self.right {
                Some(f) => (f.path.clone(), f.encoding, f.had_bom, f.content.clone()),
                None => return false,
            },
            EditSide::Left => match &self.left {
                Some(f) => (f.path.clone(), f.encoding, f.had_bom, f.content.clone()),
                None => return false,
            },
        };
        // 重建目标侧全文：块内取源侧，块外保持目标侧原样
        let mut new_lines: Vec<String> = Vec::new();
        for (i, row) in self.rows.iter().enumerate() {
            let in_block = i >= s && i <= e;
            let (src_cell, dst_cell) = if src_is_left {
                (&row.left, &row.right)
            } else {
                (&row.right, &row.left)
            };
            let keep = if in_block {
                src_cell.as_ref().map(|c| c.text.clone())
            } else {
                dst_cell.as_ref().map(|c| c.text.clone())
            };
            if let Some(text) = keep {
                new_lines.push(text);
            }
        }
        let mut new_content = new_lines.join("\n");
        // 保留原末尾换行特征
        if dst_orig.ends_with('\n') && !new_content.is_empty() {
            new_content.push('\n');
        }
        if dst_orig == new_content {
            return false;
        }
        // 备份 + 按原编码写回
        let _ = std::fs::copy(&dst_path, format!("{dst_path}.bak"));
        let bytes = crate::encoding::encode_back(
            &crate::encoding::TextFile {
                text: String::new(),
                encoding: dst_enc,
                had_bom: dst_bom,
                is_binary: false,
            },
            &new_content,
        );
        if let Err(e) = std::fs::write(&dst_path, bytes) {
            self.error = Some(fmt(I18nKey::SaveFailed, &[&e.to_string()]));
            return false;
        }
        // 入撤销栈（重做栈清空）
        self.undo_stack.push(EditSnapshot {
            side: target,
            path: dst_path.clone(),
            before: dst_orig,
            after: new_content,
        });
        self.redo_stack.clear();
        // P38-1d：标记块内已编辑行（重载前收集锚点，避免借用冲突）
        let anchors: Vec<(Option<usize>, Option<usize>)> = self
            .rows
            .iter()
            .take(e + 1)
            .skip(s)
            .map(|r| (r.left_no, r.right_no))
            .collect();
        for a in anchors {
            if !self.edited_anchors.contains(&a) {
                self.edited_anchors.push(a);
            }
        }
        // 重新加载目标侧并重算
        match target {
            EditSide::Right => self.load_right(&dst_path, self.opts.clone()),
            EditSide::Left => self.load_left(&dst_path, self.opts.clone()),
        }
        self.dirty = true; // BC：复制到另一侧后标题显示 `*`（load_* 会清除，此处重新置位）
        self.error = Some(fmt(I18nKey::Saved, &["已复制到另一侧"]));
        true
    }

    /// P38-1e：文件级联动（BC Copy File to Right/Left and Open Next Difference）。
    /// 源侧整文件内容覆盖目标侧文件，重载后跳转到下一个差异。
    pub fn copy_file_to(&mut self, target: EditSide) -> bool {
        let src_is_left = target == EditSide::Right;
        // 源侧全文 + 目标侧文件信息
        let src_content = if src_is_left {
            self.left.as_ref().map(|f| f.content.clone())
        } else {
            self.right.as_ref().map(|f| f.content.clone())
        };
        let Some(src_content) = src_content else {
            return false;
        };
        let (dst_path, dst_enc, dst_bom, dst_orig) = match target {
            EditSide::Right => match &self.right {
                Some(f) => (f.path.clone(), f.encoding, f.had_bom, f.content.clone()),
                None => return false,
            },
            EditSide::Left => match &self.left {
                Some(f) => (f.path.clone(), f.encoding, f.had_bom, f.content.clone()),
                None => return false,
            },
        };
        if dst_orig == src_content {
            return false;
        }
        // 备份 + 按目标侧原编码写回源内容
        let _ = std::fs::copy(&dst_path, format!("{dst_path}.bak"));
        let bytes = crate::encoding::encode_back(
            &crate::encoding::TextFile {
                text: String::new(),
                encoding: dst_enc,
                had_bom: dst_bom,
                is_binary: false,
            },
            &src_content,
        );
        if let Err(e) = std::fs::write(&dst_path, bytes) {
            self.error = Some(fmt(I18nKey::SaveFailed, &[&e.to_string()]));
            return false;
        }
        // 入撤销栈（重做栈清空）
        self.undo_stack.push(EditSnapshot {
            side: target,
            path: dst_path.clone(),
            before: dst_orig,
            after: src_content,
        });
        self.redo_stack.clear();
        // P38-1d：文件级变更 → 目标侧全部行标记为已编辑（重载前收集锚点）
        let anchors: Vec<(Option<usize>, Option<usize>)> = self
            .rows
            .iter()
            .map(|r| {
                if src_is_left {
                    (r.left_no, r.right_no)
                } else {
                    (r.right_no, r.left_no)
                }
            })
            .collect();
        for a in anchors {
            if !self.edited_anchors.contains(&a) {
                self.edited_anchors.push(a);
            }
        }
        // 重新加载目标侧并重算
        match target {
            EditSide::Right => self.load_right(&dst_path, self.opts.clone()),
            EditSide::Left => self.load_left(&dst_path, self.opts.clone()),
        }
        // 跳转到下一个差异（若无差异则清空定位）
        if self.diff_rows.is_empty() {
            self.diff_pos = None;
        } else {
            self.next_diff();
        }
        self.error = Some(fmt(I18nKey::Saved, &["已复制文件到另一侧"]));
        true
    }

    /// P38-1c：缩进调整（BC Increase/Decrease Indent）。
    /// 对 `row` 所在差异块内两侧存在的行整体 ±4 空格（仅行首空白）。
    /// `delta > 0` 增加缩进，`delta < 0` 减少（最多去掉 |delta| 个前导空格）。
    pub fn indent_block(&mut self, row: usize, delta: isize) -> bool {
        let Some(&(s, e)) = self
            .diff_blocks
            .iter()
            .find(|&&(s, e)| s <= row && row <= e)
        else {
            return false;
        };
        if delta == 0 {
            return false;
        }
        // P38-1d：标记块内已编辑行（重载前收集锚点，避免借用冲突）
        let anchors: Vec<(Option<usize>, Option<usize>)> = self
            .rows
            .iter()
            .take(e + 1)
            .skip(s)
            .map(|r| (r.left_no, r.right_no))
            .collect();
        for a in anchors {
            if !self.edited_anchors.contains(&a) {
                self.edited_anchors.push(a);
            }
        }
        let pad = 4usize;
        let adjust = |text: &str| -> String {
            if delta > 0 {
                format!("{}{}", " ".repeat(pad), text)
            } else {
                let n = text.len() - text.trim_start_matches(' ').len();
                let cut = n.min(pad);
                text[cut..].to_string()
            }
        };
        // 两侧文件信息
        let left_info = self
            .left
            .as_ref()
            .map(|f| (f.path.clone(), f.encoding, f.had_bom, f.content.clone()));
        let right_info = self
            .right
            .as_ref()
            .map(|f| (f.path.clone(), f.encoding, f.had_bom, f.content.clone()));
        let mut changed = false;
        for side in [EditSide::Left, EditSide::Right] {
            let Some((path, enc, bom, orig)) = (match side {
                EditSide::Left => left_info.clone(),
                EditSide::Right => right_info.clone(),
            }) else {
                continue;
            };
            // 重建该侧全文：块内该侧存在的行 ± 缩进
            let mut new_lines: Vec<String> = Vec::new();
            for (i, r) in self.rows.iter().enumerate() {
                let in_block = i >= s && i <= e;
                let cell = match side {
                    EditSide::Left => &r.left,
                    EditSide::Right => &r.right,
                };
                let keep = if in_block {
                    cell.as_ref().map(|c| adjust(&c.text))
                } else {
                    cell.as_ref().map(|c| c.text.clone())
                };
                if let Some(text) = keep {
                    new_lines.push(text);
                }
            }
            let mut new_content = new_lines.join("\n");
            if orig.ends_with('\n') && !new_content.is_empty() {
                new_content.push('\n');
            }
            if orig == new_content {
                continue;
            }
            // 备份 + 按原编码写回
            let _ = std::fs::copy(&path, format!("{path}.bak"));
            let bytes = crate::encoding::encode_back(
                &crate::encoding::TextFile {
                    text: String::new(),
                    encoding: enc,
                    had_bom: bom,
                    is_binary: false,
                },
                &new_content,
            );
            if let Err(e) = std::fs::write(&path, bytes) {
                self.error = Some(fmt(I18nKey::SaveFailed, &[&e.to_string()]));
                return false;
            }
            // 入撤销栈（重做栈清空）
            self.undo_stack.push(EditSnapshot {
                side,
                path: path.clone(),
                before: orig,
                after: new_content,
            });
            self.redo_stack.clear();
            changed = true;
            // 重新加载该侧并重算
            match side {
                EditSide::Left => self.load_left(&path, self.opts.clone()),
                EditSide::Right => self.load_right(&path, self.opts.clone()),
            }
        }
        if changed {
            self.error = Some(fmt(I18nKey::Saved, &["已调整缩进"]));
        }
        changed
    }

    /// P37-1m：复制单行到目标侧（BC Copy Line to Right/Left，行级替换该行）。
    /// `target` 是被覆盖的一侧：Right = 左侧→右侧，Left = 右侧→左侧。
    /// P56-4：复制当前差异行到目标侧（BC Edit>Copy Line to Right/Left）。
    /// 当前行 = 当前差异位置（diff_pos → diff_rows 定位）。
    pub fn copy_line_current(&mut self, target: EditSide) -> bool {
        let Some(pos) = self.diff_pos else {
            return false;
        };
        let Some(&row) = self.diff_rows.get(pos) else {
            return false;
        };
        self.copy_line_at(row, target)
    }

    pub fn copy_line_at(&mut self, row: usize, target: EditSide) -> bool {
        let Some(r) = self.rows.get(row) else {
            return false;
        };        let src_is_left = target == EditSide::Right;
        // 源侧该行文本
        let src_text = if src_is_left {
            r.left.as_ref().map(|c| c.text.clone())
        } else {
            r.right.as_ref().map(|c| c.text.clone())
        };
        let Some(src_text) = src_text else {
            return false;
        };
        // 目标侧文件信息
        let (dst_path, dst_enc, dst_bom, dst_orig) = match target {
            EditSide::Right => match &self.right {
                Some(f) => (f.path.clone(), f.encoding, f.had_bom, f.content.clone()),
                None => return false,
            },
            EditSide::Left => match &self.left {
                Some(f) => (f.path.clone(), f.encoding, f.had_bom, f.content.clone()),
                None => return false,
            },
        };
        // 目标侧该行行号（1-based）
        let dst_no = if src_is_left { r.right_no } else { r.left_no };
        let Some(dst_no) = dst_no else {
            return false;
        };
        let mut dst_lines: Vec<&str> = dst_orig.split_terminator('\n').collect();
        if dst_no == 0 || dst_no > dst_lines.len() {
            return false;
        }
        // 替换目标侧第 dst_no 行
        let idx = dst_no - 1;
        dst_lines[idx] = &src_text;
        let mut new_content = dst_lines.join("\n");
        // 保留原末尾换行特征
        if dst_orig.ends_with('\n') && !new_content.is_empty() {
            new_content.push('\n');
        }
        if dst_orig == new_content {
            return false;
        }
        // 备份 + 按原编码写回
        let _ = std::fs::copy(&dst_path, format!("{dst_path}.bak"));
        let bytes = crate::encoding::encode_back(
            &crate::encoding::TextFile {
                text: String::new(),
                encoding: dst_enc,
                had_bom: dst_bom,
                is_binary: false,
            },
            &new_content,
        );
        if let Err(e) = std::fs::write(&dst_path, bytes) {
            self.error = Some(fmt(I18nKey::SaveFailed, &[&e.to_string()]));
            return false;
        }
        // 入撤销栈（重做栈清空）
        self.undo_stack.push(EditSnapshot {
            side: target,
            path: dst_path.clone(),
            before: dst_orig,
            after: new_content,
        });
        self.redo_stack.clear();
        // P38-1d：标记该行已编辑（重载前取锚点，避免借用冲突）
        if let Some(anchor) = self.rows.get(row).map(|r| (r.left_no, r.right_no)) {
            if !self.edited_anchors.contains(&anchor) {
                self.edited_anchors.push(anchor);
            }
        }
        // 重新加载目标侧并重算
        match target {
            EditSide::Right => self.load_right(&dst_path, self.opts.clone()),
            EditSide::Left => self.load_left(&dst_path, self.opts.clone()),
        }
        self.dirty = true; // BC：复制行后标题显示 `*`
        true
    }

    // ---- P35-A2：交换左右两侧（BC Swap Sides）----

    /// 交换左右两侧文件（重新加载，撤销栈清空，含单侧/hex 情况）
    pub fn swap_sides(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
        let l = self.left.as_ref().map(|f| f.path.clone());
        let r = self.right.as_ref().map(|f| f.path.clone());
        let opts = self.opts.clone();
        match (l, r) {
            (Some(l), Some(r)) => self.load_pair(&r, &l, opts),
            (Some(l), None) => {
                self.left = None;
                self.right = None;
                self.load_right(&l, opts);
            }
            (None, Some(r)) => {
                self.left = None;
                self.right = None;
                self.load_left(&r, opts);
            }
            (None, None) => {}
        }
        self.error = Some(fmt(I18nKey::Saved, &["已交换左右"]));
    }

    /// P35-A3：判断原始行 oi 是否应显示（视图过滤 All/Diff/Same/Context）
    pub(crate) fn row_visible(
        &self,
        oi: usize,
        diff_set: &std::collections::HashSet<usize>,
    ) -> bool {
        match self.view_filter {
            DiffViewFilter::All => true,
            DiffViewFilter::Diff => diff_set.contains(&oi),
            DiffViewFilter::Same => !diff_set.contains(&oi),
            DiffViewFilter::Context => {
                if diff_set.contains(&oi) {
                    return true;
                }
                let ctx = self.context_lines as isize;
                let oi_i = oi as isize;
                self.diff_rows
                    .iter()
                    .any(|&d| (d as isize - oi_i).abs() <= ctx)
            }
        }
    }

    /// P38-1a：隔离当前差异块（BC Isolate）。
    /// 根据 diff_pos → diff_rows → 所在 diff_block 设置 isolated。
    pub fn isolate_current(&mut self) -> bool {
        let Some(pos) = self.diff_pos else {
            return false;
        };
        let Some(&cur_row) = self.diff_rows.get(pos) else {
            return false;
        };
        let Some(&(s, e)) = self
            .diff_blocks
            .iter()
            .find(|&&(s, e)| s <= cur_row && cur_row <= e)
        else {
            return false;
        };
        self.isolated = Some((s, e));
        // 隔离后 diff_pos 重置，导航从块首开始
        let start = self.diff_rows.iter().position(|&r| r >= s);
        self.diff_pos = start;
        if let Some(p) = start {
            self.jump_to_row(self.diff_rows[p]);
        } else {
            self.jump_to_row(s);
        }
        true
    }

    /// P38-1a：取消隔离（BC Show All）
    pub fn unisolate(&mut self) {
        self.isolated = None;
    }

    /// P38-1a：判断原始行 oi 是否在隔离范围内
    fn in_isolated(&self, oi: usize) -> bool {
        match self.isolated {
            Some((s, e)) => oi >= s && oi <= e,
            None => true,
        }
    }

    // ---- 搜索 ----

    pub fn update_search(&mut self) {
        let q = self.search.query.trim();
        self.search.matches.clear();
        self.search.current = None;
        if q.is_empty() {
            return;
        }
        let lower = q.to_lowercase();
        for (i, row) in self.rows.iter().enumerate() {
            let hit = [row.left.as_ref(), row.right.as_ref()]
                .into_iter()
                .flatten()
                .any(|c| c.text.to_lowercase().contains(&lower));
            if hit {
                self.search.matches.push(i);
            }
        }
    }

    /// 跳到第 k 个匹配（k 为 matches 下标），返回是否成功
    pub fn goto_match(&mut self, k: usize) -> bool {
        if let Some(&row) = self.search.matches.get(k) {
            self.search.current = Some(k);
            self.jump_to_row(row);
            true
        } else {
            false
        }
    }

    pub fn next_match(&mut self) {
        if self.search.matches.is_empty() {
            return;
        }
        let next = match self.search.current {
            Some(k) => (k + 1) % self.search.matches.len(),
            None => 0,
        };
        self.goto_match(next);
    }

    /// P43-3：下一替换（BC 搜索菜单 下一个/上一个替换）——跳到下一匹配并聚焦替换框
    pub fn next_replace(&mut self) {
        self.next_match();
        if self.search.current.is_some() {
            self.search.replace_focus = true;
        }
    }

    /// P43-3：上一替换——跳到上一匹配并聚焦替换框
    pub fn prev_replace(&mut self) {
        self.prev_match();
        if self.search.current.is_some() {
            self.search.replace_focus = true;
        }
    }

    pub fn prev_match(&mut self) {
        if self.search.matches.is_empty() {
            return;
        }
        let n = self.search.matches.len();
        let prev = match self.search.current {
            Some(k) => (k + n - 1) % n,
            None => n - 1,
        };
        self.goto_match(prev);
    }

    /// P38-1a：导航可用的差异行（隔离时只取范围内的行）
    pub fn nav_diff_rows(&self) -> Vec<usize> {
        self.diff_rows
            .iter()
            .copied()
            .filter(|&r| self.in_isolated(r))
            .collect()
    }

    /// P38-1d：当前已编辑行索引（锚点 → 当前 rows 重映射，含隔离过滤）。
    /// 锚点任一侧行号命中即视为已编辑（复制后行结构可能从 Delete/Insert 变 Equal）。
    pub fn edited_rows(&self) -> Vec<usize> {
        self.rows
            .iter()
            .enumerate()
            .filter(|(_, r)| {
                self.edited_anchors.iter().any(|&(al, ar)| {
                    (al.is_some() && al == r.left_no) || (ar.is_some() && ar == r.right_no)
                }) && self.in_isolated(r.left_no.unwrap_or(0).saturating_sub(1))
            })
            .map(|(i, _)| i)
            .collect()
    }

    /// P38-1d：下一个编辑（相对当前编辑位置循环前进）
    pub fn next_edit(&mut self) {
        let rows = self.edited_rows();
        if rows.is_empty() {
            return;
        }
        let n = rows.len();
        let next = match self.edit_pos {
            Some(p) => (p + 1) % n,
            None => 0,
        };
        self.edit_pos = Some(next);
        self.jump_to_row(rows[next]);
    }

    /// P38-1d：上一个编辑（相对当前编辑位置循环后退）
    pub fn prev_edit(&mut self) {
        let rows = self.edited_rows();
        if rows.is_empty() {
            return;
        }
        let n = rows.len();
        let prev = match self.edit_pos {
            Some(p) => (p + n - 1) % n,
            None => n - 1,
        };
        self.edit_pos = Some(prev);
        self.jump_to_row(rows[prev]);
    }

    // ---- 差异跳转 ----

    pub fn next_diff(&mut self) {
        let rows = self.nav_diff_rows();
        if rows.is_empty() {
            return;
        }
        let n = rows.len();
        // diff_pos 是 diff_rows 的索引（与 P31 竖条标记一致），循环前进
        let next = match self.diff_pos {
            Some(p) => (p + 1) % n,
            None => 0,
        };
        self.diff_pos = Some(next);
        self.jump_to_row(rows[next]);
    }

    pub fn prev_diff(&mut self) {
        let rows = self.nav_diff_rows();
        if rows.is_empty() {
            return;
        }
        let n = rows.len();
        let prev = match self.diff_pos {
            Some(p) => (p + n - 1) % n,
            None => n - 1,
        };
        self.diff_pos = Some(prev);
        self.jump_to_row(rows[prev]);
    }

    /// P39-2c：下一差异部分（区块级跳转，BC ⇧⌃↓）
    pub fn next_diff_section(&mut self) {
        if self.diff_blocks.is_empty() {
            return;
        }
        let n = self.diff_blocks.len();
        // 无当前位置 → 从第一块开始；有 → 跳到所在块的下一个
        let cur = match self.diff_pos {
            None => None,
            Some(p) => {
                let cur_row = self.diff_rows.get(p).copied().unwrap_or(0);
                self.diff_blocks
                    .iter()
                    .position(|&(s, e)| s <= cur_row && cur_row <= e)
            }
        };
        let next = match cur {
            None => 0,
            Some(c) => (c + 1) % n,
        };
        let (s, _e) = self.diff_blocks[next];
        self.jump_to_section_row(s);
    }

    /// P39-2c：上一差异部分（区块级跳转，BC ⇧⌃↑）
    pub fn prev_diff_section(&mut self) {
        if self.diff_blocks.is_empty() {
            return;
        }
        let n = self.diff_blocks.len();
        // 无当前位置 → 从最后一块开始；有 → 跳到所在块的上一个
        let cur = match self.diff_pos {
            None => None,
            Some(p) => {
                let cur_row = self.diff_rows.get(p).copied().unwrap_or(0);
                self.diff_blocks
                    .iter()
                    .position(|&(s, e)| s <= cur_row && cur_row <= e)
            }
        };
        let prev = match cur {
            None => n - 1,
            Some(c) => (c + n - 1) % n,
        };
        let (s, _e) = self.diff_blocks[prev];
        self.jump_to_section_row(s);
    }

    /// P39-2c：跳转到区块起始行（同步 diff_pos 到对应 diff_rows 索引）
    fn jump_to_section_row(&mut self, row: usize) {
        if let Some(p) = self.diff_rows.iter().position(|&r| r == row) {
            self.diff_pos = Some(p);
        }
        self.jump_to_row(row);
    }

    /// 滚动到指定行索引（虚拟化：设置 scroll.y）
    pub fn jump_to_row(&mut self, row: usize) {
        let y = row as f32 * ROW_H;
        // 尽量让目标行出现在视口中部偏上
        self.scroll.y = (y - 4.0 * ROW_H).max(0.0);
        self.scroll.x = 0.0;
    }

    /// B4：hex 模式下一差异（按行循环跳转）
    pub fn hex_next_diff(&mut self) {
        let Some(h) = &self.hex else { return };
        let n = h.rows.len();
        if n == 0 {
            return;
        }
        let start = self.hex_diff_pos.map(|p| p + 1).unwrap_or(0);
        let mut target = None;
        for i in start..n {
            if h.rows[i].diff {
                target = Some(i);
                break;
            }
        }
        if target.is_none() {
            for i in 0..start.min(n) {
                if h.rows[i].diff {
                    target = Some(i);
                    break;
                }
            }
        }
        if let Some(i) = target {
            self.hex_diff_pos = Some(i);
            self.scroll.y = (i as f32 * HEX_ROW_H - 4.0 * HEX_ROW_H).max(0.0);
            self.scroll.x = 0.0;
        }
    }

    /// B4：hex 模式上一差异（按行循环跳转）
    pub fn hex_prev_diff(&mut self) {
        let Some(h) = &self.hex else { return };
        let n = h.rows.len();
        if n == 0 {
            return;
        }
        let start = self.hex_diff_pos.map(|p| p.saturating_sub(1)).unwrap_or(n);
        let mut target = None;
        for i in (0..=start).rev() {
            if i < n && h.rows[i].diff {
                target = Some(i);
                break;
            }
        }
        if target.is_none() {
            for i in (0..n).rev() {
                if h.rows[i].diff {
                    target = Some(i);
                    break;
                }
            }
        }
        if let Some(i) = target {
            self.hex_diff_pos = Some(i);
            self.scroll.y = (i as f32 * HEX_ROW_H - 4.0 * HEX_ROW_H).max(0.0);
            self.scroll.x = 0.0;
        }
    }

    /// P45-5：HEX 复制到右边（BC 16进制 编辑>复制到右边，⇧⌃→）——当前差异行左侧字节写入右侧文件
    pub fn hex_copy_to_right(&mut self) {
        let Some(h) = &self.hex else { return };
        let Some(idx) = self.hex_diff_pos else { return };
        let Some(row) = h.rows.get(idx) else { return };
        if row.left.is_empty() || !row.diff {
            return;
        }
        let (l_path, r_path) = (h.left.clone(), h.right.clone());
        let start = row.offset;
        let new_bytes = row.left.clone();
        let mut data = std::fs::read(&r_path).unwrap_or_default();
        for (k, b) in new_bytes.iter().enumerate() {
            let pos = start + k;
            if pos < data.len() {
                data[pos] = *b;
            } else {
                data.push(*b);
            }
        }
        // A2 保存前自动备份原文件为 <path>.bak
        let _ = std::fs::copy(&r_path, format!("{r_path}.bak"));
        match std::fs::write(&r_path, &data) {
            Ok(()) => {
                // 重新加载并重建
                self.load_pair(&l_path, &r_path, self.opts.clone());
            }
            Err(e) => {
                self.error = Some(format!("保存失败: {}", e));
            }
        }
    }

    pub fn handle_keys(&mut self, ui: &egui::Ui) {
        let ctrl = ui.input(|i| i.modifiers.command);
        // P32-A2：内联编辑中 Enter 提交 / ESC 取消（优先于搜索 Enter）
        if self.inline_edit.is_some() {
            if ui.input(|i| i.key_pressed(Key::Enter)) {
                self.commit_inline_edit();
            }
            if ui.input(|i| i.key_pressed(Key::Escape)) {
                self.inline_edit = None;
            }
            return;
        }
        // P32-A6：撤销/重做（Ctrl+Z / Ctrl+Y 或 Ctrl+Shift+Z）
        if ui.input(|i| i.key_pressed(Key::Z) && ctrl) {
            if ui.input(|i| i.modifiers.shift) {
                self.redo();
            } else {
                self.undo();
            }
            return;
        }
        if ui.input(|i| i.key_pressed(Key::Y) && ctrl) {
            self.redo();
            return;
        }
        if ui.input(|i| i.key_pressed(Key::F) && ctrl) {
            if ui.input(|i| i.modifiers.shift) {
                // P39-2e：⇧⌘F 替换
                self.search.replace_focus = true;
            } else {
                self.search.focus = true;
            }
            return;
        }
        // P39-2a：⌘L 转到行（BC 快捷键）；⌘G / ⇧⌘G 查找下一/上一；⌘V 剪贴板比较
        if ui.input(|i| i.key_pressed(Key::V) && ctrl) {
            // P42-2：打开剪贴板比较（BC File>打开剪贴板，右侧）
            self.load_clipboard_right();
            return;
        }
        // P44-2：⌘A 对齐方式（BC 编辑>对齐方式...；输入框聚焦时不触发）
        if ctrl && ui.input(|i| i.key_pressed(Key::A)) && !ui.ctx().egui_wants_keyboard_input() {
            self.align_current();
            return;
        }
        // P44-2：⌘E 使用选择内容进行查找（BC 搜索>使用选择内容进行查找）
        if ctrl && ui.input(|i| i.key_pressed(Key::E)) && !ui.ctx().egui_wants_keyboard_input() {
            self.find_selection();
            return;
        }
        // P44-2：]/[ 增加/减少缩进（BC 编辑>增加缩进/减少缩进；输入框聚焦时不触发）
        if !ui.ctx().egui_wants_keyboard_input() {
            if ui.input(|i| i.key_pressed(Key::CloseBracket)) {
                self.indent_current(1);
                return;
            }
            if ui.input(|i| i.key_pressed(Key::OpenBracket)) {
                self.indent_current(-1);
                return;
            }
        }
        if ui.input(|i| i.key_pressed(Key::L) && ctrl) {
            self.goto_focus = true;
            return;
        }
        if ui.input(|i| i.key_pressed(Key::G) && ctrl) {
            if ui.input(|i| i.modifiers.shift) {
                self.prev_match();
            } else {
                self.next_match();
            }
            return;
        }
        // P39-2a：1/2/3 视图过滤（BC 显示全部/差异/相同；输入框聚焦时不触发）
        if !ui.ctx().egui_wants_keyboard_input() {
            let vf = if ui.input(|i| i.key_pressed(Key::Num1)) {
                Some(DiffViewFilter::All)
            } else if ui.input(|i| i.key_pressed(Key::Num2)) {
                Some(DiffViewFilter::Diff)
            } else if ui.input(|i| i.key_pressed(Key::Num3)) {
                Some(DiffViewFilter::Same)
            } else {
                None
            };
            // P46-3：hex 模式 1/2/3 切换 hex 视图过滤（BC 16进制 显示全部/差异/相同）
            if self.hex.is_some() {
                let hf = if ui.input(|i| i.key_pressed(Key::Num1)) {
                    Some(HexViewFilter::All)
                } else if ui.input(|i| i.key_pressed(Key::Num2)) {
                    Some(HexViewFilter::Diff)
                } else if ui.input(|i| i.key_pressed(Key::Num3)) {
                    Some(HexViewFilter::Same)
                } else {
                    None
                };
                if let Some(hf) = hf {
                    if self.hex_filter != hf {
                        self.hex_filter = hf;
                    }
                    return;
                }
            }
            if let Some(vf) = vf {
                if self.view_filter != vf {
                    self.view_filter = vf;
                }
            }
            // P39-2d：⌘⌥⌃0-9 切换书签 / ⌘0-9 转到书签（BC 书签快捷键）
            let cmd = ui.input(|i| i.modifiers.command);
            let alt = ui.input(|i| i.modifiers.alt);
            let ctrl = ui.input(|i| i.modifiers.ctrl);
            for d in 0..=9u8 {
                let key = match d {
                    0 => Key::Num0,
                    1 => Key::Num1,
                    2 => Key::Num2,
                    3 => Key::Num3,
                    4 => Key::Num4,
                    5 => Key::Num5,
                    6 => Key::Num6,
                    7 => Key::Num7,
                    8 => Key::Num8,
                    _ => Key::Num9,
                };
                if ui.input(|i| i.key_pressed(key)) {
                    if cmd && alt && ctrl {
                        self.toggle_bookmark(d);
                    } else if cmd {
                        self.goto_bookmark(d);
                    }
                }
            }
        }
        // B1：F6 下一差异 / F7 上一差异（hex 模式下走 hex 差异导航）
        if ui.input(|i| i.key_pressed(Key::F6)) {
            if self.hex.is_some() {
                self.hex_next_diff();
            } else {
                self.next_diff();
            }
        }
        if ui.input(|i| i.key_pressed(Key::F7)) {
            if self.hex.is_some() {
                self.hex_prev_diff();
            } else {
                self.prev_diff();
            }
        }
        // P39-2c：⇧⌃↓/↑ 差异部分导航（BC 区块级跳转；输入框聚焦时不触发）
        if !ui.ctx().egui_wants_keyboard_input() {
            if ui.input(|i| i.modifiers.ctrl && i.modifiers.shift && i.key_pressed(Key::ArrowDown))
            {
                self.next_diff_section();
                return;
            }
            if ui.input(|i| i.modifiers.ctrl && i.modifiers.shift && i.key_pressed(Key::ArrowUp)) {
                self.prev_diff_section();
                return;
            }
        }
        // B7：F5 重新加载
        if ui.input(|i| i.key_pressed(Key::F5)) {
            self.reload();
        }
        // P45-5：⇧⌃→ HEX 复制到右边（BC 16进制 编辑>复制到右边）
        if self.hex.is_some()
            && ui.input(|i| i.modifiers.ctrl && i.modifiers.shift && i.key_pressed(Key::ArrowRight))
        {
            self.hex_copy_to_right();
            return;
        }
        if ui.input(|i| i.key_pressed(Key::Enter)) {
            self.next_match();
        }
        if ui.input(|i| i.key_pressed(Key::Escape)) {
            self.search.query.clear();
            self.update_search();
        }
    }

    // ---- 渲染 ----

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        self.handle_keys(ui);

        // 搜索/跳转工具条
        // P42-4：工具栏开关（BC View>工具栏）
        if super::common::SHOW_TOOLBAR.load(std::sync::atomic::Ordering::Relaxed) {
            egui::Panel::top("difftab_tools").show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    // ---- 打开（BC: Open 按钮组）----
                    if ui
                        .button(format!("◀ {}", t(I18nKey::OpenLeft)))
                        .on_hover_text(t(I18nKey::OpenLeftFile))
                        .clicked()
                    {
                        self.open_left_dialog();
                    }
                    if ui
                        .button(format!("▶ {}", t(I18nKey::OpenRight)))
                        .on_hover_text(t(I18nKey::OpenRightFile))
                        .clicked()
                    {
                        self.open_right_dialog();
                    }
                    ui.separator();
                    // ---- 显示过滤（BC: All▾ Diffs Context 组）----
                    // P35-A3：视图过滤下拉（Show All/Diff/Same/Context）
                    {
                        let filters = [
                            (DiffViewFilter::All, t(I18nKey::ShowAll)),
                            (DiffViewFilter::Diff, t(I18nKey::OnlyDiff)),
                            (DiffViewFilter::Same, t(I18nKey::ShowSame)),
                            (DiffViewFilter::Context, t(I18nKey::ShowContext)),
                        ];
                        let cur = self.view_filter;
                        egui::ComboBox::from_id_salt("diff_view_filter")
                            .selected_text(
                                filters
                                    .iter()
                                    .find(|(v, _)| *v == cur)
                                    .map(|(_, l)| *l)
                                    .unwrap_or(""),
                            )
                            .show_ui(ui, |ui| {
                                for (v, l) in &filters {
                                    if ui.selectable_label(cur == *v, *l).clicked() {
                                        self.view_filter = *v;
                                    }
                                }
                            });
                    }
                    // P40-1：忽略/显示选项收进 View 菜单（原 checkbox 已移除）
                    ui.separator();
                    // ---- 编辑（BC: Copy/编辑组）----
                    // P40-1：编辑左/右收进 Edit 菜单（start_edit）；撤销/重做保留（测试与高频操作依赖）
                    // P32-A6：撤销/重做按钮
                    let can_undo = !self.undo_stack.is_empty();
                    let can_redo = !self.redo_stack.is_empty();
                    if ui
                        .add_enabled(can_undo, egui::Button::new("↩ 撤销"))
                        .on_hover_text("Ctrl+Z")
                        .clicked()
                    {
                        self.undo();
                    }
                    if ui
                        .add_enabled(can_redo, egui::Button::new("↪ 重做"))
                        .on_hover_text("Ctrl+Y")
                        .clicked()
                    {
                        self.redo();
                    }
                    ui.separator();
                    // ---- 操作（BC: Copy/Swap/Reload 组）----
                    // P35-A1：复制差异块到另一侧（BC Copy to Other Side）
                    let has_diff = self.diff_pos.is_some();
                    if ui
                        .add_enabled(
                            has_diff,
                            egui::Button::new(format!("→ {}", t(I18nKey::CopyToRight))),
                        )
                        .on_hover_text("复制当前差异块左侧内容到右侧")
                        .clicked()
                    {
                        self.copy_block_to(EditSide::Right);
                    }
                    if ui
                        .add_enabled(
                            has_diff,
                            egui::Button::new(format!("← {}", t(I18nKey::CopyToLeft))),
                        )
                        .on_hover_text("复制当前差异块右侧内容到左侧")
                        .clicked()
                    {
                        self.copy_block_to(EditSide::Left);
                    }
                    if ui
                        .button(format!("⟳ {}", t(I18nKey::Reload)))
                        .on_hover_text(t(I18nKey::ReloadHint))
                        .clicked()
                    {
                        self.reload();
                    }
                    // P35-A2：交换左右两侧（BC Swap Sides）
                    if ui
                        .button(format!("⇄ {}", t(I18nKey::SwapSides)))
                        .on_hover_text("交换左右两侧文件")
                        .clicked()
                    {
                        self.swap_sides();
                    }
                    ui.separator();
                    // ---- 差异导航（BC: Next Section/Prev Section 组）----
                    ui.label(fmt(
                        I18nKey::DiffCount,
                        &[
                            &self.diff_rows.len().to_string(),
                            &self.rows.len().to_string(),
                        ],
                    ));
                    if ui
                        .button(format!("⬇ {}", t(I18nKey::NextDiff)))
                        .on_hover_text("下一差异 (F6)")
                        .clicked()
                    {
                        self.next_diff();
                    }
                    if ui
                        .button(format!("⬆ {}", t(I18nKey::PrevDiff)))
                        .on_hover_text("上一差异 (F7)")
                        .clicked()
                    {
                        self.prev_diff();
                    }
                    // 跳转行
                    let mut goto_text = self.goto_line.map(|l| l.to_string()).unwrap_or_default();
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut goto_text)
                            .hint_text(t(I18nKey::GotoHint))
                            .desired_width(70.0),
                    );
                    if self.goto_focus {
                        resp.request_focus();
                        self.goto_focus = false;
                    }
                    self.goto_line = goto_text.parse().ok();
                    if (resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)))
                        || ui.button(format!("🎯 {}", t(I18nKey::Goto))).clicked()
                    {
                        if let Some(line) = self.goto_line {
                            if line >= 1 {
                                self.jump_to_row(line - 1);
                            }
                        }
                    }
                    ui.separator();
                    // ---- 搜索/替换（BC: 搜索组）----
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut self.search.query)
                            .id(egui::Id::new("diff_search"))
                            .hint_text(t(I18nKey::SearchHint))
                            .desired_width(220.0),
                    );
                    if self.search.focus {
                        resp.request_focus();
                        self.search.focus = false;
                    }
                    if resp.changed() {
                        self.update_search();
                    }
                    if ui
                        .button("⬆")
                        .on_hover_text(t(I18nKey::PrevMatch))
                        .clicked()
                    {
                        self.prev_match();
                    }
                    if ui
                        .button("⬇")
                        .on_hover_text(t(I18nKey::NextMatch))
                        .clicked()
                    {
                        self.next_match();
                    }
                    if let Some(k) = self.search.current {
                        ui.label(format!("{}/{}", k + 1, self.search.matches.len()));
                    }
                    // A4 文本替换
                    let rep_resp = ui.add(
                        egui::TextEdit::singleline(&mut self.search.replace)
                            .id(egui::Id::new("diff_replace"))
                            .hint_text("替换为")
                            .desired_width(100.0),
                    );
                    if self.search.replace_focus {
                        rep_resp.request_focus();
                        self.search.replace_focus = false;
                    }
                    if ui
                        .button("🔁 替换")
                        .on_hover_text("替换当前匹配（写回文件并自动备份）")
                        .clicked()
                    {
                        self.replace_current();
                    }
                    if ui
                        .button("🔁 全部替换")
                        .on_hover_text("替换所有匹配（写回文件并自动备份）")
                        .clicked()
                    {
                        self.replace_all();
                    }
                    // P33：长行横向滚动条（两栏固定各半屏，超长行栏内左右滑动查看）
                    if !self.rows.is_empty() && self.hex.is_none() {
                        let max_chars = self
                            .rows
                            .iter()
                            .flat_map(|r| [r.left.as_ref(), r.right.as_ref()])
                            .flatten()
                            .map(|c| c.text.chars().count())
                            .max()
                            .unwrap_or(0);
                        let max_line_w = max_chars as f32 * 9.0 + 24.0;
                        let avail = ui.available_width();
                        let max_no_l = self
                            .rows
                            .iter()
                            .filter_map(|r| r.left_no)
                            .max()
                            .unwrap_or(0);
                        let max_no_r = self
                            .rows
                            .iter()
                            .filter_map(|r| r.right_no)
                            .max()
                            .unwrap_or(0);
                        let gutter_l = crate::gui::common::gutter_width(max_no_l);
                        let gutter_r = crate::gui::common::gutter_width(max_no_r);
                        let mid_gap = super::theme::MID_GAP;
                        let half = ((avail - gutter_l - gutter_r - mid_gap) / 2.0).max(200.0);
                        let h_max = (max_line_w - half).max(0.0);
                        if h_max > 0.0 {
                            ui.separator();
                            ui.label("↔")
                                .on_hover_text("长行横向滚动：拖动查看超宽内容");
                            ui.add(
                                egui::Slider::new(&mut self.h_scroll, 0.0..=h_max)
                                    .show_value(false)
                                    .custom_formatter(|v, _| format!("{:.0}px", v)),
                            );
                        } else {
                            self.h_scroll = 0.0;
                        }
                    }
                });
            });
        } // P42-4：difftab_tools 门控闭合

        // 错误弹窗
        if let Some(err) = self.error.clone() {
            crate::gui::common::dialog_window(ui.ctx(), t(I18nKey::Error))
                .collapsible(false)
                .resizable(false)
                .show(ui.ctx(), |ui| {
                    ui.colored_label(super::theme::error_color(), err);
                    if ui.button(t(I18nKey::Close)).clicked() {
                        self.error = None;
                    }
                });
        }

        // 编辑窗口
        if let Some(edit) = &mut self.editing {
            let side_name = match edit.side {
                EditSide::Left => t(I18nKey::SideLeft),
                EditSide::Right => t(I18nKey::SideRight),
            };
            let mut close = false;
            let mut save = false;
            crate::gui::common::dialog_window(ui.ctx(), format!("编辑{side_name}: {}", edit.path))
                .default_size([800.0, 600.0])
                .resizable(true)
                .show(ui.ctx(), |ui| {
                    egui::ScrollArea::both()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.add(
                                egui::TextEdit::multiline(&mut edit.content)
                                    .font(egui::TextStyle::Monospace)
                                    .desired_width(f32::INFINITY)
                                    .desired_rows(30)
                                    .code_editor(),
                            );
                        });
                    ui.horizontal(|ui| {
                        if ui.button(t(I18nKey::Save)).clicked() {
                            save = true;
                        }
                        if ui.button(t(I18nKey::Cancel)).clicked() {
                            close = true;
                        }
                        ui.label(t(I18nKey::SaveHint));
                    });
                    if ui.input(|i| i.modifiers.command && i.key_pressed(Key::S)) {
                        save = true;
                    }
                });
            if save {
                // 先克隆所需数据，避免同时借用 self.editing 与 self 方法
                let (path, side) = self
                    .editing
                    .as_ref()
                    .map(|e| (e.path.clone(), e.side))
                    .unwrap();
                let content = self.editing.as_ref().map(|e| e.content.clone()).unwrap();
                // 按原编码回写（保留 BOM 与编码，避免破坏 GBK/UTF-16 文件）
                let bytes = match side {
                    EditSide::Left => self.left.as_ref().map(|f| {
                        crate::encoding::encode_back(
                            &crate::encoding::TextFile {
                                text: String::new(),
                                encoding: f.encoding,
                                had_bom: f.had_bom,
                                is_binary: false,
                            },
                            &content,
                        )
                    }),
                    EditSide::Right => self.right.as_ref().map(|f| {
                        crate::encoding::encode_back(
                            &crate::encoding::TextFile {
                                text: String::new(),
                                encoding: f.encoding,
                                had_bom: f.had_bom,
                                is_binary: false,
                            },
                            &content,
                        )
                    }),
                };
                let write_res = match bytes {
                    Some(b) => {
                        // A2 保存前自动备份原文件为 <path>.bak（BC 行为，防手滑覆盖）
                        let _ = std::fs::copy(&path, format!("{path}.bak"));
                        std::fs::write(&path, b)
                    }
                    None => Ok(()),
                };
                match write_res {
                    Ok(()) => {
                        close = true;
                        // 保存后重新加载对应侧并重算 diff
                        match side {
                            EditSide::Left => self.load_left(&path, self.opts.clone()),
                            EditSide::Right => self.load_right(&path, self.opts.clone()),
                        }
                        self.error = Some(fmt(I18nKey::Saved, &[&path]));
                    }
                    Err(e) => {
                        self.error = Some(fmt(I18nKey::SaveFailed, &[&e.to_string()]));
                    }
                }
            }
            if close {
                self.editing = None;
            }
        }

        // P50：点击文件信息头 → 更换该侧文件请求（闭包内赋值，闭包结束后处理，
        // 避免与渲染区 &self.rows 借用冲突）
        let mut header_click: Option<EditSide> = None;

        egui::CentralPanel::default().show(ui, |ui| {
            // 二进制 hex 对比模式（克隆到局部，渲染基于局部副本，保存时可自由 &mut self）
            let hex_owned = self.hex.clone();
            if let Some(h) = &hex_owned {
                if h.rows.is_empty() {
                    // P52-2：统一空状态（hex 用靛蓝色系）
                    super::common::empty_state(
                        ui,
                        "0F",
                        super::theme::card_icon_colors()[5],
                        t(I18nKey::DiffEmptyHint),
                        t(I18nKey::DragHint),
                        |_ui| {},
                    );
                    return;
                }
                let fg = text_color(ui);
                let diff_count = h.rows.iter().filter(|r| r.diff).count();
                // P46-3：视图过滤（显示全部/差异/相同）+ 布局行高（上-下 = 2 行高）
                let row_h = match self.hex_layout {
                    HexViewLayout::SideBySide => HEX_ROW_H,
                    HexViewLayout::TopBottom => HEX_ROW_H * 2.0,
                };
                let visible: Vec<usize> = (0..h.rows.len())
                    .filter(|&i| match self.hex_filter {
                        HexViewFilter::All => true,
                        HexViewFilter::Diff => h.rows[i].diff,
                        HexViewFilter::Same => !h.rows[i].diff,
                    })
                    .collect();
                let total_w = HEX_TOTAL_W;
                let mut edit_click: Option<usize> = None;
                let mut save_req = false;
                // 编辑状态下 Ctrl+S 保存
                if self.hex_edit.is_some()
                    && ui.input(|i| i.modifiers.command && i.key_pressed(Key::S))
                {
                    save_req = true;
                }
                let out =
                    super::show_rows_offset(ui, visible.len(), row_h, self.scroll, |ui, range| {
                        ui.set_min_width(total_w);
                        for vi in range {
                            let i = visible[vi];
                            let row = &h.rows[i];
                            // 正在编辑的行：显示输入框
                            if let Some(he) = &self.hex_edit {
                                if he.row == i {
                                    let (rect, resp) = ui.allocate_exact_size(
                                        Vec2::new(total_w, HEX_ROW_H),
                                        egui::Sense::click(),
                                    );
                                    if row.diff {
                                        paint_bg(ui, rect, Some(bg_replace_l()));
                                    }
                                    ui.painter().text(
                                        Pos2::new(rect.left() + HEX_OFF_X, rect.top() + 2.0),
                                        egui::Align2::LEFT_TOP,
                                        format!("{:08x}", row.offset),
                                        egui::FontId::monospace(13.0),
                                        GUTTER,
                                    );
                                    let mut buf = self
                                        .hex_edit
                                        .as_ref()
                                        .map(|e| e.buf.clone())
                                        .unwrap_or_default();
                                    let te = ui.add(
                                        egui::TextEdit::singleline(&mut buf)
                                            .font(egui::TextStyle::Monospace)
                                            .desired_width(240.0)
                                            .hint_text("hex bytes, e.g. 01 0a ff"),
                                    );
                                    if let Some(he) = self.hex_edit.as_mut() {
                                        he.buf = buf;
                                    }
                                    if resp.double_clicked() {
                                        edit_click = Some(i);
                                    }
                                    let _ = te;
                                    continue;
                                }
                            }
                            paint_hex_row(ui, row, fg, h.addr_hex, h.value_mode, h.show_addr);
                            // 双击进入编辑（编辑该行左/右侧字节）
                            let (rect, resp) = ui.allocate_exact_size(
                                Vec2::new(total_w, HEX_ROW_H),
                                egui::Sense::click(),
                            );
                            if resp.double_clicked() {
                                edit_click = Some(i);
                            }
                            let _ = rect;
                        }
                    });
                // 双击 → 打开编辑（默认编辑差异行左侧）
                if let Some(i) = edit_click {
                    if let Some(h) = &self.hex {
                        let row = &h.rows[i];
                        let bytes = if row.left.len() >= row.right.len() {
                            &h.left_bytes
                        } else {
                            &h.right_bytes
                        };
                        // 取该行字节作为初始缓冲区
                        let start = row.offset;
                        let end = (start + 16).min(bytes.len());
                        let chunk = bytes.get(start..end).unwrap_or_default();
                        let hex_str: Vec<String> =
                            chunk.iter().map(|b| format!("{:02x}", b)).collect();
                        self.hex_edit = Some(HexEditState {
                            side: EditSide::Left,
                            row: i,
                            buf: hex_str.join(" "),
                        });
                    }
                }
                // 保存编辑
                if save_req {
                    let (side, row_idx, buf) = self
                        .hex_edit
                        .as_ref()
                        .map(|e| (e.side, e.row, e.buf.clone()))
                        .unwrap();
                    // 解析十六进制输入
                    let mut new_bytes: Vec<u8> = Vec::new();
                    let mut ok = true;
                    for tok in buf.split_whitespace() {
                        match u8::from_str_radix(tok, 16) {
                            Ok(b) => new_bytes.push(b),
                            Err(_) => {
                                ok = false;
                                break;
                            }
                        }
                    }
                    if ok && !new_bytes.is_empty() {
                        // 写回文件对应偏移（先克隆路径避免借用冲突）
                        let (l_path, r_path) = self
                            .hex
                            .as_ref()
                            .map(|h| (h.left.clone(), h.right.clone()))
                            .unwrap();
                        let path = match side {
                            EditSide::Left => l_path.clone(),
                            EditSide::Right => r_path.clone(),
                        };
                        let start = self.hex.as_ref().unwrap().rows[row_idx].offset;
                        let mut data = std::fs::read(&path).unwrap_or_default();
                        for (k, b) in new_bytes.iter().enumerate() {
                            let pos = start + k;
                            if pos < data.len() {
                                data[pos] = *b;
                            } else {
                                data.push(*b);
                            }
                        }
                        // A2 保存前自动备份原文件为 <path>.bak
                        let _ = std::fs::copy(&path, format!("{path}.bak"));
                        match std::fs::write(&path, &data) {
                            Ok(()) => {
                                self.hex_edit = None;
                                // 重新加载并重建
                                self.load_pair(&l_path, &r_path, self.opts.clone());
                            }
                            Err(e) => {
                                self.error = Some(format!("保存失败: {}", e));
                            }
                        }
                    } else {
                        self.error = Some("十六进制输入无效".to_string());
                    }
                }
                self.scroll = out.state.offset;
                if self.show_stats {
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label(fmt(
                            I18nKey::DiffCount,
                            &[&diff_count.to_string(), &h.rows.len().to_string()],
                        ));
                        ui.separator();
                        ui.label(fmt(I18nKey::HexModeHint, &[]));
                        ui.separator();
                        ui.label(format!("{}  ↔  {}", h.left, h.right));
                    });
                }
                return;
            }

            if self.rows.is_empty() {
                // P52-2：统一空状态（大图标底片 + 标题 + 操作）
                super::common::empty_state(
                    ui,
                    "⇄",
                    super::theme::card_icon_colors()[0],
                    t(I18nKey::DiffEmptyHint),
                    t(I18nKey::DragHint),
                    |ui| {
                        // P34：分别打开左右两侧（BC 式：不强求一次选满两个）
                        ui.horizontal(|ui| {
                            if ui.button(t(I18nKey::OpenLeft)).clicked() {
                                self.open_left_dialog();
                            }
                            if ui.button(t(I18nKey::OpenRight)).clicked() {
                                self.open_right_dialog();
                            }
                        });
                    },
                );
                return;
            }

            let max_no_l = self
                .rows
                .iter()
                .filter_map(|r| r.left_no)
                .max()
                .unwrap_or(0);
            let max_no_r = self
                .rows
                .iter()
                .filter_map(|r| r.right_no)
                .max()
                .unwrap_or(0);
            let gutter_l = if self.show_line_numbers {
                gutter_width(max_no_l)
            } else {
                0.0
            };
            let gutter_r = if self.show_line_numbers {
                gutter_width(max_no_r)
            } else {
                0.0
            };
            // P33：两栏固定各占半屏（BC 式等分），长行栏内横向滚动查看；随窗口缩放自适应
            // P39-2d：布局切换 —— SideBySide 左右并排各半宽；TopBottom/Web 单栏全宽上下堆叠
            let avail = ui.available_width();
            let mid_gap = super::theme::MID_GAP;
            let (content_w, total_w, half) = match self.layout {
                DiffLayout::SideBySide => {
                    let half = ((avail - gutter_l - gutter_r - mid_gap) / 2.0).max(200.0);
                    (half, gutter_l + half + mid_gap + gutter_r + half, half)
                }
                DiffLayout::TopBottom | DiffLayout::Web => {
                    let w = (avail - gutter_l.max(gutter_r)).max(200.0);
                    (w, avail, w)
                }
            };
            // 最长行所需宽度（供横向滚动条范围计算）
            let max_chars = self
                .rows
                .iter()
                .flat_map(|r| [r.left.as_ref(), r.right.as_ref()])
                .flatten()
                .map(|c| c.text.chars().count())
                .max()
                .unwrap_or(0);
            let max_line_w = max_chars as f32 * 9.0 + 24.0;
            let _ = max_line_w; // P33：横向滚动条范围在工具栏计算（栏宽固定半屏）
            let fg = text_color(ui);

            // 匹配行集合（搜索高亮）
            let match_set: std::collections::HashSet<usize> =
                self.search.matches.iter().copied().collect();
            // P38-1d：已编辑行集合（渲染小圆点标记）
            let edited_set: std::collections::HashSet<usize> =
                self.edited_rows().into_iter().collect();
            let current_match = self
                .search
                .current
                .and_then(|k| self.search.matches.get(k).copied());
            let rows = &self.rows;
            // A8 自动换行：展开为视觉行（保留原始行索引映射，供搜索/跳转）
            let (render_rows, orig_idx): (Vec<crate::sideview::SideRow>, Vec<usize>) = if self.wrap
            {
                let wrap_chars = ((half - 24.0) / 8.5).max(8.0) as usize;
                crate::sideview::wrap_rows(rows, wrap_chars)
            } else {
                (Vec::new(), Vec::new())
            };
            let display_rows: &[crate::sideview::SideRow] =
                if self.wrap { &render_rows } else { rows };
            let orig_of = |vi: usize| -> usize {
                if self.wrap {
                    orig_idx.get(vi).copied().unwrap_or(vi)
                } else {
                    vi
                }
            };
            // 左右语法（按文件路径解析，供行内语法高亮；P44-6 可开关）
            let syn_l = if self.show_syntax {
                self.left.as_ref().and_then(|f| f.syntax)
            } else {
                None
            };
            let syn_r = if self.show_syntax {
                self.right.as_ref().and_then(|f| f.syntax)
            } else {
                None
            };

            // 受控滚动 + 虚拟化渲染（统一走 common::show_rows）
            // P32-A2：取出行内编辑状态，渲染循环内传入；双击请求在下循环结束后处理
            let mut inline = self.inline_edit.take();
            let mut dbl: Option<(usize, EditSide)> = None;
            // P32-A5：构建折叠显示映射 (显示行索引 vi, 折叠占位块)
            // 折叠块只保留首行 + 一个“N 行已折叠”占位行；块内其余行隐藏
            let mut view: Vec<(usize, Option<usize>)> = Vec::new();
            let mut placed_block: std::collections::HashSet<usize> =
                std::collections::HashSet::new();
            let mut seen_first: std::collections::HashSet<usize> = std::collections::HashSet::new();
            // P35-A3：差异行集合（视图过滤用）
            let diff_set: std::collections::HashSet<usize> =
                self.diff_rows.iter().copied().collect();
            for vi in 0..display_rows.len() {
                let oi = orig_of(vi);
                // P38-1a：隔离过滤（只显示隔离范围内的行）
                if !self.in_isolated(oi) {
                    continue;
                }
                // P35-A3：视图过滤（All/Diff/Same/Context）
                if !self.row_visible(oi, &diff_set) {
                    continue;
                }
                // oi 属于哪个差异块？
                let blk = self
                    .diff_blocks
                    .iter()
                    .position(|&(s, e)| oi >= s && oi <= e);
                match blk {
                    Some(bi) if self.collapsed_blocks.contains(&bi) => {
                        let (s, _e) = self.diff_blocks[bi];
                        if oi == s && !seen_first.contains(&s) {
                            // 块首行保留，随后追加占位行
                            view.push((vi, None));
                            if !placed_block.contains(&bi) {
                                view.push((vi, Some(bi)));
                                placed_block.insert(bi);
                            }
                            seen_first.insert(s);
                        }
                        // 其余行（含首行的后续视觉行）隐藏
                    }
                    _ => view.push((vi, None)),
                }
            }
            let mut fold_toggle: Option<usize> = None;
            let mut ignore_req: Option<usize> = None;
            // P38-1a：隔离/取消隔离 + 提示条点击取消（借用安全：闭包内只置标志）
            let mut isolate_req = false;
            let mut unisolate_req = false;
            let mut banner_unisolate = false;
            // P38-1b：对齐方式请求（源侧, 源行号）+ 清除对齐 + 目标行点击
            let mut align_req: Option<(EditSide, usize)> = None;
            let mut clear_align_req = false;
            let mut align_target_click: Option<usize> = None;
            let mut align_cancel = false;
            // P38-1c：缩进调整请求（行索引, delta）
            let mut indent_req: Option<(usize, isize)> = None;
            // P35-A1：右键复制差异块到另一侧请求 (行索引, 目标侧)
            let mut copy_req: Option<(usize, EditSide)> = None;
            // P38-1e：文件级联动请求（目标侧）
            let mut copy_file_req: Option<EditSide> = None;
            // P37-1m：右键复制行到另一侧请求 (行索引, 目标侧)
            let mut copy_line_req: Option<(usize, EditSide)> = None;
            // P37-1j：右键外部工具对比请求 (左路径, 右路径)
            let mut external_req: Option<(String, String)> = None;

            // BC 式左右两页：顶部文件名头部（固定视口宽度，不随内容横向滚动移动）
            // P33：两行结构 — 第一行文件名，第二行详情（时间 | 大小 | 编码），对标 BC 5
            {
                let head_h = 42.0;
                let head_bg = Some(super::theme::head_bg(ui.visuals().dark_mode));
                let head_fg = super::theme::head_fg(ui.visuals().dark_mode);
                let detail_fg = ui.visuals().weak_text_color();
                // 头部两栏各占视口半宽（gutter + half），长行内容超宽时头部不跟随滚动
                let head_l_w = gutter_l + half;
                let head_r_w = gutter_r + half;
                // 每侧头部信息：全路径（BC 风格，可点击更换）+ 详情行
                let l_info = self
                    .left
                    .as_ref()
                    .map(|f| (f.path.clone(), file_detail_line(f)));
                let r_info = self
                    .right
                    .as_ref()
                    .map(|f| (f.path.clone(), file_detail_line(f)));
                // 全路径截断：超宽时保留头部目录 + … + 文件名（BC 行为）
                let l_path = l_info
                    .as_ref()
                    .map(|(p, _)| p.clone())
                    .unwrap_or_else(|| t(I18nKey::OpenLeft).to_string());
                let r_path = r_info
                    .as_ref()
                    .map(|(p, _)| p.clone())
                    .unwrap_or_else(|| t(I18nKey::OpenRight).to_string());
                let l_name = truncate_path(&l_path, ((head_l_w - 42.0) / 7.0) as usize);
                let r_name = truncate_path(&r_path, ((head_r_w - 42.0) / 7.0) as usize);
                let l_detail = l_info.as_ref().map(|(_, d)| d.clone()).unwrap_or_default();
                let r_detail = r_info.as_ref().map(|(_, d)| d.clone()).unwrap_or_default();
                ui.horizontal(|ui| {
                    // 左头部：两行（路径 13px + 详情 11px）
                    let (l_rect, l_resp) =
                        ui.allocate_exact_size(Vec2::new(head_l_w, head_h), egui::Sense::click());
                    paint_bg(ui, l_rect, head_bg);
                    // 📁 文件夹图标：点击可更换文件路径（BC 风格）
                    ui.painter().text(
                        Pos2::new(l_rect.left() + 10.0, l_rect.top() + 11.0),
                        egui::Align2::LEFT_CENTER,
                        "📁",
                        egui::FontId::proportional(13.0),
                        head_fg,
                    );
                    ui.painter().text(
                        Pos2::new(l_rect.left() + 30.0, l_rect.top() + 11.0),
                        egui::Align2::LEFT_CENTER,
                        l_name,
                        egui::FontId::proportional(13.0),
                        head_fg,
                    );
                    ui.painter().text(
                        Pos2::new(l_rect.left() + 10.0, l_rect.top() + 30.0),
                        egui::Align2::LEFT_CENTER,
                        l_detail,
                        egui::FontId::proportional(11.0),
                        detail_fg,
                    );
                    if l_resp.clicked() {
                        header_click = Some(EditSide::Left);
                    }
                    // hover 提示：完整路径 + 更换说明（BC 风格）
                    l_resp
                        .clone()
                        .on_hover_text(format!("{}\n点击更换文件", l_path));
                    // 中间空隙
                    ui.allocate_exact_size(Vec2::new(mid_gap, head_h), egui::Sense::hover());
                    // 右头部
                    let (r_rect, r_resp) =
                        ui.allocate_exact_size(Vec2::new(head_r_w, head_h), egui::Sense::click());
                    paint_bg(ui, r_rect, head_bg);
                    // 📁 文件夹图标：点击可更换文件路径（BC 风格）
                    ui.painter().text(
                        Pos2::new(r_rect.left() + 10.0, r_rect.top() + 11.0),
                        egui::Align2::LEFT_CENTER,
                        "📁",
                        egui::FontId::proportional(13.0),
                        head_fg,
                    );
                    ui.painter().text(
                        Pos2::new(r_rect.left() + 30.0, r_rect.top() + 11.0),
                        egui::Align2::LEFT_CENTER,
                        r_name,
                        egui::FontId::proportional(13.0),
                        head_fg,
                    );
                    ui.painter().text(
                        Pos2::new(r_rect.left() + 10.0, r_rect.top() + 30.0),
                        egui::Align2::LEFT_CENTER,
                        r_detail,
                        egui::FontId::proportional(11.0),
                        detail_fg,
                    );
                    if r_resp.clicked() {
                        header_click = Some(EditSide::Right);
                    }
                    // hover 提示：完整路径 + 更换说明（BC 风格）
                    r_resp
                        .clone()
                        .on_hover_text(format!("{}\n点击更换文件", r_path));
                });
                ui.separator();
            }

            // P42-3：字符列标尺（BC 标尺，内容区顶部绘制 10/20/... 刻度）
            if self.show_ruler && self.hex.is_none() {
                let ruler_h = 16.0;
                let (ruler_rect, _) =
                    ui.allocate_exact_size(Vec2::new(total_w, ruler_h), egui::Sense::hover());
                let ruler_bg = super::theme::ruler_bg(ui.visuals().dark_mode);
                paint_bg(ui, ruler_rect, Some(ruler_bg));
                let tick_fg = ui.visuals().weak_text_color();
                let font = egui::FontId::monospace(9.0);
                // 左栏刻度（gutter + content 起点对齐行内容）
                let left_start = ruler_rect.left() + gutter_l;
                let right_start = ruler_rect.left() + gutter_l + content_w + mid_gap + gutter_r;
                for (start, w) in [(left_start, content_w), (right_start, content_w)] {
                    for col in (10..=200).step_by(10) {
                        let x = start + col as f32 * 8.0; // 约每字符 8px（等宽 14px 的 0.57 倍）
                        if x > start + w - 4.0 {
                            break;
                        }
                        ui.painter().text(
                            Pos2::new(x, ruler_rect.top() + ruler_h / 2.0),
                            egui::Align2::LEFT_CENTER,
                            col.to_string(),
                            font.clone(),
                            tick_fg,
                        );
                    }
                }
                ui.separator();
            }

            // P38-1a：隔离提示条（已隔离 行 X–Y [✕ 显示全部]）
            if let Some((s, e)) = self.isolated {
                let (rect, resp) =
                    ui.allocate_exact_size(Vec2::new(total_w, 26.0), egui::Sense::click());
                let bg = super::theme::banner_isolate_bg(ui.visuals().dark_mode);
                ui.painter().rect_filled(rect, 2.0, bg);
                let fg = ui.visuals().strong_text_color();
                ui.painter().text(
                    Pos2::new(rect.left() + 10.0, rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    format!("🔍 已隔离 行 {}–{}（仅显示该差异区域）", s + 1, e + 1),
                    egui::FontId::proportional(12.0),
                    fg,
                );
                ui.painter().text(
                    Pos2::new(rect.right() - 10.0, rect.center().y),
                    egui::Align2::RIGHT_CENTER,
                    "✕ 显示全部",
                    egui::FontId::proportional(12.0),
                    fg,
                );
                if resp.clicked() {
                    banner_unisolate = true;
                }
                ui.separator();
            }

            // P38-1b：对齐模式提示条（请点击另一侧行完成对齐 [✕ 取消]）
            if let Some((side, src_no)) = self.align_pick {
                let (rect, resp) =
                    ui.allocate_exact_size(Vec2::new(total_w, 26.0), egui::Sense::click());
                let bg = super::theme::banner_align_bg(ui.visuals().dark_mode);
                ui.painter().rect_filled(rect, 2.0, bg);
                let fg = ui.visuals().strong_text_color();
                let side_name = match side {
                    EditSide::Left => "左侧",
                    EditSide::Right => "右侧",
                };
                ui.painter().text(
                    Pos2::new(rect.left() + 10.0, rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    format!("⇋ 对齐方式：{side_name} 行 {src_no}，请点击另一侧行完成对齐"),
                    egui::FontId::proportional(12.0),
                    fg,
                );
                ui.painter().text(
                    Pos2::new(rect.right() - 10.0, rect.center().y),
                    egui::Align2::RIGHT_CENTER,
                    "✕ 取消",
                    egui::FontId::proportional(12.0),
                    fg,
                );
                if resp.clicked() {
                    align_cancel = true;
                }
                ui.separator();
            }

            let out = super::show_rows(ui, view.len(), self.row_h(), |ui, range| {
                ui.set_min_width(total_w);
                // 当前差异行（diff_pos → diff_rows 中的行索引，P31 竖条标记）
                let cur_diff_orig = self.diff_pos.and_then(|k| self.diff_rows.get(k)).copied();
                for i in range {
                    let (vi, placeholder) = view[i];
                    let oi = orig_of(vi);
                    if let Some(bi) = placeholder {
                        // 折叠占位行：点击展开
                        let (s, e) = self.diff_blocks[bi];
                        let n = e - s + 1;
                        let (rect, resp) = ui.allocate_exact_size(
                            Vec2::new(total_w, self.row_h()),
                            egui::Sense::click(),
                        );
                        paint_bg(
                            ui,
                            rect,
                            Some(super::theme::fold_bg(ui.visuals().dark_mode)),
                        );
                        let fg = ui.visuals().weak_text_color();
                        ui.painter().text(
                            Pos2::new(rect.left() + 10.0, rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            format!("⏵ {n} 行已折叠（点击展开）"),
                            egui::FontId::proportional(12.0),
                            fg,
                        );
                        if resp.clicked() {
                            fold_toggle = Some(bi);
                        }
                        continue;
                    }
                    let row = &display_rows[vi];
                    let (bg_l, bg_r) = match row.tag {
                        RowTag::Equal => (None, None),
                        RowTag::Delete => (Some(bg_delete()), None),
                        RowTag::Insert => (None, Some(bg_insert())),
                        RowTag::Replace => (Some(bg_replace_l()), Some(bg_replace_r())),
                    };
                    // P43-2：文本选区高亮（蓝色系叠加）
                    let (bg_l, bg_r) = if self.selection.is_some_and(|(s, e)| oi >= s && oi <= e) {
                        let sel = Some(super::theme::selection_overlay());
                        (bg_l.or(sel), bg_r.or(sel))
                    } else {
                        (bg_l, bg_r)
                    };
                    // 搜索命中高亮（按原始行索引映射）
                    let (bg_l, bg_r) = if match_set.contains(&oi) {
                        let c = if current_match == Some(oi) {
                            bg_match_current()
                        } else {
                            bg_match()
                        };
                        (Some(bg_l.unwrap_or(c)), Some(bg_r.unwrap_or(c)))
                    } else {
                        (bg_l, bg_r)
                    };
                    let (hl_l, hl_r) = match row.tag {
                        RowTag::Replace => (Some(hl_replace_l()), Some(hl_replace_r())),
                        RowTag::Delete => (Some(hl_delete()), None),
                        RowTag::Insert => (None, Some(hl_insert())),
                        RowTag::Equal => (None, None),
                    };
                    // P32-B5：忽略行弱化显示（半透明灰）
                    let ignored = self.ignored_rows.contains(&oi);
                    let (bg_l, bg_r) = if ignored {
                        let dim = super::theme::ignored_dim(ui.visuals().dark_mode);
                        (Some(dim), Some(dim))
                    } else {
                        (bg_l, bg_r)
                    };
                    // P32-A5：块首行左侧画折叠箭头 ▾（点击折叠）
                    let block_start = self.diff_blocks.iter().position(|&(s, _)| s == oi);
                    let (hit, resp) = paint_diff_row(
                        ui,
                        row,
                        gutter_l,
                        gutter_r,
                        content_w,
                        bg_l,
                        bg_r,
                        hl_l,
                        hl_r,
                        fg,
                        syn_l,
                        syn_r,
                        cur_diff_orig == Some(oi),
                        inline.as_mut().filter(|ie| ie.row == oi),
                        block_start,
                        ignored,
                        self.h_scroll,
                        self.show_whitespace,
                        self.layout,
                    );
                    match hit {
                        Some(RowHit::Edit(side)) => dbl = Some((oi, side)),
                        Some(RowHit::FoldToggle(bi)) => fold_toggle = Some(bi),
                        None => {}
                    }
                    // P38-1d：已编辑行小圆点标记（右上角）
                    if edited_set.contains(&oi) {
                        let rect = resp.rect;
                        ui.painter().circle_filled(
                            Pos2::new(rect.right() - 8.0, rect.top() + 8.0),
                            3.0,
                            crate::gui::theme::diff_modify(ui.visuals().dark_mode),
                        );
                    }
                    // P38-1b：对齐模式下点击行 → 记录目标行号（另一侧行号）
                    if resp.clicked() && self.align_pick.is_some() {
                        let row = &display_rows[vi];
                        let target_no = match self.align_pick {
                            Some((EditSide::Left, _)) => row.right_no,
                            Some((EditSide::Right, _)) => row.left_no,
                            None => None,
                        };
                        if let Some(no) = target_no {
                            align_target_click = Some(no);
                        }
                    }
                    // P32-A4：行右键菜单（复制路径/打开文件/忽略）——闭包内只收集请求
                    let row_idx = oi;
                    // P35-A1：该行是否属于某个差异块（决定是否显示“复制到另一侧”）
                    let row_in_diff = self
                        .diff_blocks
                        .iter()
                        .any(|&(s, e)| s <= row_idx && row_idx <= e);
                    let (lp, rp) = (
                        self.left.as_ref().map(|f| f.path.clone()),
                        self.right.as_ref().map(|f| f.path.clone()),
                    );
                    resp.context_menu(|ui| {
                        // P38-1e：文件级联动（BC Copy File and Open Next Difference，置顶）
                        if let (Some(_l), Some(_r)) = (&lp, &rp) {
                            if ui.button("⇨ 复制文件到右侧并打开下一个差异").clicked()
                            {
                                copy_file_req = Some(EditSide::Right);
                                ui.close();
                            }
                            if ui.button("⇦ 复制文件到左侧并打开下一个差异").clicked()
                            {
                                copy_file_req = Some(EditSide::Left);
                                ui.close();
                            }
                            ui.separator();
                        }
                        // P38-1a：隔离（BC Isolate）/ 取消隔离（Show All）
                        if self.isolated.is_some() {
                            if ui.button("显示全部").clicked() {
                                unisolate_req = true;
                                ui.close();
                            }
                        } else if row_in_diff && ui.button("隔离").clicked() {
                            isolate_req = true;
                            ui.close();
                        }
                        // P38-1b：对齐方式（BC Align With）——左侧行与右侧行手动强制对齐
                        let lno = display_rows[row_idx].left_no;
                        let rno = display_rows[row_idx].right_no;
                        if let (Some(ln), Some(_)) = (lno, rno) {
                            // 两侧都有内容：可直接与另一侧当前行对齐
                            if ui.button("对齐方式").clicked() {
                                align_req = Some((EditSide::Left, ln));
                                ui.close();
                            }
                        } else if let Some(ln) = lno {
                            // 左侧独有：与右侧某行对齐（点击目标行）
                            if ui.button("对齐方式…").clicked() {
                                align_req = Some((EditSide::Left, ln));
                                ui.close();
                            }
                        } else if let Some(rn) = rno {
                            if ui.button("对齐方式…").clicked() {
                                align_req = Some((EditSide::Right, rn));
                                ui.close();
                            }
                        }
                        if !self.manual_aligns.is_empty() && ui.button("清除对齐").clicked() {
                            clear_align_req = true;
                            ui.close();
                        }
                        // P38-1c：缩进调整（BC Increase/Decrease Indent，整块 ±4 空格）
                        if row_in_diff {
                            ui.separator();
                            if ui.button("↦ 增加缩进").clicked() {
                                indent_req = Some((row_idx, 1));
                                ui.close();
                            }
                            if ui.button("↤ 减少缩进").clicked() {
                                indent_req = Some((row_idx, -1));
                                ui.close();
                            }
                        }
                        // P35-A1：复制差异块到另一侧（最核心操作，置顶）
                        if row_in_diff {
                            if ui.button(t(I18nKey::CopyToRight)).clicked() {
                                copy_req = Some((row_idx, EditSide::Right));
                                ui.close();
                            }
                            if ui.button(t(I18nKey::CopyToLeft)).clicked() {
                                copy_req = Some((row_idx, EditSide::Left));
                                ui.close();
                            }
                            ui.separator();
                        }
                        // P37-1m：行级复制（BC Copy Line to Right/Left）
                        if row_idx < self.rows.len() {
                            if ui.button("复制行到右侧").clicked() {
                                copy_line_req = Some((row_idx, EditSide::Right));
                                ui.close();
                            }
                            if ui.button("复制行到左侧").clicked() {
                                copy_line_req = Some((row_idx, EditSide::Left));
                                ui.close();
                            }
                            ui.separator();
                        }
                        if let Some(p) = &lp {
                            if ui.button(t(I18nKey::CopyLeftPath)).clicked() {
                                ui.ctx().copy_text(p.clone());
                                ui.close();
                            }
                        }
                        if let Some(p) = &rp {
                            if ui.button(t(I18nKey::CopyRightPath)).clicked() {
                                ui.ctx().copy_text(p.clone());
                                ui.close();
                            }
                        }
                        ui.separator();
                        if let Some(p) = &lp {
                            if ui.button(t(I18nKey::OpenLeftFile)).clicked() {
                                super::common::open_with_system_app(p);
                                ui.close();
                            }
                        }
                        if let Some(p) = &rp {
                            if ui.button(t(I18nKey::OpenRightFile)).clicked() {
                                super::common::open_with_system_app(p);
                                ui.close();
                            }
                        }
                        ui.separator();
                        // P37-1j：外部工具对比（~/.bcr-external.toml 扩展名映射）
                        if let (Some(lp2), Some(rp2)) = (&lp, &rp) {
                            if ui
                                .button("⚙ 外部工具对比")
                                .on_hover_text(
                                    "用 ~/.bcr-external.toml 配置的第三方工具对比两侧文件",
                                )
                                .clicked()
                            {
                                external_req = Some((lp2.clone(), rp2.clone()));
                                ui.close();
                            }
                        }
                        ui.separator();
                        if ignored {
                            if ui.button("取消忽略此行").clicked() {
                                ignore_req = Some(row_idx);
                                ui.close();
                            }
                        } else if ui.button("忽略此行").clicked() {
                            ignore_req = Some(row_idx);
                            ui.close();
                        }
                    });
                }
            });
            // P37-1j：右键外部工具对比（闭包外执行）
            if let Some((l, r)) = external_req {
                if let Some(err) = super::common::external_compare(&l, &r) {
                    self.error = Some(err);
                }
            }
            // P32-A5：处理折叠切换
            if let Some(bi) = fold_toggle {
                if !self.collapsed_blocks.remove(&bi) {
                    self.collapsed_blocks.insert(bi);
                }
                self.inline_edit = inline;
                return;
            }
            // P32-B5：右键忽略/取消忽略该行（会话级）
            if let Some(row) = ignore_req {
                if !self.ignored_rows.remove(&row) {
                    self.ignored_rows.insert(row);
                }
                self.inline_edit = inline;
                self.recompute();
                return;
            }
            // P38-1e：文件级联动（闭包外执行）
            if let Some(target) = copy_file_req {
                self.copy_file_to(target);
                self.inline_edit = inline;
                return;
            }
            // P38-1a：右键隔离/取消隔离（请求处理，处理后返回避免借用冲突）
            if isolate_req {
                self.isolate_current();
                self.inline_edit = inline;
                return;
            }
            if unisolate_req || banner_unisolate {
                self.unisolate();
                self.inline_edit = inline;
                return;
            }
            // P38-1b：对齐方式（请求处理，处理后返回避免借用冲突）
            if let Some((side, no)) = align_req {
                self.start_align(side, no);
                self.inline_edit = inline;
                return;
            }
            if clear_align_req {
                self.clear_aligns();
                self.inline_edit = inline;
                return;
            }
            if align_cancel {
                self.align_pick = None;
            }
            if let Some(target_no) = align_target_click {
                self.finish_align(target_no);
                self.inline_edit = inline;
                return;
            }
            // P38-1c：缩进调整（闭包外执行）
            if let Some((row, delta)) = indent_req {
                self.indent_block(row, delta);
                self.inline_edit = inline;
                return;
            }
            // P35-A1：右键复制差异块到另一侧（改变文件内容，清空行内编辑）
            if let Some((row, side)) = copy_req {
                self.copy_block_at(row, side);
                self.inline_edit = None;
                return;
            }
            // P37-1m：右键复制行到另一侧（行级替换）
            if let Some((row, side)) = copy_line_req {
                self.copy_line_at(row, side);
                self.inline_edit = None;
                return;
            }
            // P32-A2：双击行内容 → 进入行内编辑（buf 取该侧当前行文本）
            if let Some((i, side)) = dbl {
                let text = match side {
                    EditSide::Left => display_rows[i].left.as_ref().map(|c| c.text.clone()),
                    EditSide::Right => display_rows[i].right.as_ref().map(|c| c.text.clone()),
                };
                if let Some(text) = text {
                    inline = Some(InlineEditState {
                        side,
                        row: i,
                        buf: text,
                    });
                    self.inline_edit = inline;
                }
            } else {
                self.inline_edit = inline;
            }
            self.scroll = out.state.offset;

            // A11 缩略图总览：右侧迷你差异地图（点击跳转到对应行）
            if self.show_overview && !display_rows.is_empty() {
                let panel_rect = ui.max_rect();
                let ov_w = 10.0;
                let ov_rect = Rect::from_min_size(
                    Pos2::new(panel_rect.right() - ov_w - 3.0, panel_rect.top()),
                    vec2(ov_w, panel_rect.height()),
                );
                let n = display_rows.len();
                let row_h = ov_rect.height() / n as f32;
                for (i, row) in display_rows.iter().enumerate() {
                    let color = match row.tag {
                        RowTag::Equal => Color32::TRANSPARENT,
                        RowTag::Delete => super::theme::stat_delete(ui.visuals().dark_mode),
                        RowTag::Insert => super::theme::stat_insert(ui.visuals().dark_mode),
                        RowTag::Replace => super::theme::stat_modify(ui.visuals().dark_mode),
                    };
                    if color != Color32::TRANSPARENT {
                        let y = ov_rect.top() + i as f32 * row_h;
                        ui.painter().rect_filled(
                            Rect::from_min_size(
                                Pos2::new(ov_rect.left(), y),
                                vec2(ov_w, row_h.max(1.0)),
                            ),
                            0.0,
                            color,
                        );
                    }
                }
                let resp = ui.interact(ov_rect, ui.id().with("overview"), egui::Sense::click());
                if resp.clicked() {
                    if let Some(pos) = resp.interact_pointer_pos() {
                        let t = ((pos.y - ov_rect.top()) / ov_rect.height()).clamp(0.0, 1.0);
                        let row_idx = (t * n as f32) as usize;
                        let orig = if self.wrap {
                            orig_idx.get(row_idx).copied().unwrap_or(row_idx)
                        } else {
                            row_idx
                        };
                        self.jump_to_row(orig);
                    }
                }
            }

            // 底部统计栏
            if self.show_stats {
                ui.separator();
                ui.horizontal(|ui| {
                    let st = self.stats;
                    ui.label(format!("{} {}", t(I18nKey::StatSame), st.equal));
                    ui.colored_label(
                        super::theme::stat_delete(ui.visuals().dark_mode),
                        format!("{} {}", t(I18nKey::StatDelete), st.delete),
                    );
                    ui.colored_label(
                        super::theme::stat_insert(ui.visuals().dark_mode),
                        format!("{} {}", t(I18nKey::StatInsert), st.insert),
                    );
                    ui.colored_label(
                        super::theme::stat_modify(ui.visuals().dark_mode),
                        format!("{} {}", t(I18nKey::StatReplace), st.replace),
                    );
                    ui.separator();
                    match (&self.left, &self.right) {
                        (Some(l), Some(r)) => {
                            ui.label(format!("{}  ↔  {}", l.path, r.path));
                        }
                        (Some(l), None) => {
                            ui.label(fmt(I18nKey::NotOpenRight, &[&l.path]));
                        }
                        (None, Some(r)) => {
                            ui.label(fmt(I18nKey::NotOpenLeft, &[&r.path]));
                        }
                        (None, None) => {}
                    }
                });
            }
        });

        // 点击文件信息头 → 打开文件对话框更换该侧文件（BC：点路径换文件）
        // 必须在 CentralPanel 闭包外处理：渲染区持有 &self.rows 借用
        if let Some(side) = header_click {
            match side {
                EditSide::Left => self.open_left_dialog(),
                EditSide::Right => self.open_right_dialog(),
            }
        }
    }
}

/// P32-A2/A5/B5：行交互命中结果
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RowHit {
    /// 双击行内容（进入行内编辑）
    Edit(EditSide),
    /// 点击折叠箭头（切换差异块折叠）
    FoldToggle(usize),
}

#[allow(clippy::too_many_arguments)] // egui 行绘制参数较多，保持扁平可读
fn paint_diff_row(
    ui: &mut egui::Ui,
    row: &SideRow,
    gutter_l: f32,
    gutter_r: f32,
    content_w: f32,
    bg_l: Option<Color32>,
    bg_r: Option<Color32>,
    hl_l: Option<Color32>,
    hl_r: Option<Color32>,
    fg: Color32,
    syn_l: Option<&'static syntect::parsing::SyntaxReference>,
    syn_r: Option<&'static syntect::parsing::SyntaxReference>,
    is_current: bool,
    mut inline: Option<&mut InlineEditState>,
    block_start: Option<usize>,
    ignored: bool,
    // P33：横向滚动偏移（长行栏内左右滑动查看）
    h_scroll: f32,
    // P35-A4：显示空白符
    show_ws: bool,
    // P39-2d：布局（SideBySide 左右并排；TopBottom/Web 上下堆叠）
    layout: DiffLayout,
) -> (Option<RowHit>, egui::Response) {
    // P39-2d：上-下/网页布局 → 垂直堆叠（左内容上半、右内容下半，行高 2*ROW_H）
    if layout != DiffLayout::SideBySide {
        return paint_diff_row_v(
            ui, row, gutter_l, gutter_r, content_w, bg_l, bg_r, hl_l, hl_r, fg, syn_l, syn_r,
            is_current, inline, ignored, h_scroll, show_ws,
        );
    }
    let mid_gap = super::theme::MID_GAP;
    let (rect, resp) = ui.allocate_exact_size(
        Vec2::new(gutter_l + content_w + mid_gap + gutter_r + content_w, ROW_H),
        egui::Sense::click(),
    );
    let x = rect.left();
    let y = rect.top();

    // P32-A5：块首行左侧画折叠箭头 ▾（点击折叠）
    if let Some(bi) = block_start {
        let arrow_x = x + super::theme::CURRENT_BAR + 3.0;
        let arrow_rect = Rect::from_min_size(Pos2::new(arrow_x - 2.0, y), vec2(14.0, ROW_H));
        let arrow_color = if ignored {
            ui.visuals().weak_text_color()
        } else {
            super::theme::diff_modify(ui.visuals().dark_mode)
        };
        ui.painter().text(
            arrow_rect.center(),
            egui::Align2::CENTER_CENTER,
            "▾",
            egui::FontId::proportional(11.0),
            arrow_color,
        );
        if resp.clicked() && arrow_rect.contains(resp.interact_pointer_pos().unwrap_or(Pos2::ZERO))
        {
            return (Some(RowHit::FoldToggle(bi)), resp);
        }
    }

    // BC 风格当前差异行：左侧 3px 竖条（P31；P39-2b 改蓝色系对齐 BC）
    if is_current {
        ui.painter().rect_filled(
            Rect::from_min_size(Pos2::new(x, y), vec2(super::theme::CURRENT_BAR, ROW_H)),
            0.0,
            super::theme::current_bar(ui.visuals().dark_mode),
        );
    }

    // 左 gutter + 内容（P31：gutter 用微灰底色与内容区分，BC 观感）
    // P39-2b：深色下 gutter 底色略亮于内容区，行号更易读
    let gutter_bg = Some(super::theme::gutter_bg(ui.visuals().dark_mode));
    let gutter_rect = Rect::from_min_size(Pos2::new(x, y), vec2(gutter_l, ROW_H));
    paint_bg(ui, gutter_rect, gutter_bg);
    paint_line_no(ui, gutter_rect, row.left_no);
    let content_rect = Rect::from_min_size(Pos2::new(x + gutter_l, y), vec2(content_w, ROW_H));
    paint_bg(ui, content_rect, bg_l);
    // P51-4：行 hover 高亮（半透明弱色叠加，与 DirTab 观感一致）
    if resp.hovered() && !ignored {
        ui.painter()
            .rect_filled(content_rect, 0.0, super::theme::bg_match());
    }
    // P32-A2：行内编辑命中左侧 → 就地 TextEdit
    let editing_side = inline.as_ref().map(|ie| ie.side);
    match editing_side {
        Some(EditSide::Left) => {
            if let Some(ie) = inline.as_mut() {
                ui.add(
                    egui::TextEdit::singleline(&mut ie.buf)
                        .font(egui::TextStyle::Monospace)
                        .desired_width(content_w - 8.0),
                );
            }
        }
        _ => {
            paint_cell(
                ui,
                content_rect,
                row.left.as_ref(),
                fg,
                hl_l,
                syn_l,
                h_scroll,
                show_ws,
            );
        }
    }

    // P32-A1：左右面板空隙画差异连接线（有差异的行画线连接两侧，BC 观感）
    let mid_x = x + gutter_l + content_w;
    let mid_rect = Rect::from_min_size(Pos2::new(mid_x, y), vec2(mid_gap, ROW_H));
    let mid_color = diff_mid_line_color(ui.visuals().dark_mode, row.tag);
    if let Some(c) = mid_color {
        // 空隙底色（比 gutter 略深一档，突出连接线）
        paint_bg(
            ui,
            mid_rect,
            Some(super::theme::mid_bg(ui.visuals().dark_mode)),
        );
        // 水平连接线（行垂直居中，左右各留 2px）
        let cy = y + ROW_H / 2.0;
        ui.painter().line_segment(
            [
                Pos2::new(mid_x + 2.0, cy),
                Pos2::new(mid_x + mid_gap - 2.0, cy),
            ],
            egui::Stroke::new(1.5, c),
        );
    } else {
        // 无差异行：弱色垂直分隔线（延续面板分隔感）
        let sep = super::theme::mid_sep(ui.visuals().dark_mode);
        ui.painter().line_segment(
            [
                Pos2::new(mid_x + mid_gap / 2.0, y),
                Pos2::new(mid_x + mid_gap / 2.0, y + ROW_H),
            ],
            egui::Stroke::new(1.0, sep),
        );
    }

    // 右 gutter + 内容
    let x_r = mid_x + mid_gap;
    let gutter_rect = Rect::from_min_size(Pos2::new(x_r, y), vec2(gutter_r, ROW_H));
    paint_bg(ui, gutter_rect, gutter_bg);
    paint_line_no(ui, gutter_rect, row.right_no);
    let content_rect = Rect::from_min_size(Pos2::new(x_r + gutter_r, y), vec2(content_w, ROW_H));
    paint_bg(ui, content_rect, bg_r);
    // P51-4：行 hover 高亮（右栏同步）
    if resp.hovered() && !ignored {
        ui.painter()
            .rect_filled(content_rect, 0.0, super::theme::bg_match());
    }
    // P32-A2：行内编辑命中右侧 → 就地 TextEdit
    let editing_side = inline.as_ref().map(|ie| ie.side);
    match editing_side {
        Some(EditSide::Right) => {
            if let Some(ie) = inline.as_mut() {
                ui.add(
                    egui::TextEdit::singleline(&mut ie.buf)
                        .font(egui::TextStyle::Monospace)
                        .desired_width(content_w - 8.0),
                );
            }
        }
        _ => {
            paint_cell(
                ui,
                content_rect,
                row.right.as_ref(),
                fg,
                hl_r,
                syn_r,
                h_scroll,
                show_ws,
            );
        }
    }

    // P32-A2：双击行内容 → 进入行内编辑（左/右内容区命中）
    if resp.double_clicked() {
        if let Some(pos) = resp.interact_pointer_pos() {
            let left_zone = Rect::from_min_size(Pos2::new(x + gutter_l, y), vec2(content_w, ROW_H));
            let right_zone =
                Rect::from_min_size(Pos2::new(x_r + gutter_r, y), vec2(content_w, ROW_H));
            if left_zone.contains(pos) {
                return (Some(RowHit::Edit(EditSide::Left)), resp);
            }
            if right_zone.contains(pos) {
                return (Some(RowHit::Edit(EditSide::Right)), resp);
            }
        }
    }
    (None, resp)
}

/// P39-2d：上-下 / 网页布局的行绘制（左内容上半、右内容下半，行高 2*ROW_H）
#[allow(clippy::too_many_arguments)]
fn paint_diff_row_v(
    ui: &mut egui::Ui,
    row: &SideRow,
    gutter_l: f32,
    gutter_r: f32,
    content_w: f32,
    bg_l: Option<Color32>,
    bg_r: Option<Color32>,
    hl_l: Option<Color32>,
    hl_r: Option<Color32>,
    fg: Color32,
    syn_l: Option<&'static syntect::parsing::SyntaxReference>,
    syn_r: Option<&'static syntect::parsing::SyntaxReference>,
    is_current: bool,
    mut inline: Option<&mut InlineEditState>,
    ignored: bool,
    h_scroll: f32,
    show_ws: bool,
) -> (Option<RowHit>, egui::Response) {
    let row_h = ROW_H * 2.0;
    let (rect, resp) = ui.allocate_exact_size(
        Vec2::new(gutter_l.max(gutter_r) + content_w, row_h),
        egui::Sense::click(),
    );
    let x = rect.left();
    let y = rect.top();
    // 忽略行弱化色
    let dim = super::theme::ignored_dim(ui.visuals().dark_mode);
    let gutter_bg = super::theme::gutter_bg(ui.visuals().dark_mode);
    // BC 风格当前差异行：左侧 3px 竖条（整行高）
    if is_current {
        ui.painter().rect_filled(
            Rect::from_min_size(Pos2::new(x, y), vec2(super::theme::CURRENT_BAR, row_h)),
            0.0,
            super::theme::current_bar(ui.visuals().dark_mode),
        );
    }
    // ---- 上半：左 gutter + 左内容 ----
    let l_bg = if ignored { Some(dim) } else { bg_l };
    {
        let gutter_rect = Rect::from_min_size(Pos2::new(x, y), vec2(gutter_l, ROW_H));
        paint_bg(ui, gutter_rect, Some(gutter_bg));
        paint_line_no(ui, gutter_rect, row.left_no);
        let content_rect = Rect::from_min_size(Pos2::new(x + gutter_l, y), vec2(content_w, ROW_H));
        paint_bg(ui, content_rect, l_bg);
        // P51-4：行 hover 高亮（半透明弱色叠加，与 SideBySide 布局一致）
        if resp.hovered() && !ignored {
            ui.painter()
                .rect_filled(content_rect, 0.0, super::theme::bg_match());
        }
        let editing_side = inline.as_ref().map(|ie| ie.side);
        match editing_side {
            Some(EditSide::Left) => {
                if let Some(ie) = inline.as_mut() {
                    ui.add(
                        egui::TextEdit::singleline(&mut ie.buf)
                            .font(egui::TextStyle::Monospace)
                            .desired_width(content_w - 8.0),
                    );
                }
            }
            _ => {
                paint_cell(
                    ui,
                    content_rect,
                    row.left.as_ref(),
                    fg,
                    hl_l,
                    syn_l,
                    h_scroll,
                    show_ws,
                );
            }
        }
    }
    // ---- 下半：右 gutter + 右内容 ----
    let r_bg = if ignored { Some(dim) } else { bg_r };
    {
        let y2 = y + ROW_H;
        let gutter_rect = Rect::from_min_size(Pos2::new(x, y2), vec2(gutter_r, ROW_H));
        paint_bg(ui, gutter_rect, Some(gutter_bg));
        paint_line_no(ui, gutter_rect, row.right_no);
        let content_rect = Rect::from_min_size(Pos2::new(x + gutter_r, y2), vec2(content_w, ROW_H));
        paint_bg(ui, content_rect, r_bg);
        // P51-4：行 hover 高亮（右栏同步）
        if resp.hovered() && !ignored {
            ui.painter()
                .rect_filled(content_rect, 0.0, super::theme::bg_match());
        }
        let editing_side = inline.as_ref().map(|ie| ie.side);
        match editing_side {
            Some(EditSide::Right) => {
                if let Some(ie) = inline.as_mut() {
                    ui.add(
                        egui::TextEdit::singleline(&mut ie.buf)
                            .font(egui::TextStyle::Monospace)
                            .desired_width(content_w - 8.0),
                    );
                }
            }
            _ => {
                paint_cell(
                    ui,
                    content_rect,
                    row.right.as_ref(),
                    fg,
                    hl_r,
                    syn_r,
                    h_scroll,
                    show_ws,
                );
            }
        }
    }
    // 双击 → 编辑（上半=左，下半=右）
    if resp.double_clicked() {
        if let Some(pos) = resp.interact_pointer_pos() {
            if pos.y < y + ROW_H {
                return (Some(RowHit::Edit(EditSide::Left)), resp);
            }
            return (Some(RowHit::Edit(EditSide::Right)), resp);
        }
    }
    (None, resp)
}

/// P32-A1：差异连接线颜色（有差异的行返回对应颜色，无差异返回 None）
pub(crate) fn diff_mid_line_color(dark: bool, tag: RowTag) -> Option<Color32> {
    match tag {
        RowTag::Equal => None,
        RowTag::Delete => Some(super::theme::diff_delete(dark)),
        RowTag::Insert => Some(super::theme::diff_insert(dark)),
        RowTag::Replace => Some(super::theme::diff_modify(dark)),
    }
}

fn basename(p: &str) -> String {
    std::path::Path::new(p)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.to_string())
}

/// P50：全路径截断显示（BC 风格）——超宽时保留「目录…文件名」，
/// 避免长路径溢出头部区域；完整路径在 hover 提示中可见。
fn truncate_path(p: &str, max_chars: usize) -> String {
    let count = p.chars().count();
    if count <= max_chars || max_chars < 16 {
        return p.to_string();
    }
    let file = basename(p);
    let file_len = file.chars().count();
    // 保留尾部文件名 + 前段目录（… 占 1 字符）
    let head = max_chars.saturating_sub(file_len + 3).max(4);
    let mut out: String = p.chars().take(head).collect();
    out.push('…');
    out.push_str(&file);
    out
}

/// P33：文件大小（详情行显示，BC 式 "12,345 bytes"）
fn file_size(p: &str) -> u64 {
    std::fs::metadata(p).map(|m| m.len()).unwrap_or(0)
}

/// P33：修改时间可读串（详情行显示，BC 式 "2026-08-13 08:09:20"）
fn file_mtime_str(p: &str) -> String {
    let meta = match std::fs::metadata(p) {
        Ok(m) => m,
        Err(_) => return String::new(),
    };
    let t = match meta.modified() {
        Ok(t) => t,
        Err(_) => return String::new(),
    };
    crate::report::fmt_mtime_pub(t)
}

/// P33：文件详情行（BC 式 "时间 | 大小 bytes | 编码"）
fn file_detail_line(f: &LoadedFile) -> String {
    let mut parts: Vec<String> = Vec::new();
    if !f.mtime.is_empty() {
        parts.push(f.mtime.clone());
    }
    if f.size > 0 {
        parts.push(format!("{} bytes", f.size));
    }
    // 编码 + BOM 标记
    let mut enc = f.encoding.name().to_string();
    if f.had_bom {
        enc.push_str(" BOM");
    }
    parts.push(enc);
    // 语法名（如 Python/JavaScript），BC 的 "Delphi Source" 语义
    if let Some(syn) = f.syntax {
        if !syn.name.is_empty() {
            parts.push(syn.name.clone());
        }
    }
    parts.join("  |  ")
}

/// A4：替换 content 中指定行（1-based）里的第一个匹配。返回是否替换。
fn replace_line(content: &mut String, line_no: usize, from: &str, to: &str) -> bool {
    if from.is_empty() {
        return false;
    }
    let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
    if line_no < 1 || line_no > lines.len() {
        return false;
    }
    let idx = line_no - 1;
    if !lines[idx].contains(from) {
        return false;
    }
    lines[idx] = lines[idx].replacen(from, to, 1);
    let had_trailing_nl = content.ends_with('\n');
    *content = lines.join("\n");
    if had_trailing_nl {
        content.push('\n');
    }
    true
}

// ---- 二进制 hex 视图 ----

/// hex 行高（与 ROW_H 一致）
const HEX_ROW_H: f32 = ROW_H;
/// hex 视图总宽度：偏移(9) + L hex(50) + L ascii(18) + R hex(50) + R ascii(18) + 间距
const HEX_TOTAL_W: f32 = 9.0 + 50.0 + 18.0 + 50.0 + 18.0 + 80.0;
/// 各列 x 起点
const HEX_OFF_X: f32 = 4.0;
const HEX_L_X: f32 = 16.0;
const HEX_L_ASCII_X: f32 = 66.0;
const HEX_R_X: f32 = 86.0;
const HEX_R_ASCII_X: f32 = 136.0;

/// 绘制一行 hex 对比（偏移 + L hex + L ascii + R hex + R ascii）
///
/// P37-1d：偏移列支持 hex/dec 与隐藏；字节值支持逐字节/小尾/大端。
fn paint_hex_row(
    ui: &mut egui::Ui,
    row: &crate::hexview::HexRow,
    fg: Color32,
    addr_hex: bool,
    value_mode: crate::hexview::HexValueMode,
    show_addr: bool,
) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(HEX_TOTAL_W, HEX_ROW_H), egui::Sense::hover());
    let x = rect.left();
    let y = rect.top();

    // 差异行底色
    if row.diff {
        paint_bg(ui, rect, Some(bg_replace_l()));
    }

    // 偏移（P37-1d：hex/dec 可切换、可隐藏）
    if show_addr {
        ui.painter().text(
            Pos2::new(x + HEX_OFF_X, y + 2.0),
            egui::Align2::LEFT_TOP,
            crate::hexview::format_offset(row.offset, addr_hex),
            egui::FontId::monospace(13.0),
            GUTTER,
        );
    }

    // 左侧 hex（P37-1d：按值模式渲染；Raw 保留差异字节红/绿底色逻辑）
    let l_hex = if value_mode == crate::hexview::HexValueMode::Raw {
        hex_bytes_text(&row.left, &row.right, true)
    } else {
        crate::hexview::hex_values_text(&row.left, value_mode)
    };
    ui.painter().text(
        Pos2::new(x + HEX_L_X, y + 2.0),
        egui::Align2::LEFT_TOP,
        l_hex,
        egui::FontId::monospace(13.0),
        fg,
    );
    // 左侧 ascii
    let l_ascii: String = row
        .left
        .iter()
        .map(|&b| crate::hexview::ascii_byte(b))
        .collect();
    ui.painter().text(
        Pos2::new(x + HEX_L_ASCII_X, y + 2.0),
        egui::Align2::LEFT_TOP,
        l_ascii,
        egui::FontId::monospace(13.0),
        fg,
    );

    // 右侧 hex（P37-1d：按值模式渲染）
    let r_hex = if value_mode == crate::hexview::HexValueMode::Raw {
        hex_bytes_text(&row.right, &row.left, false)
    } else {
        crate::hexview::hex_values_text(&row.right, value_mode)
    };
    ui.painter().text(
        Pos2::new(x + HEX_R_X, y + 2.0),
        egui::Align2::LEFT_TOP,
        r_hex,
        egui::FontId::monospace(13.0),
        fg,
    );
    // 右侧 ascii
    let r_ascii: String = row
        .right
        .iter()
        .map(|&b| crate::hexview::ascii_byte(b))
        .collect();
    ui.painter().text(
        Pos2::new(x + HEX_R_ASCII_X, y + 2.0),
        egui::Align2::LEFT_TOP,
        r_ascii,
        egui::FontId::monospace(13.0),
        fg,
    );
}

/// hex 字节文本：16 字节宽、8 字节处空格；与本侧不同的字节用颜色标记
fn hex_bytes_text(bytes: &[u8], _other: &[u8], _is_left: bool) -> String {
    let mut s = String::new();
    for i in 0..16 {
        if i == 8 {
            s.push(' ');
        }
        if i < bytes.len() {
            // 差异字节由底色表达（egui 文本内嵌颜色不生效，返回纯文本）
            s.push_str(&format!("{:02X} ", bytes[i]));
        } else {
            s.push_str("   ");
        }
    }
    s.trim_end().to_string()
}

/// 检测文件是否为二进制（读取前 8KB 做启发式判定）
fn is_binary_file(path: &str) -> bool {
    crate::encoding::read_text(path)
        .map(|tf| tf.is_binary)
        .unwrap_or(false)
}

/// A3 剪贴板对比：读取系统剪贴板文本（arboard，跨平台）
fn read_clipboard_text() -> Option<String> {
    let mut cb = arboard::Clipboard::new().ok()?;
    cb.get_text().ok()
}

/// A3 剪贴板对比：把剪贴板文本写入临时文件（供 load_left/load_right 加载）
fn write_clipboard_temp(text: &str) -> Option<String> {
    let dir = std::env::temp_dir();
    let name = format!("bcr-clipboard-{}.txt", std::process::id());
    let path = dir.join(name);
    std::fs::write(&path, text).ok()?;
    Some(path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod dirty_title_tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn diff_title_shows_star_when_dirty() {
        // P56-1：dirty=true → 标题前缀 `*`，load 后清除
        let d = tempdir().unwrap();
        let l = d.path().join("l.txt");
        let r = d.path().join("r.txt");
        fs::write(&l, "a\n").unwrap();
        fs::write(&r, "a\n").unwrap();
        let mut t = DiffTab::new();
        t.load_pair(
            l.to_str().unwrap(),
            r.to_str().unwrap(),
            ViewOptions::default(),
        );
        assert!(!t.title().starts_with('*'), "加载后不应显示 *");
        t.dirty = true;
        assert!(t.title().starts_with('*'), "dirty 后应显示 *");
        t.load_pair(
            l.to_str().unwrap(),
            r.to_str().unwrap(),
            ViewOptions::default(),
        );
        assert!(!t.title().starts_with('*'), "重新加载后应清除 *");
    }
}
