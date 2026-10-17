mod common;
use common::*;
use bum_core::config::Config;
use bum_core::handlers::{list_prompts, PromptFilter};
use bum_core::model::RawEvent;
use bum_core::pricing::Pricing;
use bum_core::store::Store;

fn loaded() -> Store {
    let mut s = Store::open_in_memory().unwrap();
    s.ingest_batch(&offset(MAIN), &main_events(), 0).unwrap();
    let kiro = vec![
        RawEvent::Session { session_id: "k1".into(), tool: "kiro".into(), project: "TICKET-203".into(), cwd: None, version: None, ts: "2026-09-12T01:00:00.000Z".into() },
        RawEvent::Prompt { key: "kp1".into(), session_id: "k1".into(), ts: "2026-09-12T01:00:00.000Z".into(), text: "read the project".into() },
        RawEvent::ApiCall { request_id: "kiro:k1:0".into(), session_id: "k1".into(), prompt_key: Some("kp1".into()), ts: "2026-09-12T01:00:07.000Z".into(), model: "kiro:claude-opus-4.6".into(), usage: u(125_000, 0, 0, 0), tool_uses: vec![], sidechain: false },
        RawEvent::Metered { request_id: "kiro:k1:0".into(), credits: 0.13 },
    ];
    s.ingest_batch(&offset("/kiro/k1.json"), &kiro, 0).unwrap();
    s
}

#[test]
fn credits_and_tool_reach_prompt_stats_and_kiro_is_not_priced_as_claude() {
    let s = loaded();
    let p = s.prompt_stats("kp1").unwrap().unwrap();
    assert_eq!(p.tool, "kiro");
    assert!((p.credits - 0.13).abs() < 1e-9);
    let l = list_prompts(&s, &Pricing::default_table(), &Config::default(), &PromptFilter::default()).unwrap();
    let row = l.groups.iter().flat_map(|g| &g.rows).find(|r| r.stats.key == "kp1").unwrap();
    assert_eq!(row.cost_usd, None, "Kiro is billed in credits, not converted to $");
    assert_eq!(s.prompt_stats("p1").unwrap().unwrap().tool, "claude_code");
}

#[test]
fn list_can_be_filtered_by_tool() {
    let s = loaded();
    let only = |tool: &str| {
        let f = PromptFilter { tool: Some(tool.into()), ..Default::default() };
        list_prompts(&s, &Pricing::default_table(), &Config::default(), &f).unwrap().groups.iter().flat_map(|g| g.rows.iter().map(|r| r.stats.key.clone())).collect::<Vec<_>>()
    };
    assert_eq!(only("kiro"), vec!["kp1"]);
    assert_eq!(only("claude_code"), vec!["p2", "p1"]);
}

#[test]
fn model_stats_group_calls_by_model_with_cost_or_credits() {
    use bum_core::handlers::model_stats;
    let mut s = loaded();
    s.ingest_batch(&offset(SUB), &sub_events(), 0).unwrap();
    let m = model_stats(&s, &Pricing::default_table()).unwrap();
    let get = |id: &str| m.iter().find(|x| x.model == id).unwrap();
    let opus = get("claude-opus-5");
    assert_eq!((opus.tool.as_str(), opus.calls, opus.prompts, opus.tokens), ("claude_code", 3, 2, 3489));
    assert!(opus.cost_usd.unwrap() > 0.0);
    assert_eq!(get("claude-sonnet-5").tokens, 60);
    let kiro = get("kiro:claude-opus-4.6");
    assert_eq!(kiro.tool, "kiro");
    assert_eq!(kiro.cost_usd, None);
    assert!((kiro.credits - 0.13).abs() < 1e-9);
    assert_eq!(m[0].model, "kiro:claude-opus-4.6", "most tokens first");
}

#[test]
fn prompt_rows_list_the_models_they_used() {
    let mut s = loaded();
    s.ingest_batch(&offset(SUB), &sub_events(), 0).unwrap();
    let mut models = s.prompt_stats("p1").unwrap().unwrap().models;
    models.sort();
    assert_eq!(models, vec!["claude-opus-5".to_string(), "claude-sonnet-5".to_string()]);
}

#[test]
fn tool_stats_count_sessions_prompts_and_last_activity() {
    let mut s = loaded();
    // A Kiro subagent session has no prompts: it does not count as a conversation.
    let sub = bum_core::model::RawEvent::Session { session_id: "k-sub".into(), tool: "kiro".into(), project: "x".into(), cwd: None, version: None, ts: "2026-09-12T01:00:00.000Z".into() };
    s.ingest_batch(&offset("/kiro/k-sub.json"), &[sub], 0).unwrap();
    let t = bum_core::handlers::tool_stats(&s).unwrap();
    let kiro = t.iter().find(|x| x.tool == "kiro").unwrap();
    assert_eq!((kiro.sessions, kiro.prompts), (1, 1));
    assert_eq!(kiro.last_ts.as_deref(), Some("2026-09-12T01:00:00.000Z"));
    assert_eq!(t.iter().find(|x| x.tool == "claude_code").unwrap().prompts, 2);
}

#[test]
fn credit_meter_forecasts_from_the_period_pace() {
    use bum_core::meter::CreditMeter;
    use chrono::{Local, NaiveDate, TimeZone, Utc};
    let meter = |used| CreditMeter { used, limit: 1000.0, next_reset: NaiveDate::from_ymd_opt(2026, 11, 1).unwrap(), fetched_at: None, forecast: None };
    let oct11 = Local.with_ymd_and_hms(2026, 10, 11, 0, 0, 0).unwrap().with_timezone(&Utc);
    let f = meter(482.0).with_forecast(oct11).forecast.expect("10 days into the period");
    assert!(f.at_risk);
    let days = (f.hits_at.unwrap() - oct11.timestamp()) as f64 / 86_400.0;
    assert!((10.0..11.5).contains(&days), "{days}"); // 518 left at 48.2/day ≈ 10.7 days
    let oct2 = Local.with_ymd_and_hms(2026, 10, 2, 12, 0, 0).unwrap().with_timezone(&Utc);
    assert_eq!(meter(100.0).with_forecast(oct2).forecast, None, "under 2 days into the period");
    assert!(!meter(100.0).with_forecast(oct11).forecast.unwrap().at_risk, "10/day lasts past the reset");
}
