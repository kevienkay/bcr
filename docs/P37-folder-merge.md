# P37-1i 文件夹合并 GUI（对标 BC Folder Merge）

> 背景：BC-UI-study.md 实测文件夹合并视图（`-fv="Folder Merge"`）：
> 会话：交换两边/后退/前进/浏览文件夹/上一层/文件夹合并信息/和输出比较/合并父文件夹
> 操作：打开文件夹/比较内容/**合并.../复制到输出...**/排除/复制文件名/已忽略/刷新选择内容(R)
> 编辑：展开全部/折叠全部/全选/选择所有文件/选择较新项/选择独有项/反向选择
> bcr 现状：只有 CLI `bcr merge3`（build_merge3_plan + execute_plan），无 GUI。本批新增 FolderMergeTab。

## 实施内容

### src/gui/foldermergetab.rs（新文件）
- `pub struct FolderMergeTab { base: String, left: String, right: String, out: String, plan: Option<Vec<Merge3PlanItem>>, stats: Merge3Stats, error, msg, scroll }`
- `new(base, left, right, out)` / `reload()`：vfs::open 三侧 + build_merge3_plan
- 渲染：
  - 三路径头（base/left/right/out）+ 「生成计划」「执行合并」按钮
  - 计划列表（虚拟化行）：操作徽标（copy/merge/conflict/delete/same）+ 相对路径 + 来源侧
    - conflict 行红色高亮（BC 冲突语义）
  - 底部统计：copied/merged/conflicts/deleted/same
- `execute()`：调 execute_plan 写入输出目录，返回冲突数
- 空会话：P34 风格分别打开 BASE/LEFT/RIGHT/OUT

### 路由（mod.rs）
- Tab 枚举加 `FolderMerge(FolderMergeTab)`（title/ui/状态栏）
- GuiArgs 加 `--merge-dir`（4 参数 BASE LEFT RIGHT OUT）；拖放 3 目录 → 自动识别？暂用 CLI 入口 + Session 菜单
- Session 菜单加「文件夹合并」入口（空会话）

### i18n
- 新 key ×10 语言：FolderMergeTitle / GeneratePlan / ExecuteMerge / MergeOut / MergeStats / PlanConflict

### 测试
- 单元测试：build 计划 + execute 到临时目录（复用 merge3 逻辑，验证 copy/merge/conflict）
- uikit 测试：FolderMergeTab 渲染不 panic；计划生成后列表有行
