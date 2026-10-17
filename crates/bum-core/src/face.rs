//! Choose the Bum expression for the tray icon (spec 3.4). Each case must match `bumStateFor`
//! in `app/ui/src/components/bum.ts`.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Face {
    Happy,
    Calm,
    Worry,
    Blaze,
    Sleep,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FaceInput {
    /// Quota data is stale.
    pub stale: bool,
    /// Minutes since the last conversation turn; None if there never was one.
    pub minutes_since_last_turn: Option<i64>,
    /// 5-hour quota %; None if unavailable.
    pub quota_pct: Option<f64>,
    /// (warning threshold, danger threshold).
    pub thresholds: (u8, u8),
    /// Minutes since the most recent unusually costly turn.
    pub minutes_since_anomaly: Option<i64>,
    pub today_tokens: u64,
    pub avg_daily_tokens: Option<u64>,
    /// A quota forecast says the limit comes before the reset.
    pub quota_at_risk: bool,
}

const STALE_NO_TURN_MINUTES: i64 = 30;
const ANOMALY_RECENT_MINUTES: i64 = 10;

/// Priority order: sleep → blaze → worry → happy → calm.
pub fn face_for(i: &FaceInput) -> Face {
    let no_turn_recently = i.minutes_since_last_turn.is_none_or(|m| m >= STALE_NO_TURN_MINUTES);
    let no_data_at_all = i.minutes_since_last_turn.is_none() && i.quota_pct.is_none() && i.today_tokens == 0;
    if (i.stale && no_turn_recently) || no_data_at_all {
        return Face::Sleep;
    }
    let danger = i.quota_pct.is_some_and(|q| q >= f64::from(i.thresholds.1));
    let anomaly = i.minutes_since_anomaly.is_some_and(|m| m <= ANOMALY_RECENT_MINUTES);
    if danger || anomaly {
        return Face::Blaze;
    }
    if i.quota_at_risk || i.quota_pct.is_some_and(|q| q >= f64::from(i.thresholds.0)) {
        return Face::Worry;
    }
    if i.avg_daily_tokens.is_some_and(|a| i.today_tokens < a) {
        return Face::Happy;
    }
    Face::Calm
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> FaceInput {
        FaceInput { stale: false, minutes_since_last_turn: Some(5), quota_pct: Some(10.0), thresholds: (70, 90), minutes_since_anomaly: None, today_tokens: 200, avg_daily_tokens: Some(100), quota_at_risk: false }
    }

    #[test]
    fn at_risk_quota_worries_even_below_the_warning_threshold() {
        assert_eq!(face_for(&FaceInput { quota_pct: Some(40.0), quota_at_risk: true, ..base() }), Face::Worry);
        assert_eq!(face_for(&FaceInput { quota_pct: Some(95.0), quota_at_risk: true, ..base() }), Face::Blaze, "danger still wins");
    }

    #[test]
    fn default_is_calm() {
        assert_eq!(face_for(&base()), Face::Calm);
    }

    #[test]
    fn stale_without_recent_turn_sleeps() {
        assert_eq!(face_for(&FaceInput { stale: true, minutes_since_last_turn: Some(31), ..base() }), Face::Sleep);
        assert_eq!(face_for(&FaceInput { stale: true, minutes_since_last_turn: Some(30), ..base() }), Face::Sleep);
        assert_eq!(face_for(&FaceInput { stale: true, minutes_since_last_turn: None, ..base() }), Face::Sleep);
    }

    #[test]
    fn no_data_at_all_sleeps() {
        assert_eq!(face_for(&FaceInput { minutes_since_last_turn: None, quota_pct: None, today_tokens: 0, ..base() }), Face::Sleep);
    }

    #[test]
    fn stale_but_recent_turn_is_not_sleep() {
        assert_eq!(face_for(&FaceInput { stale: true, minutes_since_last_turn: Some(5), quota_pct: Some(95.0), ..base() }), Face::Blaze);
    }

    #[test]
    fn blaze_on_danger_quota_inclusive() {
        assert_eq!(face_for(&FaceInput { quota_pct: Some(92.0), ..base() }), Face::Blaze);
        assert_eq!(face_for(&FaceInput { quota_pct: Some(90.0), ..base() }), Face::Blaze);
    }

    #[test]
    fn blaze_on_recent_anomaly() {
        assert_eq!(face_for(&FaceInput { quota_pct: Some(5.0), minutes_since_anomaly: Some(10), ..base() }), Face::Blaze);
        assert_eq!(face_for(&FaceInput { quota_pct: Some(5.0), minutes_since_anomaly: Some(11), ..base() }), Face::Calm);
    }

    #[test]
    fn worry_on_warning_quota_inclusive() {
        assert_eq!(face_for(&FaceInput { quota_pct: Some(75.0), ..base() }), Face::Worry);
        assert_eq!(face_for(&FaceInput { quota_pct: Some(70.0), ..base() }), Face::Worry);
    }

    #[test]
    fn missing_quota_ignores_quota_rules() {
        assert_eq!(face_for(&FaceInput { quota_pct: None, ..base() }), Face::Calm);
    }

    #[test]
    fn happy_when_below_average() {
        assert_eq!(face_for(&FaceInput { today_tokens: 50, avg_daily_tokens: Some(200), ..base() }), Face::Happy);
        assert_eq!(face_for(&FaceInput { today_tokens: 0, avg_daily_tokens: None, ..base() }), Face::Calm);
        assert_eq!(face_for(&FaceInput { today_tokens: 200, avg_daily_tokens: Some(200), ..base() }), Face::Calm);
    }

    #[test]
    fn priority_order() {
        assert_eq!(face_for(&FaceInput { stale: true, minutes_since_last_turn: Some(40), quota_pct: Some(99.0), minutes_since_anomaly: Some(1), ..base() }), Face::Sleep);
        assert_eq!(face_for(&FaceInput { quota_pct: Some(95.0), ..base() }), Face::Blaze);
        assert_eq!(face_for(&FaceInput { quota_pct: Some(75.0), today_tokens: 10, avg_daily_tokens: Some(200), ..base() }), Face::Worry);
    }
}
