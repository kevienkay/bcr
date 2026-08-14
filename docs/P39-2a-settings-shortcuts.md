# P39-2a：设置对话框 + 新建标签/窗口 + 快捷键系统化

> 依据 `docs/P39-UI-study.md` 差距清单（功能缺口 P0/P1 + 快捷键速查表），
> 对标 BC 5.2.5：设置集中管理、新建标签页/窗口、系统化快捷键。

## 设置对话框（`⌘,`）

- View 菜单「设置…」或 `⌘,` 打开，集中管理：
  - **忽略选项**：空白 / 行尾空白 / 大小写 / 行尾 CR/LF（新增 `Settings.ignore_crlf`，接线到 DiffTab ViewOptions）
  - **编码**：auto / utf-8 / utf-16le / utf-16be / utf-32le / utf-32be / gbk / big5 / shift_jis
  - **大小上限（MB）**：0 = 默认
- 保存时 `apply_settings_env()`：写入 `BCR_ENCODING` / `BCR_MAX_SIZE` 环境变量（对标 CLI `--encoding` / `--max-size` 行为），并持久化到 `~/.bcr-gui.toml`；启动时同样应用

## 新建标签页 / 新建窗口

- `⌘T` / Session 菜单「新建标签页」：`new_tab_like_current()` 按当前会话类型开新空标签（Diff/Dir/Merge/Image/Csv/TextEdit/Patch/FolderMerge 全类型）
- `⌘N` / Session 菜单「新建窗口」：`open_new_window()` 启动新进程 GUI（对标 BC 多窗口）
- `⌥⌘S`：打开会话中心（保存会话）
- `⌥⌘C` / Session 菜单「清除会话」：`clear_active_tab()` 把当前标签重置为同类型空会话

## 快捷键系统化（对齐 BC 速查表）

| 快捷键 | 功能 | 实现 |
|---|---|---|
| `⌘,` | 设置 | show_settings 弹窗 |
| `⌘T` / `⌘N` | 新建标签页 / 窗口 | new_tab_like_current / open_new_window |
| `⌘L` | 转到行 | DiffTab goto_focus（替换原 ⌘G 语义） |
| `⌘G` / `⇧⌘G` | 查找下一 / 上一 | DiffTab next_match / prev_match |
| `⌥⌘S` / `⌥⌘C` | 保存 / 清除会话 | 会话中心 / clear_active_tab |
| `1/2/3` | 显示全部/差异/相同 | DiffTab 视图过滤（DirTab 已有） |

- Search 菜单补「查找下一 / 上一 / 转到行…」入口
- 快捷键帮助弹窗更新为 BC 式 ⌘ 符号列表

## 文件改动

- `src/gui/mod.rs`：Settings 扩展（ignore_crlf/encoding/max_size）、show_settings 弹窗、handle_global_shortcuts、new_tab_like_current/open_new_window/clear_active_tab、apply_settings_env
- `src/gui/menubar.rs`：Session 菜单（新建标签/窗口/清除会话）、View 菜单（设置…）、Search 菜单（查找下一/上一/转到行）
- `src/gui/difftab.rs`：⌘L/⌘G/⇧⌘G/1/2/3 快捷键
- `src/i18n.rs` + `src/i18n_tables.rs`：14 个新 key × 10 语言（MenuNewTab/MenuNewWindow/MenuSettings/MenuClearSession/SettingsTitle/SettingsIgnoreWs/Trail/Case/Crlf/SettingsEncoding/SettingsMaxSize/MenuGotoLine/MenuFindNext/MenuFindPrev）

## 测试（新增 4 个 uikit）

- `difftab_view_filter_hotkeys_1_2_3`：Num1/2/3 切换 All/Diff/Same
- `difftab_cmd_l_goto_focus`：⌘L 聚焦行号输入框
- `difftab_cmd_g_next_prev_match`：⌘G / ⇧⌘G 循环匹配导航
- `global_shortcuts_new_tab_settings_clear`：⌘T 新标签 / ⌘, 设置 / ⌥⌘S 会话 / ⌥⌘C 清除（用 handle_global_shortcuts_safe 避开 ⌘N 新进程）

本地 489 单元 + 4 kittest 全绿 / clippy 0 / fmt 干净。
