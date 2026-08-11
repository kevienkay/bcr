//! P24：特殊格式比较器 — MP3 音频标签对比（ID3v1 + ID3v2 解析）。
//!
//! 自研解析器，无外部依赖：
//! - ID3v1：文件末尾 128 字节定长块（TAG + 30B 标题/艺术家/专辑/年份/注释 + 流派）
//! - ID3v2：文件头变长块（ID3 + 版本 + syncsafe 大小 + 帧：TIT2 标题/TPE1 艺术家/
//!   TALB 专辑/TYER 年份/TCON 流派/TRCK 音轨/COMM 注释 等，UTF-16/UTF-8/ISO-8859-1 编码）
//!
//! 标签对比输出字段级差异（标题/艺术家/专辑/年份/流派/音轨/注释），
//! 与 Beyond Compare 的 MP3 比较器行为对齐：同字段不同 → 差异，缺字段 → 缺失。
//!
//! CLI（`bcr mp3tag`）与 GUI（Mp3Tab）共用 `compare_mp3()`。

use std::collections::BTreeMap;

/// MP3 标签字段（与 BC MP3 比较器对齐的字段集）
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Mp3Tags {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub year: Option<String>,
    pub genre: Option<String>,
    pub track: Option<String>,
    pub comment: Option<String>,
}

impl Mp3Tags {
    /// 所有字段（含空值），用于遍历输出
    pub fn fields(&self) -> BTreeMap<&'static str, Option<&str>> {
        let mut m = BTreeMap::new();
        m.insert("title", self.title.as_deref());
        m.insert("artist", self.artist.as_deref());
        m.insert("album", self.album.as_deref());
        m.insert("year", self.year.as_deref());
        m.insert("genre", self.genre.as_deref());
        m.insert("track", self.track.as_deref());
        m.insert("comment", self.comment.as_deref());
        m
    }
}

/// 字段级对比结果
#[derive(Debug, Clone)]
pub struct FieldDiff {
    pub field: &'static str,
    pub left: Option<String>,
    pub right: Option<String>,
}

/// MP3 标签对比结果
#[derive(Debug, Clone)]
pub struct Mp3Compare {
    pub left: Mp3Tags,
    #[allow(dead_code)] // GUI Mp3Tab 使用
    pub right: Mp3Tags,
    /// 差异字段（仅值不同的字段，含单侧缺失）
    pub diffs: Vec<FieldDiff>,
}

impl Mp3Compare {
    pub fn has_differences(&self) -> bool {
        !self.diffs.is_empty()
    }
}

/// 解析 MP3 文件标签（ID3v1 + ID3v2，v2 优先覆盖同名帧）。
/// 非 MP3/无标签返回空标签（不算错误，与 BC 一致：无标签视为全空）。
pub fn parse_mp3(path: &str) -> std::io::Result<Mp3Tags> {
    let data = std::fs::read(path)?;
    Ok(parse_mp3_bytes(&data))
}

/// 从字节解析标签（分离以便单元测试构造）
pub fn parse_mp3_bytes(data: &[u8]) -> Mp3Tags {
    let mut tags = Mp3Tags::default();
    // ID3v2 优先（在文件头）
    if let Some(v2) = parse_id3v2(data) {
        tags.merge(v2);
    }
    // ID3v1 兜底（在文件尾 128 字节）
    if let Some(v1) = parse_id3v1(data) {
        tags.merge(v1);
    }
    tags
}

impl Mp3Tags {
    /// 合并另一个标签集：仅填充自身缺失的字段（已有值保留 → v2 优先于 v1）
    fn merge(&mut self, other: Mp3Tags) {
        macro_rules! take {
            ($f:ident) => {
                if self.$f.is_none() && other.$f.is_some() {
                    self.$f = other.$f;
                }
            };
        }
        take!(title);
        take!(artist);
        take!(album);
        take!(year);
        take!(genre);
        take!(track);
        take!(comment);
    }
}

