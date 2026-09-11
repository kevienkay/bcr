//! P43-6 媒体比较标签页（简化版）：并排显示左右音视频文件元数据 + 字段级差异高亮。
//!
//! 打开流程：`open_diff_files` / 拖放时检测两侧文件为音视频（mediacmp::read_media_info
//! 能识别容器头或扩展名匹配）→ 创建 `MediaTab` 而非 `DiffTab`。
//! 展示字段：格式/大小/时长/采样率/声道/位深/码率；差异字段红色标记，缺失字段灰色。

use super::{icons, widgets};
use crate::i18n::{t, Key as I18nKey};
use crate::mediacmp::{compare_media, MediaFieldDiff, PcmData};
use eframe::egui::{self, RichText};
use std::collections::BTreeMap;

/// 媒体标签页
pub struct MediaTab {
    pub left: String,
    pub right: String,
    /// 字段级差异（compare_media 结果）
    pub diffs: Vec<MediaFieldDiff>,
    pub error: Option<String>,
    /// P2：左侧 PCM 波形数据（仅 WAV 可解，其余格式为 None）
    pub wave_l: Option<PcmData>,
    /// P2：右侧 PCM 波形数据
    pub wave_r: Option<PcmData>,
}

impl MediaTab {
    pub fn new(left: &str, right: &str) -> Self {
        let mut t = MediaTab {
            left: left.to_string(),
            right: right.to_string(),
            diffs: Vec::new(),
            error: None,
            wave_l: None,
            wave_r: None,
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

    /// P58：打开左侧媒体文件（文件对话框），保留右侧（供原生菜单分发）。
    pub fn open_left(&mut self) {
        if let Some(p) = super::pick_file() {
            let (l, r) = (p, self.right.clone());
            self.load_pair(&l, &r);
        }
    }

    /// P58：打开右侧媒体文件（文件对话框），保留左侧。
    pub fn open_right(&mut self) {
        if let Some(p) = super::pick_file() {
            let (l, r) = (self.left.clone(), p);
            self.load_pair(&l, &r);
        }
    }

    /// 加载两侧媒体并对比元数据
    pub fn load_pair(&mut self, l: &str, r: &str) {
        self.left = l.to_string();
        self.right = r.to_string();
        self.wave_l = None;
        self.wave_r = None;
        if l.is_empty() || r.is_empty() {
            self.error = Some("需要左右两个媒体文件".to_string());
            return;
        }
        self.diffs = compare_media(l, r);
        // P2（BC 5.2.5）：真实波形——解析两侧 PCM（非 WAV/损坏文件 → None，不 panic）
        self.wave_l = crate::mediacmp::read_wav_pcm(l);
        self.wave_r = crate::mediacmp::read_wav_pcm(r);
        self.error = None;
    }

    /// P2：波形时长（优先左，其次右；均无则 None）
    pub(crate) fn wave_duration_secs(&self) -> Option<f64> {
        for p in [self.wave_l.as_ref(), self.wave_r.as_ref()].into_iter().flatten() {
            if !p.samples.is_empty() {
                return Some(p.duration_secs());
            }
        }
        None
    }

    /// P2：两侧波形包络差异区段数（列数对齐后逐列比较）
    pub(crate) fn wave_regions(&self) -> usize {
        match (&self.wave_l, &self.wave_r) {
            (Some(l), Some(r)) => {
                let n = l.samples.len().max(r.samples.len());
                if n == 0 {
                    return 0;
                }
                let cols = 512usize.min(n).max(1);
                let el = crate::mediacmp::downsample_envelope(&l.samples, cols);
                let er = crate::mediacmp::downsample_envelope(&r.samples, cols);
                wave_diff_regions(&el, &er)
            }
            _ => 0,
        }
    }

    /// P2：播放进度文案（无播放器，位置恒为 0）
    pub(crate) fn progress_text(&self) -> String {
        progress_label(0.0, self.wave_duration_secs())
    }

    /// 左侧媒体信息（展示用）
    fn left_info(&self) -> BTreeMap<&'static str, Option<String>> {
        crate::mediacmp::read_media_info(&self.left).fields()
    }

    /// 右侧媒体信息（展示用）
    fn right_info(&self) -> BTreeMap<&'static str, Option<String>> {
        crate::mediacmp::read_media_info(&self.right).fields()
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        // 工具栏：重新加载 / 交换两侧
        egui::Panel::top("mediatab_tools").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                if widgets::stack_button(ui, icons::Icon::Refresh, t(I18nKey::Reload), "", 15.0)
                    .clicked()
                {
                    let (l, r) = (self.left.clone(), self.right.clone());
                    self.load_pair(&l, &r);
                }
                if widgets::stack_button(
                    ui,
                    icons::Icon::Swap,
                    t(I18nKey::SwapSides),
                    "交换左右两侧",
                    15.0,
                )
                .clicked()
                {
                    std::mem::swap(&mut self.left, &mut self.right);
                    let (l, r) = (self.left.clone(), self.right.clone());
                    self.load_pair(&l, &r);
                }
            });
        });

        // P2（BC 5.2.5 状态栏第二行）：差异项数 · 播放进度 · 播放模式
        // 说明：全局 status_bar 由 mod.rs（P0）固定，这里以会话内底部条呈现同一组字段。
        egui::Panel::bottom("mediatab_status_row2")
            .default_size(super::theme::STATUSBAR_ROW_H)
            .resizable(false)
            .show(ui, |ui| self.status_row2(ui));

        egui::CentralPanel::default().show(ui, |ui| {
            if self.is_empty() {
                // P52-2：统一空状态（媒体用粉色系）
                super::common::empty_state(
                    ui,
                    "🎵",
                    super::theme::card_icon_colors()[6],
                    t(I18nKey::DiffEmptyHint),
                    t(I18nKey::DragHint),
                    |_ui| {},
                );
                return;
            }
            if let Some(err) = &self.error {
                ui.colored_label(super::theme::error_color(), err);
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
                            ui.label(RichText::new(*field).weak());
                            let lv_disp = lv.clone().unwrap_or_else(|| "—".to_string());
                            let rv_disp = rv.clone().unwrap_or_else(|| "—".to_string());
                            if is_diff {
                                // 差异值高亮（设计稿：左右值对照，差异值着色）
                                ui.colored_label(
                                    super::theme::diff_delete(ui.visuals().dark_mode),
                                    RichText::new(lv_disp).monospace(),
                                );
                                ui.colored_label(
                                    super::theme::diff_delete(ui.visuals().dark_mode),
                                    RichText::new(rv_disp).monospace(),
                                );
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
                            .color(super::theme::diff_delete(ui.visuals().dark_mode)),
                    );
                }
            });

            // P2（BC 5.2.5）：真实波形（PCM 降采样 min/max 包络，左右各一条）
            ui.separator();
            ui.label(RichText::new("波形").weak());
            let dark = ui.visuals().dark_mode;
            let wave_color = super::theme::accent(dark);
            for (side, pcm) in [("左", &self.wave_l), ("右", &self.wave_r)] {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(side).weak().size(11.0));
                    draw_wave(ui, pcm.as_ref(), 48.0, wave_color);
                });
            }
            if self.wave_l.is_none() && self.wave_r.is_none() {
                ui.label(
                    RichText::new("无音频数据（波形仅支持 WAV PCM；其他格式仅比对元数据）")
                        .weak()
                        .size(11.0),
                );
            }
        });
    }

    /// P2（BC 5.2.5 状态栏第二行）：差异项数 · 播放进度 · 播放模式
    fn status_row2(&self, ui: &mut egui::Ui) {
        let dark = ui.visuals().dark_mode;
        let full = ui.max_rect();
        ui.painter().rect_filled(full, 0.0, super::theme::bg_status(dark));
        ui.painter().hline(
            full.x_range(),
            full.top(),
            egui::Stroke::new(1.0, super::theme::mid_sep(dark)),
        );
        ui.horizontal_centered(|ui| {
            ui.add_space(8.0);
            let regions = self.wave_regions();
            ui.label(
                RichText::new(wave_regions_label(regions)).color(if regions > 0 {
                    super::theme::diff_modify(dark)
                } else {
                    ui.visuals().weak_text_color()
                }),
            );
            ui.separator();
            ui.label(self.progress_text());
            ui.separator();
            ui.label(PLAYBACK_MODE_LABEL);
        });
    }
}

