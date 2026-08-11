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
    let (lw, lh) = left.dimensions();
    let (rw, rh) = right.dimensions();
    let size_differs = (lw, lh) != (rw, rh);
    let w = lw.min(rw);
    let h = lh.min(rh);

    let mut overlay = left.clone();
    let mut diff_pixels: u64 = 0;
    let mut total: u64 = 0;

    // 公共区域逐像素比较
    for y in 0..h {
        for x in 0..w {
            let lp = *left.get_pixel(x, y);
            let rp = *right.get_pixel(x, y);
            total += 1;
            if lp != rp {
                diff_pixels += 1;
                overlay.put_pixel(x, y, blend_overlay(lp, HIGHLIGHT_OVERLAY));
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
            }
        }
        for y in h..lh {
            for x in 0..w {
                diff_pixels += 1;
                total += 1;
                overlay.put_pixel(x, y, HIGHLIGHT);
            }
        }
    }

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
    };

    ImgPair {
        left,
        right,
        overlay,
        stats,
    }
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
}

/// CLI 入口：输出尺寸/差异统计，退出码 0=无差异，1=有差异，2=错误
pub fn run(args: &ImgcmpArgs) -> i32 {
    let pair = match compare_paths(&args.left, &args.right) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("bcr: {}", e);
            return 2;
        }
    };
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
