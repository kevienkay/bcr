//! P43-6：媒体比较（简化版）——音视频文件元数据对比（自研容器头解析，无外部依赖）。
//!
//! 解析常见容器头部提取基础元数据：
//! - WAV（RIFF + fmt 块）：声道/采样率/位深/字节率 → 时长 = data 大小 / 字节率
//! - MP3（MPEG 帧头同步字）：码率表估算 → 时长 ≈ 文件大小 / 码率
//! - FLAC（fLaC + STREAMINFO）：采样率/声道/位深/总采样数 → 时长
//! - 其他格式：退化为 文件大小 + 扩展名
//!
//! 字段级对比（与 P24 mp3tag 风格一致）：同字段不同 → 差异，缺字段 → 缺失。

use std::collections::BTreeMap;
use std::io::Read;

/// 媒体文件元数据（可解析出的字段，未解析为 None）
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MediaInfo {
    /// 容器/格式名（"WAV"/"MP3"/"FLAC"/"unknown"）
    pub format: Option<String>,
    pub size: u64,
    /// 时长（秒）
    pub duration_secs: Option<u64>,
    /// 采样率（Hz）
    pub sample_rate: Option<u32>,
    /// 声道数
    pub channels: Option<u16>,
    /// 位深
    pub bit_depth: Option<u16>,
    /// 码率（bps）
    pub bitrate: Option<u64>,
}

impl MediaInfo {
    /// 全部字段（含空值），用于遍历输出
    pub fn fields(&self) -> BTreeMap<&'static str, Option<String>> {
        let mut m = BTreeMap::new();
        m.insert("format", self.format.clone());
        m.insert("size", Some(self.size.to_string()));
        m.insert("duration", self.duration_secs.map(|s| format!("{}s", s)));
        m.insert("sample_rate", self.sample_rate.map(|v| format!("{} Hz", v)));
        m.insert("channels", self.channels.map(|v| format!("{}", v)));
        m.insert("bit_depth", self.bit_depth.map(|v| format!("{} bit", v)));
        m.insert(
            "bitrate",
            self.bitrate.map(|v| format!("{} kbps", v / 1000)),
        );
        m
    }
}

/// 读取媒体文件元数据（前 64KB 扫描容器头）
pub fn read_media_info(path: &str) -> MediaInfo {
    let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let mut head = vec![0u8; 65536];
    let n = std::fs::File::open(path)
        .and_then(|mut f| f.read(&mut head))
        .unwrap_or(0);
    head.truncate(n);
    let mut info = MediaInfo {
        format: None,
        size,
        ..Default::default()
    };
    if head.starts_with(b"RIFF") && &head[8..12] == b"WAVE" {
        parse_wav(&head, &mut info);
    } else if head.starts_with(b"fLaC") {
        parse_flac(&head, &mut info);
    } else if head.starts_with(b"ID3") || is_mp3_frame(&head) {
        info.format = Some("MP3".to_string());
        estimate_mp3(&head, size, &mut info);
    } else {
        info.format = Some("unknown".to_string());
    }
    info
}

/// WAV：RIFF + fmt 块（音频格式 2B + 声道 2B + 采样率 4B + 字节率 4B + 位深 2B）+ data 大小
fn parse_wav(head: &[u8], info: &mut MediaInfo) {
    info.format = Some("WAV".to_string());
    // 找 fmt 块
    let mut pos = 12usize;
    while pos + 8 <= head.len() {
        let id = &head[pos..pos + 4];
        let len = u32::from_le_bytes([head[pos + 4], head[pos + 5], head[pos + 6], head[pos + 7]])
            as usize;
        if id == b"fmt " && pos + 8 + 16 <= head.len() {
            info.channels = Some(u16::from_le_bytes([head[pos + 10], head[pos + 11]]));
            info.sample_rate = Some(u32::from_le_bytes([
                head[pos + 12],
                head[pos + 13],
                head[pos + 14],
                head[pos + 15],
            ]));
            info.bit_depth = Some(u16::from_le_bytes([head[pos + 22], head[pos + 23]]));
            let byte_rate = u32::from_le_bytes([
                head[pos + 16],
                head[pos + 17],
                head[pos + 18],
                head[pos + 19],
            ]);
            info.bitrate = Some(byte_rate as u64 * 8);
        }
        if id == b"data" {
            info.duration_secs = Some(len as u64 / byte_rate(head, pos).max(1) as u64);
            break;
        }
        pos += 8 + len + (len & 1); // RIFF 块按 2 字节对齐
    }
}

