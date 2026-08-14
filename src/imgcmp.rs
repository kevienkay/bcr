//! P8：图片对比 — 逐像素差异、差异掩码叠加、统计。
//!
//! 纯逻辑模块，CLI（`bcr imgcmp`）与 GUI（ImageTab）共用：
//! - 解码失败返回 Err(描述)，由上层决定回退 hex 视图
//! - 尺寸不同视为差异：公共区域逐像素比较，超出区域全部计为差异并染红
//! - `overlay` 为差异叠加图：相同区域保留左图，不同区域半透明红

use image::{Rgba, RgbaImage};

/// 差异统计
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DiffStats {
    pub left_w: u32,
    pub left_h: u32,
    pub right_w: u32,
    pub right_h: u32,
    /// 两侧尺寸不同
    pub size_differs: bool,
    /// 差异像素数（含尺寸不同时的超出区域）
    pub diff_pixels: u64,
    /// 参与比较的像素总数
    pub total_pixels: u64,
    /// 差异比例 0.0~1.0（total 为 0 时为 0）
    pub diff_ratio: f64,
    /// 差异像素包围盒（x, y, w, h，原始像素坐标；无差异时 None）
    pub bounds: Option<(u32, u32, u32, u32)>,
}

impl DiffStats {
    pub fn has_differences(&self) -> bool {
        self.size_differs || self.diff_pixels > 0
    }
}

/// 图片对比结果
pub struct ImgPair {
    pub left: RgbaImage,
    pub right: RgbaImage,
    /// 差异叠加图：相同区域 = 左图原样，不同区域 = 半透明红
    pub overlay: RgbaImage,
    pub stats: DiffStats,
}

/// 判断文件是否为受支持的图片格式（魔数嗅探：PNG/JPEG/GIF/WebP/BMP）
pub fn is_image_file(path: &str) -> bool {
    let Ok(mut f) = std::fs::File::open(path) else {
        return false;
    };
    let mut head = [0u8; 12];
    use std::io::Read;
    let n = match f.read(&mut head) {
        Ok(n) => n,
        Err(_) => return false,
    };
    let b = &head[..n];
    // PNG
    if b.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) {
        return true;
    }
    // JPEG
    if b.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return true;
    }
    // GIF87a / GIF89a
    if b.starts_with(b"GIF87a") || b.starts_with(b"GIF89a") {
        return true;
    }
    // WebP: RIFF....WEBP
    if b.len() >= 12 && &b[0..4] == b"RIFF" && &b[8..12] == b"WEBP" {
        return true;
    }
    // BMP
    if b.starts_with(b"BM") {
        return true;
    }
    false
}

/// P37-1k：图片格式名（魔数嗅探：PNG/JPEG/GIF/WebP/BMP；未知返回 "?"）
pub fn image_format_name(path: &str) -> String {
    let Ok(mut f) = std::fs::File::open(path) else {
        return "?".to_string();
    };
    let mut head = [0u8; 12];
    use std::io::Read;
    let n = match f.read(&mut head) {
        Ok(n) => n,
        Err(_) => return "?".to_string(),
    };
    let b = &head[..n];
    if b.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) {
        "PNG".to_string()
    } else if b.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "JPEG".to_string()
    } else if b.starts_with(b"GIF87a") || b.starts_with(b"GIF89a") {
        "GIF".to_string()
    } else if b.len() >= 12 && &b[0..4] == b"RIFF" && &b[8..12] == b"WEBP" {
        "WebP".to_string()
    } else if b.starts_with(b"BM") {
        "BMP".to_string()
    } else {
        "?".to_string()
    }
}

/// 解码全部帧（GIF/WebP 动图返回多帧；静态图返回单帧）
pub fn load_frames(data: &[u8], label: &str) -> Result<Vec<RgbaImage>, String> {
    use image::AnimationDecoder;
    let format =
        image::guess_format(data).map_err(|e| format!("{}: 图片格式识别失败: {}", label, e))?;
    match format {
        image::ImageFormat::Gif => {
            let dec = image::codecs::gif::GifDecoder::new(std::io::Cursor::new(data))
                .map_err(|e| format!("{}: GIF 解码失败: {}", label, e))?;
            let frames = dec
                .into_frames()
                .collect_frames()
                .map_err(|e| format!("{}: GIF 帧解码失败: {}", label, e))?;
            Ok(frames.into_iter().map(|f| f.into_buffer()).collect())
        }
        image::ImageFormat::WebP => {
            let dec = image::codecs::webp::WebPDecoder::new(std::io::Cursor::new(data))
                .map_err(|e| format!("{}: WebP 解码失败: {}", label, e))?;
            let frames = dec
                .into_frames()
                .collect_frames()
                .map_err(|e| format!("{}: WebP 帧解码失败: {}", label, e))?;
            Ok(frames.into_iter().map(|f| f.into_buffer()).collect())
        }
        _ => {
            let img = image::load_from_memory(data)
                .map_err(|e| format!("{}: 图片解码失败: {}", label, e))?;
            Ok(vec![img.to_rgba8()])
        }
    }
}

