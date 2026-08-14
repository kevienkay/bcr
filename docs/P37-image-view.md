# P37-1e 图片视图补齐（对标 BC Picture Compare）

> 背景：BC-UI-study.md 实测图片视图菜单：
> 视图：容差模式(1)/不匹配范围模式(2)/混合模式(3)、忽略不重要差异、自动缩放、
> 顺时针旋转/逆时针旋转/水平翻转/垂直翻转、重置差异偏移、比较元数据、混合切换、文件信息、全屏/缩放。
> bcr 已有：逐像素精确比较、差异叠加、缩放/fit、多帧导航、定位差异。本批补齐旋转/翻转 + 差异判定模式。

## BC 命令语义

| BC 菜单项 | 语义 |
|---|---|
| Tolerance Mode (1) | 容差模式：像素通道差在阈值内视为相同 |
| Mismatch Range Mode (2) | 不匹配范围模式：忽略小于最小面积的孤立差异块 |
| Mixed Mode (3) | 混合模式：容差 + 忽略孤立块同时生效 |
| Rotate Clockwise / Counter-clockwise | 顺时针 / 逆时针旋转 90° |
| Flip Horizontal / Vertical | 水平 / 垂直翻转 |
| Reset Difference Offset | 重置差异偏移（复位旋转/翻转变换） |

## 实施内容

### imgcmp.rs
- `pub fn rotate_image(img: &RgbaImage, deg: u32) -> RgbaImage`（0/90/180/270）
- `pub fn flip_image(img: &RgbaImage, horizontal: bool) -> RgbaImage`
- `pub enum DiffMode { Exact, Tolerance, MismatchRange, Mixed }`
- `pub struct CompareOptions { pub mode: DiffMode, pub tolerance: u8, pub min_diff_area: u32 }`
- `pub fn compare_images_opt(left, right, opts) -> ImgPair`：
  - Tolerance：RGB 曼哈顿距离 ≤ tolerance 视为相同
  - MismatchRange/Mixed：对差异像素做 4-邻接连通域标记，面积 < min_diff_area 的块忽略（不计差异/不染红/不进 bounds）
- `compare_images` 保持默认（Exact）兼容既有调用

### ImageTab（imagetab.rs）
- 新增状态：`rotation: u32`（0/90/180/270）、`flip_h: bool`、`flip_v: bool`
- `recompute_current` 先对当前帧应用变换再比较（pair/纹理基于变换后图像）
- 工具栏加：
  - 变换按钮：↻ 顺时针 / ↺ 逆时针 / ⇋ 水平翻转 / ⇅ 垂直翻转 / ↩ 重置
  - 模式下拉（精确/容差/不匹配范围/混合）+ 容差滑块 + 最小差异块面积滑块
- 缩略图条用变换后帧（保证方向一致）

### i18n
- 新 key ×10 语言：ImgModeExact/ImgModeTolerance/ImgModeMismatch/ImgModeMixed、
  ImgTolerance/ImgMinArea、ImgRotateCw/ImgRotateCcw/ImgFlipH/ImgFlipV/ImgResetTransform

### 测试
- imgcmp 单元测试：旋转 90/180/270 尺寸与像素、翻转、容差忽略微小色差、孤立块忽略、混合模式
- uikit 测试：旋转按钮 → 状态变化；模式下拉切换