fn byte_rate(head: &[u8], data_pos: usize) -> u32 {
    // 复用 fmt 块里的字节率（若已在 info.bitrate 存了 *8，这里重新解析）
    let mut pos = 12usize;
    while pos + 8 <= data_pos.min(head.len()) {
        let id = &head[pos..pos + 4];
        let len = u32::from_le_bytes([head[pos + 4], head[pos + 5], head[pos + 6], head[pos + 7]])
            as usize;
        if id == b"fmt " && pos + 8 + 20 <= head.len() {
            return u32::from_le_bytes([
                head[pos + 16],
                head[pos + 17],
                head[pos + 18],
                head[pos + 19],
            ]);
        }
        pos += 8 + len + (len & 1);
    }
    1
}

/// FLAC：fLaC 标记 + STREAMINFO（最小块：采样率 20bit / 声道 3bit / 位深 5bit / 总采样数 36bit）
fn parse_flac(head: &[u8], info: &mut MediaInfo) {
    info.format = Some("FLAC".to_string());
    // 第一个 metadata block：1B 头（0x80|类型 + 24bit 长度）
    if head.len() < 4 + 4 + 34 {
        return;
    }
    let si = &head[4 + 4..];
    if si.len() < 18 {
        return;
    }
    // STREAMINFO 18 字节：
    // 采样率 = [0..2] 20bit（0x0FFFFF）
    let sr_raw = ((si[0] as u32) << 12) | ((si[1] as u32) << 4) | ((si[2] as u32) >> 4);
    info.sample_rate = Some(sr_raw);
    // 声道 = [2] 高 3bit + 1
    info.channels = Some(((si[2] >> 1) & 0x07) as u16 + 1);
    // 位深 = [2..3] 低 5bit + 1
    info.bit_depth = Some((((si[2] & 0x01) << 4) | (si[3] >> 4)) as u16 + 1);
    // 总采样数 = [4..7] 36bit
    let total = ((si[4] as u64) << 32)
        | ((si[5] as u64) << 24)
        | ((si[6] as u64) << 16)
        | ((si[7] as u64) << 8)
        | (si[8] as u64);
    if sr_raw > 0 {
        info.duration_secs = Some(total / sr_raw as u64);
        info.bitrate = Some(info.size * 8 / total.max(1));
    }
}

/// 判断是否为 MPEG 帧头（11bit 同步字 0xFFE）
fn is_mp3_frame(head: &[u8]) -> bool {
    if head.len() < 4 {
        return false;
    }
    let b0 = head[0];
    let b1 = head[1];
    (b0 == 0xFF) && (b1 & 0xE0) == 0xE0
}

/// MP3 码率表（kbps，按 版本/层 索引；简化取 V1/L3）
const MP3_BITRATES: [u32; 16] = [
    0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 0,
];

/// MP3：从第一个帧头取码率，时长 ≈ 文件大小 / 码率（CBR 近似）
fn estimate_mp3(head: &[u8], size: u64, info: &mut MediaInfo) {
    if head.len() < 4 {
        return;
    }
    let b2 = head[2];
    let idx = ((b2 >> 4) & 0x0F) as usize;
    let kbps = MP3_BITRATES.get(idx).copied().unwrap_or(0) * 1000;
    if kbps > 0 {
        info.bitrate = Some(kbps as u64);
        // 采样率（V1）：bits 3-2：00=44100 01=48000 10=32000
        let sr = match (b2 >> 2) & 0x03 {
            0 => 44100,
            1 => 48000,
            2 => 32000,
            _ => 0,
        };
        if sr > 0 {
            info.sample_rate = Some(sr);
        }
        // 声道：V1 第 4 位（b3 bit6）
        info.channels = Some(if (head[3] & 0x80) == 0 { 2 } else { 1 });
        info.duration_secs = Some(size * 8 / kbps as u64);
    }
}

/// 两文件媒体元数据对比：返回字段级差异（与 mp3tag 风格一致）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaFieldDiff {
    pub field: String,
    pub left: Option<String>,
    pub right: Option<String>,
}

