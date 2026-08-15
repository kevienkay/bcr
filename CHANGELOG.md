# Changelog

bcr — Beyond Compare 风格的文件对比工具（Rust 实现）。本文件遵循
[Keep a Changelog](https://keepachangelog.com/zh-CN/1.0.0/) 格式，
版本号遵循 [SemVer](https://semver.org/lang/zh-CN/)。

## [Unreleased]

### 剩余差距补齐（P44，对标 BC 5.2.5 菜单树全量扫描）

- **窗口菜单**：选择下一/上一标签页（⌘]/⌘[，循环）、最小化（⌘M，ViewportCommand）、关闭所有窗口（⌘⇧W，清空标签回主页）
- **文本比较快捷键**：⌘A 对齐方式（当前差异块左侧行与右侧当前行对齐）、]/[ 增加/减少缩进（当前差异块 ±4 空格）、⌘E 使用选择内容进行查找（选区文本填入查找框并聚焦）
- **文本合并快捷键**：⇧←/⇧→ 采用左边/右边（块级）、⌘B 采用左边然后右边、⇧⌘B 采用右边然后左边、⌘⇧⌃↓/↑ 下一/上一冲突部分；Edit 菜单 MergeTab 分支冲突采用 5 项
- **会话/文件菜单补齐**：⌥⌘O 打开会话、⌘R 重新比较（Diff/Merge/Dir/FolderMerge/Csv 转发）、⌘⇧S 保存文件为（TextEdit 另存对话框）、已锁定（DiffTab 防编辑开关）、打开方式（系统应用打开/在查找器中显示，DiffTab 左右文件）
- **工具菜单补齐**：导出/导入设置（TOML 文件）、恢复出厂默认、编辑文本文件/查看补丁（TextEdit/PatchTab 入口）
- **视图开关 + 表格快捷键**：DiffTab 行号/语法加亮开关（View 菜单）；CsvTab ⇧⌃↩ 修改（预填选中单元格）、⌘⌥⌃↩ 前面插入行、⌥⌃↩ 后面插入行、排序对话框（选左右侧列 + 升/降序）
- **搜索补齐**：DirTab 查找文件名（⌘F，不区分大小写匹配基名 + 过滤面板输入框）、TextEdit 在多个文件中查找（⌘⇧F，默认当前文件目录）

### 导航/选区/媒体（P43，对标 BC 5.2.5 会话/编辑/搜索/视图）

- **DirTab 导航历史**：后退/前进/上一层/比较父文件夹（Session 菜单 DirTab 分支；navigate 首次调用把当前路径对入栈作历史起点，back 可回退；空会话不入栈）
- **DiffTab 文本选区**：当前差异块选为选区（蓝色高亮）→ 选择内容/把选择内容和剪贴板比较（Edit 菜单；选区文本写临时文件 → 右侧加载）；修复 P42-1 转换文件子菜单误入 DirTab 分支（改独立 DiffTab 分支）
- **替换导航**：下一/上一替换（Search 菜单，跳匹配 + 聚焦替换框）
- **差异文件导航**：下一/上一差异文件（DirTab，isize 取模循环跳 status≠Same 文件并选中+滚动）
- **会话菜单补齐**：合并文件（当前 Diff 左右文件进三路合并，BASE 留空）/ 和输出比较（FolderMerge 输出目录 vs 左侧开 DirTab）
- **信息弹窗**：Session>信息——按当前标签显示统计（Diff 编码/大小/行数/差异行、Dir 条目/统计、Merge 冲突/行、Image 帧/差异像素、Csv 行列、TextEdit 字符/行、Patch 标题、FolderMerge 输出、Media 差异字段）
- **媒体比较（简化版）**：mediacmp 自研容器头解析（WAV RIFF fmt 块→声道/采样率/位深/时长；MP3 帧头同步字→码率表估算时长；FLAC STREAMINFO→采样率/声道/位深/时长）+ 字段级对比；MediaTab 双栏元数据并排 + 差异字段红色标记 + 重新加载/交换两侧；主页第 9 张卡片 🎵 + Session 菜单新建媒体比较 + 拖放两文件自动识别（is_media_file 扩展名兜底）；状态栏显示差异字段数

### 交互模型对齐（P34，对标 BC 空会话 + 拖拽导入）

- **空会话入口**：首页卡片/菜单 Session 直接进空会话（Diff/Dir/Merge/Image/Csv），空面板分别打开左右侧（BASE/LEFT/RIGHT），不再强制一次选满文件
- **拖拽导入填充**：空会话中拖文件/目录自动填充对应侧（新增 `fill_empty_session`），空面板显示拖拽提示（DragHint）

### 文本对比核心交互（P35，对标 BC 命令参考）

- **复制差异块到另一侧（Copy to Other Side）**：工具栏 →/← 按钮 + 右键菜单置顶，重建目标侧全文 + 按原编码写回 + 撤销快照 + .bak 备份
- **交换左右两侧（Swap Sides）**：工具栏 ⇄ 按钮，支持双边/单侧/hex，撤销栈清空
- **视图过滤（Show All/Diff/Same/Context）**：工具栏下拉，渲染层过滤（差异行集合 + Context 上下文行）
- **显示空白符（Visible Whitespace）**：空格→·、制表符→→，语法高亮自动降级避免字节错位

### 文件夹对比交互补齐（P36，实测 BC 5.2.5）

- **交换两边（Swap Sides）**：工具栏 ⇄ 按钮（BC 会话菜单「交换两边」）
- **逐文件操作**：右键菜单复制到右侧/复制到左侧/删除右侧/删除左侧/排除（复用 SyncOp + execute_op；排除 = 会话级隐藏集合）
- **视图过滤快捷键**：1/2/3 切换显示全部/差异/相同（输入框聚焦不触发，不受列表空影响）

### 全视图对标补齐（P37，实测 BC 5.2.5 各视图菜单）

- **三路合并顺序合并（1a）**：Resolution 加 LeftThenRight/RightThenLeft（BC「采用左边然后右边/右边然后左边」），工具栏 ⇉/⇇ 按钮 + 解决状态绿标
- **三路合并导航（1b）**：清除冲突区段并跳下一（未解决默认取左）、差异导航（非 Context 块）、左/右采用导航（Next/Previous Left/Right Taken）；Search 菜单 MergeTab 分支
- **表格视图（1c）**：复制单元格至右侧（点击选中 + 写回右侧文件 + .bak 备份）、隐藏相同列、列宽自适应（RFC 4180 serialize_csv）
- **hex 地址与字节序格式（1d）**：地址列 hex/dec 切换与隐藏、值显示 逐字节/小尾/大端（4 字节分组 u32 解释）
- **图片视图（1e）**：旋转 90/180/270 + 水平/垂直翻转 + 重置变换；差异模式 精确/容差/不匹配范围/混合（RGB 曼哈顿距离 + 4 邻接孤立块过滤）
- **文件夹同步操作集（1f）**：立即同步（⚡ 一键生成计划并执行）、独自离开（Leave Alone 跳过同步、再点取消）、批量复制→左/批量删除左侧（镜像 →右/删右侧）
- **文本编辑视图（1g）**：独立单文件编辑器（BC Text Edit）——打开/保存（编码回写 + .bak）、撤销/重做、查找/替换、转换文件（Trim 行尾空白 / Tabs→空格 / CRLF↔LF 逐行保留行尾）、语法高亮预览 + 行号；CLI `--edit`
- **补丁视图（1h）**：unified diff 解析（---/+++/@@ hunk）→ 旧 vs 新双栏对比 + added/removed 统计 + 应用补丁写回目标（.bak 备份）；CLI `--patch`
- **文件夹合并 GUI（1i）**：三目录 BASE/LEFT/RIGHT + 输出（BC Folder Merge）——生成计划（build_merge3_plan）列表展示 copy/merge/conflict/delete 徽标 + 冲突红标，执行写输出并自动建目录；CLI `--merge-dir BASE LEFT RIGHT OUT`

### 文本对比深化（P38，实测 BC 5.2.5 文本菜单）

- **隔离 Isolate（1a）**：右键隔离当前差异块（仅显示该区域），提示条点击显示全部；next/prev 导航限定隔离范围
- **对齐方式 Align With（1b）**：右键启动手动强制行对齐（选一侧行 → 点击另一侧行配对合并为 Replace 行），支持清除对齐
- **缩进调整（1c）**：增加/减少缩进 ±4 空格（仅行首空白，备份 + 编码回写 + 撤销快照 + 重载）
- **编辑导航 Next/Prev Edit（1d）**：已编辑行锚点（左/右行号重映射）+ 循环跳转 + 右上角圆点标记
- **复制文件并打开下一差异（1e）**：右键复制整个文件到另一侧并跳转下一差异（BC Copy File and Open Next Difference）
- **修复**：macOS 双击裸二进制弹出命令行即退出（无参数一律启动 GUI）；主页会话卡片点击不跳转（Frame::show 默认无 click Sense）

### 全视图对标收口（P39，BC 5.2.5 实机扫描 2668 行菜单树 + 12 实机截图）

- **设置对话框（2a，⌘,）**：忽略选项（空白/行尾/大小写/CRLF）+ 编码 + 大小上限集中管理，`apply_settings_env` 写 BCR_ENCODING/BCR_MAX_SIZE；启动时应用
- **新建标签/窗口（2a）**：⌘T 新建当前类型标签（new_tab_like_current）、⌘N 新建窗口（新进程 GUI）、⌥⌘S 会话中心、⌥⌘C 清除会话（重置当前标签）
- **快捷键系统化（2a）**：⌘L 转到行 / ⌘G-⇧⌘G 查找下一上一 / 1-2-3 视图过滤（DiffTab）；快捷键帮助弹窗更新为 BC 式 ⌘ 列表
- **UI 精修（2b）**：等宽字体优先（JetBrains Mono/系统等宽）+ CJK fallback；diff 配色对齐 BC 柔和色调（淡红/淡绿/淡黄）；当前差异行竖条改蓝色 current_bar；工具栏 emoji→矢量符号（⧉ 剪贴板/⚙ 外部工具）；gutter 行号 128/深色底 38
- **会话中心保存当前会话（2c）**：名称输入 + 保存按钮，从 DirTab/DiffTab 提取左右路径写 ~/.bcr-sessions.toml
- **报告生成（2c，⌘P）**：TXT/HTML 格式选择 + 实时预览 + rfd 保存；DirTab 接 report/htmlreport，DiffTab 文本报告（统计 + 差异行摘要）
- **差异部分导航（2c，⇧⌃↓↑）**：按 diff_blocks 区块级跳转（无当前位置→首/末块，有→相邻块），同步 diff_pos 竖条
- **细节三模式（2d）**：文本/16进制（文本文件强制构建字节网格）/对齐方式，View 菜单「细节」子菜单
- **布局（2d）**：边并排/上-下（行高 2x，paint_diff_row_v 垂直堆叠）/网页，View 菜单「布局」子菜单
- **主页卡片精修（2d）**：hover 蓝色描边高亮（1.5px rgb(86,148,240)）+ 圆角 + PointingHand
- **书签（2d）**：⌘⌥⌃0-9 切换书签 / ⌘0-9 跳转（当前顶部行绑定，diff_pos 同步高亮）
- **替换菜单（2e，⇧⌘F）**：Search 菜单「替换…」聚焦替换框（修复 ⌘F 分支抢先）；View 菜单「忽略不重要差异」四忽略开关一键同步；Tools「保存快照」；Session「比较文件使用」子菜单（文本/16进制/图片/表格视图切换 reopen_as_*）

### 工具栏精简（P40，对标 BC 一行 6 组）

- **DiffTab 工具栏 34→~18 控件（1）**：剪贴板→左/右、编辑左/右、忽略空白/行尾/大小写、自动换行、显示空白符、hex 显示选项（地址/值格式）、缩略图等低频控件全部收进菜单（Edit 新增编辑左/右 `start_edit`；View 新增单项忽略/换行/空白/hex 选项）；工具栏保留 BC 式 6 组：打开 | 视图过滤 | 复制→右/左 | 下一/上一差异+计数 | 交换/重载 | 搜索替换
- **DirTab 工具栏精简（2）**：include/exclude glob 输入 + 应用过滤收进左侧过滤面板（「清除全部过滤」同步清空），工具栏保留路径/刷新/交换/内容哈希/仅差异/显示相同/状态过滤/统计/同步/批量
- **其余视图评估**：CsvTab 8 / ImageTab 20（旋转翻转核心）/ MergeTab 15 / FolderMergeTab 2 控件均高频，保留不动

### DirTab 选择操作与过滤扩展（P41，对标 BC 文件夹比较 编辑/视图菜单）

- **展开/折叠全部（1）**：`expand_all`/`collapse_all`（从 entries 收集目录路径折叠、重建树），Edit 菜单 DirTab 分支加两项
- **视图过滤扩展（2）**：ViewFilter 加 `LeftNewer`/`RightNewer`（仅左侧/右侧较新，Differ + mtime 比较 `is_left_newer`/`is_right_newer`），工具栏下拉加 2 项
- **选择操作（3）**：多选集合 `selected_set`（flat 索引）+ 行渲染高亮；`select_all`/`select_none`/`invert_selection`/`select_orphans`（LeftOnly+RightOnly）/`select_newer`（较新项）；Edit 菜单 DirTab 分支加 5 项

### 文本比较编辑补齐与视图辅助（P42，对标 BC 文本比较 编辑/视图菜单）

- **转换文件（1）**：`ConvertMode`+`convert_content` 纯函数（Trim 行尾空白/Tabs→空格/CRLF↔LF）抽到 textedit 供 DiffTab 复用；DiffTab `convert_file` 作用于两侧（.bak 备份+编码回写+撤销快照+重载）；Edit 菜单 DiffTab 分支「转换文件」子菜单
- **剪贴板比较（2）**：DiffTab `⌘V` → load_clipboard_right（BC File>打开剪贴板）；TextEditTab `open_clipboard`（读剪贴板填充内容+撤销快照+未命名另存）；File 菜单 TextEdit 分支转发
- **字符列标尺（3）**：DiffTab `show_ruler` 内容区顶部左右栏 10/20/...200 刻度，View 菜单 checkbox
- **图例/日志/工具栏开关（4）**：View 菜单图例弹窗（差异色/状态徽标含义）、日志面板（最近操作滚动+清空）、工具栏开关（全局 `SHOW_TOOLBAR` AtomicBool 门控 8 个 tab 工具栏渲染）

## [0.3.0] - 2026-08-13

### UI 重构（P33，对标 Beyond Compare 5.2.5 真实界面）

- **标准菜单栏**：新增 BC 式 7 菜单（Session/File/Edit/Search/View/Tools/Help），替代顶部扁平按钮排；语言/主题切换移入 View 菜单；新增快捷键/关于/外部工具说明弹窗（30 个菜单 i18n key × 10 语言）
- **工具栏分组重排**：DiffTab 工具栏按 BC 语义分 6 组（打开/显示选项/编辑/操作/差异导航/搜索替换），组间分隔线
- **文件信息头两行化**：文件名 + 详情行（时间 | 大小 | 编码 | 语法），对标 BC 文件头
- **主页 BC 式改造**：左侧 Sessions 会话列表面板 + 8 类会话大按钮（文本/文件夹/合并/图片/CSV/Hex/文件夹合并/同步）+ 拖放提示语
- **文件夹对比列头**：DirTab 顶部加 BC 式列头（名称/大小/修改时间）
- **状态色对齐 BC 实测**：孤儿（仅一侧）= 紫 rgb(83,44,199)、差异 = 红 rgb(246,39,16)、相同 = 默认黑、未知 = 灰（替换原 L红/R蓝/C黄）

## [0.2.2] - 2026-08-13

### 修复（macOS 安装包）

- **macOS dmg 双击报「bcr 已损坏」**：打包脚本加 ad-hoc codesign（无证书时 `codesign --force --deep --sign -`），避免 Gatekeeper 拦截未签名 .app；本地验证 `Signature=adhoc` + `codesign --verify --deep --strict` 通过

## [0.2.1] - 2026-08-12

### 修复（UI 精修跟进）

- **文本对比左右两页**：两栏固定各占半屏并排（不再被长行撑出视口），顶部加左右文件名头部（固定视口宽度，不随滚动移动）
- **长行不截断**：内容宽度按最长行扩展，超宽时出现水平滚动条，可左右拖动查看完整内容（鼠标拖动/滚动条/触摸板）
- **CI 修复**：恢复 Cargo.lock 至干净状态（windows/windows-core 依赖误降级导致 Windows clippy 编译 wgpu-hal 失败）

## [0.2.0] - 2026-08-12

### UI 差距消除（P32，对标 Beyond Compare 4 全量盘点，A类核心交互 + B类次要差距）

- **差异连接线（A1）**：DiffTab 左右面板间空隙 + 差异行水平连接线（红/绿/黄按差异类型着色），等行弱色分隔；单 ScrollArea 双栏渲染天然同步滚动（A3）
- **直接内联编辑（A2）+ 撤销/重做（A6）**：DiffTab 双击行就地编辑（Enter 提交 / ESC 取消）；编辑/替换入撤销栈（Ctrl+Z / Ctrl+Y 或 Ctrl+Shift+Z，工具栏 ↩/↪ 按钮）
- **右键菜单全覆盖（A4）**：DiffTab/CsvTab/ImageTab/MergeTab 全部行级 context_menu（复制路径/打开所在位置/系统打开/交换左右/忽略差异），DirTab 扩展；`open_with_system_app` 与 `reveal_in_file_manager` 跨平台（macOS open / Windows explorer / Linux xdg-open）提升到 common 复用
- **差异块折叠（A5）**：DiffTab 差异块行首 ▾ 折叠箭头，折叠后隐藏中间行显示「N 行已折叠」占位，点击展开
- **标记忽略差异（B5）**：差异行右键「忽略此行/此块」→ 从导航/统计排除（会话级，右键可取消），弱化显示
- **会话类型起始页（A7）**：欢迎页扩展为网格卡片（文本对比/文件夹对比/三路合并/图片对比/CSV 表格），新增 `open_image_compare` / `open_csv_compare` 独立会话入口，10 语言 i18n
- **快捷键系统化（B1）**：DiffTab F6 下一差异/F7 上一差异（循环跳转，修正索引语义）、F5 重载；DirTab F2 重命名（vfs 跨后端弹窗）+ F5 刷新；全局 Ctrl+W 关闭当前标签；按钮 tooltip 带快捷键提示
- **DirTab 过滤/显示面板（B2）**：左侧可折叠 SidePanel——扩展名（逗号分隔）、大小范围（字节）、修改时间范围（YYYY-MM-DD，自研公历转换）过滤，与工具栏联动，清除/计数显示
- **状态栏信息（B3）**：底部状态栏补当前标签路径（DiffTab 文件名）、行数/行列数（DiffTab 行数、CsvTab 行×列）、选中项数（DirTab 选中 x/y）
- **标签拖拽重排（B6）**：标签栏拖拽换位（`move_tab` 保持 active 指向原标签）
- **独立 Hex 差异导航（B4）**：二进制自动切换 hex 对比视图已有基础上补 F6/F7 差异行循环跳转（滚动定位，`show_rows_offset` 支持初始偏移）
- **工具栏图标细节（B7）**：按钮 tooltip 统一补快捷键提示（如「⟳ 重新加载 (F5)」「下一差异 (F6)」）
- **质量**：每批新增 uikit 测试（egui_kittest 驱动真实标签页交互，含键盘事件），累计 392+ 测试全绿；clippy 0 警告；fmt 干净

### UI 精修（P31，对标 Beyond Compare 视觉）

- **主题引擎**：新增 `src/gui/theme.rs`——集中视觉常量（行高 22、圆角 4、间距统一），差异配色对齐 BC（仅左红/仅右绿/修改黄）按深浅主题微调，启动时对 Dark/Light 两套主题全局应用
- **主窗口**：底部全局状态栏（当前标签统计：Diff 行级 / Dir 目录 / Csv 表格 / Merge 冲突 / Image 帧差异）；标签栏美化（当前标签 strong 高亮 + 关闭按钮 hover 变红）
- **DiffTab**：当前差异行 BC 风格左侧竖条标记（CURRENT_BAR 3px，黄色）
- **DirTab**：目录名文件夹色（浅蓝，深浅主题区分）；文件行状态徽标（圆形底色 + 状态字母，替代纯文本 `[L]`）
- **CsvTab**：表头底色与内容区区分；新增 `stats()` 访问器供状态栏
- **MergeTab**：冲突行左侧黄色竖条标记

### 优化（按差距分析优先级推进）

- **归档内存保护**：tar/7z 全量解压加上限（默认 1 GiB，`BCR_MAX_ARCHIVE_SIZE` 环境变量可调），超限报 `OutOfMemory` 错误而非 OOM 崩溃；`read_tar_with_limit` 支持注入上限（测试用）
- **benchmarks**：新增 `benches/core.rs`（criterion，黑盒 CLI 方式）——文本 diff（1K/10K/50K 行）、文件夹对比（100/1K/5K 文件）、CSV 对齐（1K/10K 行）、同步计划（100/1K 文件）；`cargo bench` 可运行，`cargo bench --bench core -- --test` 验证模式
- **测试稳定性修复**：`dirtab_collapse_hides_children` Windows 偶发失败（默认 `ViewFilter::Diff` 把两侧相同文件滤掉 → flat 为空 → `unwrap()` panic）——测试显式设置 `ViewFilter::All`，消除平台时序依赖
- **基线数据**（本机 arm64）：diff 1K 行 ≈6.8ms / 10K ≈129ms / 50K ≈2.57s；compare 100 文件 ≈57ms

### 文档（P30 收尾）

- **README 新增「安装」章节**：macOS dmg（拖入应用程序/未签名说明）、Windows zip（解压+PATH）、Linux deb/tar.gz（dpkg -i / 解压到 ~/.local/bin），含 SHA256SUMS 校验指引；补充 cargo install（git tag）与 Python 绑定 pip 安装方式
- **README 新增「快速开始」章节**：GUI 与命令行常用用法（diff/compare/sync/merge3/csv/--json），常用选项说明
- **README 已知限制修正**：删除“不处理 No newline 标记”过期条目（P16 已实现 `\ No newline at end of file`，与功能列表矛盾）

### 分发（P30 扩展）

- **应用图标**：新增 `assets/icon.png`（1024px，PIL 生成“双面板差异条”设计）+ `bcr.icns`（iconutil，macOS）+ `bcr.ico`（多尺寸，Windows）
- **macOS 打包**：Info.plist 加 CFBundleIconFile，icns 拷入 Resources
- **Linux 打包**：deb 含 256px 图标（/usr/share/icons/hicolor）+ bcr.desktop 桌面入口（Exec=bcr gui）
- **Windows 打包**：zip 内含 bcr.ico

## [0.1.0] - 2026-08-11

首个里程碑版本：完整对标 Beyond Compare 核心场景的对比/合并/同步工具。

### 里程碑 M1-M6（核心）

- **M1 文本 diff**：unified 输出 + 行内高亮（Myers/Patience 双算法）+ 忽略空白/行尾/大小写 + git 兼容退出码
- **M2 文件夹对比**：walkdir 递归 + blake3 深度模式 + glob 过滤 + 状态输出
- **M3 三路合并**：diff3 归并 + git 风格冲突标记 + git mergetool 兼容
- **M4 同步引擎**：update/mirror/two-way 三模式 + dry-run 预览 + mtime 保留
- **M5 GUI**：egui 桌面应用（并排 Diff/目录树/三路合并多标签）+ 虚拟化渲染 + 搜索/跳转 + 主题/设置持久化 + Git 集成
- **M6 虚拟文件系统**：Vfs trait 统一抽象 + 跨后端混合对比

### P0-P7（增强）

- **P0** 编码检测与二进制检测（GBK/UTF-16/UTF-32/BOM + 二进制识别）
- **P1** 语法高亮（syntect）+ 二进制 hex 对比（CLI `bcr hex` + GUI 自动切换）
- **P2** 重命名/移动检测（内容哈希匹配 `[M]` 标记）
- **P3** 分块流式读取（大文件内存 O(64KB)）
- **P4** HTML 对比报告 + 会话保存/恢复（session save/list/run/delete）
- **P5** 同步增强：mirror/update 移动检测 `[MOVE]` + mirror 空目录清理 `[RMDIR]`
- **P6** 三路文件夹对比（compare3，10 状态标记）
- **P7** CSV/表格对比（主键/行号对齐、列级 diff、引号字段解析）

### P8-P16（GUI/工作流完善）

- **P8** 图片对比（imgcmp 逐像素 diff + GUI ImageTab 并排/叠加/缩放；GIF/WebP 多帧导航 + 缩略图条 + fit-to-window + 🎯 差异区域定位）
- **P9** GUI 同步操作面板（build_plan/execute_op 纯逻辑接口 + 同步向导 + 单项操作）
- **P10** 比较规则 Profile（save/list/delete + `--profile` 合并 + GUI 规则面板）
- **P11** 报告格式（--txt/--csv）+ 字段定制（--report-fields）
- **P12** 文件属性与符号链接（--compare-attrs）
- **P13** hex 编辑（GUI 二进制视图双击行编辑）
- **P14** 细节打磨（自动刷新 2s / 手动对齐 / 批量复制/删除）
- **P15** 文本 diff 细节（`\ No newline at end of file` + --ignore-crlf）
- **P16** 会话收藏（★ + 最近使用排序）

### P17-P23（性能/远程/报告深度）

- **P17** 比较结果缓存（size+mtime 快照，LRU 64 条，命中秒开）
- **P18** 报告布局模板（--report-title / --report-no-stats）
- **P19** Profile export/import（迁移/分享）
- **P20** 目录对比右键菜单（复制路径/系统应用打开）
- **P21** 内容过滤规则（--ignore-lines 正则忽略差异行）
- **P22** 报告深度定制（--report-sort / --report-group 状态分组）
- **P23** GUI 拖放排序

### P24-P27（特殊格式 / 版本比较 / 云盘浏览 / 自动化）

- **P24** MP3 标签比较器（自研 ID3v1/v2 解析，UTF-8/UTF-16/ISO-8859-1，`bcr mp3tag` 字段级差异）
- **P25** 版本比较模式（--compare-version，从 FileVersion/ProductVersion 提取，UTF-16/ASCII，段数值比较）
- **P26** GUI 云盘浏览（☁ 窗口输入 webdav/s3/onedrive/dropbox/sftp/ftp URL，扫描并打开目录对比）
- **P27** 自动化：全命令 `--json` 契约（compare.v1/sync.v1/compare3.v1/csv.v1/merge.v1/mp3tag.v1/imgcmp.v1）+ Python 绑定 `bindings/bcr.py`（纯标准库 dataclass API）+ 纯数据任务清单 `bcr task run/check`（JSON/TOML，%date%/%time%/%fn_time%/%env:VAR%/%1-%9 动态变量，差异不中止语义）

### P29（CSV 表格 GUI）

- **CsvTab 标签页**：并排渲染左右表格，行按主键/行号对齐，行级状态着色 + 单元格级差异高亮（左红右黄）
- **工具栏**：主键下拉、分隔符（`,`/`\t`）、显示相同、状态过滤（全部/仅差异/仅左/仅右/仅修改/仅相同）
- **表头点击排序**（升/降，纯显示排序）+ 统计栏
- **入口路由**：目录双击/手动对齐/打开对比/拖放/CLI 启动自动识别 .csv/.tsv/.tab 进表格视图
- **结构化 API**：`csvcmp::align_tables` 返回逐行状态 + 变化列（RowStatus/AlignedRow）；CLI 文本输出契约 csv.v1 不变
- i18n：10 语言新增 CsvTitle/CsvKeyCol/CsvDelimiter/CsvRowAlign/CsvFilter* 条目

### P28（全面对标收口，A/B/C 差距消除）

- **A1** 三路文件夹合并 `merge3`（BASE/L/R → 输出目录，文本自动三路合并 + 冲突标记，--dry-run/--json）
- **A2** 保存自动备份（GUI 编辑保存前生成 .bak）
- **A3** 剪贴板对比（GUI 剪贴板→左/右，arboard 跨平台）
- **A4** 文本替换（GUI 查找升级为替换/全部替换，按原编码回写）
- **A5** 通用音频标签（mp3tag 魔数自动识别 MP3/FLAC/OGG/MP4/AAC，Vorbis comment + MP4 ilst 自研解析）
- **A6** FTPS 后端（ftps:// implicit TLS，端口默认 990）
- **A7** 忽略文件夹结构（compare/sync --ignore-structure 按文件名跨目录对齐）
- **A8** 自动换行（GUI word wrap，BC5 特性）
- **A9** 转换后比较（diff --convert 统一换行符）
- **A10** 打印报告（compare --print，lp/lpr/PowerShell Out-Printer）
- **A11** 缩略图总览（GUI 文本对比右侧迷你差异地图，点击跳转）
- **A12** CAB/ISO 归档后端（cab:// 纯 Rust + iso:// 外部 7z/bsdtar）
- **A13** SVN 后端（svn:// 外部 svn 命令，只读）
- **A14** 第三方对比工具（~/.bcr-external.toml 扩展名映射，diff --external）
- **B1** 目录状态过滤（GUI 下拉：全部/差异/仅左/仅右/仅移动/仅相同）
- **B2** 后台多任务（GUI 对比/同步后台线程 + 进度条 + 暂停/继续/取消）
- **B3** HTML 报告深度（--report-template 模板 + --report-link-files 文件级链接）
- **B4** 符号链接跟随（compare/sync --follow-symlinks）
- **C1** 大文件内存优化（memmap2 只读映射，默认上限 64MB→256MB）
- **C2** 7z 可写（sevenz-rust2 LZMA2 全量重写 + 原子替换）
- **C3** SFTP host key 校验（TOFU 保存 ~/.bcr-known-hosts，sftp+insecure:// 兼容跳过）
- **C4** FTP mtime（MFMT 扩展设置，不支持时静默降级）

### 后端扩展（M6b-M6h）

- **M6b** tar/tar.gz/tar.bz2/tar.xz/7z 只读后端（`tar://` / `7z://`）
- **M6c** ZIP 后端可写（write/delete/set_mtime/rename 全量重写+原子替换）
- **M6d** FTP 后端（可读写，匿名登录/被动模式）
- **M6e** tar 可写（tar/tar.gz/tar.xz 全量重写）
- **M6f** WebDAV 后端（PROPFIND/GET/PUT/DELETE/MKCOL/MOVE，Basic Auth）
- **M6g** Amazon S3 后端（`s3://`，rust-s3，MinIO endpoint）
- **M6h** OneDrive（Graph API）+ Dropbox（Dropbox API）云存储后端

### 工程/质量

- **I18N**：10 语言支持（zh/en/de/ja/ko/es/pt/ar/ru/fr），翻译表宏保证穷尽
- **CI**：GitHub Actions 三端矩阵（ubuntu/windows/macos）fmt/clippy 门禁 + 验收测试 + 跨平台专项
- **验收套件**：142 用例（tests/acceptance.sh + cross_platform.sh）
- **单元测试**：275 用例

### 文档

- README：完整功能矩阵 + 使用示例 + 云凭证配置说明
- docs/P0-P1-design.md：编码检测/语法高亮设计稿
- docs/P27-python-binding-design.md：自动化方案 v3 设计稿
- docs/automation.md：自动化指南（JSON 契约参考 + bcr.py API + 任务清单 + 场景示例）

[0.1.0]: https://github.com/kevienkay/bcr/releases/tag/v0.1.0
[0.2.2]: https://github.com/kevienkay/bcr/releases/tag/v0.2.2
[0.2.1]: https://github.com/kevienkay/bcr/releases/tag/v0.2.1
[0.2.0]: https://github.com/kevienkay/bcr/releases/tag/v0.2.0