/// 解码字节为 RGBA 图（PNG/JPEG/GIF/WebP/BMP）
fn decode(data: &[u8], label: &str) -> Result<RgbaImage, String> {
    let img =
        image::load_from_memory(data).map_err(|e| format!("{}: 图片解码失败: {}", label, e))?;
    Ok(img.to_rgba8())
}

/// 两侧图片字节对比
pub fn compare_bytes(l: &[u8], r: &[u8]) -> Result<ImgPair, String> {
    let left = decode(l, "left")?;
    let right = decode(r, "right")?;
    Ok(compare_images(left, right))
}

/// 读取两侧图片文件并对比
pub fn compare_paths(l: &str, r: &str) -> Result<ImgPair, String> {
    let l = std::fs::read(l).map_err(|e| format!("读取 {} 失败: {}", l, e))?;
    let r = std::fs::read(r).map_err(|e| format!("读取 {} 失败: {}", r, e))?;
    compare_bytes(&l, &r)
}

/// 已解码 RGBA 图的像素级比较
pub fn compare_images(left: RgbaImage, right: RgbaImage) -> ImgPair {
    compare_images_opt(left, right, CompareOptions::default())
}

/// P37-1e：差异判定模式（BC Picture Compare 视图菜单）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DiffMode {
    /// 精确逐像素比较（默认）
    #[default]
    Exact,
    /// 容差模式：RGB 曼哈顿距离 ≤ tolerance 视为相同
    Tolerance,
    /// 不匹配范围模式：面积 < min_diff_area 的孤立差异块忽略
    MismatchRange,
    /// 混合模式：容差 + 忽略孤立块同时生效
    Mixed,
}

/// P37-1e：比较选项
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CompareOptions {
    pub mode: DiffMode,
    /// 容差阈值 0-255（Tolerance/Mixed 生效）
    pub tolerance: u8,
    /// 最小差异块面积（MismatchRange/Mixed 生效，像素数）
    pub min_diff_area: u32,
}

/// 两像素 RGB 曼哈顿距离
fn rgb_dist(a: &Rgba<u8>, b: &Rgba<u8>) -> u32 {
    (a[0] as i32 - b[0] as i32).unsigned_abs()
        + (a[1] as i32 - b[1] as i32).unsigned_abs()
        + (a[2] as i32 - b[2] as i32).unsigned_abs()
}

