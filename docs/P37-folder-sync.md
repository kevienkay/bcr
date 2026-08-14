# P37-1f 文件夹同步操作集补齐（对标 BC Folder Sync）

> 背景：BC-UI-study.md 实测文件夹同步菜单：
> 操作：独自离开/复制右边到左边/复制左边到右边/删除左边/删除右边/排除.../刷新选择内容(R)
> 会话：交换两边/文件夹同步信息/立即同步
> bcr 已有：交换两边、单文件复制到右/左、删除右/左、排除、批量复制→右、批量删除右侧、
> 同步面板（update/mirror/two-way + 勾选计划 + 后台执行）。本批补齐 4 项。

## BC 命令语义

| BC 菜单项 | 语义 |
|---|---|
| 立即同步（Sync Now） | 一键执行当前同步计划（不需要展开面板） |
| 独自离开（Leave Alone） | 标记文件两侧都不动，同步计划生成时跳过该文件；可取消 |
| 复制左边到右边 / 复制右边到左边 | 批量方向补齐（bcr 已有→右，补→左） |
| 删除左边 / 删除右边 | 批量方向补齐（bcr 已有删右侧，补删左侧） |

## 实施内容

### dirtab.rs
- 新增状态：
  - `leave_alone: HashSet<String>`（会话级：同步计划生成时过滤这些文件）
  - `sync_now_req: bool`（工具栏「立即同步」请求）
- **立即同步**：工具栏按钮「⚡ 立即同步」→ 生成计划（若空）→ 直接全选可执行项执行（后台线程）
- **独自离开**：右键菜单「🚫 独自离开」→ 加入 leave_alone + 重新生成计划跳过；
  已标记文件右键显示「✓ 已独自离开」（点击取消）；gen_sync_plan 里过滤 leave_alone
- **批量复制→左**：run_batch_copy_to_left()（镜像 run_batch_copy_to_right）
- **批量删除左侧**：run_batch_delete_left()（镜像 run_batch_delete_right）

### i18n
- 新 key ×10 语言：SyncNow / LeaveAlone / LeaveAloneOn / CopyBatchToLeft / DeleteBatchLeft

### 测试
- 单元测试：gen_sync_plan 过滤 leave_alone；批量复制→左/删除左侧执行
- uikit 测试：右键「独自离开」→ 计划跳过；「立即同步」按钮触发
