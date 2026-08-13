//! 国际化（i18n）：10 种语言支持。
//!
//! 支持语言：中文(zh)、英语(en)、德语(de)、日语(ja)、韩语(ko)、
//! 西班牙语(es)、葡萄牙语(pt)、阿拉伯语(ar)、俄语(ru)、法语(fr)。
//!
//! 用法：
//! - CLI：`bcr --lang de ...` 或环境变量 `BCR_LANG=de`
//! - GUI：工具栏语言下拉（持久化到 `~/.bcr-gui.toml`）
//! - 代码内：`crate::i18n::t(Key::NotDir)` 返回当前语言的静态字符串

use std::sync::atomic::{AtomicU8, Ordering};

/// 支持的语言
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Zh,
    En,
    De,
    Ja,
    Ko,
    Es,
    Pt,
    Ar,
    Ru,
    Fr,
}

impl Lang {
    /// 全部语言（GUI 下拉顺序）
    pub const ALL: [Lang; 10] = [
        Lang::Zh,
        Lang::En,
        Lang::De,
        Lang::Ja,
        Lang::Ko,
        Lang::Es,
        Lang::Pt,
        Lang::Ar,
        Lang::Ru,
        Lang::Fr,
    ];

    /// 解析语言代码：支持 "zh"、"zh-CN"、"en" 等；前缀匹配
    pub fn parse(s: &str) -> Option<Lang> {
        let code = s
            .split(['-', '_'])
            .next()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        match code.as_str() {
            "zh" | "cn" => Some(Lang::Zh),
            "en" => Some(Lang::En),
            "de" => Some(Lang::De),
            "ja" | "jp" => Some(Lang::Ja),
            "ko" | "kr" => Some(Lang::Ko),
            "es" => Some(Lang::Es),
            "pt" => Some(Lang::Pt),
            "ar" => Some(Lang::Ar),
            "ru" => Some(Lang::Ru),
            "fr" => Some(Lang::Fr),
            _ => None,
        }
    }

    /// ISO 639-1 代码
    pub fn code(self) -> &'static str {
        match self {
            Lang::Zh => "zh",
            Lang::En => "en",
            Lang::De => "de",
            Lang::Ja => "ja",
            Lang::Ko => "ko",
            Lang::Es => "es",
            Lang::Pt => "pt",
            Lang::Ar => "ar",
            Lang::Ru => "ru",
            Lang::Fr => "fr",
        }
    }

    /// 语言本地名称（GUI 下拉显示）
    pub fn native_name(self) -> &'static str {
        match self {
            Lang::Zh => "中文",
            Lang::En => "English",
            Lang::De => "Deutsch",
            Lang::Ja => "日本語",
            Lang::Ko => "한국어",
            Lang::Es => "Español",
            Lang::Pt => "Português",
            Lang::Ar => "العربية",
            Lang::Ru => "Русский",
            Lang::Fr => "Français",
        }
    }

    /// 尝试从环境变量 BCR_LANG 推断。
    ///
    /// 只认应用自己的 BCR_LANG，不读系统 LANG/LC_ALL：
    /// 默认语言固定为中文（验收标准），避免 CI/服务器 locale 为
    /// en_US 等时程序被意外切成英文。用户想换语言请显式用
    /// `--lang` 或 `BCR_LANG`。
    pub fn from_env() -> Option<Lang> {
        if let Ok(v) = std::env::var("BCR_LANG") {
            return Lang::parse(&v);
        }
        None
    }
}

