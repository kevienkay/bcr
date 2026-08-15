//! P43-6 媒体比较标签页（简化版）：并排显示左右音视频文件元数据 + 字段级差异高亮。
//!
//! 打开流程：`open_diff_files` / 拖放时检测两侧文件为音视频（mediacmp::read_media_info
//! 能识别容器头或扩展名匹配）→ 创建 `MediaTab` 而非 `DiffTab`。
//! 展示字段：格式/大小/时长/采样率/声道/位深/码率；差异字段红色标记，缺失字段灰色。

use crate::i18n::{t, Key as I18nKey};
use crate::mediacmp::{compare_media, MediaFieldDiff};
use eframe::egui::{self, Color32, RichText};

/// 媒体标签页
pub struct MediaTab {
    pub left: String,
    pub right: String,
    /// 字段级差异（compare_media 结果）
    pub diffs: Vec<MediaFieldDiff>,
    pub error: Option<String>,
}

impl MediaTab {
    pub fn new(left: &str, right: &str) -> Self {
        let mut t = MediaTab {
            left: left.to_string(),
            right: right.to_string(),
            diffs: Vec::new(),
            error: None,
        };
        t.load_pair(left, right);
        t
    }

    pub fn title(&self) -> String {
        format!(
            "{}: {} ↔ {}",
            t(I18nKey::SessionMedia),
            basename(&self.left),
            basename(&self.right)
        )
    }

    pub fn is_empty(&self) -> bool {
        self.left.is_empty() && self.right.is_empty()
    }

    /// 加载两侧媒体并对比元数据
    pub fn load_pair(&mut self, l: &str, r: &str) {
        self.left = l.to_string();
        self.right = r.to_string();
        if l.is_empty() || r.is_empty() {
            self.error = Some("需要左右两个媒体文件".to_string());
            return;
        }
        self.diffs = compare_media(l, r);
        self.error = None;
    }

    /// 左侧媒体信息（展示用）
    fn left_info(&self) -> std::collections::BTreeMap<&'static str, Option<String>> {
        crate::mediacmp::read_media_info(&self.left).fields()
    }

    /// 右侧媒体信息（展示用）
    fn right_info(&self) -> std::collections::BTreeMap<&'static str, Option<String>> {
        crate::mediacmp::read_media_info(&self.right).fields()
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        // 工具栏：重新加载 / 交换两侧
        egui::Panel::top("mediatab_tools").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui.button(format!("⟳ {}", t(I18nKey::Reload))).clicked() {
                    let (l, r) = (self.left.clone(), self.right.clone());
                    self.load_pair(&l, &r);
                }
                if ui.button(format!("⇄ {}", t(I18nKey::SwapSides))).clicked() {
                    std::mem::swap(&mut self.left, &mut self.right);
                    let (l, r) = (self.left.clone(), self.right.clone());
                    self.load_pair(&l, &r);
                }
            });
        });

        egui::CentralPanel::default().show(ui, |ui| {
            if self.is_empty() {
                ui.centered_and_justified(|ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(t(I18nKey::DiffEmptyHint))
                                .size(16.0)
                                .color(ui.visuals().weak_text_color()),
                        );
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            if ui.button(t(I18nKey::OpenLeft)).clicked() {
                                if let Some(p) = super::pick_file() {
                                    let (l, r) = (p, self.right.clone());
                                    self.load_pair(&l, &r);
                                }
                            }
                            if ui.button(t(I18nKey::OpenRight)).clicked() {
                                if let Some(p) = super::pick_file() {
                                    let (l, r) = (self.left.clone(), p);
                                    self.load_pair(&l, &r);
                                }
                            }
                        });
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new(t(I18nKey::DragHint))
                                .size(11.0)
                                .color(ui.visuals().weak_text_color()),
                        );
                    });
                });
                return;
            }
            if let Some(err) = &self.error {
                ui.colored_label(Color32::from_rgb(240, 110, 110), err);
            }
            let li = self.left_info();
            let ri = self.right_info();
            // 头部：左右文件名
            ui.horizontal(|ui| {
                ui.label(RichText::new(basename(&self.left)).strong().size(13.0));
                ui.separator();
                ui.label(RichText::new(basename(&self.right)).strong().size(13.0));
            });
            ui.separator();
            // 字段表：字段 | 左值 | 右值 | 状态
            egui::ScrollArea::vertical().show(ui, |ui| {
                let diff_map: std::collections::HashMap<&str, &MediaFieldDiff> =
                    self.diffs.iter().map(|d| (d.field.as_str(), d)).collect();
                egui::Grid::new("media_fields")
                    .num_columns(3)
                    .striped(true)
                    .show(ui, |ui| {
                        for (field, lv) in &li {
                            let rv = ri.get(field).cloned().flatten();
                            let is_diff = diff_map.contains_key(field);
                            ui.label(RichText::new(*field).strong());
                            let lv_disp = lv.clone().unwrap_or_else(|| "—".to_string());
                            let rv_disp = rv.clone().unwrap_or_else(|| "—".to_string());
                            if is_diff {
                                ui.colored_label(Color32::from_rgb(226, 110, 110), lv_disp);
                                ui.colored_label(Color32::from_rgb(226, 110, 110), rv_disp);
                            } else {
                                ui.monospace(lv_disp);
                                ui.monospace(rv_disp);
                            }
                            ui.end_row();
                        }
                    });
                ui.add_space(8.0);
                if self.diffs.is_empty() {
                    ui.label(RichText::new("✅ 元数据一致").color(ui.visuals().weak_text_color()));
                } else {
                    ui.label(
                        RichText::new(format!("⚠ {} 个字段不同", self.diffs.len()))
                            .color(Color32::from_rgb(226, 110, 110)),
                    );
                }
            });
        });
    }
}

fn basename(p: &str) -> String {
    std::path::Path::new(p)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.to_string())
}

/// P43-6：判断路径是否为媒体文件（容器头可识别或扩展名匹配）
pub fn is_media_file(path: &str) -> bool {
    let info = crate::mediacmp::read_media_info(path);
    if info.format.as_deref() == Some("unknown") {
        return false;
    }
    // 已知扩展名兜底（容器头识别不到的格式）
    let ext = std::path::Path::new(path)
        .extension()
        .map(|s| s.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    matches!(
        ext.as_str(),
        "wav" | "mp3" | "flac" | "ogg" | "m4a" | "aac" | "mp4" | "mkv" | "avi" | "mov" | "wmv"
    )
}
