# bcr — Beyond Compare 风格的文件对比工具（Rust）

Rust 实现的 Beyond Compare 替代品，当前完成 **M1：文本 diff 引擎** + **M2：文件夹对比**。

## 功能

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
```

关键设计：

- **比较键与输出分离**：忽略选项作用于归一化后的"比较键"，输出始终保留原始行，不会因忽略空白而丢内容
- **两级 diff**：行级 diff 定位变更行，变更行对再跑字符级 diff 得到行内高亮区间
- **hunk 分组**：仅按变更 op 之间的间隔决定是否断开（间隔 > 2×3 行上下文则新开 hunk）

## Roadmap

- [x] M1 文本 diff
- [x] M2 文件夹对比（walkdir + blake3 + 过滤规则）
- [ ] M3 三路合并 + 冲突标记
- [ ] M4 同步引擎（镜像/双向/更新 + dry-run 预览）
- [ ] M5 GUI（egui 并排 Diff 视图）
- [ ] M6 远程/压缩包适配层（SFTP / ZIP 虚拟 FS）

## 已知限制（M1/M2）

- 整文件读入内存，超大文件（> 数百 MB）需后续引入分块比较
- 不处理 "No newline at end of file" 标记
- 二进制文件未做检测（M1 仅文本）
- 快速模式依赖 mtime，跨文件系统/拷贝场景建议用 `--compare-content` 保证准确
