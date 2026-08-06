# bcr — Beyond Compare 风格的文件对比工具（Rust）

Rust 实现的 Beyond Compare 替代品，当前完成 **M1：文本 diff 引擎**。

## 功能（M1 已完成）

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
src/main.rs    CLI 入口（clap 子命令分发）
src/diff.rs    参数解析、输入读取、diff 引擎（similar::capture_diff_slices）
src/render.rs  unified 渲染：hunk 分组、行内高亮、ANSI 着色
```

关键设计：

- **比较键与输出分离**：忽略选项作用于归一化后的"比较键"，输出始终保留原始行，不会因忽略空白而丢内容
- **两级 diff**：行级 diff 定位变更行，变更行对再跑字符级 diff 得到行内高亮区间
- **hunk 分组**：仅按变更 op 之间的间隔决定是否断开（间隔 > 2×3 行上下文则新开 hunk）

## Roadmap

- [x] M1 文本 diff（本里程碑）
- [ ] M2 文件夹对比（walkdir + blake3 + 过滤规则）
- [ ] M3 三路合并 + 冲突标记
- [ ] M4 同步引擎（镜像/双向/更新 + dry-run 预览）
- [ ] M5 GUI（egui 并排 Diff 视图）
- [ ] M6 远程/压缩包适配层（SFTP / ZIP 虚拟 FS）

## 已知限制（M1）

- 整文件读入内存，超大文件（> 数百 MB）需 M2 阶段引入分块比较
- 不处理 "No newline at end of file" 标记
- 二进制文件未做检测（M1 仅文本）
