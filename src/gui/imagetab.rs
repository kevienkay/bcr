//! 图片对比标签页：并排渲染左右图 + 差异叠加图，缩放/fit-to-window，GIF/WebP 多帧导航 + 缩略图条。
//!
//! 打开流程：`open_diff_files` / 目录双击 / 拖放时检测两侧文件魔数，
//! 若均为图片则创建 `ImageTab` 而非 `DiffTab`。

use crate::imgcmp::ImgPair;
use eframe::egui::{self, Color32, RichText};
use image::RgbaImage;

/// GPU 纹理最大边长（超限先缩小再转纹理，统计仍基于原始像素）
const MAX_TEX: u32 = 4096;
/// 缩略图条最大帧数（超出只显示窗口附近帧，防 UI 卡顿）
const MAX_THUMB_FRAMES: usize = 100;

/// 图片标签页
pub struct ImageTab {
    pub left: String,
    pub right: String,
    pub pair: Option<ImgPair>,
    pub error: Option<String>,
    /// 显示缩放（0.05 ~ 8.0，1.0 = 原始尺寸）
    pub zoom: f32,
    /// 是否显示差异叠加图
    pub show_overlay: bool,
    /// 是否显示统计
    pub show_stats: bool,
    /// 自适应窗口（fit-to-window）
    pub fit: bool,
    /// 左侧全部帧（多帧动图；静态图为单帧）
    frames_l: Vec<RgbaImage>,
    /// 右侧全部帧
    frames_r: Vec<RgbaImage>,
    /// 当前帧索引
    pub(crate) frame_idx: usize,
    /// 每帧是否有差异（帧号 -> bool，逐帧差异导航用）
    pub(crate) frame_diffs: Vec<bool>,
    /// 滚动偏移（定位差异区域用，受控滚动）
    pub(crate) scroll: egui::Vec2,
    /// 请求定位差异区域（帧渲染后消费）
    locate_diff_req: bool,
    textures: Option<ImgTextures>,
}

/// 三张纹理（左 / 右 / 差异叠加）
struct ImgTextures {
    left: egui::TextureHandle,
    right: egui::TextureHandle,
    overlay: egui::TextureHandle,
}

impl ImageTab {
    pub fn new(left: &str, right: &str) -> Self {
        let mut t = ImageTab {
            left: left.to_string(),
            right: right.to_string(),
            pair: None,
            error: None,
            zoom: 1.0,
            show_overlay: false,
            show_stats: true,
            fit: false,
            frames_l: Vec::new(),
            frames_r: Vec::new(),
            frame_idx: 0,
            frame_diffs: Vec::new(),
            scroll: egui::Vec2::ZERO,
            locate_diff_req: false,
            textures: None,
        };
        t.load_pair(left, right);
        t
    }