/// 翻译键（覆盖 CLI 输出与 GUI 文案）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    // ---- CLI 通用 ----
    /// 无法读取 {}: {}
    CannotRead,
    /// 不是目录: {}
    NotDir,
    /// 过滤规则错误: {}
    FilterError,
    /// 打开 {} 失败: {}
    OpenFailed,
    /// 扫描失败: {}
    ScanFailed,
    /// 写入 {} 失败: {}
    WriteFailed,
    /// 复制 {} 失败: {}
    CopyFailed,
    /// 删除 {} 失败: {}
    DeleteFailed,
    /// 读取 {} 失败: {}（保留：部分路径仍可能使用）
    #[allow(dead_code)]
    ReadFailed,
    /// 二进制文件: {}（不支持文本处理）
    BinaryFile,
    /// 文件过大: {}（超过 {}MB 上限）
    FileTooLarge,
    /// 统计: {} 相同, {} 仅左侧, {} 仅右侧, {} 内容不同
    SummaryCompare,
    /// 移动/重命名: {} 对
    SummaryMoved,
    /// 报告生成时间（HTML 页脚）
    ReportGeneratedAt,
    /// 会话已存在: {}
    SessionExists,
    /// 会话已保存: {}
    SessionSaved,
    /// 会话写入失败: {}
    SessionWriteFailed,
    /// 暂无会话（用 bcr session save <name> <left> <right> 保存）
    SessionEmpty,
    /// 会话不存在: {}
    SessionNotFound,
    /// 会话已删除: {}
    SessionDeleted,
    /// Profile 已存在: {}（P10）
    ProfileExists,
    /// Profile 已保存: {}（P10）
    ProfileSaved,
    /// Profile 写入失败: {}（P10）
    ProfileWriteFailed,
    /// 暂无 Profile（用 bcr profile save <name> 保存）（P10）
    ProfileEmpty,
    /// Profile 不存在: {}（P10）
    ProfileNotFound,
    /// Profile 已删除: {}（P10）
    ProfileDeleted,
    /// [MOVE] {} -> {}（同步重命名标签）
    TagRename,
    /// 重命名 {} -> {} 失败: {}
    RenameFailed,
    /// [RMDIR] {}（删除空目录标签）
    TagRmDir,
    /// 删除目录 {} 失败: {}
    RmDirFailed,
    /// 清理空目录: {} 个
    SummaryRmDir,
    /// 三路统计: {} 相同, {} 仅BASE, {} 仅LEFT, {} 仅RIGHT, {} 删除, {} 修改, {} 冲突
    SummaryCompare3,
    /// 表头差异提示
    CsvHeaderDiff,
    /// 两个 CSV 完全相同
    CsvIdentical,
    /// 非法分隔符: {}
    CsvBadDelimiter,
    /// CSV 统计: {} 相同行, {} 仅左侧, {} 仅右侧, {} 修改
    SummaryCsv,
    // ---- CsvTab（P29 表格 GUI）----
    /// CSV: {} ↔ {}
    CsvTitle,
    /// 主键
    CsvKeyCol,
    /// 分隔符
    CsvDelimiter,
    /// 行号对齐
    CsvRowAlign,
    /// 全部
    CsvFilterAll,
    /// 仅差异
    CsvFilterDiff,
    /// 仅左侧
    CsvFilterLeft,
    /// 仅右侧
    CsvFilterRight,
    /// 仅修改
    CsvFilterModified,
    /// 仅相同
    CsvFilterSame,
    /// 统计: {} 复制, {} 删除, {} 跳过, {} 冲突, {} 错误
    SummarySync,
    /// [COPY]   {} -> {}
    TagCopy,
    /// [DELETE] {}
    TagDelete,
    /// [SKIP]   {} ({})
    TagSkip,
    /// [CONFLICT] {} (两侧同时修改且无法判定新者，跳过)
    TagConflict,
    /// 目标侧较新
    ReasonDstNewer,
    /// 仅存在于目标侧
    ReasonDstOnly,

    // ---- GUI 通用 ----
    /// 关闭
    Close,
    /// 错误
    Error,
    /// 提示
    Hint,
    /// 保存
    Save,
    /// Ctrl+S 保存
    SaveHint,
    /// 取消
    Cancel,
    /// 重新加载
    Reload,
    /// 左侧（编辑侧名）
    SideLeft,
    /// 右侧（编辑侧名）
    SideRight,
    /// 已保存 {}
    Saved,
    /// 保存失败: {}
    SaveFailed,

    // ---- DiffTab ----
    /// Diff: {} ↔ {}（标签标题）
    DiffTitle,
    /// Hex: {} ↔ {}（二进制对比标签标题）
    HexTitle,
    /// 二进制文件，已切换十六进制对比视图
    HexModeHint,
    /// 打开左侧…
    OpenLeft,
    /// 打开右侧…
    OpenRight,
    /// 统计栏
    StatsPanel,
    /// 忽略空白
    IgnoreWs,
    /// 忽略行尾空白
    IgnoreTrailing,
    /// 忽略大小写
    IgnoreCase,
    /// 自动换行（A8）
    WordWrap,
    /// ✏️ 编辑左侧
    EditLeft,
    /// ✏️ 编辑右侧
    EditRight,
    /// 搜索 (Ctrl+F, Enter 下一个, Esc 清除)
    SearchHint,
    /// 上一个匹配
    PrevMatch,
    /// 下一个匹配
    NextMatch,
    /// 行号 (Ctrl+G)
    GotoHint,
    /// 跳转
    Goto,
    /// 差异 {} / {}
    DiffCount,
    /// F7 下一个差异
    NextDiff,
    /// Shift+F7 上一个差异
    PrevDiff,
    /// 打开两个文件开始并排对比\n...
    DiffEmptyHint,
    /// 相同 {}
    StatSame,
    /// 删除 {}
    StatDelete,
    /// 插入 {}
    StatInsert,
    /// 修改 {}
    StatReplace,
    /// {}  ↔  (未打开右侧)
    NotOpenRight,
    /// (未打开左侧)  ↔  {}
    NotOpenLeft,

    // ---- DirTab ----
    /// 目录: {} ↔ {}
    DirTitle,
    /// 刷新
    Refresh,
    /// 内容比对(哈希)
    ContentHash,
    /// 仅显示差异
    OnlyDiff,
    /// 显示相同
    ShowSame,
    /// 包含 glob（逗号分隔）
    IncludeGlob,
    /// 排除 glob（逗号分隔）
    ExcludeGlob,
    /// 应用过滤
    ApplyFilter,
    /// 相同 {} / 仅左 {} / 仅右 {} / 不同 {}
    DirStats,
    /// 无差异文件（或目录为空）\n↑↓ 选择 · ← → 折叠/展开 · Enter 打开
    NoDiff,
    /// 选择左右两个目录开始对比（或拖入目录）
    DirEmpty,

    // ---- MergeTab ----
    /// 合并: {} ↔ {} ↔ {}
    MergeTitle,
    /// 保存合并结果…
    SaveMerged,
    /// 实时预览
    LivePreview,
    /// 冲突 {} 处
    ConflictsCount,
    /// F7 下一冲突
    NextConflict,
    /// Shift+F7 上一冲突
    PrevConflict,
    /// 取左侧
    TakeLeft,
    /// 取右侧
    TakeRight,
    /// 取 BASE
    TakeBase,
    /// 未解决（默认取左）
    ResAuto,
    /// 已解决
    Resolved,
    /// 合并结果预览
    MergePreview,
    /// {} 行
    MergeLines,
    /// ⚠ {} 处冲突未解决（输出含冲突标记）
    MergeUnresolved,
    /// ✓ 全部冲突已解决
    MergeAllResolved,
    /// bcr gui --merge BASE LEFT RIGHT\n或打开三路合并
    MergeEmpty,
    /// 已保存 {}（未解决冲突 {} 处，输出 git 风格冲突标记）
    MergeSaved,

    // ---- 主窗口 ----
    /// 📁 打开文件对比…
    MenuOpenFiles,
    /// 📂 目录对比…
    MenuOpenDir,
    /// 🔀 三路合并…
    MenuOpenMerge,
    /// 🐙 Git
    MenuGit,
    /// 关闭标签页
    CloseTab,
    /// 新建并排 Diff 标签
    NewDiffTab,
    /// 主题:
    Theme,
    /// 系统
    ThemeSystem,
    /// 深色
    ThemeDark,
    /// 浅色
    ThemeLight,
    /// 语言:
    Language,

    // ---- P33：标准菜单栏（对标 BC Session/File/Edit/Search/View/Tools/Help）----
    /// 会话
    MenuSession,
    /// 文件
    MenuFile,
    /// 编辑
    MenuEdit,
    /// 搜索
    MenuSearch,
    /// 视图
    MenuView,
    /// 工具
    MenuTools,
    /// 帮助
    MenuHelp,
    /// 新建文本对比
    MenuNewText,
    /// 新建文件夹对比
    MenuNewDir,
    /// 新建三路合并
    MenuNewMerge,
    /// 新建图片对比
    MenuNewImage,
    /// 新建 CSV 表格
    MenuNewCsv,
    /// 新建 Hex 对比
    MenuNewHex,
    /// 保存会话
    MenuSaveSession,
    /// 打开左侧
    MenuOpenLeft,
    /// 打开右侧
    MenuOpenRight,
    /// 打开云盘
    MenuOpenCloud,
    /// 剪贴板→左
    MenuClipLeft,
    /// 剪贴板→右
    MenuClipRight,
    /// 撤销
    MenuUndo,
    /// 重做
    MenuRedo,
    /// 查找
    MenuFind,
    /// 下一差异
    MenuNextDiff,
    /// 上一差异
    MenuPrevDiff,
    /// 重新加载
    MenuReload,
    /// 统计栏
    MenuStats,
    /// 缩略图
    MenuThumb,
    /// 外部工具
    MenuExternal,
    /// 关于
    MenuAbout,
    /// 快捷键
    MenuShortcuts,

    // ---- P32-A7：会话类型起始页 ----
    /// 文本对比
    SessionText,
    /// 并排文本差异
    SessionTextDesc,
    /// 文件夹对比
    SessionDir,
    /// 目录与文件差异
    SessionDirDesc,
    /// 三路合并
    SessionMerge,
    /// BASE/LEFT/RIGHT 冲突解决
    SessionMergeDesc,
    /// 图片对比
    SessionImage,
    /// 像素级差异叠加
    SessionImageDesc,
    /// CSV 表格
    SessionCsv,
    /// 行级表格对比
    SessionCsvDesc,
    /// Git 集成
    GitTitle,
    /// 把 bcr 作为 git difftool / mergetool（写入 ~/.gitconfig）：
    GitDesc,
    /// 📋 复制配置
    GitCopy,
    /// 使用：
    GitUsage,
    /// 退出码与 git 兼容（0=无差异/无冲突，1=有差异/冲突，2=错误）
    GitExit,
    /// bcr GUI — 并排 Diff / 目录对比 / 三路合并\n\n打开文件对比，或将文件/目录拖入窗口
    MainHint,
    /// bcr — 对比工具
    WinTitle,
    /// bcr: GUI 启动失败: {}
    GuiFail,
}

