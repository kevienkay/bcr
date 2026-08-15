# P45 深层交互补齐（对标 BC 5.2.5 菜单树剩余差距）

> 依据 `docs/ui-study/bc-menus-*.txt`（11 份菜单树）在 P44 完成后的剩余差距。
> 侧重各视图专属菜单的深层交互（行级操作/视图过滤扩展/格式细节），非全局菜单。

## 差距总览（BC 有 / bcr 无或未对齐）

### A. 文本合并（MergeTab）行级采用
| BC 项 | 快捷键 | bcr 现状 |
|---|---|---|
| 采用左边的行 | ⌥⇧← | ❌ 仅块级采用（P44-3 resolve_current） |
| 采用中心行 | — | ❌ |
| 采用右边行 | ⌥⇧→ | ❌ |

### B. 文件夹合并（FolderMergeTab）视图过滤
| BC 项 | 快捷键 | bcr 现状 |
|---|---|---|
| 显示全部 | 1 | ❌ 无视图过滤 |
| 显示更改 | 2 | ❌ |
| 显示冲突 | 3 | ❌ |
| 显示左边变化 | 4 | ❌ |
| 显示右边变化 | 5 | ❌ |
| 显示可合并 | 6 | ❌ |
| 显示未变化项 | 7 | ❌ |

### C. 文件夹比较（DirTab）视图过滤扩展
| BC 项 | bcr 现状 |
|---|---|
| 显示独有 / 不独有 / 差异但无独有 | ⚠️ 有 LeftOnly/RightOnly，无「独有」「不独有」组合 |
| 显示左边较新和左边独有 / 右边较新和右边独有 | ❌ |
| 总是显示文件夹 | ❌ |
| 比较文件和文件夹结构 / 仅比较文件 | ❌ |

### D. 图片比较（ImageTab）补齐
| BC 项 | bcr 现状 |
|---|---|
| 重置差异偏移 | ⚠️ 有 reset_transform（旋转/翻转），无独立偏移重置 |
| 比较元数据 | ⚠️ 有 show_meta 显示，无「比较元数据」动作 |
| 混合切换 | ⚠️ 有 diff_mode 下拉 |

### E. 表格/HEX/补丁/文本编辑补齐
| BC 项 | 快捷键 | bcr 现状 |
|---|---|---|
| 表格 在前面插入列 / 在后面插入列 | — | ⚠️ 仅 insert_col（前插） |
| HEX 复制到右边 | ⇧⌃→ | ❌ |
| 补丁 选择选择内容 | D | ❌ |
| 文本编辑 使用选择内容进行查找 | ⌘E | ❌（DiffTab 有） |

## 批次计划（每批：cargo test 全绿 → fmt → clippy 0 → 单提交推送）

- **P45-1 文本合并行级采用**：MergeTab 加行级采用（采用左边的行/中心行/右边行，⌥⇧←/→ 与无快捷键菜单项），Edit 菜单 MergeTab 分支补行级采用 3 项
- **P45-2 文件夹合并视图过滤**：FolderMergeTab 加 `view_filter` 枚举（全部/更改/冲突/左变化/右变化/可合并/未变化）+ 快捷键 1-7 + View 菜单项
- **P45-3 文件夹比较视图过滤扩展**：DirTab 加「显示独有/不独有/差异但无独有/左较新+左独有/右较新+右独有」组合过滤 + 总是显示文件夹 + 比较文件与文件夹结构/仅比较文件
- **P45-4 图片比较补齐**：ImageTab 加重置差异偏移 + 比较元数据（尺寸/格式/大小对比弹窗）
- **P45-5 表格/HEX/补丁/文本编辑补齐**：CsvTab 在后面插入列、HEX 复制到右边（⇧⌃→）、PatchTab 选择选择内容、TextEdit 使用选择内容查找（⌘E）
- **P45-docs 收尾**：README/CHANGELOG + docs/P45 实施记录

## 文件改动

- `src/gui/mergetab.rs`：行级采用
- `src/gui/foldermergetab.rs`：视图过滤枚举 + 渲染过滤
- `src/gui/dirtab.rs`：视图过滤扩展 + 结构比较选项
- `src/gui/imagetab.rs`：重置差异偏移 + 比较元数据
- `src/gui/csvtab.rs` / `src/gui/difftab.rs` / `src/gui/patchtab.rs` / `src/gui/textedit.rs`：杂项补齐
- `src/gui/menubar.rs`：菜单项
- `src/i18n.rs` + `src/i18n_tables.rs`：新 key × 10 语言
- `src/gui/uikit_tests.rs`：测试

每批本地 cargo test 全绿 → fmt/clippy → 单提交推送；全部完成后统一查 CI。
