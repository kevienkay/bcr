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