/// 对比两个媒体文件，返回差异字段列表（仅列出不同的字段）
pub fn compare_media(left: &str, right: &str) -> Vec<MediaFieldDiff> {
    let l = read_media_info(left);
    let r = read_media_info(right);
    let lf = l.fields();
    let rf = r.fields();
    let mut out = Vec::new();
    for (k, lv) in &lf {
        let rv = rf.get(k).cloned().flatten();
        if lv.as_deref() != rv.as_deref() {
            out.push(MediaFieldDiff {
                field: (*k).to_string(),
                left: lv.clone(),
                right: rv,
            });
        }
    }
    out
}

// ===== P2（BC 5.2.5）：真实波形（PCM 降采样 min/max 包络）=====

/// 单声道 PCM 采样（f32，约 -1.0 ~ 1.0）
#[derive(Debug, Clone, PartialEq)]
pub struct PcmData {
    pub sample_rate: u32,
    pub channels: u16,
    pub bits: u16,
    pub samples: Vec<f32>,
}

impl PcmData {
    /// 时长（秒）；无采样或采样率缺失时为 0
    pub fn duration_secs(&self) -> f64 {
        if self.sample_rate == 0 {
            0.0
        } else {
            self.samples.len() as f64 / self.sample_rate as f64
        }
    }
}

/// 单个采样字节 → f32（按位深；越界返回 None）
fn decode_sample(b: &[u8], bits: u16, is_float: bool) -> Option<f32> {
    match (bits, is_float) {
        (8, _) => Some((*b.first()? as f32 - 128.0) / 128.0),
        (16, false) => {
            let v = i16::from_le_bytes([*b.first()?, *b.get(1)?]);
            Some(v as f32 / 32768.0)
        }
        (24, false) => {
            let raw = (*b.first()? as i32)
                | ((*b.get(1)? as i32) << 8)
                | ((*b.get(2)? as i32) << 16);
            // 24 → 32 位符号扩展
            let v = (raw << 8) >> 8;
            Some(v as f32 / 8_388_608.0)
        }
        (32, true) => {
            let v = f32::from_le_bytes([*b.first()?, *b.get(1)?, *b.get(2)?, *b.get(3)?]);
            if v.is_finite() {
                Some(v)
            } else {
                None
            }
        }
        (32, false) => {
            let v = i32::from_le_bytes([*b.first()?, *b.get(1)?, *b.get(2)?, *b.get(3)?]);
            Some(v as f32 / 2_147_483_648.0)
        }
        _ => None,
    }
}

/// 解析 WAV（RIFF）为 PCM 采样。
/// 仅支持未压缩 PCM（fmt 的 audio_format = 1）与 IEEE float（= 3）；
/// 多声道按平均混为单声道；格式不支持/文件损坏/路径不可读一律返回 None（不 panic）。
pub fn read_wav_pcm(path: &str) -> Option<PcmData> {
    let data = std::fs::read(path).ok()?;
    read_wav_pcm_bytes(&data)
}

/// 从字节解析 WAV PCM（供单测直接构造字节流）
pub fn read_wav_pcm_bytes(data: &[u8]) -> Option<PcmData> {
    if data.len() < 12 || &data[0..4] != b"RIFF" || &data[8..12] != b"WAVE" {
        return None;
    }
    let mut pos = 12usize;
    let mut fmt: Option<(u16, u16, u32, u16)> = None; // format, channels, rate, bits
    let mut pcm_bytes: Option<&[u8]> = None;
    while pos + 8 <= data.len() {
        let id = &data[pos..pos + 4];
        let size = u32::from_le_bytes([
            data[pos + 4],
            data[pos + 5],
            data[pos + 6],
            data[pos + 7],
        ]) as usize;
        let body_start = pos + 8;
        let body_end = body_start.saturating_add(size).min(data.len());
        let body = &data[body_start..body_end];
        if id == b"fmt " && body.len() >= 16 {
            let audio_format = u16::from_le_bytes([body[0], body[1]]);
            let channels = u16::from_le_bytes([body[2], body[3]]);
            let rate = u32::from_le_bytes([body[4], body[5], body[6], body[7]]);
            let bits = u16::from_le_bytes([body[14], body[15]]);
            fmt = Some((audio_format, channels, rate, bits));
        } else if id == b"data" {
            pcm_bytes = Some(body);
        }
        // chunk 按偶数字节对齐
        pos = body_start + size + (size & 1);
        if pos <= body_start {
            break; // 防零步进死循环
        }
    }
    let (audio_format, channels, sample_rate, bits) = fmt?;
    if !matches!(audio_format, 1 | 3) || channels == 0 || sample_rate == 0 || bits == 0 {
        return None;
    }
    let is_float = audio_format == 3;
    let bytes_per_sample = (bits as usize).div_ceil(8);
    let frame_bytes = bytes_per_sample.checked_mul(channels as usize)?;
    if frame_bytes == 0 {
        return None;
    }
    let raw = pcm_bytes?;
    let frames = raw.len() / frame_bytes;
    let mut samples = Vec::with_capacity(frames);
    for f in 0..frames {
        let base = f * frame_bytes;
        let mut sum = 0f32;
        let mut n = 0u32;
        for c in 0..channels as usize {
            let off = base + c * bytes_per_sample;
            if let Some(v) = decode_sample(&raw[off..], bits, is_float) {
                sum += v;
                n += 1;
            }
        }
        if n > 0 {
            samples.push(sum / n as f32);
        }
    }
    Some(PcmData {
        sample_rate,
        channels,
        bits,
        samples,
    })
}

