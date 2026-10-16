use tk_core::meter::{ring_rgba, RingTone, RING_SIZE};

const ORANGE: [u8; 3] = [0xff, 0x7a, 0x1a];
const RED: [u8; 3] = [0xff, 0x4b, 0x2b];
const GRAY: [u8; 3] = [0x9a, 0x9a, 0x9a];

fn px(img: &[u8], x: usize, y: usize) -> [u8; 4] {
    let i = (y * RING_SIZE + x) * 4;
    [img[i], img[i + 1], img[i + 2], img[i + 3]]
}

/// Pixel nằm giữa nét vòng ở góc `deg` (theo chiều kim đồng hồ từ 12 giờ).
fn on_ring(img: &[u8], deg: f64) -> [u8; 4] {
    let a = deg.to_radians();
    let r = 12.5;
    let (x, y) = (16.0 + r * a.sin(), 16.0 - r * a.cos());
    px(img, x.floor() as usize, y.floor() as usize)
}

fn opaque(p: [u8; 4], rgb: [u8; 3]) -> bool {
    p[3] == 255 && p[..3] == rgb
}

#[test]
fn ring_is_32x32_rgba_with_transparent_center_and_corners() {
    for (pct, tone) in [(0, RingTone::Normal), (21, RingTone::Normal), (86, RingTone::Alert), (100, RingTone::Stale)] {
        let img = ring_rgba(pct, tone);
        assert_eq!(RING_SIZE, 32);
        assert_eq!(img.len(), 32 * 32 * 4);
        assert_eq!(px(&img, 16, 16)[3], 0, "tâm trong suốt");
        assert_eq!(px(&img, 0, 0)[3], 0, "góc trong suốt");
    }
}

#[test]
fn zero_percent_shows_only_translucent_track() {
    let img = ring_rgba(0, RingTone::Normal);
    for deg in [0.0, 90.0, 180.0, 270.0] {
        let p = on_ring(&img, deg);
        assert!(p[3] > 40 && p[3] < 140, "track mờ: {p:?}");
        assert!(p[0] == p[1] && p[1] == p[2], "track xám trung tính: {p:?}");
    }
    assert!(img.chunks(4).all(|p| p[3] < 140), "không có pixel cung đậm");
}

#[test]
fn full_ring_at_100_and_arc_starts_at_12_oclock_clockwise() {
    let img = ring_rgba(100, RingTone::Alert);
    for deg in (0..360).step_by(15) {
        assert!(opaque(on_ring(&img, deg as f64), RED), "{deg}°");
    }
    let img = ring_rgba(21, RingTone::Normal);
    assert!(opaque(px(&img, 16, 3), ORANGE), "12 giờ có màu cung");
    assert!(opaque(on_ring(&img, 40.0), ORANGE), "trong cung (21% ≈ 76°)");
    let after = on_ring(&img, 120.0);
    assert!(after[3] < 140, "ngoài cung chỉ còn track: {after:?}");
    let ccw = on_ring(&img, 300.0);
    assert!(ccw[3] < 140, "cung đi theo chiều kim đồng hồ: {ccw:?}");
}

#[test]
fn tone_picks_arc_color() {
    assert!(opaque(on_ring(&ring_rgba(50, RingTone::Normal), 90.0), ORANGE));
    assert!(opaque(on_ring(&ring_rgba(50, RingTone::Alert), 90.0), RED));
    assert!(opaque(on_ring(&ring_rgba(50, RingTone::Stale), 90.0), GRAY));
}

#[test]
fn edges_are_antialiased() {
    let img = ring_rgba(100, RingTone::Normal);
    assert!(img.chunks(4).any(|p| p[3] > 0 && p[3] < 255), "có pixel bán trong suốt ở mép");
}

/// Ghi 3 PNG mẫu cho người xem: `RING_SAMPLES_DIR=… cargo test -p tk-core --test ring -- --ignored`.
#[test]
#[ignore]
fn write_sample_pngs() {
    let dir = std::path::PathBuf::from(std::env::var("RING_SAMPLES_DIR").expect("RING_SAMPLES_DIR"));
    std::fs::create_dir_all(&dir).unwrap();
    for (name, pct, tone) in [("ring-21-orange", 21, RingTone::Normal), ("ring-86-red", 86, RingTone::Alert), ("ring-60-stale", 60, RingTone::Stale)] {
        std::fs::write(dir.join(format!("{name}.png")), tk_core::meter::ring_png(pct, tone)).unwrap();
    }
}
