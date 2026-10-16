use bum_core::model::Usage;
use bum_core::pricing::Pricing;

const M: u64 = 1_000_000;

#[test]
fn default_table_prices_each_token_kind() {
    let p = Pricing::default_table();
    assert_eq!(p.cost("claude-opus-5", &Usage { input: M, ..Default::default() }), Some(5.0));
    assert_eq!(p.cost("claude-opus-5", &Usage { cache_write_1h: M, ..Default::default() }), Some(10.0));
    assert_eq!(p.cost("claude-opus-5-5", &Usage { cache_read: M, ..Default::default() }), Some(0.20));
    assert_eq!(p.cost("claude-sonnet-5", &Usage { output: M, ..Default::default() }), Some(10.0));
}

#[test]
fn falls_back_to_longest_prefix_and_rejects_unknown() {
    let p = Pricing::default_table();
    assert_eq!(p.cost("claude-opus-4-8", &Usage { input: M, ..Default::default() }), Some(5.0));
    assert_eq!(p.cost("gpt-9", &Usage { input: M, ..Default::default() }), None);
}

#[test]
fn parses_user_file() {
    let p = Pricing::from_toml("[models.\"x\"]\ninput = 1.0\ncache_write_5m = 0.0\ncache_write_1h = 0.0\ncache_read = 0.0\noutput = 2.0\n").unwrap();
    assert_eq!(p.cost("x", &Usage { input: M, output: M, ..Default::default() }), Some(3.0));
}