/// 降采样为每列 min/max 包络（columns 列）。
/// 空采样 → 全 0 包络；列数多于采样数时多列会落在同一样本上（包络重复）。
pub fn downsample_envelope(samples: &[f32], columns: usize) -> Vec<(f32, f32)> {
    if columns == 0 {
        return Vec::new();
    }
    if samples.is_empty() {
        return vec![(0.0, 0.0); columns];
    }
    let n = samples.len() as f64;
    let mut out = Vec::with_capacity(columns);
    for c in 0..columns {
        let start = (((c as f64 / columns as f64) * n).floor() as usize).min(samples.len());
        let end = ((((c + 1) as f64 / columns as f64) * n).ceil() as usize)
            .max(start + 1)
            .min(samples.len());
        let slice = &samples[start.min(end)..end];
        let mut mn = f32::INFINITY;
        let mut mx = f32::NEG_INFINITY;
        for &s in slice {
            if s < mn {
                mn = s;
            }
            if s > mx {
                mx = s;
            }
        }
        if !mn.is_finite() || !mx.is_finite() {
            mn = 0.0;
            mx = 0.0;
        }
        out.push((mn, mx));
    }
    out
}

/// `bcr media` 子命令参数（P49-2：P27 契约扩展新视图）
#[derive(clap::Args, Debug)]
pub struct MediaArgs {
    /// 左侧媒体文件
    pub left: String,

    /// 右侧媒体文件
    pub right: String,

    /// 输出 JSON 契约（media.v1，P27 自动化格式）
    #[arg(long)]
    pub json: bool,
}

