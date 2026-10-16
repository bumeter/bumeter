use bum_core::attribution::{attribute, AttributedSkill, How};
use bum_core::model::SkillKind;

fn p(k: &str, ts: &str) -> (String, String) {
    (k.into(), ts.into())
}

fn d(k: &str, s: &str) -> (String, String, SkillKind) {
    (k.into(), s.into(), SkillKind::Direct)
}

#[test]
fn next_prompt_continues_the_last_direct_skill() {
    let r = attribute(&[p("a", "2026-09-11T04:00:00Z"), p("b", "2026-09-11T04:07:00Z")], &[d("a", "pj-review")], &[]);
    assert_eq!(r["a"], vec![AttributedSkill { skill: "pj-review".into(), how: How::Direct }]);
    assert_eq!(r["b"], vec![AttributedSkill { skill: "pj-review".into(), how: How::Continued }]);
}

#[test]
fn compaction_breaks_the_chain() {
    let r = attribute(&[p("a", "2026-09-11T04:00:00Z"), p("b", "2026-09-11T04:07:00Z")], &[d("a", "pj-review")], &["2026-09-11T04:05:00Z".into()]);
    assert!(r.get("b").is_none_or(|v| v.is_empty()));
}

#[test]
fn a_gap_over_30_minutes_breaks_the_chain() {
    let r = attribute(&[p("a", "2026-09-11T04:00:00Z"), p("b", "2026-09-11T04:31:00Z")], &[d("a", "pj-review")], &[]);
    assert!(r.get("b").is_none_or(|v| v.is_empty()));
}

#[test]
fn a_new_direct_skill_replaces_the_chain() {
    let r = attribute(
        &[p("a", "2026-09-11T04:00:00Z"), p("b", "2026-09-11T04:01:00Z"), p("c", "2026-09-11T04:02:00Z")],
        &[d("a", "pj-review"), d("b", "pj-estimate")],
        &[],
    );
    assert_eq!(r["b"], vec![AttributedSkill { skill: "pj-estimate".into(), how: How::Direct }]);
    assert_eq!(r["c"], vec![AttributedSkill { skill: "pj-estimate".into(), how: How::Continued }]);
}

#[test]
fn commands_are_listed_but_do_not_start_a_chain() {
    let r = attribute(
        &[p("a", "2026-09-11T04:00:00Z"), p("b", "2026-09-11T04:01:00Z")],
        &[("a".into(), "/model".into(), SkillKind::Command)],
        &[],
    );
    assert_eq!(r["a"], vec![AttributedSkill { skill: "/model".into(), how: How::Command }]);
    assert!(r.get("b").is_none_or(|v| v.is_empty()));
}