/// 解析 ID3v1（文件尾 128 字节）：TAG + 30B*5 + 流派
fn parse_id3v1(data: &[u8]) -> Option<Mp3Tags> {
    if data.len() < 128 {
        return None;
    }
    let tail = &data[data.len() - 128..];
    if &tail[..3] != b"TAG" {
        return None;
    }
    let s = |b: &[u8]| -> Option<String> {
        let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
        let s = String::from_utf8_lossy(&b[..end]).trim().to_string();
        if s.is_empty() {
            None
        } else {
            Some(s)
        }
    };
    let genre_idx = tail[127] as usize;
    let genre = if genre_idx < ID3V1_GENRES.len() {
        Some(ID3V1_GENRES[genre_idx].to_string())
    } else {
        None
    };
    Some(Mp3Tags {
        title: s(&tail[3..33]),
        artist: s(&tail[33..63]),
        album: s(&tail[63..93]),
        year: s(&tail[93..97]),
        genre,
        track: None,
        comment: s(&tail[97..127]),
    })
}

/// ID3v2 帧 ID → 字段名
fn v2_frame_field(frame_id: &str) -> Option<&'static str> {
    match frame_id {
        "TIT2" => Some("title"),
        "TPE1" => Some("artist"),
        "TALB" => Some("album"),
        "TYER" | "TDRC" => Some("year"),
        "TCON" => Some("genre"),
        "TRCK" => Some("track"),
        "COMM" => Some("comment"),
        _ => None,
    }
}

/// 解析 ID3v2（文件头，含 syncsafe 大小；仅取文本帧，跳过附加图片等二进制帧）
fn parse_id3v2(data: &[u8]) -> Option<Mp3Tags> {
    if data.len() < 10 || &data[..3] != b"ID3" {
        return None;
    }
    let size = syncsafe_u32(&data[6..10]) as usize;
    if data.len() < 10 + size {
        return None;
    }
    let body = &data[10..10 + size];
    let mut tags = Mp3Tags::default();
    let mut pos = 0usize;
    // v2.2 帧头 6 字节（3 字节 ID + 3 字节大小），v2.3/2.4 帧头 10 字节（4+4+2）
    let header_len = match data[3] {
        2 => 6usize,
        _ => 10usize,
    };
    let id_len = match data[3] {
        2 => 3usize,
        _ => 4usize,
    };
    while pos + header_len <= body.len() {
        let id_bytes = &body[pos..pos + id_len];
        let id = String::from_utf8_lossy(id_bytes).to_string();
        if id_bytes.iter().all(|&b| b == 0) {
            break; // 填充区
        }
        let fsize = if data[3] == 2 {
            u24(&body[pos + 3..pos + 6]) as usize
        } else {
            // v2.3 用普通 u32，v2.4 用 syncsafe
            let raw =
                u32::from_be_bytes([body[pos + 4], body[pos + 5], body[pos + 6], body[pos + 7]]);
            if data[3] >= 4 {
                syncsafe_u32(&body[pos + 4..pos + 8]) as usize
            } else {
                raw as usize
            }
        };
        let frame_data_start = pos + header_len;
        if frame_data_start + fsize > body.len() {
            break;
        }
        let frame_data = &body[frame_data_start..frame_data_start + fsize];
        if let Some(field) = v2_frame_field(&id) {
            // 解码文本帧：首字节编码标志；COMM 帧需跳过语言+描述
            if !frame_data.is_empty() {
                let value = if field == "comment" {
                    decode_v2_comment(frame_data)
                } else {
                    decode_v2_text(frame_data)
                };
                if let Some(v) = value {
                    let existing = match field {
                        "title" => &mut tags.title,
                        "artist" => &mut tags.artist,
                        "album" => &mut tags.album,
                        "year" => &mut tags.year,
                        "genre" => &mut tags.genre,
                        "track" => &mut tags.track,
                        "comment" => &mut tags.comment,
                        _ => unreachable!(),
                    };
                    if existing.is_none() {
                        *existing = Some(v);
                    }
                }
            }
        }
        pos = frame_data_start + fsize;
        // v2.3 帧头含 2 字节 flag；v2.4 可能在帧头前有扩展
        if data[3] == 3 {
            pos = frame_data_start + fsize;
        }
    }
    Some(tags)
}