/// 运行 media 子命令，返回进程退出码（0=无差异，1=有差异，2=错误）
pub fn run(args: &MediaArgs) -> i32 {
    let l = read_media_info(&args.left);
    let r = read_media_info(&args.right);
    let diffs = compare_media(&args.left, &args.right);
    if args.json {
        let fields: Vec<(String, Option<String>, Option<String>)> = diffs
            .iter()
            .map(|d| (d.field.clone(), d.left.clone(), d.right.clone()))
            .collect();
        let v = crate::jsonout::envelope_media(
            &args.left,
            &args.right,
            l.format.clone(),
            r.format.clone(),
            &fields,
        );
        println!("{}", serde_json::to_string(&v).unwrap_or_default());
    } else {
        println!(
            "左侧: {} (格式 {})",
            args.left,
            l.format.as_deref().unwrap_or("unknown")
        );
        println!(
            "右侧: {} (格式 {})",
            args.right,
            r.format.as_deref().unwrap_or("unknown")
        );
        if diffs.is_empty() {
            println!("元数据一致");
        } else {
            for d in &diffs {
                println!(
                    "- {}: 左={} 右={}",
                    d.field,
                    d.left.as_deref().unwrap_or("(无)"),
                    d.right.as_deref().unwrap_or("(无)")
                );
            }
        }
    }
    if diffs.is_empty() {
        0
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_metadata_parsed() {
        let d = tempfile::tempdir().unwrap();
        // 构造最小 WAV：RIFF + fmt（PCM 16bit 44100 立体声）+ data 1s
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&36u32.to_le_bytes());
        bytes.extend_from_slice(b"WAVE");
        bytes.extend_from_slice(b"fmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes()); // PCM
        bytes.extend_from_slice(&2u16.to_le_bytes()); // channels
        bytes.extend_from_slice(&44100u32.to_le_bytes()); // sample rate
        bytes.extend_from_slice(&(44100u32 * 2 * 2).to_le_bytes()); // byte rate
        bytes.extend_from_slice(&4u16.to_le_bytes()); // block align
        bytes.extend_from_slice(&16u16.to_le_bytes()); // bit depth
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&(44100u32 * 4).to_le_bytes()); // 1s data
        bytes.extend_from_slice(&vec![0u8; 44100 * 4]);
        let p = d.path().join("a.wav");
        std::fs::write(&p, &bytes).unwrap();
        let info = read_media_info(p.to_str().unwrap());
        assert_eq!(info.format.as_deref(), Some("WAV"));
        assert_eq!(info.channels, Some(2));
        assert_eq!(info.sample_rate, Some(44100));
        assert_eq!(info.bit_depth, Some(16));
        assert_eq!(info.duration_secs, Some(1));
    }

    #[test]
    fn mp3_duration_estimated() {
        let d = tempfile::tempdir().unwrap();
        // 构造 MP3 帧头：0xFF 0xFB（V1 L3）0x90（128kbps 44100）0x00
        let mut bytes = vec![0xFF, 0xFB, 0x90, 0x00];
        // 填 128kbps * 2s = 32KB
        bytes.extend_from_slice(&vec![0u8; 32 * 1024]);
        let p = d.path().join("a.mp3");
        std::fs::write(&p, &bytes).unwrap();
        let info = read_media_info(p.to_str().unwrap());
        assert_eq!(info.format.as_deref(), Some("MP3"));
        assert_eq!(info.sample_rate, Some(44100));
        assert!(info.duration_secs.is_some(), "应有时长估算");
        assert_eq!(info.duration_secs, Some(2));
    }
}

/// P2：媒体波形新增逻辑的纯函数单测（WAV PCM 解析 / 包络降采样）
#[cfg(test)]
mod p2_tests {
    use super::*;

