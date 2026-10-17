use bum_core::model::Usage;

#[test]
fn usage_context_excludes_output_and_total_includes_it() {
    let u = Usage { input: 2, cache_write_5m: 30, cache_write_1h: 70, cache_read: 1000, output: 50, reasoning: 0 };
    assert_eq!(u.cache_write(), 100);
    assert_eq!(u.context(), 1102);
    assert_eq!(u.total(), 1152);
}

#[test]
fn usage_add_sums_every_field() {
    let mut a = Usage { input: 1, cache_write_5m: 2, cache_write_1h: 3, cache_read: 4, output: 5, reasoning: 0 };
    a.add(&Usage { input: 10, cache_write_5m: 20, cache_write_1h: 30, cache_read: 40, output: 50, reasoning: 0 });
    assert_eq!(a, Usage { input: 11, cache_write_5m: 22, cache_write_1h: 33, cache_read: 44, output: 55, reasoning: 0 });
}

#[test]
fn reasoning_counts_in_total_and_add_but_not_in_context() {
    let u = Usage { input: 2, cache_read: 1000, output: 50, reasoning: 30, ..Default::default() };
    assert_eq!(u.context(), 1002, "reasoning is output-side");
    assert_eq!(u.total(), 1082);
    let mut a = Usage { reasoning: 1, ..Default::default() };
    a.add(&Usage { reasoning: 10, ..Default::default() });
    assert_eq!(a.reasoning, 11);
}

#[test]
fn usage_without_reasoning_field_still_deserializes() {
    let u: Usage = serde_json::from_str(r#"{"input":1,"cache_write_5m":0,"cache_write_1h":0,"cache_read":2,"output":3}"#).unwrap();
    assert_eq!(u.reasoning, 0);
}
