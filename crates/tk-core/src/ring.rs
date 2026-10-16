//! Icon vòng % cho menu tray: ảnh RGBA 32×32 vẽ thuần Rust (khử răng cưa bằng lấy mẫu 4×4 mỗi pixel).

/// Cạnh ảnh vòng (px).
pub const RING_SIZE: usize = 32;

/// Màu cung: dưới ngưỡng cảnh báo 1 / từ ngưỡng 1 trở lên / dữ liệu cũ hoặc chưa có.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RingTone {
    Normal,
    Alert,
    Stale,
}

impl RingTone {
    fn rgb(self) -> [f64; 3] {
        match self {
            RingTone::Normal => [255.0, 122.0, 26.0], // #ff7a1a
            RingTone::Alert => [255.0, 75.0, 43.0],   // #ff4b2b
            RingTone::Stale => [154.0, 154.0, 154.0], // #9a9a9a
        }
    }
}

/// Nền vòng xám trung tính, mờ để hợp cả menu sáng lẫn tối.
const TRACK_RGB: f64 = 138.0; // #8a8a8a
const TRACK_ALPHA: f64 = 90.0 / 255.0;
const CENTER: f64 = RING_SIZE as f64 / 2.0;
/// Bán kính giữa nét và nửa bề rộng nét (nét 5px).
const MID_R: f64 = 12.5;
const HALF_W: f64 = 2.5;
const SS: usize = 4;

/// Lượng tử hoá % về số nguyên 0–100 (>100 → đầy) và chọn màu: stale/không có số → xám, ≥ ngưỡng → đỏ.
pub fn ring_key(pct: Option<f64>, stale: bool, threshold: u8) -> (u8, RingTone) {
    let p = pct.filter(|p| !p.is_nan()).unwrap_or(0.0);
    let q = p.round().clamp(0.0, 100.0) as u8;
    let tone = if stale || pct.is_none() {
        RingTone::Stale
    } else if p >= threshold as f64 {
        RingTone::Alert
    } else {
        RingTone::Normal
    };
    (q, tone)
}

/// Góc theo chiều kim đồng hồ từ 12 giờ, trong [0, 2π).
fn angle(dx: f64, dy: f64) -> f64 {
    let a = dx.atan2(-dy);
    if a < 0.0 { a + std::f64::consts::TAU } else { a }
}

/// Ảnh RGBA `RING_SIZE`² (hàng trên xuống, không nhân trước alpha): track xám + cung `pct`% đầu nét tròn.
pub fn ring_rgba(pct: u8, tone: RingTone) -> Vec<u8> {
    let sweep = pct.min(100) as f64 / 100.0 * std::f64::consts::TAU;
    let full = pct >= 100;
    let cap = |a: f64| (CENTER + MID_R * a.sin(), CENTER - MID_R * a.cos());
    let (c0, c1) = (cap(0.0), cap(sweep));
    let in_cap = |x: f64, y: f64, c: (f64, f64)| (x - c.0).powi(2) + (y - c.1).powi(2) <= HALF_W * HALF_W;
    let arc_rgb = tone.rgb();
    let mut out = vec![0u8; RING_SIZE * RING_SIZE * 4];
    for py in 0..RING_SIZE {
        for px in 0..RING_SIZE {
            let (mut band, mut arc) = (0usize, 0usize);
            for sy in 0..SS {
                for sx in 0..SS {
                    let x = px as f64 + (sx as f64 + 0.5) / SS as f64;
                    let y = py as f64 + (sy as f64 + 0.5) / SS as f64;
                    let (dx, dy) = (x - CENTER, y - CENTER);
                    let on_band = ((dx * dx + dy * dy).sqrt() - MID_R).abs() <= HALF_W;
                    band += on_band as usize;
                    let on_arc = pct > 0 && ((on_band && (full || angle(dx, dy) <= sweep)) || in_cap(x, y, c0) || in_cap(x, y, c1));
                    arc += on_arc as usize;
                }
            }
            let n = (SS * SS) as f64;
            let a = arc as f64 / n;
            let t = band as f64 / n * TRACK_ALPHA;
            // Cung chồng lên track (source-over).
            let oa = a + t * (1.0 - a);
            let i = (py * RING_SIZE + px) * 4;
            if oa > 0.0 {
                for k in 0..3 {
                    out[i + k] = ((arc_rgb[k] * a + TRACK_RGB * t * (1.0 - a)) / oa).round() as u8;
                }
                out[i + 3] = (oa * 255.0).round() as u8;
            }
        }
    }
    out
}

/// PNG (không nén) của vòng — để xem thử / lưu mẫu.
pub fn ring_png(pct: u8, tone: RingTone) -> Vec<u8> {
    encode_png(&ring_rgba(pct, tone), RING_SIZE as u32, RING_SIZE as u32)
}

fn crc32(data: &[u8]) -> u32 {
    let mut c = !0u32;
    for &b in data {
        c ^= b as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
    }
    !c
}

/// PNG RGBA 8-bit tối giản: zlib với các khối "stored".
fn encode_png(rgba: &[u8], w: u32, h: u32) -> Vec<u8> {
    let mut raw = Vec::with_capacity(rgba.len() + h as usize);
    for row in rgba.chunks(w as usize * 4) {
        raw.push(0);
        raw.extend_from_slice(row);
    }
    let mut z = vec![0x78, 0x01];
    let blocks: Vec<&[u8]> = raw.chunks(65_535).collect();
    for (k, b) in blocks.iter().enumerate() {
        z.push((k + 1 == blocks.len()) as u8);
        let len = b.len() as u16;
        z.extend_from_slice(&len.to_le_bytes());
        z.extend_from_slice(&(!len).to_le_bytes());
        z.extend_from_slice(b);
    }
    let (mut s1, mut s2) = (1u32, 0u32);
    for &b in &raw {
        s1 = (s1 + b as u32) % 65_521;
        s2 = (s2 + s1) % 65_521;
    }
    z.extend_from_slice(&((s2 << 16) | s1).to_be_bytes());
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut chunk = |kind: &[u8], data: &[u8]| {
        png.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut body = kind.to_vec();
        body.extend_from_slice(data);
        png.extend_from_slice(&body);
        png.extend_from_slice(&crc32(&body).to_be_bytes());
    };
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    chunk(b"IHDR", &ihdr);
    chunk(b"IDAT", &z);
    chunk(b"IEND", &[]);
    png
}
