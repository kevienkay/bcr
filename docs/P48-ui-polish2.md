# P48 UI 精修续 实施记录

用户指定做「主页卡片动效 / 标签栏样式 / 对话框统一（UI 精修续）」，1 个功能提交 + 1 个 docs 收尾，全部推送 `origin/master`。

## P48 UI 精修续（`45095dc`）

**主页卡片 hover 动效**（BC 卡片观感）：

- `ui.ctx().animate_bool(Id::new(("card_hover", i)), resp.hovered())` 动画插值 0~1
- hover 时卡片 rect 上浮 3px（`rect.translate(0, -lift)`），阴影用叠层模拟（`Color32::from_black_alpha(18 * hover_anim)` 的圆角矩形下移绘制）
- 兼容深浅主题；保留 P39-2d 蓝色描边
- 关键：绘制用位移后的 `rect`，点击命中仍用原 `resp`（交互不变）

**标签栏 BC 风格**：

- 选中标签用 `egui::Frame::new().fill(高亮底色).corner_radius(6).inner_margin(symmetric(8,4))` 包裹
- 深浅主题各自底色（深色 52,58,70 / 浅色 228,232,240），普通标签透明底
- 关闭按钮颜色分层：hover 红（diff_delete）/ 选中强色 / 普通弱色
- 保留 B6 拖拽重排逻辑（drag_started/dragged/drag_stopped）

**对话框统一**：

- 设置/报告/信息/会话中心 default_size 对齐：460/460/460/560（原 440/420/440/560）
- 统一 `.frame(Frame::new().inner_margin(Margin::same(14)))` 内边距

## 测试与质量

- 本地 **530 单元 + 4 kittest 全绿** / clippy 0 / fmt 干净
- 无新增 i18n key、无新增测试（纯渲染层改动，既有测试覆盖交互不变）
