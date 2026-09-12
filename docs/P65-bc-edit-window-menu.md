# P65：BC 5.2.5 设计稿 —— 编辑菜单标准动作 + 窗口菜单多窗口

> 设计基准：`design/BC菜单和状态栏/screens/menus.html` 画板①②、
> `design/BC菜单和状态栏/交接/design-tokens.json` 的 `menus.grayedRules`
> 前置：P60–P64（token / 状态栏 / 逐视图 / 菜单态核对），本阶段收口 P64 记录的 7 项缺口。

## 一、编辑菜单：剪切 / 复制 / 粘贴 / 删除 / 全选

| 关注点 | 落点 |
|---|---|
| 纯逻辑（选区、替换、全选区间、剪贴板） | `src/gui/edit_ops.rs`（`EditOp`、`clamp_range`/`selection_text`/`splice`/`all_range`、`read_clipboard`/`write_clipboard`） |
| 动作执行（文本编辑会话） | `src/gui/textedit.rs`：`apply_edit_op` / `replace_selection` / `sync_egui_cursor` / `selection` / `is_editable` |
| 应用级路由 | `src/gui/mod.rs`：`active_edit_op`（`EditOp` → 当前标签）；`Cmd::EditCut/Copy/Paste/Delete/SelectAll` |
| 窗口内菜单（Linux） | `src/gui/menubar.rs` 编辑菜单：`EditOp::ALL` 循环渲染，`clipboard_ops_enabled` 决定置灰 |
| 原生菜单（macOS/Windows） | `src/gui/native_menu.rs`：`edit_op_item`（id=`EditOp::cmd_id`、Accelerator ⌘X/⌘C/⌘V/⌘A，删除无快捷键） |
| i18n | `MenuCut/MenuCopy/MenuPaste/MenuDelete` × 10 语言 |

**选区模型**：选区由 `egui::TextEdit` 持有（char 索引），渲染时经 `TextEditOut.cursor_range`
捕获到 `TextEditTab::sel_range`；菜单动作直接改缓冲区，再把新选区写回
`TextEditState`（`state.cursor.set_char_range` + `store`）并 `request_focus`。
因此动作**不依赖合成键盘事件**，headless 可单测。

**置灰规则**（`menu_rules::clipboard_ops_enabled`）：

| 会话 | 状态 |
|---|---|
| Diff / Dir / Csv / Image / Media / Patch（只读比较） | 置灰（设计稿画板②左） |
| 主页（无标签） | 置灰 |
| TextEdit（编辑模式） | 可用 |
| TextEdit（语法高亮预览 = 只读渲染） | 置灰 |
| Merge | 置灰（左/右栏为绘制行，无文本选区模型；见「仍缺」） |

## 二、撤销 / 重做路由修复

- 症状：窗口内菜单与原生菜单的撤销/重做此前只转发 `DiffTab`，在**文本编辑/合并会话**里点击是空操作，
  但菜单按设计稿显示为可用。
- 修法：`DiffApp::undo_active` / `redo_active` 按 `Tab` 变体转发；
  `MergeTab` 新增解决动作撤销/重做栈（`push_snapshot` / `undo` / `redo`，
  快照 = `Vec<BlockInfo>` + `conflict_idx`，上限 100 步，撤销后重算未解决冲突数）。

## 三、窗口菜单：移动标签页到新窗口 / 合并所有窗口

bcr 的多窗口模型是**多进程**（`⌘N` 新建窗口 = 新进程），故用窗口注册表跨进程协调（`src/gui/windows.rs`）：

| 关注点 | 说明 |
|---|---|
| 注册表文件 | `~/.bcr-windows/window-<pid>.toml`（`BCR_WINDOWS_DIR` 可覆盖），含 `pid/updated/total/tabs[]`，原子写（tmp + rename） |
| 心跳 | 每 2 秒重写；`PEER_TTL = 10` 秒内无心跳视为已退出（并清理），崩溃可自愈 |
| 移动标签 | 写单标签工作空间 → `bcr gui --workspace <file>` 新进程 → 本窗口关闭该标签 |
| 合并窗口 | 读对端注册表 → 在本窗口重建其会话 → 仅对「全部标签可重建」的对端写 `<pid>.merge` 退出请求 |
| 退出轮询 | 每秒检查 `<pid>.merge`；发现即注销注册表、保存设置、关闭窗口 |
| 空闲帧 | `ui()` 里 `request_repaint_after(1s)`：egui 事件驱动，空闲窗口不重绘会让心跳与退出请求停摆 |

**可重建会话**（`tab_session` / `tab_from_session`，与 `save_workspace`/`load_workspace` 共用）：
diff / dir / merge / image / csv / media。文本编辑、补丁、文件夹合并含未保存内存态 → **拒绝移动**、
且使所属窗口不参与「合并即退出」（避免关掉未保存内容）。

**置灰规则**（`menu_rules::move_tab_enabled` / `merge_windows_enabled`）：
单标签时两项都置灰（设计稿窗口菜单）；移动还要求当前标签可重建；合并还要求存在心跳有效的对端窗口。

## 四、验收

```bash
cargo test --bin bcr          # 657 通过 / 12 ignored（基线 621）
cargo clippy --all-targets -- -D warnings
cargo fmt --check

# 无头截图（需 GPU；沙箱内 wgpu 找不到适配器时在沙箱外跑）
BCR_SNAP_DIR=/tmp/bcr-snap cargo test --bin bcr -- --ignored gui::ui_snap
```

新增测试（36 条）：`edit_ops` 纯函数 5、`textedit` 会话动作 6、`mergetab` 撤销/重做 2、
`menu_rules` 编辑/窗口规则 4、`native_menu` 命令映射 2、`windows` 注册表 6、
应用级路由 3、kittest 交互 8（含用 `/bin/echo` 注入启动器、临时注册表目录模拟对端窗口）。

真实 GUI 实测（macOS，AX `kAXEnabledAttribute` + 辅助功能点击 + `pbpaste` 回读）：
只读会话五项 off / 文本编辑会话五项 on；全选→⌘C 得到整篇文件内容；粘贴→内容追加；
全选→删除→粘贴后读回仅剩粘贴内容；剪切→撤销内容恢复；多标签时窗口两项 on，
点击移动后写出 `bcr-move-<pid>-1.toml` 并新起进程（源窗口 3→2 标签）；
点击合并后对端进程退出、源窗口 2→3 标签。

## 五、仍缺（明确边界）

1. **合并会话的左/右栏文本编辑模型**：设计稿要求「左右栏可编辑 · 输出栏只读」，
   剪切/复制/粘贴/删除/全选 因此应在合并会话可用。本实现左/右栏是对齐后的绘制行
   （块级解决 + 行级采用），没有文本选区，故这 5 项在合并会话仍置灰。补齐需先把
   左右栏改成可编辑文本并在编辑后重算对齐（BC 行对齐语义），属独立任务。
2. **macOS 系统级窗口项**：最小化全部 / 缩放 / 缩放全部 / 全部前移 / 在前面排列
   未实现（画板①为可用态，但不属于对比工具功能；AppKit 只在菜单被登记为
   `NSApp.windowsMenu` 时自动补这些项）。