    /// 构造 WAV 字节流（未压缩 PCM / IEEE float）
    fn wav(audio_format: u16, channels: u16, rate: u32, bits: u16, data: &[u8]) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(b"RIFF");
        b.extend_from_slice(&(36u32 + data.len() as u32).to_le_bytes());
        b.extend_from_slice(b"WAVE");
        b.extend_from_slice(b"fmt ");
        b.extend_from_slice(&16u32.to_le_bytes());
        b.extend_from_slice(&audio_format.to_le_bytes());
        b.extend_from_slice(&channels.to_le_bytes());
        b.extend_from_slice(&rate.to_le_bytes());
        let block = channels * bits / 8;
        b.extend_from_slice(&(rate * block as u32).to_le_bytes());
        b.extend_from_slice(&block.to_le_bytes());
        b.extend_from_slice(&bits.to_le_bytes());
        b.extend_from_slice(b"data");
        b.extend_from_slice(&(data.len() as u32).to_le_bytes());
        b.extend_from_slice(data);
        b
    }

    #[test]
    fn wav_pcm16_samples_decoded() {
        // 4 个 16 位采样：0, 32767, -32768, 16384
        let mut data = Vec::new();
        for v in [0i16, 32767, -32768, 16384] {
            data.extend_from_slice(&v.to_le_bytes());
        }
        let pcm = read_wav_pcm_bytes(&wav(1, 1, 44100, 16, &data)).unwrap();
        assert_eq!(pcm.sample_rate, 44100);
        assert_eq!(pcm.channels, 1);
        assert_eq!(pcm.bits, 16);
        assert_eq!(pcm.samples.len(), 4);
        assert!((pcm.samples[0] - 0.0).abs() < 1e-6);
        assert!((pcm.samples[1] - 32767.0 / 32768.0).abs() < 1e-6);
        assert!((pcm.samples[2] + 1.0).abs() < 1e-6);
        assert!((pcm.samples[3] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn wav_stereo_mixed_to_mono() {
        // 立体声：左 32767 / 右 -32768 → 平均 ≈ -0.0000153
        let mut data = Vec::new();
        data.extend_from_slice(&32767i16.to_le_bytes());
        data.extend_from_slice(&(-32768i16).to_le_bytes());
        let pcm = read_wav_pcm_bytes(&wav(1, 2, 8000, 16, &data)).unwrap();
        assert_eq!(pcm.channels, 2);
        assert_eq!(pcm.samples.len(), 1, "两声道合成一个采样");
        assert!(pcm.samples[0].abs() < 1e-4);
    }

    #[test]
    fn wav_8bit_and_float32_decoded() {
        // 8 位无符号：128 → 0.0，255 → ≈0.992
        let pcm8 = read_wav_pcm_bytes(&wav(1, 1, 8000, 8, &[128, 255])).unwrap();
        assert!((pcm8.samples[0]).abs() < 1e-6);
        assert!((pcm8.samples[1] - 127.0 / 128.0).abs() < 1e-6);
        // IEEE float32（audio_format = 3）
        let mut f = Vec::new();
        f.extend_from_slice(&0.25f32.to_le_bytes());
        let pcmf = read_wav_pcm_bytes(&wav(3, 1, 8000, 32, &f)).unwrap();
        assert!((pcmf.samples[0] - 0.25).abs() < 1e-6);
    }

    #[test]
    fn wav_24bit_signed_decoded() {
        // 24 位 -1 → ≈ -1/8388608；+8388607 → ≈ 1
        let neg: [u8; 3] = [0xFF, 0xFF, 0xFF];
        let pos: [u8; 3] = [0xFF, 0xFF, 0x7F];
        let mut data = Vec::new();
        data.extend_from_slice(&neg);
        data.extend_from_slice(&pos);
        let pcm = read_wav_pcm_bytes(&wav(1, 1, 8000, 24, &data)).unwrap();
        assert!((pcm.samples[0] + 1.0 / 8_388_608.0).abs() < 1e-9);
        assert!((pcm.samples[1] - 8_388_607.0 / 8_388_608.0).abs() < 1e-6);
    }

    #[test]
    fn invalid_or_unsupported_wav_returns_none() {
        assert!(read_wav_pcm_bytes(b"").is_none(), "空输入不 panic");
        assert!(read_wav_pcm_bytes(b"NOTAWAVE....").is_none());
        // 压缩格式（audio_format = 0x0055 = MP3-in-WAV）不支持
        assert!(read_wav_pcm_bytes(&wav(0x0055, 1, 44100, 16, &[0, 0])).is_none());
        // 截断的 RIFF 头不 panic
        let mut truncated = wav(1, 1, 44100, 16, &[0, 0]);
        truncated.truncate(30);
        let _ = read_wav_pcm_bytes(&truncated);
    }

    #[test]
    fn envelope_min_max_per_column() {
        let samples: Vec<f32> = vec![-1.0, 0.5, 1.0, -0.5, 0.25, 0.75, -0.25, 0.0];
        let env = downsample_envelope(&samples, 4);
        assert_eq!(env.len(), 4);
        // 每列取均分区间 [i/4, (i+1)/4) 上的 min/max
        assert_eq!(env[0], (-1.0, 0.5));
        assert_eq!(env[1], (-0.5, 1.0));
        assert_eq!(env[2], (0.25, 0.75));
        assert_eq!(env[3], (-0.25, 0.0));
        // min 与 max 顺序固定（min <= max）
        for (mn, mx) in &env {
            assert!(mn <= mx, "包络 min 必须 <= max");
        }
    }

    #[test]
    fn envelope_empty_and_oversized_column_counts() {
        let empty = downsample_envelope(&[], 5);
        assert_eq!(empty, vec![(0.0, 0.0); 5], "空采样 → 全 0 包络（不 panic）");
        assert!(downsample_envelope(&[0.5], 0).is_empty());
        // 列数多于采样数：多列落在同一样本上（不 panic，包络可重复）
        let env = downsample_envelope(&[1.0, -1.0], 4);
        assert_eq!(env.len(), 4);
        assert_eq!(env[0], (1.0, 1.0));
        assert_eq!(env[1], (1.0, 1.0));
        assert_eq!(env[2], (-1.0, -1.0));
        assert_eq!(env[3], (-1.0, -1.0));
        // 单列 → 覆盖全部采样的 min/max
        assert_eq!(downsample_envelope(&[0.5, -0.5, 1.0], 1), vec![(-0.5, 1.0)]);
    }
}
