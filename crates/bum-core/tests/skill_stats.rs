mod common;
use common::*;
use bum_core::handlers::skill_stats;
use bum_core::store::Store;

#[test]
fn stats_count_direct_uses_and_include_continued_work_in_tokens() {
    let mut s = Store::open_in_memory().unwrap();
    s.ingest_batch(&offset(MAIN), &main_events(), 0).unwrap();
    s.ingest_batch(&offset(SUB), &sub_events(), 0).unwrap();
    let stats = skill_stats(&s).unwrap();

    let review = stats.iter().find(|x| x.name == "pj-review").unwrap();
    assert_eq!(review.uses, 1);
    assert_eq!(review.projects, 1);
    assert_eq!(review.tokens, 2343 + 1206, "gồm cả lần hỏi làm tiếp");
    assert_eq!(review.avg_tokens, Some(3549));
    assert_eq!(review.last_used.as_deref(), Some("2026-09-11T04:28:23.938Z"));

    let model = stats.iter().find(|x| x.name == "/model").unwrap();
    assert_eq!(model.uses, 1, "slash command không gắn với lần hỏi vẫn được đếm");
    assert_eq!(model.avg_tokens, None);
    assert_eq!(model.last_used.as_deref(), Some("2026-09-11T04:36:00.000Z"));
}

#[test]
fn empty_store_has_no_stats() {
    assert!(skill_stats(&Store::open_in_memory().unwrap()).unwrap().is_empty());
}

#[test]
fn a_slash_invoked_skill_is_counted_once() {
    use bum_core::model::{RawEvent, SkillKind};
    let mut s = Store::open_in_memory().unwrap();
    s.ingest_batch(&offset(MAIN), &main_events(), 0).unwrap();
    let slash = vec![
        RawEvent::Skill { prompt_key: Some("p5".into()), session_id: "s1".into(), ts: "2026-09-11T05:00:00.000Z".into(), skill: "/pj-review".into(), kind: SkillKind::Command },
        RawEvent::Skill { prompt_key: Some("p5".into()), session_id: "s1".into(), ts: "2026-09-11T05:00:01.000Z".into(), skill: "pj-review".into(), kind: SkillKind::Direct },
    ];
    s.ingest_batch(&offset("/root/x.jsonl"), &slash, 0).unwrap();
    let stats = skill_stats(&s).unwrap();
    assert_eq!(stats.iter().find(|x| x.name == "pj-review").unwrap().uses, 2, "p1 + p5");
    assert!(stats.iter().all(|x| x.name != "/pj-review"), "{stats:?}");
}
