# bcr — Beyond Compare 风格的文件对比工具（Rust）

Rust 实现的 Beyond Compare 替代品，当前完成 **M1：文本 diff** + **M2：文件夹对比** + **M3：三路合并** + **M4：同步引擎** + **M5：GUI 并排 Diff 视图**。

## 功能

### M5 GUI 并排 Diff（`bcr gui`）

- `bcr gui [LEFT] [RIGHT]`：egui 桌面窗口，左右并排渲染 diff
- 行级着色：删除（红底）/ 插入（绿底）/ 修改（红绿底）；修改行做行内字符级高亮
- 两侧行号独立跟踪，共享滚动，支持横向/纵向滚动
- 顶部工具栏：打开文件（系统对话框）、忽略空白/行尾空白/大小写（即时重算）、重新加载
- 支持拖放文件到窗口（两个文件成对加载，单个文件补充另一侧）
- 底部统计栏：相同/删除/插入/修改行数 + 当前文件对
- 复用 M1 的 diff 引擎与归一化选项，CLI/GUI 行为一致

### M4 目录同步（`bcr sync`）

- `bcr sync <LEFT> <RIGHT>`，三种模式：
  - `--mode update`（默认）：单向复制新增/更新（源较新才覆盖），不删除
  - `--mode mirror`：单向镜像，源为准无条件覆盖，并删除目标侧独有文件
  - `--mode two-way`：双向，新增/更新以 mtime 新者胜；mtime 相同且内容不同（需 `--compare-content`）报冲突跳过
- `--reverse` 反转方向（默认 LEFT → RIGHT）、`--dry-run` 只预览不执行
- 复制保留源 mtime（幂等：同步后再跑无操作）、自动建目录、`--include/--exclude` 过滤、`--summary`
- 退出码：0=成功，1=有冲突/有计划(dry-run)，2=错误

### M3 三路合并（`bcr merge`）

- `bcr merge <BASE> <LEFT> <RIGHT>`：经典 diff3 算法，两侧变更基于 base 行号归并
- 合并规则：单侧修改取该侧；两侧相同修改无冲突；两侧不同修改输出 git 风格冲突块
- 冲突标记：`<<<<<<< LEFT / ======= / >>>>>>> RIGHT`，可用 `-L` 自定义标签
- `-o` 输出到文件（有冲突也写出，供人工处理）、`-` 支持 stdin
- 退出码：0=无冲突，1=有冲突，2=错误（可直接作为 `git mergetool`）

### M2 文件夹对比（`bcr compare`）

- 递归对比两个目录树，输出差异状态列表：`[L]` 仅左侧 / `[R]` 仅右侧 / `[C]` 内容不同 / `[S]` 相同
- 双模式：默认按 `大小+修改时间` 快速比较；`--compare-content` 对大小相同的文件对做 blake3 哈希深度比对
- 过滤规则：`--include <glob>` 白名单、`--exclude <glob>` 黑名单（可重复，目录级剪枝）
- `--show-same` 显示相同文件、`--summary` 输出统计
- git 兼容退出码：0=无差异，1=有差异，2=错误

### M1 文本 diff（`bcr diff`）

- 两个文本文件的 unified diff 输出（兼容 GNU diff / git apply 格式）
- 行内差异高亮（intra-line diff，字符级二次比对）
- 双算法：`--algo myers` / `--algo patience`（默认 patience，更适合代码）
- 忽略选项：`--ignore-whitespace` / `--ignore-trailing` / `--ignore-case`
- `--color auto|always|never`（默认按 TTY 自动）
- `-L` 自定义标签、`-` 从 stdin 读取
- git 兼容退出码：**0=无差异，1=有差异，2=错误**

## 使用