/// P37-1e：按选项做像素级比较
pub fn compare_images_opt(left: RgbaImage, right: RgbaImage, opts: CompareOptions) -> ImgPair {
    let (lw, lh) = left.dimensions();
    let (rw, rh) = right.dimensions();
    let size_differs = (lw, lh) != (rw, rh);
    let w = lw.min(rw);
    let h = lh.min(rh);

    let mut overlay = left.clone();
    let mut diff_pixels: u64 = 0;
    let mut total: u64 = 0;
    // 差异包围盒（原始像素坐标，闭区间外扩 1px 保证可见）
    let mut min_x: u32 = u32::MAX;
    let mut min_y: u32 = u32::MAX;
    let mut max_x: u32 = 0;
    let mut max_y: u32 = 0;
    // 差异像素标记（公共区域，供孤立块过滤）
    let mut diff_mask: Vec<bool> = vec![false; (w as usize) * (h as usize)];

    // 公共区域逐像素比较
    for y in 0..h {
        for x in 0..w {
            let lp = *left.get_pixel(x, y);
            let rp = *right.get_pixel(x, y);
            total += 1;
            let diff = match opts.mode {
                DiffMode::Exact => lp != rp,
                DiffMode::Tolerance | DiffMode::Mixed => {
                    lp != rp && rgb_dist(&lp, &rp) > opts.tolerance as u32
                }
                DiffMode::MismatchRange => lp != rp,
            };
            if diff {
                diff_mask[(y as usize) * (w as usize) + x as usize] = true;
            }
        }
    }

    // MismatchRange/Mixed：过滤孤立差异块（4-邻接连通域，面积 < min_diff_area 忽略）
    if matches!(opts.mode, DiffMode::MismatchRange | DiffMode::Mixed) && opts.min_diff_area > 0 {
        let mut visited: Vec<bool> = vec![false; (w as usize) * (h as usize)];
        for y in 0..h {
            for x in 0..w {
                let idx = (y as usize) * (w as usize) + x as usize;
                if !diff_mask[idx] || visited[idx] {
                    continue;
                }
                // BFS 收集连通块
                let mut stack = vec![(x, y)];
                visited[idx] = true;
                let mut area = 0u32;
                let mut cells: Vec<(u32, u32)> = Vec::new();
                while let Some((cx, cy)) = stack.pop() {
                    area += 1;
                    cells.push((cx, cy));
                    for (nx, ny) in [
                        (cx.wrapping_sub(1), cy),
                        (cx + 1, cy),
                        (cx, cy.wrapping_sub(1)),
                        (cx, cy + 1),
                    ] {
                        if nx < w && ny < h {
                            let ni = (ny as usize) * (w as usize) + nx as usize;
                            if diff_mask[ni] && !visited[ni] {
                                visited[ni] = true;
                                stack.push((nx, ny));
                            }
                        }
                    }
                }
                if area < opts.min_diff_area {
                    // 忽略该块：清除差异标记
                    for (cx, cy) in cells {
                        diff_mask[(cy as usize) * (w as usize) + cx as usize] = false;
                    }
                }
            }
        }
    }

    // 汇总差异（公共区域）
    for y in 0..h {
        for x in 0..w {
            if diff_mask[(y as usize) * (w as usize) + x as usize] {
                diff_pixels += 1;
                let lp = *left.get_pixel(x, y);
                overlay.put_pixel(x, y, blend_overlay(lp, HIGHLIGHT_OVERLAY));
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
    }

    // 尺寸不同：左侧超出区域（右侧无对应像素）全部计为差异并染红
    if size_differs {
        for y in 0..lh {
            for x in w..lw {
                diff_pixels += 1;
                total += 1;
                overlay.put_pixel(x, y, HIGHLIGHT);
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
        for y in h..lh {
            for x in 0..w {
                diff_pixels += 1;
                total += 1;
                overlay.put_pixel(x, y, HIGHLIGHT);
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
    }

    // 差异包围盒（无差异时 None）
    let bounds = if diff_pixels > 0 {
        let bx = min_x.saturating_sub(1);
        let by = min_y.saturating_sub(1);
        let bw = (max_x + 2).min(lw.max(rw)) - bx;
        let bh = (max_y + 2).min(lh.max(rh)) - by;
        Some((bx, by, bw, bh))
    } else {
        None
    };

    let stats = DiffStats {
        left_w: lw,
        left_h: lh,
        right_w: rw,
        right_h: rh,
        size_differs,
        diff_pixels,
        total_pixels: total,
        diff_ratio: if total > 0 {
            diff_pixels as f64 / total as f64
        } else {
            0.0
        },
        bounds,
    };

    ImgPair {
        left,
        right,
        overlay,
        stats,
    }
}

/// P37-1e：旋转图像（0/90/180/270，顺时针）
pub fn rotate_image(img: &RgbaImage, deg: u32) -> RgbaImage {
    match deg % 360 {
        90 => {
            let (w, h) = img.dimensions();
            let mut out = RgbaImage::new(h, w);
            for y in 0..h {
                for x in 0..w {
                    out.put_pixel(y, w - 1 - x, *img.get_pixel(x, y));
                }
            }
            out
        }
        180 => {
            let (w, h) = img.dimensions();
            let mut out = RgbaImage::new(w, h);
            for y in 0..h {
                for x in 0..w {
                    out.put_pixel(w - 1 - x, h - 1 - y, *img.get_pixel(x, y));
                }
            }
            out
        }
        270 => {
            let (w, h) = img.dimensions();
            let mut out = RgbaImage::new(h, w);
            for y in 0..h {
                for x in 0..w {
                    out.put_pixel(h - 1 - y, x, *img.get_pixel(x, y));
                }
            }
            out
        }
        _ => img.clone(),
    }
}

/// P37-1e：翻转图像（horizontal=true 水平镜像，false 垂直镜像）
pub fn flip_image(img: &RgbaImage, horizontal: bool) -> RgbaImage {
    let (w, h) = img.dimensions();
    let mut out = img.clone();
    if horizontal {
        for y in 0..h {
            for x in 0..w / 2 {
                let p = *out.get_pixel(x, y);
                out.put_pixel(x, y, *out.get_pixel(w - 1 - x, y));
                out.put_pixel(w - 1 - x, y, p);
            }
        }
    } else {
        for y in 0..h / 2 {
            for x in 0..w {
                let p = *out.get_pixel(x, y);
                out.put_pixel(x, y, *out.get_pixel(x, h - 1 - y));
                out.put_pixel(x, h - 1 - y, p);
            }
        }
    }
    out
}

/// 差异高亮色：不透明红（尺寸超出区域）或作为叠加层 alpha 混合
const HIGHLIGHT: Rgba<u8> = Rgba([255, 40, 40, 255]);
/// 叠加层红色（半透明）
const HIGHLIGHT_OVERLAY: Rgba<u8> = Rgba([255, 40, 40, 150]);

// ---------- CLI ----------

/// `bcr imgcmp` 子命令参数
#[derive(clap::Args, Debug)]
pub struct ImgcmpArgs {
    /// 左侧图片
    pub left: String,

    /// 右侧图片
    pub right: String,

    /// 同时显示相同像素（默认只输出差异统计）
    #[arg(long)]
    pub show_same: bool,

    /// 输出统计信息
    #[arg(long)]
    pub summary: bool,

    /// 颜色输出：auto | always | never
    #[arg(long, default_value = "auto", value_parser = ["auto", "always", "never"])]
    pub color: String,

    /// 以 JSON 契约输出结果（schema: imgcmp.v1）
    #[arg(long)]
    pub json: bool,
}

/// CLI 入口：输出尺寸/差异统计，退出码 0=无差异，1=有差异，2=错误
pub fn run(args: &ImgcmpArgs) -> i32 {
    let pair = match compare_paths(&args.left, &args.right) {
        Ok(p) => p,
        Err(e) => {
            if args.json {
                println!(
                    "{}",
                    serde_json::to_string(&crate::jsonout::error_envelope(
                        "imgcmp.v1",
                        "imgcmp",
                        &e.to_string(),
                    ))
                    .unwrap_or_default()
                );
            }
            eprintln!("bcr: {}", e);
            return 2;
        }
    };

    // JSON 契约输出(imgcmp.v1)
    if args.json {
        let s = pair.stats;
        let v = crate::jsonout::envelope_imgcmp(
            &args.left,
            &args.right,
            s.left_w,
            s.left_h,
            s.right_w,
            s.right_h,
            s.size_differs,
            s.diff_pixels,
            s.total_pixels,
            s.diff_ratio,
            s.bounds,
        );
        println!("{}", serde_json::to_string(&v).unwrap_or_default());
        return if s.has_differences() { 1 } else { 0 };
    }

    let s = pair.stats;
    let use_color = match args.color.as_str() {
        "always" => true,
        "never" => false,
        _ => std::io::IsTerminal::is_terminal(&std::io::stdout()),
    };
    let (w1, w2, w3, w4) = if use_color {
        ("\x1b[1;31m", "\x1b[0m", "\x1b[1;32m", "\x1b[0m")
    } else {
        ("", "", "", "")
    };
    // 差异行
    let status = if s.has_differences() {
        "[DIFF]"
    } else {
        "[SAME]"
    };
    let status = if use_color {
        if s.has_differences() {
            format!("{}{}{}", w1, status, w2)
        } else {
            format!("{}{}{}", w3, status, w4)
        }
    } else {
        status.to_string()
    };
    println!(
        "{} {} {}x{} -> {}x{}",
        status,
        if s.size_differs { "(size differs)" } else { "" },
        s.left_w,
        s.left_h,
        s.right_w,
        s.right_h
    );
    if args.show_same || s.has_differences() || args.summary {
        println!(
            "差异像素: {} / {} ({:.2}%)",
            s.diff_pixels,
            s.total_pixels,
            s.diff_ratio * 100.0
        );
    }
    if s.has_differences() {
        1
    } else {
        0
    }
}

/// 在 base 上叠加半透明红色
fn blend_overlay(base: Rgba<u8>, red: Rgba<u8>) -> Rgba<u8> {
    let a = red[3] as u32;
    let inv = 255 - a;
    Rgba([
        ((red[0] as u32 * a + base[0] as u32 * inv) / 255) as u8,
        ((red[1] as u32 * a + base[1] as u32 * inv) / 255) as u8,
        ((red[2] as u32 * a + base[2] as u32 * inv) / 255) as u8,
        255,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造纯色 RGBA 图
    fn solid(w: u32, h: u32, rgba: [u8; 4]) -> RgbaImage {
        RgbaImage::from_pixel(w, h, Rgba(rgba))
    }

    #[test]
    fn identical_images_no_diff() {
        let a = solid(4, 4, [10, 20, 30, 255]);
        let b = solid(4, 4, [10, 20, 30, 255]);
        let p = compare_images(a, b);
        assert_eq!(p.stats.diff_pixels, 0);
        assert!(!p.stats.has_differences());
        assert_eq!(p.stats.diff_ratio, 0.0);
    }

    #[test]
    fn single_pixel_diff_detected() {
        let mut a = solid(3, 3, [0, 0, 0, 255]);
        a.put_pixel(1, 1, Rgba([255, 255, 255, 255]));
        let b = solid(3, 3, [0, 0, 0, 255]);
        let p = compare_images(a, b);
        assert_eq!(p.stats.diff_pixels, 1);
        assert_eq!(p.stats.total_pixels, 9);
        assert!(p.stats.has_differences());
    }

    #[test]
    fn size_mismatch_counts_all_extra() {
        let a = solid(4, 4, [1, 1, 1, 255]);
        let b = solid(2, 2, [1, 1, 1, 255]);
        let p = compare_images(a, b);
        assert!(p.stats.size_differs);
        // 公共 2x2 相同；左侧超出 12 像素计差异
        assert_eq!(p.stats.diff_pixels, 12);
        assert_eq!(p.stats.total_pixels, 16);
        // 超出区域染红
        assert_eq!(*p.overlay.get_pixel(3, 3), HIGHLIGHT);
    }

    #[test]
    fn alpha_difference_counts() {
        let a = solid(2, 2, [10, 10, 10, 255]);
        let b = solid(2, 2, [10, 10, 10, 254]);
        let p = compare_images(a, b);
        assert_eq!(p.stats.diff_pixels, 4);
    }

    #[test]
    fn overlay_preserves_same_region() {
        let a = solid(2, 2, [7, 8, 9, 255]);
        let b = solid(2, 2, [7, 8, 9, 255]);
        let p = compare_images(a, b);
        assert_eq!(*p.overlay.get_pixel(0, 0), Rgba([7, 8, 9, 255]));
    }

    #[test]
    fn png_roundtrip_compare() {
        // 编码一张 2x2 PNG 与同内容 PNG 对比 → 无差异
        let img = solid(2, 2, [5, 6, 7, 255]);
        let mut buf = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut buf, image::ImageFormat::Png)
            .unwrap();
        let p = compare_bytes(buf.get_ref(), buf.get_ref()).unwrap();
        assert_eq!(p.stats.diff_pixels, 0);
    }

    #[test]
    fn invalid_bytes_error() {
        assert!(compare_bytes(b"not an image", b"also not").is_err());
    }

    // ---- P37-1e：旋转 / 翻转 ----------------

    #[test]
    fn rotate_90_swaps_dimensions_and_pixels() {
        // 2x1 图：左像素红、右像素蓝 → 顺时针 90° 后 1x2：上蓝下红
        let mut a = RgbaImage::new(2, 1);
        a.put_pixel(0, 0, Rgba([255, 0, 0, 255]));
        a.put_pixel(1, 0, Rgba([0, 0, 255, 255]));
        let r = rotate_image(&a, 90);
        assert_eq!(r.dimensions(), (1, 2));
        assert_eq!(*r.get_pixel(0, 0), Rgba([0, 0, 255, 255]));
        assert_eq!(*r.get_pixel(0, 1), Rgba([255, 0, 0, 255]));
    }

    #[test]
    fn rotate_180_and_270() {
        let mut a = RgbaImage::new(2, 2);
        a.put_pixel(0, 0, Rgba([1, 0, 0, 255]));
        a.put_pixel(1, 0, Rgba([2, 0, 0, 255]));
        a.put_pixel(0, 1, Rgba([3, 0, 0, 255]));
        a.put_pixel(1, 1, Rgba([4, 0, 0, 255]));
        let r180 = rotate_image(&a, 180);
        assert_eq!(*r180.get_pixel(0, 0), Rgba([4, 0, 0, 255]));
        assert_eq!(*r180.get_pixel(1, 1), Rgba([1, 0, 0, 255]));
        let r270 = rotate_image(&a, 270);
        assert_eq!(r270.dimensions(), (2, 2));
        // 270 = 顺时针 3 次：角点映射验证
        assert_eq!(*r270.get_pixel(0, 0), Rgba([3, 0, 0, 255]));
    }

    #[test]
    fn rotate_0_is_identity() {
        let a = RgbaImage::from_pixel(3, 2, Rgba([9, 9, 9, 255]));
        let r = rotate_image(&a, 0);
        assert_eq!(r.dimensions(), (3, 2));
        assert_eq!(r.as_raw(), a.as_raw());
    }

    #[test]
    fn flip_horizontal_mirrors() {
        let mut a = RgbaImage::new(2, 1);
        a.put_pixel(0, 0, Rgba([10, 0, 0, 255]));
        a.put_pixel(1, 0, Rgba([20, 0, 0, 255]));
        let f = flip_image(&a, true);
        assert_eq!(*f.get_pixel(0, 0), Rgba([20, 0, 0, 255]));
        assert_eq!(*f.get_pixel(1, 0), Rgba([10, 0, 0, 255]));
    }

    #[test]
    fn flip_vertical_mirrors() {
        let mut a = RgbaImage::new(1, 2);
        a.put_pixel(0, 0, Rgba([30, 0, 0, 255]));
        a.put_pixel(0, 1, Rgba([40, 0, 0, 255]));
        let f = flip_image(&a, false);
        assert_eq!(*f.get_pixel(0, 0), Rgba([40, 0, 0, 255]));
        assert_eq!(*f.get_pixel(0, 1), Rgba([30, 0, 0, 255]));
    }

    // ---- P37-1e：容差 / 不匹配范围 / 混合 ----------------

    #[test]
    fn tolerance_ignores_small_color_delta() {
        let a = solid(2, 2, [100, 100, 100, 255]);
        let b = solid(2, 2, [102, 100, 100, 255]); // 差值 2
                                                   // 精确模式：有差异
        let exact = compare_images(a.clone(), b.clone());
        assert_eq!(exact.stats.diff_pixels, 4);
        // 容差 3：视为相同
        let tol = compare_images_opt(
            a.clone(),
            b.clone(),
            CompareOptions {
                mode: DiffMode::Tolerance,
                tolerance: 3,
                min_diff_area: 0,
            },
        );
        assert_eq!(tol.stats.diff_pixels, 0, "容差内应无差异");
        // 容差 1：仍有差异
        let strict = compare_images_opt(
            a.clone(),
            b.clone(),
            CompareOptions {
                mode: DiffMode::Tolerance,
                tolerance: 1,
                min_diff_area: 0,
            },
        );
        assert_eq!(strict.stats.diff_pixels, 4);
    }

    #[test]
    fn mismatch_range_ignores_isolated_blocks() {
        // 4x4 黑色图；b 在 (0,0) 单像素差异 + 右下 2x2 块差异
        let a = solid(4, 4, [0, 0, 0, 255]);
        let mut b = solid(4, 4, [0, 0, 0, 255]);
        b.put_pixel(0, 0, Rgba([255, 255, 255, 255]));
        for y in 2..4 {
            for x in 2..4 {
                b.put_pixel(x, y, Rgba([255, 255, 255, 255]));
            }
        }
        // min_diff_area=2：忽略面积 1 的孤立像素，保留 2x2 块（面积 4）
        let p = compare_images_opt(
            a,
            b,
            CompareOptions {
                mode: DiffMode::MismatchRange,
                tolerance: 0,
                min_diff_area: 2,
            },
        );
        assert_eq!(p.stats.diff_pixels, 4, "仅保留 2x2 块");
        // 精确模式：5 像素
        let a2 = solid(4, 4, [0, 0, 0, 255]);
        let mut b2 = solid(4, 4, [0, 0, 0, 255]);
        b2.put_pixel(0, 0, Rgba([255, 255, 255, 255]));
        for y in 2..4 {
            for x in 2..4 {
                b2.put_pixel(x, y, Rgba([255, 255, 255, 255]));
            }
        }
        let exact = compare_images(a2, b2);
        assert_eq!(exact.stats.diff_pixels, 5);
    }

    #[test]
    fn mixed_mode_combines_tolerance_and_area() {
        // 1 个孤立小差异（容差内）+ 1 个孤立大差异（容差外）→ 混合模式两者都忽略（面积均 < min）
        let a = solid(6, 1, [100, 100, 100, 255]);
        let mut b = solid(6, 1, [100, 100, 100, 255]);
        b.put_pixel(0, 0, Rgba([101, 100, 100, 255])); // 容差内
        b.put_pixel(5, 0, Rgba([0, 0, 0, 255])); // 大差异，面积 1
        let p = compare_images_opt(
            a,
            b,
            CompareOptions {
                mode: DiffMode::Mixed,
                tolerance: 3,
                min_diff_area: 2,
            },
        );
        // 两处差异面积均 < 2 → 全部忽略
        assert_eq!(p.stats.diff_pixels, 0, "混合模式应忽略小面积差异块");
    }
}

#[cfg(test)]
mod frame_tests {
    use super::*;

    /// 生成 2 帧 GIF（用 image crate 编码）
    fn make_2frame_gif() -> Vec<u8> {
        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut encoder = image::codecs::gif::GifEncoder::new(&mut buf);
            let f1 = RgbaImage::from_pixel(2, 2, Rgba([10, 20, 30, 255]));
            let f2 = RgbaImage::from_pixel(2, 2, Rgba([40, 50, 60, 255]));
            encoder.encode_frame(image::Frame::new(f1)).unwrap();
            encoder.encode_frame(image::Frame::new(f2)).unwrap();
        }
        buf.into_inner()
    }

    #[test]
    fn gif_multiple_frames_loaded() {
        let gif = make_2frame_gif();
        let frames = load_frames(&gif, "gif").unwrap();
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].dimensions(), (2, 2));
        assert_ne!(frames[0].as_raw(), frames[1].as_raw());
    }

    #[test]
    fn static_png_single_frame() {
        let img = RgbaImage::from_pixel(2, 2, Rgba([1, 2, 3, 255]));
        let mut buf = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut buf, image::ImageFormat::Png)
            .unwrap();
        let frames = load_frames(buf.get_ref(), "png").unwrap();
        assert_eq!(frames.len(), 1);
    }

    #[test]
    fn invalid_bytes_frame_error() {
        assert!(load_frames(b"not an image", "x").is_err());
    }
}

#[cfg(test)]
mod bounds_tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn diff_bounds_cover_diff_region() {
        // 4x4 图,右下角 2x2 区域不同
        let mut a = RgbaImage::from_pixel(4, 4, Rgba([0, 0, 0, 255]));
        let b = RgbaImage::from_pixel(4, 4, Rgba([0, 0, 0, 255]));
        for y in 2..4 {
            for x in 2..4 {
                a.put_pixel(x, y, Rgba([255, 255, 255, 255]));
            }
        }
        let p = compare_images(a, b);
        let (bx, by, bw, bh) = p.stats.bounds.unwrap();
        // 包围盒应包含 (2,2)-(3,3) 并外扩 1px
        assert!(bx <= 2 && by <= 2);
        assert!(bx + bw >= 4 && by + bh >= 4);
    }

    #[test]
    fn no_diff_no_bounds() {
        let a = RgbaImage::from_pixel(3, 3, Rgba([1, 2, 3, 255]));
        let b = RgbaImage::from_pixel(3, 3, Rgba([1, 2, 3, 255]));
        let p = compare_images(a, b);
        assert!(p.stats.bounds.is_none());
    }

    // ---- P37-1k：元数据格式名 ----

    #[test]
    fn image_format_name_detects_magic() {
        let d = tempdir().unwrap();
        // 写一个合法 PNG 魔数
        let png = d.path().join("a.png");
        std::fs::write(
            &png,
            [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 0],
        )
        .unwrap();
        assert_eq!(image_format_name(png.to_str().unwrap()), "PNG".to_string());
        // 未知内容 → ?
        let txt = d.path().join("x.bin");
        std::fs::write(&txt, b"not an image").unwrap();
        assert_eq!(image_format_name(txt.to_str().unwrap()), "?".to_string());
        // 不存在文件 → ?
        assert_eq!(image_format_name("/nonexistent/x.png"), "?".to_string());
    }
}