// ===== P2（BC 5.2.5）：波形差异区段 / 状态栏第二行 =====

/// 波形差异判定阈值（包络 min/max 差超过该值视为该列有差异）
pub(crate) const WAVE_DIFF_EPS: f32 = 0.02;

/// 波形差异区段数（连续差异列合并为一段）
pub(crate) fn wave_diff_regions(a: &[(f32, f32)], b: &[(f32, f32)]) -> usize {
    let mut regions = 0usize;
    let mut in_region = false;
    for ((amn, amx), (bmn, bmx)) in a.iter().copied().zip(b.iter().copied()) {
        let diff = (amn - bmn).abs() > WAVE_DIFF_EPS || (amx - bmx).abs() > WAVE_DIFF_EPS;
        if diff && !in_region {
            regions += 1;
            in_region = true;
        } else if !diff {
            in_region = false;
        }
    }
    regions
}

/// 差异区段文案
pub(crate) fn wave_regions_label(n: usize) -> String {
    format!("波形差异区段 {n}")
}

/// 播放模式（并列显示；当前无音频输出）
pub(crate) const PLAYBACK_MODE_LABEL: &str = "并列播放";

/// 秒 → HH:MM:SS.mmm
pub(crate) fn timecode(secs: f64) -> String {
    let total_ms = (secs.max(0.0) * 1000.0).round() as u64;
    let ms = total_ms % 1000;
    let s = total_ms / 1000;
    format!("{}:{}:{}.{:03}", fmt2(s / 3600), fmt2((s / 60) % 60), fmt2(s % 60), ms)
}

fn fmt2(v: u64) -> String {
    format!("{v:02}")
}

