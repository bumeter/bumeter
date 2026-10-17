use bum_core::config::Config;
use bum_core::insight::{anomaly, explain, fmt_tokens};
use bum_core::model::Usage;

#[test]
fn anomaly_by_ratio_or_by_big_context() {
    let cfg = Config::default();
    assert!(anomaly(1_000, 10, Some(1_000), &cfg).is_none());
    let a = anomaly(4_000, 10, Some(1_000), &cfg).unwrap();
    assert_eq!(a.ratio, Some(4.0));
    assert!(!a.big_context);
    assert!(anomaly(10, 300_001, None, &cfg).unwrap().big_context);
    assert!(anomaly(10, 10, None, &cfg).is_none());
}

#[test]
fn explain_blames_context_reread_when_cache_read_dominates() {
    let u = Usage { input: 10, cache_write_5m: 0, cache_write_1h: 1_000, cache_read: 9_400_000, output: 17_000, reasoning: 0 };
    let e = explain(&u, 841_000);
    assert!(matches!(e, bum_core::insight::Explain::Reread { context: 841_000, .. }), "{e:?}");
    assert!(explain(&u, 841_000).text(bum_core::i18n::Lang::En).contains("re-read"));
    let text = e.text(bum_core::i18n::Lang::Vi);
    assert!(text.contains("đọc lại"), "{text}");
    assert!(text.contains("841K"), "{text}");
    assert!(text.contains("/compact"), "{text}");
}

#[test]
fn formats_token_counts() {
    assert_eq!(fmt_tokens(950), "950");
    assert_eq!(fmt_tokens(841_000), "841K");
    assert_eq!(fmt_tokens(9_405_907), "9.4M");
}

#[test]
fn config_save_then_load_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("config.toml");
    let c = Config { context_threshold: 250_000, notify: bum_core::config::Notify { anomaly: false, ..Default::default() }, ..Default::default() };
    c.save(&p).unwrap();
    assert_eq!(Config::load(&p), c);
}

#[test]
fn language_defaults_to_vietnamese_and_round_trips() {
    use bum_core::config::Config;
    use bum_core::i18n::Lang;
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("config.toml");
    std::fs::write(&p, "anomaly_multiplier = 4.0\n").unwrap();
    assert_eq!(Config::load(&p).language, Lang::Vi, "old config has no language");
    let cfg = Config { language: Lang::En, ..Config::default() };
    cfg.save(&p).unwrap();
    assert!(std::fs::read_to_string(&p).unwrap().contains("language = \"en\""));
    assert_eq!(Config::load(&p).language, Lang::En);
}

#[test]
fn alerts_are_localized() {
    use bum_core::alerts::Alert;
    use bum_core::i18n::Lang;
    let a = Alert::Blocked { limit_type: "five_hour".into(), resets_at: 0 };
    assert_eq!(a.title(Lang::Vi), "Hết lượt");
    assert_eq!(a.title(Lang::En), "Limit reached");
    assert!(a.body(Lang::En).starts_with("Hit the 5-hour limit"), "{}", a.body(Lang::En));
}

#[test]
fn theme_defaults_to_system_and_round_trips() {
    use bum_core::config::{Config, Theme};
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("config.toml");
    std::fs::write(&p, "anomaly_multiplier = 4.0\n").unwrap();
    assert_eq!(Config::load(&p).theme, Theme::System, "old config has no theme");
    let cfg = Config { theme: Theme::Dark, ..Config::default() };
    cfg.save(&p).unwrap();
    assert!(std::fs::read_to_string(&p).unwrap().contains("theme = \"dark\""));
    assert_eq!(Config::load(&p).theme, Theme::Dark);
}
