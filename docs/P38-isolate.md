# P38-1a 文本对比隔离（对标 BC Isolate）

> 背景：P37 审计差距清零后进入 P38 深化阶段。BC-UI-study.md 差距补充表剩余项：
> T2 对齐方式 / T3 隔离 / T4 缩进调整 / T5 转换文件 / T6 选区操作 / T7 编辑导航 / T8 文件级联动。
> 本批做 **T3 隔离（Isolate）**：选中差异区域后只显示该区域，聚焦比较。

## BC 命令语义

| BC 菜单项 | 语义 |
|---|---|
| 右键 → 隔离 (Isolate) | 重新对齐使选中行独立（只显示选中差异区域） |
| 右键 → 显示全部 (Show All) | 取消隔离，恢复全部行 |

## 实施内容

### difftab.rs
- 新增 `isolated: Option<(usize, usize)>`：隔离的差异块范围（rows 行索引，与 diff_blocks 同坐标系）
- `isolate_current()`：根据当前行（diff_pos → 所在块）设置 isolated；右键「隔离」项
- `unisolate()`：清除 isolated
- 渲染/统计/导航过滤：
  - `visible_row_count()` / `visible_rows()`：isolated 时只返回范围内行（虚拟化渲染用）
  - 状态栏/统计（不同行数等）仍按全局统计显示（BC 行为）
- 顶部提示条：「已隔离 行 X–Y [✕ 取消隔离]」按钮
- 隔离时导航（next/prev diff）只在该范围内循环（diff_rows 过滤）

### 测试
- difftab 单元测试：isolate 后 visible 行数 = 块大小；unisolate 恢复；diff_rows 过滤
- uikit 测试：右键菜单含「隔离」，点击后顶部提示条出现，取消后恢复
