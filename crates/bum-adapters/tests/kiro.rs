use std::path::{Path, PathBuf};

use bum_adapters::kiro::Kiro;
use bum_core::engine::Engine;
use bum_core::model::RawEvent;
use bum_core::source::LogSource;
use bum_core::store::Store;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/kiro/cli")
}

#[test]
fn parses_turns_into_prompts_calls_credits_and_interruptions() {
    let k = Kiro::new(root());
    assert!(k.whole_file());
    assert!(k.accepts(&root().join("k1.json")));
    assert!(!k.accepts(&root().join("k1.jsonl")) && !k.accepts(&root().join("k1.lock")));
    let ev = k.parse_file(&root().join("k1.json")).unwrap();

    assert!(ev.iter().any(|e| matches!(e, RawEvent::Session { tool, project, .. } if tool == "kiro" && project == "TICKET-203")));
    let prompts: Vec<_> = ev.iter().filter_map(|e| match e { RawEvent::Prompt { key, text, ts, .. } => Some((key.clone(), text.clone(), ts.clone())), _ => None }).collect();
    assert_eq!(prompts.len(), 2);
    assert_eq!(prompts[0].1, "đọc dự án <b>này</b>");
    assert!(prompts[0].2.starts_with("2026-09-12T01:00:00"));

    let calls: Vec<_> = ev.iter().filter_map(|e| match e { RawEvent::ApiCall { usage, model, tool_uses, prompt_key, .. } => Some((usage.context(), model.clone(), tool_uses.len(), prompt_key.clone())), _ => None }).collect();
    assert_eq!(calls[0], (125_000, "kiro:claude-opus-4.6".to_string(), 2, Some(prompts[0].0.clone())));
    assert_eq!(calls[1].0, 500_000);
    assert!(ev.iter().any(|e| matches!(e, RawEvent::Metered { credits, .. } if (*credits - 0.13).abs() < 1e-9)));
    assert!(ev.iter().any(|e| matches!(e, RawEvent::Interrupted { prompt_key: Some(k), .. } if *k == prompts[1].0)));
}

#[test]
fn subagent_sessions_count_credits_but_are_not_user_prompts() {
    let ev = Kiro::new(root()).parse_file(&root().join("k2.json")).unwrap();
    assert!(!ev.iter().any(|e| matches!(e, RawEvent::Prompt { .. })));
    assert!(ev.iter().any(|e| matches!(e, RawEvent::ApiCall { sidechain: true, .. })));
    assert!(ev.iter().any(|e| matches!(e, RawEvent::Metered { .. })));
}

#[test]
fn engine_reingests_a_rewritten_file_without_double_counting() {
    let dir = tempfile::tempdir().unwrap();
    for f in ["k1.json", "k1.jsonl", "k2.json", "k2.jsonl"] {
        std::fs::copy(root().join(f), dir.path().join(f)).unwrap();
    }
    let mut e = Engine::new(Store::open_in_memory().unwrap(), vec![Box::new(Kiro::new(dir.path()))]);
    e.full_scan().unwrap();
    let before = e.store().usage_totals().unwrap().total();
    assert_eq!(before, 125_000 + 500_000 + 20_000);
    // Kiro ghi đè file (thêm khoảng trắng để đổi kích thước) → đọc lại, không cộng dồn
    let p = dir.path().join("k1.json");
    let mut s = std::fs::read_to_string(&p).unwrap();
    s.push(' ');
    std::fs::write(&p, s).unwrap();
    e.ingest_path(&p).unwrap();
    assert_eq!(e.store().usage_totals().unwrap().total(), before);
    assert!(e.full_scan().unwrap().lines == 0, "không đổi thì không đọc lại");
}

#[test]
fn kiro_title_is_emitted() {
    let ev = Kiro::new(root()).parse_file(&root().join("k1.json")).unwrap();
    assert!(ev.iter().any(|e| matches!(e, RawEvent::Title { title, .. } if title == "đọc dự án")));
}

#[test]
fn registry_describes_each_tool_and_its_capabilities() {
    let r = bum_adapters::registry(Path::new("/c"), Path::new("/k"));
    let ids: Vec<_> = r.iter().map(|(i, _)| i.id.as_str()).collect();
    assert_eq!(ids, vec!["claude_code", "kiro"]);
    let (claude, root) = &r[0];
    assert_eq!((claude.letter.as_str(), claude.real_tokens, claude.composition, claude.limits, claude.credits), ("C", true, true, true, false));
    assert_eq!(root, &PathBuf::from("/c"));
    let kiro = &r[1].0;
    assert_eq!((kiro.letter.as_str(), kiro.real_tokens, kiro.composition, kiro.credits, kiro.transcript), ("K", false, false, true, true));
    // Logo và màu theo thương hiệu; UI tải từ `tool-icons/<icon>`.
    assert_eq!((claude.icon.as_deref(), claude.color.as_str()), (Some("claude_code.svg"), "#d77655"));
    assert_eq!((kiro.icon.as_deref(), kiro.color.as_str()), (Some("kiro.svg"), "#9046ff"));
}

/// Kiro CLI (từ ~06/2026) ghi `session_created_reason: "subagent"` cho cả phiên người dùng chat;
/// phiên có `.history` (lịch sử gõ) là phiên người dùng.
#[test]
fn session_marked_subagent_but_with_history_is_a_user_session() {
    let ev = Kiro::new(root()).parse_file(&root().join("k3.json")).unwrap();
    assert_eq!(ev.iter().filter(|e| matches!(e, RawEvent::Prompt { .. })).count(), 2);
    assert!(ev.iter().any(|e| matches!(e, RawEvent::Title { .. })));
    assert!(!ev.iter().any(|e| matches!(e, RawEvent::ApiCall { sidechain: true, .. })));
}

/// DB cũ đã đọc file Kiro bằng parser cũ (mất câu hỏi): mở DB bản mới phải đọc lại, không cộng dồn.
#[test]
fn old_database_rereads_kiro_files_once_without_double_counting() {
    let dir = tempfile::tempdir().unwrap();
    let logs = dir.path().join("cli");
    std::fs::create_dir_all(&logs).unwrap();
    for f in ["k3.json", "k3.jsonl", "k3.history"] {
        std::fs::copy(root().join(f), logs.join(f)).unwrap();
    }
    let db = dir.path().join("t.db");
    let total = {
        let mut e = Engine::new(Store::open(&db).unwrap(), vec![Box::new(Kiro::new(&logs))]);
        e.full_scan().unwrap();
        e.store().usage_totals().unwrap().total()
    };
    // Giả lập trạng thái do parser cũ để lại: có offset, không có câu hỏi, schema v4.
    rusqlite::Connection::open(&db).unwrap()
        .execute_batch("DELETE FROM prompts; DELETE FROM session_titles; PRAGMA user_version = 4;").unwrap();

    let mut e = Engine::new(Store::open(&db).unwrap(), vec![Box::new(Kiro::new(&logs))]);
    e.full_scan().unwrap();
    assert_eq!(e.store().prompt_items(None, None, None, None).unwrap().len(), 2);
    assert_eq!(e.store().usage_totals().unwrap().total(), total);
}