    pub fn title(&self) -> String {
        format!(
            "🖼 {} ↔ {}",
            std::path::Path::new(&self.left)
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| self.left.clone()),
            std::path::Path::new(&self.right)
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| self.right.clone())
        )
    }

    /// 加载两侧图片全部帧并计算差异（失败时 error 置位，pair 清空）
    /// 任一侧为空串时仅更新路径、不加载（等待另一侧填充）
    pub fn load_pair(&mut self, l: &str, r: &str) {
        self.left = l.to_string();
        self.right = r.to_string();
        self.textures = None;
        self.frame_idx = 0;
        if l.is_empty() || r.is_empty() {
            self.error = None;
            self.pair = None;
            self.frames_l.clear();
            self.frames_r.clear();
            return;
        }
        let (lb, rb) = match (std::fs::read(l), std::fs::read(r)) {
            (Ok(lb), Ok(rb)) => (lb, rb),
            (Err(e), _) => {
                self.error = Some(format!("读取 {} 失败: {}", l, e));
                self.pair = None;
                return;
            }
            (_, Err(e)) => {
                self.error = Some(format!("读取 {} 失败: {}", r, e));
                self.pair = None;
                return;
            }
        };
        match (
            crate::imgcmp::load_frames(&lb, "left"),
            crate::imgcmp::load_frames(&rb, "right"),
        ) {
            (Ok(fl), Ok(fr)) => {
                self.frames_l = fl;
                self.frames_r = fr;
                self.frame_diffs = self.compute_frame_diffs();
                self.error = None;
                self.recompute_current();
            }
            (Err(e), _) | (_, Err(e)) => {
                self.pair = None;
                self.error = Some(e);
            }
        }
    }

    /// 预计算每帧是否有差异（用于差异帧导航与缩略图标记）
    fn compute_frame_diffs(&self) -> Vec<bool> {
        let total = self.frames_l.len().max(self.frames_r.len());
        (0..total)
            .map(|i| {
                let li = i.min(self.frames_l.len().saturating_sub(1));
                let ri = i.min(self.frames_r.len().saturating_sub(1));
                if self.frames_l.is_empty() || self.frames_r.is_empty() {
                    return false;
                }
                crate::imgcmp::compare_images(self.frames_l[li].clone(), self.frames_r[ri].clone())
                    .stats
                    .has_differences()
            })
            .collect()
    }

    /// 跳到下一个有差异的帧（循环）
    pub fn next_diff_frame(&mut self) {
        let total = self.total_frames();
        if total <= 1 {
            return;
        }
        for step in 1..=total {
            let idx = (self.frame_idx + step) % total;
            if self.frame_diffs.get(idx).copied().unwrap_or(false) {
                self.goto_frame(idx);
                return;
            }
        }
    }

    /// 跳到上一个有差异的帧（循环）
    pub fn prev_diff_frame(&mut self) {
        let total = self.total_frames();
        if total <= 1 {
            return;
        }
        for step in 1..=total {
            let idx = (self.frame_idx + total - step) % total;
            if self.frame_diffs.get(idx).copied().unwrap_or(false) {
                self.goto_frame(idx);
                return;
            }
        }
    }

    /// 定位到当前帧的差异区域：按包围盒缩放并滚动到中心（无差异时无操作）
    pub fn locate_diff(&mut self, avail: egui::Vec2) {
        let Some(pair) = &self.pair else { return };
        let Some((bx, by, bw, bh)) = pair.stats.bounds else {
            return;
        };
        self.fit = false;
        // 缩放目标：让包围盒占据可视区约 70%，同时不放大超过 8x
        let cols = if self.show_overlay { 3.0 } else { 2.0 };
        let cell_w = (avail.x / cols - 24.0).max(50.0);
        let cell_h = (avail.y - 56.0).max(50.0);
        let zoom = ((cell_w / bw as f32).min(cell_h / bh as f32) * 0.7).clamp(0.05, 8.0);
        self.zoom = zoom;
        // 滚动到包围盒中心（考虑标题栏高度 ~18px）
        let cx = (bx as f32 + bw as f32 / 2.0) * zoom;
        let cy = (by as f32 + bh as f32 / 2.0) * zoom + 18.0;
        self.scroll = egui::Vec2::new((cx - cell_w / 2.0).max(0.0), (cy - cell_h / 2.0).max(0.0));
    }

    /// 用当前帧重算差异对（纹理缓存失效，下次渲染重建）
    fn recompute_current(&mut self) {
        let li = self.frame_idx.min(self.frames_l.len().saturating_sub(1));
        let ri = self.frame_idx.min(self.frames_r.len().saturating_sub(1));
        if self.frames_l.is_empty() || self.frames_r.is_empty() {
            self.pair = None;
            return;
        }
        self.pair = Some(crate::imgcmp::compare_images(
            self.frames_l[li].clone(),
            self.frames_r[ri].clone(),
        ));
        self.textures = None;
    }

    /// 跳转到指定帧（越界截断）
    pub fn goto_frame(&mut self, idx: usize) {
        let total = self.total_frames();
        let idx = idx.min(total.saturating_sub(1));
        if idx != self.frame_idx {
            self.frame_idx = idx;
            self.recompute_current();
        }
    }

    /// 总帧数（两侧取较大值）
    pub(crate) fn total_frames(&self) -> usize {
        self.frames_l.len().max(self.frames_r.len())
    }

    /// 懒加载纹理（需要 ctx）
    fn ensure_textures(&mut self, ctx: &egui::Context) {
        if self.textures.is_some() {
            return;
        }
        let Some(pair) = &self.pair else { return };
        let left = to_texture(ctx, &pair.left, "img-left");
        let right = to_texture(ctx, &pair.right, "img-right");
        let overlay = to_texture(ctx, &pair.overlay, "img-overlay");
        self.textures = Some(ImgTextures {
            left,
            right,
            overlay,
        });
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        self.ensure_textures(ui.ctx());
        // 工具栏
        egui::Panel::top("img-toolbar").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui.button("↺").on_hover_text("重新加载").clicked() {
                    let (l, r) = (self.left.clone(), self.right.clone());
                    self.load_pair(&l, &r);
                }
                ui.separator();
                // 多帧导航
                let total = self.total_frames();
                if total > 1 {
                    if ui.button("⏮").on_hover_text("第一帧").clicked() {
                        self.goto_frame(0);
                    }
                    if ui.button("◀").on_hover_text("上一帧").clicked() {
                        self.goto_frame(self.frame_idx.saturating_sub(1));
                    }
                    ui.label(format!("{}/{}", self.frame_idx + 1, total));
                    if ui.button("▶").on_hover_text("下一帧").clicked() {
                        self.goto_frame(self.frame_idx + 1);
                    }
                    if ui.button("⏭").on_hover_text("最后一帧").clicked() {
                        self.goto_frame(total);
                    }
                    // 差异帧导航：跳到下一个/上一个有差异的帧
                    let diff_count = self.frame_diffs.iter().filter(|&&d| d).count();
                    if diff_count > 0 {
                        ui.separator();
                        if ui.button("⏮!").on_hover_text("上一个差异帧").clicked() {
                            self.prev_diff_frame();
                        }
                        if ui.button("!▶").on_hover_text("下一个差异帧").clicked() {
                            self.next_diff_frame();
                        }
                        ui.label(format!("差异帧 {}/{}", diff_count, total));
                    }
                    ui.separator();
                }
                // 定位差异区域（当前帧）
                let has_diff = self
                    .pair
                    .as_ref()
                    .map(|p| p.stats.has_differences())
                    .unwrap_or(false);
                if has_diff
                    && ui
                        .button("🎯 定位差异")
                        .on_hover_text("缩放并滚动到差异区域")
                        .clicked()
                {
                    self.locate_diff_req = true;
                }
                ui.label("缩放");
                if ui.button("−").clicked() {
                    self.fit = false;
                    self.zoom = (self.zoom * 0.8).max(0.05);
                }
                ui.add(
                    egui::Slider::new(&mut self.zoom, 0.05..=8.0)
                        .logarithmic(true)
                        .show_value(true),
                );
                if ui.button("+").clicked() {
                    self.fit = false;
                    self.zoom = (self.zoom * 1.25).min(8.0);
                }
                if ui.button("100%").clicked() {
                    self.fit = false;
                    self.zoom = 1.0;
                }
                if ui.selectable_label(self.fit, "适应窗口").clicked() {
                    self.fit = !self.fit;
                }
                ui.separator();
                ui.checkbox(&mut self.show_overlay, "差异叠加");
                ui.checkbox(&mut self.show_stats, "统计");
                if let Some(p) = &self.pair {
                    let s = p.stats;
                    let st = if s.has_differences() {
                        RichText::new(format!(
                            "差异像素 {} / {} ({:.2}%)",
                            s.diff_pixels,
                            s.total_pixels,
                            s.diff_ratio * 100.0
                        ))
                        .color(Color32::from_rgb(230, 80, 80))
                    } else {
                        RichText::new("完全相同").color(Color32::from_rgb(90, 190, 90))
                    };
                    ui.separator();
                    ui.label(st);
                    ui.label(format!(
                        "{}x{} → {}x{}",
                        s.left_w, s.left_h, s.right_w, s.right_h
                    ));
                }
            });
        });

        if let Some(err) = &self.error {
            egui::CentralPanel::default().show(ui, |ui| {
                ui.colored_label(
                    Color32::from_rgb(230, 80, 80),
                    RichText::new(format!("错误: {}", err)).size(14.0),
                );
            });
            return;
        }
        if self.pair.is_none() && self.left.is_empty() {
            egui::CentralPanel::default().show(ui, |ui| {
                ui.centered_and_justified(|ui| {
                    ui.label(RichText::new("拖入或选择右侧图片").size(16.0));
                });
            });
            return;
        }

        egui::CentralPanel::default().show(ui, |ui| {
            // 定位差异请求：按当前可视区计算缩放与滚动（需 &mut self，先于纹理借用处理）
            if self.locate_diff_req {
                let avail = ui.available_size();
                self.locate_diff(avail);
                self.locate_diff_req = false;
            }
            let Some(tex) = &self.textures else {
                ui.label("无图片");
                return;
            };
            // fit-to-window：按可用区与列数计算缩放（不落盘到 self.zoom，切回手动时恢复）
            let mut eff_zoom = self.zoom;
            if self.fit {
                let avail = ui.available_size();
                let cols = if self.show_overlay { 3.0 } else { 2.0 };
                let iw = tex.left.size_vec2();
                let sx = (avail.x / cols - 24.0) / iw.x;
                let sy = (avail.y - 56.0) / iw.y;
                eff_zoom = sx.min(sy).clamp(0.01, 8.0);
            }
            // 受控滚动：定位差异时用 self.scroll 覆盖，否则跟随用户滚动
            let mut swap_req = false;
            let out = egui::ScrollArea::both()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.horizontal_top(|ui| {
                        let rl = img_block(ui, &tex.left, &self.left, eff_zoom, "左");
                        let rr = img_block(ui, &tex.right, &self.right, eff_zoom, "右");
                        // P32-A4：右键菜单（复制路径/打开所在位置/系统打开/交换左右）
                        let (lp, rp) = (self.left.clone(), self.right.clone());
                        for resp in [rl, rr] {
                            resp.context_menu(|ui| {
                                if ui.button("复制左侧路径").clicked() {
                                    ui.ctx().copy_text(lp.clone());
                                    ui.close();
                                }
                                if ui.button("复制右侧路径").clicked() {
                                    ui.ctx().copy_text(rp.clone());
                                    ui.close();
                                }
                                ui.separator();
                                if ui.button("打开所在位置（左）").clicked() {
                                    super::common::reveal_in_file_manager(&lp);
                                    ui.close();
                                }
                                if ui.button("打开所在位置（右）").clicked() {
                                    super::common::reveal_in_file_manager(&rp);
                                    ui.close();
                                }
                                ui.separator();
                                if ui.button("系统打开（左）").clicked() {
                                    super::common::open_with_system_app(&lp);
                                    ui.close();
                                }
                                if ui.button("系统打开（右）").clicked() {
                                    super::common::open_with_system_app(&rp);
                                    ui.close();
                                }
                                ui.separator();
                                if ui.button("交换左右").clicked() {
                                    swap_req = true;
                                    ui.close();
                                }
                            });
                        }
                        if self.show_overlay {
                            img_block(ui, &tex.overlay, "差异叠加", eff_zoom, "叠加");
                        }
                    });
                });
            if swap_req {
                let (l, r) = (self.left.clone(), self.right.clone());
                self.load_pair(&r, &l);
            }
            // 用户未主动滚动时保持受控偏移；用户滚动后跟随用户
            if out.state.offset != self.scroll {
                self.scroll = out.state.offset;
            }
            // 缩略图条（多帧动图）：底部横向缩略图导航
            let total = self.total_frames();
            if total > 1 && total <= MAX_THUMB_FRAMES {
                ui.separator();
                ui.label(
                    RichText::new("帧导航（点击跳转）")
                        .size(11.0)
                        .color(ui.visuals().weak_text_color()),
                );
                egui::ScrollArea::horizontal().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let sel = self.frame_idx;
                        for i in 0..total {
                            // 取该帧缩略图（左帧优先，缺帧用右帧）
                            let frame = self.frames_l.get(i).or_else(|| self.frames_r.get(i));
                            let Some(frame) = frame else { continue };
                            let thumb = image::imageops::resize(
                                frame,
                                48,
                                48,
                                image::imageops::FilterType::Triangle,
                            );
                            let tex = to_texture(ui.ctx(), &thumb, &format!("thumb-{i}"));
                            let resp = ui
                                .add(
                                    egui::Image::new((tex.id(), egui::vec2(48.0, 48.0)))
                                        .sense(egui::Sense::click()),
                                )
                                .on_hover_text(format!(
                                    "帧 {}{}",
                                    i + 1,
                                    if self.frame_diffs.get(i).copied().unwrap_or(false) {
                                        "（有差异）"
                                    } else {
                                        ""
                                    }
                                ));
                            let rect = resp.rect;
                            // 差异帧红色边框，当前帧蓝色边框（优先差异色）
                            let is_diff = self.frame_diffs.get(i).copied().unwrap_or(false);
                            let stroke_color = if is_diff {
                                Color32::from_rgb(230, 80, 80)
                            } else if i == sel {
                                Color32::from_rgb(80, 160, 255)
                            } else {
                                Color32::from_gray(90)
                            };
                            let width = if is_diff || i == sel { 2.5 } else { 1.0 };
                            ui.painter().rect_stroke(
                                rect,
                                2.0,
                                egui::Stroke::new(width, stroke_color),
                                egui::StrokeKind::Outside,
                            );
                            if resp.clicked() {
                                self.goto_frame(i);
                            }
                        }
                    });
                });
            }
        });
    }
}

