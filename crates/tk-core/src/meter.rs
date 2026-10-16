//! Mức dùng theo công cụ cho menu tray / `bum status`: thanh %, kỳ credit, dòng "Hôm nay".
use chrono::{Datelike, Months, NaiveDate};

/// Kỳ credit chứa `today`: (ngày reset gần nhất ≤ hôm nay, ngày reset kế tiếp). `reset_day` kẹp 1–28.
pub fn credit_period(today: NaiveDate, reset_day: u32) -> (NaiveDate, NaiveDate) {
    let day = reset_day.clamp(1, 28);
    let this_month = today.with_day(day).expect("ngày 1–28 luôn có");
    let start = if today.day() >= day { this_month } else { this_month - Months::new(1) };
    (start, start + Months::new(1))
}