/// 播放进度文案（无播放器 → 位置恒为 0；总时长未知时用占位符）
pub(crate) fn progress_label(pos: f64, total: Option<f64>) -> String {
    match total {
        Some(t) => format!("播放 {} / {}", timecode(pos), timecode(t)),
        None => format!("播放 {} / --:--:--.---", timecode(pos)),
    }
}

/// P2：绘制 PCM 波形（min/max 包络双折线）；无数据时降级为提示文字
fn draw_wave(ui: &mut egui::Ui, pcm: Option<&PcmData>, height: f32, color: egui::Color32) {
    let w = ui.available_width().max(80.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, height), egui::Sense::hover());
    let mid = rect.center().y;
    let dark = ui.visuals().dark_mode;
    ui.painter().line_segment(
        [egui::pos2(rect.left(), mid), egui::pos2(rect.right(), mid)],
        egui::Stroke::new(1.0, super::theme::mid_sep(dark)),
    );
    let samples: &[f32] = match pcm {
        Some(p) => &p.samples,
        None => &[],
    };
    if samples.is_empty() {
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "无音频数据",
            egui::FontId::proportional(11.0),
            ui.visuals().weak_text_color(),
        );
        return;
    }
    let columns = (rect.width() / 2.0).max(1.0) as usize;
    let env = crate::mediacmp::downsample_envelope(samples, columns);
    let amp = (rect.height() / 2.0 - 1.0).max(1.0);
    let stroke = egui::Stroke::new(1.0, color);
    let x_of = |i: usize| rect.left() + i as f32 * 2.0 + 1.0;
    let y_of = |v: f32| mid - v.clamp(-1.0, 1.0) * amp;
    let pts_max: Vec<egui::Pos2> = env
        .iter()
        .enumerate()
        .map(|(i, (_, mx))| egui::pos2(x_of(i), y_of(*mx)))
        .collect();
    let pts_min: Vec<egui::Pos2> = env
        .iter()
        .enumerate()
        .map(|(i, (mn, _))| egui::pos2(x_of(i), y_of(*mn)))
        .collect();
    for pts in [&pts_max, &pts_min] {
        for seg in pts.windows(2) {
            ui.painter().line_segment([seg[0], seg[1]], stroke);
        }
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

/// P2：媒体比较新增逻辑的纯函数单测（波形差异区段 / 时间码 / 进度文案）
#[cfg(test)]
mod p2_tests {
    use super::*;

    #[test]
    fn wave_diff_regions_merges_contiguous_columns() {
        let flat = vec![(0.0f32, 0.0f32); 10];
        assert_eq!(wave_diff_regions(&flat, &flat), 0, "完全相同无区段");
        // 单段差异（第 3..5 列）
        let mut b = flat.clone();
        for v in b.iter_mut().take(5).skip(3) {
            *v = (0.5, 0.9);
        }
        assert_eq!(wave_diff_regions(&flat, &b), 1);
        // 两段分离差异
        let mut c = flat.clone();
        c[1] = (0.5, 0.9);
        c[7] = (0.5, 0.9);
        assert_eq!(wave_diff_regions(&flat, &c), 2);
    }

    #[test]
    fn wave_diff_regions_below_epsilon_is_same() {
        let a = vec![(0.0f32, 0.0f32)];
        let b = vec![(0.001f32, 0.001f32)];
        assert_eq!(wave_diff_regions(&a, &b), 0, "阈值内视为相同");
        assert_eq!(wave_diff_regions(&[], &[]), 0, "空输入不 panic");
        // 长度不等时按较短者比较
        let long = vec![(0.0f32, 0.0f32); 4];
        assert_eq!(wave_diff_regions(&long, &[]), 0);
    }

    #[test]
    fn timecode_formats_hms_millis() {
        assert_eq!(timecode(0.0), "00:00:00.000");
        assert_eq!(timecode(0.86), "00:00:00.860");
        assert_eq!(timecode(1.5), "00:00:01.500");
        assert_eq!(timecode(3661.001), "01:01:01.001");
        assert_eq!(timecode(-5.0), "00:00:00.000", "负数截断为 0");
    }

    #[test]
    fn progress_label_with_and_without_duration() {
        assert_eq!(
            progress_label(0.0, Some(1.5)),
            "播放 00:00:00.000 / 00:00:01.500"
        );
        assert_eq!(
            progress_label(0.0, None),
            "播放 00:00:00.000 / --:--:--.---"
        );
    }

    #[test]
    fn status_row_labels_match_design() {
        assert_eq!(wave_regions_label(1), "波形差异区段 1");
        assert_eq!(PLAYBACK_MODE_LABEL, "并列播放");
        let eps = WAVE_DIFF_EPS;
        assert!((eps - 0.02).abs() < 1e-6, "差异阈值 0.02");
    }

    #[test]
    fn empty_pcm_has_zero_duration() {
        let p = PcmData {
            sample_rate: 44100,
            channels: 1,
            bits: 16,
            samples: Vec::new(),
        };
        assert_eq!(p.duration_secs(), 0.0);
    }
}
