use chrono::{FixedOffset, LocalResult, TimeZone};
use tk_core::handlers::start_of_day;

#[test]
fn start_of_day_skips_a_missing_midnight() {
    // Nửa đêm không tồn tại (DST bắt đầu lúc 00:00) → dùng giờ hợp lệ đầu tiên sau đó.
    let one_am = FixedOffset::east_opt(0).unwrap().with_ymd_and_hms(2026, 3, 29, 1, 0, 0).unwrap();
    let got = start_of_day(LocalResult::None, || LocalResult::Single(one_am));
    assert_eq!(got, one_am);
}

#[test]
fn start_of_day_uses_earliest_when_ambiguous() {
    let tz = FixedOffset::east_opt(0).unwrap();
    let a = tz.with_ymd_and_hms(2026, 10, 25, 0, 0, 0).unwrap();
    let b = tz.with_ymd_and_hms(2026, 10, 25, 1, 0, 0).unwrap();
    assert_eq!(start_of_day(LocalResult::Ambiguous(a, b), || unreachable!()), a);
}
