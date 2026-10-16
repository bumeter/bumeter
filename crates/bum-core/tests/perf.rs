mod common;
use std::time::Instant;

use common::*;
use bum_core::config::Config;
use bum_core::handlers::{list_prompts, prompt_detail, sidebar_counts, PromptFilter};
use bum_core::model::RawEvent;
use bum_core::pricing::Pricing;
use bum_core::store::Store;

fn big_store(prompts: usize) -> Store {
    let mut s = Store::open_in_memory().unwrap();
    let mut ev = Vec::new();
    for i in 0..prompts {
        let sid = format!("s{}", i / 100);
        let ts = format!("2026-09-{:02}T{:02}:{:02}:00.000Z", 1 + (i / 1440) % 28, (i / 60) % 24, i % 60);
        if i % 100 == 0 {
            ev.push(RawEvent::Session { session_id: sid.clone(), tool: "claude_code".into(), project: format!("P{}", i / 500), cwd: None, version: None, ts: ts.clone() });
        }
        ev.push(RawEvent::Prompt { key: format!("k{i}"), session_id: sid.clone(), ts: ts.clone(), text: format!("question number {i}") });
        for c in 0..4 {
            ev.push(call(&format!("r{i}_{c}"), &format!("k{i}"), &ts, "claude-opus-5", u(1, 100, 50_000 + i as u64, 200), &[("t", "Bash")], false));
        }
    }
    s.ingest_batch(&offset("/root/big.jsonl"), &ev, 0).unwrap();
    s
}

#[test]
fn list_detail_and_counts_stay_fast_on_thousands_of_prompts() {
    let s = big_store(3000);
    let (p, c) = (Pricing::default_table(), Config::default());
    let t = Instant::now();
    let l = list_prompts(&s, &p, &c, &PromptFilter::default()).unwrap();
    let list_ms = t.elapsed().as_millis();
    assert_eq!(l.total_prompts, 3000);
    let t = Instant::now();
    prompt_detail(&s, &p, &c, "k10").unwrap().unwrap();
    let detail_ms = t.elapsed().as_millis();
    let t = Instant::now();
    sidebar_counts(&s, &c).unwrap();
    let counts_ms = t.elapsed().as_millis();
    println!("list {list_ms}ms · detail {detail_ms}ms · counts {counts_ms}ms");
    assert!(list_ms < 150 && detail_ms < 50 && counts_ms < 100, "list {list_ms} detail {detail_ms} counts {counts_ms}");
}