/// 解码 ID3v2 文本帧值（0=ISO-8859-1，1=UTF-16，2=UTF-16BE，3=UTF-8）
fn decode_v2_text(frame_data: &[u8]) -> Option<String> {
    if frame_data.is_empty() {
        return None;
    }
    let encoding = frame_data[0];
    let payload = &frame_data[1..];
    decode_text(encoding, payload)
}

/// 解码 ID3v2 COMM 帧：encoding + language(3) + description(0 结尾) + text
fn decode_v2_comment(frame_data: &[u8]) -> Option<String> {
    if frame_data.is_empty() {
        return None;
    }
    let encoding = frame_data[0];
    let mut payload = &frame_data[1..];
    // 跳过 3 字节语言 + 描述（以 0 结尾；UTF-16 描述以 00 00 结尾）
    if payload.len() >= 3 {
        payload = &payload[3..];
    }
    let desc_end = match encoding {
        1 | 2 => {
            // UTF-16 描述以 00 00 结束
            payload
                .chunks_exact(2)
                .position(|c| c == [0, 0])
                .map(|i| i * 2)
        }
        _ => payload.iter().position(|&b| b == 0),
    };
    if let Some(end) = desc_end {
        let skip = if encoding == 1 || encoding == 2 {
            end + 2
        } else {
            end + 1
        };
        if skip <= payload.len() {
            payload = &payload[skip..];
        } else {
            return None;
        }
    }
    decode_text(encoding, payload)
}

/// 按编码解码文本字节（去掉尾部 0 终止符）
fn decode_text(encoding: u8, raw: &[u8]) -> Option<String> {
    match encoding {
        0 => {
            let end = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
            let s = String::from_utf8_lossy(&raw[..end]).trim().to_string();
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        }
        1 => {
            // UTF-16 with BOM
            if raw.len() < 2 {
                return None;
            }
            let (bom_le, body) = if raw[0] == 0xFF && raw[1] == 0xFE {
                (true, &raw[2..])
            } else if raw[0] == 0xFE && raw[1] == 0xFF {
                (false, &raw[2..])
            } else {
                (true, raw)
            };
            let units: Vec<u16> = if bom_le {
                body.chunks_exact(2)
                    .map(|c| u16::from_le_bytes([c[0], c[1]]))
                    .collect()
            } else {
                body.chunks_exact(2)
                    .map(|c| u16::from_be_bytes([c[0], c[1]]))
                    .collect()
            };
            let end = units.iter().position(|&u| u == 0).unwrap_or(units.len());
            let s = String::from_utf16_lossy(&units[..end]).trim().to_string();
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        }
        2 => {
            // UTF-16BE without BOM
            let units: Vec<u16> = raw
                .chunks_exact(2)
                .map(|c| u16::from_be_bytes([c[0], c[1]]))
                .collect();
            let end = units.iter().position(|&u| u == 0).unwrap_or(units.len());
            let s = String::from_utf16_lossy(&units[..end]).trim().to_string();
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        }
        _ => {
            // 3 = UTF-8
            let end = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
            let s = String::from_utf8_lossy(&raw[..end]).trim().to_string();
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        }
    }
}

fn syncsafe_u32(b: &[u8]) -> u32 {
    if b.len() < 4 {
        return 0;
    }
    ((b[0] as u32) << 21) | ((b[1] as u32) << 14) | ((b[2] as u32) << 7) | (b[3] as u32)
}

fn u24(b: &[u8]) -> u32 {
    if b.len() < 3 {
        return 0;
    }
    ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | (b[2] as u32)
}

