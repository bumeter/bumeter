use std::path::PathBuf;

use bum_core::transcript::{clip, tool_input_summary, TranscriptItem};

fn fixture(p: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures").join(p)
}

#[test]
fn claude_transcript_pairs_questions_answers_and_tools() {
    let t = bum_adapters::claude_code::transcript::read(&fixture("claude_code/-home-u-proj/s1.jsonl")).unwrap();
    assert_eq!(t.iter().map(|x| x.key.as_str()).collect::<Vec<_>>(), vec!["p1", "p2"], "the /model command, the compact summary, and meta are not turns");
    assert_eq!(t[0].question, "giờ bạn cập nhật tiến độ nhé");
    assert_eq!(t[0].items[0], TranscriptItem::Text { text: "ok".into() });
    assert_eq!(t[0].items[1], TranscriptItem::Tool { id: "tu_1".into(), name: "Skill".into(), input: "pj-review".into(), output: Some("loaded".into()), error: false });
    assert!(matches!(&t[0].items[2], TranscriptItem::Tool { name, input, output: None, .. } if name == "Bash" && input == "ls"));
    assert_eq!(t[1].items, vec![TranscriptItem::Text { text: "là thế này".into() }]);
    assert!(t[1].interrupted && !t[0].interrupted);
}

#[test]
fn kiro_transcript_uses_prompt_keys_of_the_parser() {
    let t = bum_adapters::kiro::transcript(&fixture("kiro/cli/k1.json")).unwrap();
    assert_eq!(t.iter().map(|x| x.key.as_str()).collect::<Vec<_>>(), vec!["kiro:k1:0", "kiro:k1:1"]);
    assert_eq!(t[0].question, "đọc dự án <b>này</b>");
    let names: Vec<_> = t[0].items.iter().map(|i| match i { TranscriptItem::Tool { name, .. } => name.clone(), TranscriptItem::Text { text } => format!("text:{text}") }).collect();
    assert_eq!(names, vec!["read", "shell", "text:xong"]);
    assert!(t[1].items.is_empty());
}

#[test]
fn clip_and_tool_summary() {
    assert_eq!(clip("progress", 4), "prog…");
    assert_eq!(clip("abc", 3), "abc");
    assert_eq!(tool_input_summary(&serde_json::json!({"file_path": "/a/b.rs", "x": 1})), "/a/b.rs");
    assert_eq!(tool_input_summary(&serde_json::json!({})), "");
    assert_eq!(tool_input_summary(&serde_json::json!({"n": 1})), "{\"n\":1}");
}