/// 当前语言（全局，Atomic 免锁）
static CURRENT: AtomicU8 = AtomicU8::new(Lang::Zh as u8);

/// 设置全局语言
pub fn set_lang(lang: Lang) {
    CURRENT.store(lang as u8, Ordering::Relaxed);
}

/// 读取全局语言
pub fn current() -> Lang {
    let v = CURRENT.load(Ordering::Relaxed);
    Lang::ALL.get(v as usize).copied().unwrap_or(Lang::Zh)
}

/// 当前语言下的翻译
pub fn t(key: Key) -> &'static str {
    translate(current(), key)
}

/// 翻译并填入参数（模板用 {} 占位，按序替换）
///
/// 例：`fmt(Key::CannotRead, &[&left, &err])`
pub fn fmt(key: Key, args: &[&str]) -> String {
    let mut s = t(key).to_string();
    for a in args {
        s = s.replacen("{}", a, 1);
    }
    s
}

include!("i18n_tables.rs");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lang_parse_all_codes() {
        assert_eq!(Lang::parse("zh"), Some(Lang::Zh));
        assert_eq!(Lang::parse("zh-CN"), Some(Lang::Zh));
        assert_eq!(Lang::parse("en"), Some(Lang::En));
        assert_eq!(Lang::parse("de"), Some(Lang::De));
        assert_eq!(Lang::parse("ja"), Some(Lang::Ja));
        assert_eq!(Lang::parse("ko"), Some(Lang::Ko));
        assert_eq!(Lang::parse("es"), Some(Lang::Es));
        assert_eq!(Lang::parse("pt"), Some(Lang::Pt));
        assert_eq!(Lang::parse("ar"), Some(Lang::Ar));
        assert_eq!(Lang::parse("ru"), Some(Lang::Ru));
        assert_eq!(Lang::parse("fr"), Some(Lang::Fr));
        assert_eq!(Lang::parse("xx"), None);
        assert_eq!(Lang::parse(""), None);
        // 大小写不敏感
        assert_eq!(Lang::parse("EN"), Some(Lang::En));
        assert_eq!(Lang::parse("Ja_JP"), Some(Lang::Ja));
    }

    #[test]
    fn lang_code_roundtrip() {
        for l in Lang::ALL {
            assert_eq!(Lang::parse(l.code()), Some(l));
        }
    }

    #[test]
    fn native_names_nonempty() {
        for l in Lang::ALL {
            assert!(!l.native_name().is_empty());
        }
    }

    #[test]
    fn every_key_has_all_languages() {
        // 宏生成穷尽 match，编译期已保证完整性；这里做运行时抽查
        let keys = [
            Key::CannotRead,
            Key::SummaryCompare,
            Key::TagConflict,
            Key::OpenLeft,
            Key::DiffEmptyHint,
            Key::MergeSaved,
            Key::MainHint,
        ];
        for lang in Lang::ALL {
            for k in keys {
                let s = translate(lang, k);
                assert!(!s.is_empty(), "{:?} 缺少 {:?} 翻译", lang, k);
            }
        }
    }

    #[test]
    fn translations_differ_between_languages() {
        // 抽查：OpenLeft 在至少两个语言中不同
        let zh = translate(Lang::Zh, Key::OpenLeft);
        let en = translate(Lang::En, Key::OpenLeft);
        assert_ne!(zh, en);
        // 模板占位符保留
        let zh_fmt = translate(Lang::Zh, Key::CannotRead);
        assert!(zh_fmt.contains("{}"));
    }

    #[test]
    fn fmt_substitutes_placeholders() {
        set_lang(Lang::Zh);
        assert_eq!(
            fmt(Key::CannotRead, &["a.txt", "not found"]),
            "无法读取 a.txt: not found"
        );
        set_lang(Lang::En);
        assert_eq!(
            fmt(Key::CannotRead, &["a.txt", "not found"]),
            "cannot read a.txt: not found"
        );
        // 多占位符
        assert_eq!(
            fmt(Key::SummaryCompare, &["1", "2", "3", "4"]),
            "summary: 1 same, 2 left-only, 3 right-only, 4 differ"
        );
        // 恢复默认
        set_lang(Lang::Zh);
    }

    #[test]
    fn current_defaults_to_zh() {
        set_lang(Lang::Zh);
        assert_eq!(current(), Lang::Zh);
        assert_eq!(t(Key::Close), "关闭");
    }
}