```bash
cargo build --release

# GUI 并排 Diff 视图
bcr gui old.rs new.rs
bcr gui --ignore-whitespace old.rs new.rs

# 预览同步计划（不执行）
bcr sync src/ backup/ --mode mirror --dry-run

# 单向更新（左→右）
bcr sync src/ backup/ --mode update

# 双向同步，内容级冲突检测
bcr sync laptop/ nas/ --mode two-way --compare-content --summary

# 三路合并（无冲突自动合并）
bcr merge base.py left.py right.py -o merged.py

# 冲突时输出标记块，退出码 1
# <<<<<<< LEFT / ======= / >>>>>>> RIGHT

# 作为 git mergetool 使用（M3 后支持）
git mergetool --tool=bcr

# 目录对比（快速模式）
bcr compare old-dir new-dir

# 深度内容对比 + 统计
bcr compare old-dir new-dir --compare-content --summary

# 排除 build 目录和日志，只看源码差异
bcr compare src/ src-copy/ --exclude 'target/**' --exclude '*.log'

# 作为 git difftool 使用
# 基本对比
bcr diff old.rs new.rs

# 忽略空白 + 强制颜色
bcr diff --ignore-whitespace --color=always old.rs new.rs

# stdin 对比
printf 'a\nb\n' | bcr diff - new.txt -L stdin -L file

# 作为 git difftool 使用
git difftool --tool=bcr
```

git 配置：

```ini
[difftool "bcr"]
    cmd = bcr diff "$LOCAL" "$REMOTE" -L "$LOCAL" -L "$REMOTE"
```

## 架构

```
src/main.rs     CLI 入口（clap 子命令分发）
src/diff.rs     M1 参数解析、输入读取、diff 引擎（similar::capture_diff_slices）
src/render.rs   M1 unified 渲染：hunk 分组、行内高亮、ANSI 着色
src/compare.rs  M2 目录扫描（walkdir）、双模式比较、glob 过滤、状态输出
src/merge.rs    M3 三路合并：diff3 归并（collect_block + apply_regions）、冲突标记
src/fsscan.rs   共享扫描/过滤/哈希模块（compare 与 sync 共用）
src/sync.rs     M4 同步引擎：三模式计划生成、dry-run、mtime 保留复制
src/sideview.rs M5 并排 diff 数据模型：行级 ops 展开为并排行（行号+行内高亮），纯逻辑可单测
src/gui.rs      M5 egui 窗口：工具栏/统计栏/双栏滚动渲染（基于 sideview 模型）
```

关键设计：

- **比较键与输出分离**：忽略选项作用于归一化后的"比较键"，输出始终保留原始行，不会因忽略空白而丢内容
- **两级 diff**：行级 diff 定位变更行，变更行对再跑字符级 diff 得到行内高亮区间
- **hunk 分组**：仅按变更 op 之间的间隔决定是否断开（间隔 > 2×3 行上下文则新开 hunk）

## Roadmap

- [x] M1 文本 diff
- [x] M2 文件夹对比（walkdir + blake3 + 过滤规则）
- [x] M3 三路合并 + 冲突标记
- [x] M4 同步引擎（镜像/双向/更新 + dry-run 预览）
- [x] M5 GUI（egui 并排 Diff 视图）
- [ ] M6 远程/压缩包适配层（SFTP / ZIP 虚拟 FS）

## 已知限制（M1-M5）

- 整文件读入内存，超大文件（> 数百 MB）需后续引入分块比较
- 不处理 "No newline at end of file" 标记
- 二进制文件未做检测（M1/M3/M5 仅文本）
- M5 GUI 未做虚拟化渲染，超大文件（> 数万行）会卡顿；拖放仅支持本地文件
- 快速模式依赖 mtime，跨文件系统/拷贝场景建议用 `--compare-content` 保证准确
- M3 三处 stdin 不能同时用（`-` 只能出现一次）
- 与 git 的行为差异：两侧对**相邻行**的独立修改，bcr 按经典 diff3 语义无冲突合并，git 保守判冲突
- sync 快速模式下无法检测“mtime 相同但内容不同”，two-way 冲突检测需 `--compare-content`
- sync 的 mirror 删除只删文件，不清理空目录