/// 单图渲染块（标题 + 图片，按 zoom 缩放），返回 Response 供右键菜单使用
fn img_block(
    ui: &mut egui::Ui,
    tex: &egui::TextureHandle,
    label: &str,
    zoom: f32,
    side: &str,
) -> egui::Response {
    let size = tex.size_vec2() * zoom;
    ui.vertical(|ui| {
        ui.label(
            RichText::new(format!("[{}] {}", side, label))
                .size(12.0)
                .color(ui.visuals().weak_text_color()),
        );
        ui.add(
            egui::Image::new((tex.id(), size))
                .fit_to_exact_size(size)
                .sense(egui::Sense::click()),
        )
    })
    .inner
}

/// RgbaImage → egui 纹理（超限缩小，防 GPU 纹理超尺寸）
fn to_texture(ctx: &egui::Context, img: &RgbaImage, name: &str) -> egui::TextureHandle {
    let (w, h) = img.dimensions();
    let (dw, dh) = if w > MAX_TEX || h > MAX_TEX {
        let scale = MAX_TEX as f32 / w.max(h) as f32;
        (
            ((w as f32 * scale) as u32).max(1),
            ((h as f32 * scale) as u32).max(1),
        )
    } else {
        (w, h)
    };
    let rgba = if (dw, dh) != (w, h) {
        image::imageops::resize(img, dw, dh, image::imageops::FilterType::Lanczos3).into_raw()
    } else {
        img.as_raw().clone()
    };
    let color = egui::ColorImage::from_rgba_unmultiplied([dw as usize, dh as usize], &rgba);
    ctx.load_texture(name, color, egui::TextureOptions::LINEAR)
}
