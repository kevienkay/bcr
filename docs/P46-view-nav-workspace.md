# P46 视图开关与导航补齐（对标 BC 5.2.5 剩余差距）

> 依据 `docs/ui-study/bc-menus-*.txt`（11 份菜单树）在 P44/P45 完成后的剩余差距。
> 侧重各视图的视图菜单开关（行号/语法/自动换行/文件信息）、差异导航、结构选项与工作空间。

## 差距总览（BC 有 / bcr 无或未对齐）

### A. 文本编辑（TextEditTab）视图开关补齐
| BC 项 | 快捷键 | bcr 现状 |
|---|---|---|
| 行号（开关） | — | ⚠️ 有 gutter 渲染，无菜单开关 |
| 语法加亮（开关） | — | ✅ 有 show_syntax |
| 自动换行（开关） | — | ❌ |
| 网页（布局） | — | ❌ |
| 文件信息（开关） | — | ❌ |

### B. 文本补丁（PatchTab）差异导航
| BC 项 | 快捷键 | bcr 现状 |
|---|---|---|
| 下一个差异 | ⇧⌥⌃↓ | ❌ 无差异导航 |
| 上一个差异 | ⇧⌥⌃↑ | ❌ |
| 下一个差异部分 | ⇧⌃↓ | ❌ |
| 上一个差异部分 | ⇧⌃↑ | ❌ |

### C. 16进制（DiffTab hex）视图过滤与布局
| BC 项 | 快捷键 | bcr 现状 |
|---|---|---|
| 显示全部 / 差异 / 相同 | 1/2/3 | ❌ hex 无视图过滤 |
| 字节地址（开关） | — | ✅ 有 show_addr |
| 边并排 / 上-下布局 | — | ❌ hex 无布局切换 |

### D. 文件夹比较（DirTab）结构选项
| BC 项 | bcr 现状 |
|---|---|
| 总是显示文件夹 | ❌ |
| 比较文件和文件夹结构 | ❌（有 is_dir 字段未暴露选项） |
| 仅比较文件 | ❌ |

### E. 文件夹同步（DirTab sync 面板）视图
| BC 项 | bcr 现状 |
|---|---|
| 抑制过滤 | ❌ |
| 列（显示列选择） | ❌ |
| 图例（⇧L） | ⚠️ 有 show_legend 弹窗无快捷键 |

### F. 工作空间（会话菜单）
| BC 项 | bcr 现状 |
|---|---|
| 加载工作空间 | ❌（无工作空间概念，= 多标签布局持久化） |
| 保存工作空间为... | ❌ |

## 批次计划（每批：cargo test 全绿 → fmt → clippy 0 → 单提交推送）

- **P46-1 TextEdit 视图开关**：行号/自动换行/文件信息开关（show_line_numbers/show_wrap/show_file_info 字段 + View 菜单 TextEdit 分支）
- **P46-2 PatchTab 差异导航**：next_diff/prev_diff/next_diff_section/prev_diff_section（RowTag Delete/Insert/Replace 行跳转 + 滚动）+ Search 菜单 + 快捷键 ⇧⌥⌃↓/↑、⇧⌃↓/↑
- **P46-3 hex 视图过滤与布局**：hex 显示全部/差异/相同（1/2/3）+ 边并排/上-下布局切换（View 菜单 hex 分支）
- **P46-4 DirTab 结构选项**：总是显示文件夹 / 比较文件和文件夹结构 / 仅比较文件（View 菜单 DirTab 分支）
- **P46-5 文件夹同步视图 + 工作空间**：DirTab sync 面板 抑制过滤/列选择/图例 ⇧L 快捷键；会话菜单 保存/加载工作空间（标签布局 TOML 持久化）
- **P46-docs 收尾**：README/CHANGELOG + docs/P46 实施记录

## 文件改动

- `src/gui/textedit.rs`：视图开关字段 + 渲染条件
- `src/gui/patchtab.rs`：差异导航方法
- `src/gui/difftab.rs`：hex 视图过滤/布局
- `src/gui/dirtab.rs`：结构选项 + sync 面板视图项
- `src/gui/mod.rs`：工作空间保存/加载
- `src/gui/menubar.rs`：View/Search/Session 菜单项
- `src/i18n.rs` + `src/i18n_tables.rs`：新 key × 10 语言
- `src/gui/uikit_tests.rs`：测试

每批本地 cargo test 全绿 → fmt/clippy → 单提交推送；全部完成后统一查 CI。