/// 对比两个 MP3 的标签，返回字段级差异
pub fn compare_mp3(left_path: &str, right_path: &str) -> std::io::Result<Mp3Compare> {
    let left = parse_mp3(left_path)?;
    let right = parse_mp3(right_path)?;
    Ok(compare_tags(&left, &right))
}

/// 标签字段级对比（分离以便单元测试）
pub fn compare_tags(left: &Mp3Tags, right: &Mp3Tags) -> Mp3Compare {
    let mut diffs = Vec::new();
    let lf = left.fields();
    let rf = right.fields();
    for (k, lv) in &lf {
        let rv = rf.get(k).copied().flatten();
        let lvs = lv.map(|s| s.to_string());
        if lvs != rv.map(|s| s.to_string()) {
            diffs.push(FieldDiff {
                field: k,
                left: lvs,
                right: rv.map(|s| s.to_string()),
            });
        }
    }
    Mp3Compare {
        left: left.clone(),
        right: right.clone(),
        diffs,
    }
}

/// 判断文件是否为 MP3（魔数嗅探：ID3 头或 MPEG 帧同步）
#[allow(dead_code)] // GUI Mp3Tab 使用
pub fn is_mp3_file(path: &str) -> bool {
    let Ok(mut f) = std::fs::File::open(path) else {
        return false;
    };
    use std::io::Read;
    let mut head = [0u8; 4];
    let n = match f.read(&mut head) {
        Ok(n) => n,
        Err(_) => return false,
    };
    let b = &head[..n];
    // ID3v2 头
    if b.len() >= 3 && &b[..3] == b"ID3" {
        return true;
    }
    // MPEG 帧同步：11 位全 1（0xFFE0 掩码）
    if b.len() >= 2 && b[0] == 0xFF && (b[1] & 0xE0) == 0xE0 {
        return true;
    }
    // 扩展名兜底
    let lower = path.to_lowercase();
    lower.ends_with(".mp3")
}

/// ID3v1 流派表（前 80 个常见值，其余显示编号）
const ID3V1_GENRES: [&str; 80] = [
    "Blues",
    "Classic Rock",
    "Country",
    "Dance",
    "Disco",
    "Funk",
    "Grunge",
    "Hip-Hop",
    "Jazz",
    "Metal",
    "New Age",
    "Oldies",
    "Other",
    "Pop",
    "R&B",
    "Rap",
    "Reggae",
    "Rock",
    "Techno",
    "Industrial",
    "Alternative",
    "Ska",
    "Death Metal",
    "Pranks",
    "Soundtrack",
    "Euro-Techno",
    "Ambient",
    "Trip-Hop",
    "Vocal",
    "Jazz+Funk",
    "Fusion",
    "Trance",
    "Classical",
    "Instrumental",
    "Acid",
    "House",
    "Game",
    "Sound Clip",
    "Gospel",
    "Noise",
    "Alternative Rock",
    "Bass",
    "Soul",
    "Punk",
    "Space",
    "Meditative",
    "Instrumental Pop",
    "Instrumental Rock",
    "Ethnic",
    "Gothic",
    "Darkwave",
    "Techno-Industrial",
    "Electronic",
    "Pop-Folk",
    "Eurodance",
    "Dream",
    "Southern Rock",
    "Comedy",
    "Cult",
    "Gangsta",
    "Top 40",
    "Christian Rap",
    "Pop/Funk",
    "Jungle",
    "Native American",
    "Cabaret",
    "New Wave",
    "Psychedelic",
    "Rave",
    "Showtunes",
    "Trailer",
    "Lo-Fi",
    "Tribal",
    "Acid Punk",
    "Acid Jazz",
    "Polka",
    "Retro",
    "Musical",
    "Rock & Roll",
    "Hard Rock",
];

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

