//! 图片对比标签页：并排渲染左右图 + 差异叠加图，缩放控制，差异统计。
//!
//! 打开流程：`open_diff_files` / 目录双击 / 拖放时检测两侧文件魔数，
//! 若均为图片则创建 `ImageTab` 而非 `DiffTab`。

use crate::imgcmp::ImgPair;
use eframe::egui::{self, Color32, RichText};
use image::RgbaImage;

/// GPU 纹理最大边长（超限先缩小再转纹理，统计仍基于原始像素）
const MAX_TEX: u32 = 4096;

/// 图片标签页
pub struct ImageTab {
    pub left: String,
    pub right: String,
    pub pair: Option<ImgPair>,
    pub error: Option<String>,
    /// 显示缩放（0.05 ~ 4.0，1.0 = 原始尺寸）
    pub zoom: f32,
    /// 是否显示差异叠加图
    pub show_overlay: bool,
    /// 是否显示统计
    pub show_stats: bool,
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

    /// 加载两侧图片并计算差异（失败时 error 置位，pair 清空）
    /// 任一侧为空串时仅更新路径、不加载（等待另一侧填充）
    pub fn load_pair(&mut self, l: &str, r: &str) {
        self.left = l.to_string();
        self.right = r.to_string();
        self.textures = None;
        if l.is_empty() || r.is_empty() {
            self.error = None;
            self.pair = None;
            return;
        }
        match crate::imgcmp::compare_paths(l, r) {
            Ok(p) => {
                self.pair = Some(p);
                self.error = None;
            }
            Err(e) => {
                self.pair = None;
                self.error = Some(e);
            }
        }
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
                ui.label("缩放");
                if ui.button("−").clicked() {
                    self.zoom = (self.zoom * 0.8).max(0.05);
                }
                ui.add(
                    egui::Slider::new(&mut self.zoom, 0.05..=4.0)
                        .logarithmic(true)
                        .show_value(true),
                );
                if ui.button("+").clicked() {
                    self.zoom = (self.zoom * 1.25).min(4.0);
                }
                if ui.button("100%").clicked() {
                    self.zoom = 1.0;
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
            let Some(tex) = &self.textures else {
                ui.label("无图片");
                return;
            };
            egui::ScrollArea::both()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.horizontal_top(|ui| {
                        img_block(ui, &tex.left, &self.left, self.zoom, "左");
                        img_block(ui, &tex.right, &self.right, self.zoom, "右");
                        if self.show_overlay {
                            img_block(ui, &tex.overlay, "差异叠加", self.zoom, "叠加");
                        }
                    });
                });
        });
    }
}

/// 单图渲染块（标题 + 图片，按 zoom 缩放）
fn img_block(ui: &mut egui::Ui, tex: &egui::TextureHandle, label: &str, zoom: f32, side: &str) {
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
                .sense(egui::Sense::hover()),
        );
    });
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