/// `bcr mp3tag <left> <right>` 参数
#[derive(clap::Args, Debug)]
pub struct Mp3tagArgs {
    /// 左侧 MP3 文件
    pub left: String,

    /// 右侧 MP3 文件
    pub right: String,

    /// 同时显示相同字段（默认只输出差异字段）
    #[arg(long)]
    pub show_same: bool,

    /// 颜色输出：auto | always | never
    #[arg(long, default_value = "auto", value_parser = ["auto", "always", "never"])]
    pub color: String,
}

/// CLI 入口：输出字段级差异，退出码 0=标签一致，1=标签有差异，2=错误
pub fn run(args: &Mp3tagArgs) -> i32 {
    let cmp = match compare_mp3(&args.left, &args.right) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("bcr: {}", e);
            return 2;
        }
    };
    let use_color = match args.color.as_str() {
        "always" => true,
        "never" => false,
        _ => std::io::IsTerminal::is_terminal(&std::io::stdout()),
    };
    let (red, green, reset) = if use_color {
        ("\x1b[1;31m", "\x1b[1;32m", "\x1b[0m")
    } else {
        ("", "", "")
    };
    let status = if cmp.has_differences() {
        format!("{red}[DIFF]{reset}")
    } else {
        format!("{green}[SAME]{reset}")
    };
    println!("{} {} vs {}", status, args.left, args.right);
    for d in &cmp.diffs {
        let l = d.left.as_deref().unwrap_or("(missing)");
        let r = d.right.as_deref().unwrap_or("(missing)");
        println!("  {:10} {red}{l}{reset}  ->  {green}{r}{reset}", d.field);
    }
    if cmp.diffs.is_empty() && args.show_same {
        for (k, v) in cmp.left.fields() {
            if let Some(val) = v {
                println!("  {:10} {val}", k);
            }
        }
    }
    if cmp.has_differences() {
        1
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_id3v2(frames: &[(&str, &[u8])]) -> Vec<u8> {
        let mut body = Vec::new();
        for (id, data) in frames {
            body.extend_from_slice(id.as_bytes());
            let sz = (data.len() as u32).to_be_bytes();
            body.extend_from_slice(&sz);
            body.extend_from_slice(&[0, 0]); // flags
            body.extend_from_slice(data);
        }
        let mut out = Vec::new();
        out.extend_from_slice(b"ID3");
        out.push(3); // v2.3
        out.push(0);
        out.push(0);
        // syncsafe size
        let n = body.len() as u32;
        out.push(((n >> 21) & 0x7F) as u8);
        out.push(((n >> 14) & 0x7F) as u8);
        out.push(((n >> 7) & 0x7F) as u8);
        out.push((n & 0x7F) as u8);
        out.extend_from_slice(&body);
        out
    }

    fn utf8_frame(value: &str) -> Vec<u8> {
        let mut d = vec![3u8]; // UTF-8
        d.extend_from_slice(value.as_bytes());
        d
    }

    fn build_id3v1(title: &str, artist: &str) -> Vec<u8> {
        let mut out = vec![0u8; 128];
        out[..3].copy_from_slice(b"TAG");
        let put = |out: &mut [u8], off: usize, s: &str| {
            let b = s.as_bytes();
            let n = b.len().min(30);
            out[off..off + n].copy_from_slice(&b[..n]);
        };
        put(&mut out, 3, title);
        put(&mut out, 33, artist);
        out[127] = 17; // Rock
        out
    }

    #[test]
    fn no_tags_is_empty() {
        let t = parse_mp3_bytes(b"\x00\x01\x02\x03");
        assert_eq!(t, Mp3Tags::default());
    }

    #[test]
    fn id3v2_utf8_frames() {
        let data = build_id3v2(&[
            ("TIT2", &utf8_frame("Song")),
            ("TPE1", &utf8_frame("Artist")),
            ("TALB", &utf8_frame("Album")),
            ("TYER", &utf8_frame("2024")),
            ("TCON", &utf8_frame("Rock")),
            ("TRCK", &utf8_frame("3")),
        ]);
        let t = parse_mp3_bytes(&data);
        assert_eq!(t.title.as_deref(), Some("Song"));
        assert_eq!(t.artist.as_deref(), Some("Artist"));
        assert_eq!(t.album.as_deref(), Some("Album"));
        assert_eq!(t.year.as_deref(), Some("2024"));
        assert_eq!(t.genre.as_deref(), Some("Rock"));
        assert_eq!(t.track.as_deref(), Some("3"));
    }

    #[test]
    fn id3v1_parse() {
        let mut data = vec![0u8; 200];
        let v1 = build_id3v1("Old Title", "Old Artist");
        data[72..].copy_from_slice(&v1);
        let t = parse_mp3_bytes(&data);
        assert_eq!(t.title.as_deref(), Some("Old Title"));
        assert_eq!(t.artist.as_deref(), Some("Old Artist"));
        assert_eq!(t.genre.as_deref(), Some("Rock"));
    }

    #[test]
    fn id3v2_precedence_over_v1() {
        // v2 在前，v1 在后 → v2 的 title 生效
        let mut data = build_id3v2(&[("TIT2", &utf8_frame("New Title"))]);
        data.extend_from_slice(&build_id3v1("Old Title", "Old Artist"));
        let t = parse_mp3_bytes(&data);
        assert_eq!(t.title.as_deref(), Some("New Title"));
        assert_eq!(t.artist.as_deref(), Some("Old Artist"));
    }

    #[test]
    fn utf16_le_frame() {
        let mut d = vec![1u8]; // UTF-16
        d.extend_from_slice(&[0xFF, 0xFE]);
        for u in "歌名".encode_utf16() {
            d.extend_from_slice(&u.to_le_bytes());
        }
        let data = build_id3v2(&[("TIT2", &d)]);
        let t = parse_mp3_bytes(&data);
        assert_eq!(t.title.as_deref(), Some("歌名"));
    }

    #[test]
    fn syncsafe_size_decode() {
        // 0x0505060D 的 syncsafe 编码 → 0x0505060D = 84215309；但 syncsafe 解码按 7bit 分段：
        // 0x05<<21 | 0x05<<14 | 0x06<<7 | 0x0D = 10568461
        let b = [0x05, 0x05, 0x06, 0x0D];
        assert_eq!(syncsafe_u32(&b), 10568461);
    }

    #[test]
    fn compare_detects_field_diff() {
        let a = Mp3Tags {
            title: Some("A".into()),
            artist: Some("X".into()),
            ..Default::default()
        };
        let b = Mp3Tags {
            title: Some("B".into()),
            artist: Some("X".into()),
            ..Default::default()
        };
        let c = compare_tags(&a, &b);
        assert!(c.has_differences());
        assert_eq!(c.diffs.len(), 1);
        assert_eq!(c.diffs[0].field, "title");
        assert_eq!(c.diffs[0].left.as_deref(), Some("A"));
        assert_eq!(c.diffs[0].right.as_deref(), Some("B"));
    }

    #[test]
    fn compare_missing_field_is_diff() {
        let a = Mp3Tags {
            title: Some("A".into()),
            ..Default::default()
        };
        let b = Mp3Tags::default();
        let c = compare_tags(&a, &b);
        assert!(c.has_differences());
        assert_eq!(c.diffs[0].right, None);
    }

    #[test]
    fn is_mp3_magic() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("x.mp3");
        std::fs::write(&p, b"ID3\x04\x00\x00\x00\x00\x00\x00").unwrap();
        assert!(is_mp3_file(p.to_str().unwrap()));
        let p2 = dir.path().join("y.mp3");
        std::fs::write(&p2, b"\xFF\xFB\x90\x00").unwrap(); // MPEG frame sync
        assert!(is_mp3_file(p2.to_str().unwrap()));
    }
}
