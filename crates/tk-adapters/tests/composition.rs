use std::path::Path;

use tk_adapters::claude_code::composition::{analyze_file, call_detail, Category};

fn fixture() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/claude_code_session.jsonl")
}

fn part(call: &tk_adapters::claude_code::composition::CallPoint, c: Category) -> u64 {
    call.parts.iter().find(|p| p.0 == c).map(|p| p.1).unwrap_or(0)
}

#[test]
fn every_call_sums_to_its_real_context() {
    let a = analyze_file(&fixture()).unwrap();
    assert_eq!(a.calls.len(), 6);
    for c in &a.calls {
        assert_eq!(c.parts.iter().map(|p| p.1).sum::<u64>(), c.context, "{}", c.request_id);
    }
}

#[test]
fn categories_accumulate_and_reset_after_compaction() {
    let a = analyze_file(&fixture()).unwrap();
    let c1 = &a.calls[0];
    assert_eq!(part(c1, Category::User), 10);
    assert_eq!(part(c1, Category::FileRead), 0);
    assert_eq!(part(c1, Category::Thinking), 0, "thinking của r1 chỉ vào context từ lượt sau");
    assert_eq!(part(c1, Category::System), 1010 - 10 - part(c1, Category::ToolIo), "phần không đo được thuộc Hệ thống");
    assert_eq!(part(&a.calls[1], Category::Thinking), 200, "thinking ẩn: ước lượng từ độ dài chữ ký");
    let c4 = &a.calls[3];
    assert_eq!(part(c4, Category::FileRead), 3000, "3 lần đọc a.rs");
    let c5 = &a.calls[4];
    assert_eq!(part(c5, Category::Skill), 200);
    let c6 = &a.calls[5];
    assert_eq!(part(c6, Category::FileRead), 0, "compact xoá context cũ");
    assert!(part(c6, Category::System) >= 200, "tóm tắt compact tính vào Hệ thống");
    assert_eq!(a.compactions, vec![5]);
    let prompts: Vec<_> = a.prompts.iter().map(|p| (p.call_index, p.key.as_str())).collect();
    assert_eq!(prompts, vec![(0, "p1"), (5, "p2")]);
}

#[test]
fn suggestions_point_at_repeated_reads_and_huge_outputs() {
    let a = analyze_file(&fixture()).unwrap();
    use tk_adapters::claude_code::composition::Suggestion;
    assert!(a.suggestions.iter().any(|s| matches!(s, Suggestion::RepeatedRead { file, times: 3, .. } if file == "/src/a.rs")), "{:?}", a.suggestions);
    assert!(a.suggestions.iter().any(|s| matches!(s, Suggestion::LongOutput { label, .. } if label.contains("cat big.log"))), "{:?}", a.suggestions);
}

#[test]
fn call_detail_lists_tools_heaviest_blocks_and_truncates() {
    let d = call_detail(&fixture(), "r5").unwrap();
    assert_eq!(d.tools.len(), 1);
    let bash = &d.tools[0];
    assert_eq!(bash.name, "Bash");
    assert_eq!(bash.summary, "cat big.log");
    assert!(bash.result.chars().count() <= 4000 && bash.truncated);
    assert!(d.top_blocks[0].tokens >= d.top_blocks.last().unwrap().tokens);
    assert!(d.top_blocks.iter().any(|b| b.label.contains("/src/a.rs")));

    let edit = &call_detail(&fixture(), "r6").unwrap().tools[0];
    assert_eq!(edit.name, "Edit");
    assert_eq!(edit.old_text.as_deref(), Some("let a = 1;\nlet b = 2;"));
    assert_eq!(edit.new_text.as_deref(), Some("let a = 1;\nlet b = 3;"));
    assert!(call_detail(&fixture(), "nope").is_none());
}

#[test]
fn attachments_are_measured_from_all_text_fields_but_prompt_snapshots_are_skipped() {
    let d = call_detail(&fixture(), "r1").unwrap();
    assert!(d.top_blocks.iter().any(|b| b.label.contains("edited_text_file") && b.tokens == 100), "{:?}", d.top_blocks);
    assert!(!d.top_blocks.iter().any(|b| b.label.contains("prompt_snapshot")));
}

#[test]
fn invalid_utf8_missing_usage_and_huge_commands_are_tolerated() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("s.jsonl");
    let mut bytes = std::fs::read(fixture()).unwrap();
    let long = "x".repeat(20_000);
    bytes.extend_from_slice(format!(
        "{{\"type\":\"assistant\",\"sessionId\":\"s9\",\"timestamp\":\"t\",\"requestId\":\"rX\",\"message\":{{\"id\":\"mX\",\"model\":\"claude-opus-5\",\"content\":[]}}}}\n\
         {{\"type\":\"assistant\",\"sessionId\":\"s9\",\"timestamp\":\"t\",\"requestId\":\"rY\",\"message\":{{\"id\":\"mY\",\"model\":\"claude-opus-5\",\"content\":[{{\"type\":\"tool_use\",\"id\":\"tY\",\"name\":\"Bash\",\"input\":{{\"command\":\"cat {long}\"}}}}],\"usage\":{{\"input_tokens\":1,\"cache_creation_input_tokens\":0,\"cache_read_input_tokens\":9000,\"output_tokens\":1}}}}}}\n").as_bytes());
    bytes.extend_from_slice(b"{\"type\":\"user\",\"x\":\"\xff\xfe\"}\n");
    std::fs::write(&p, bytes).unwrap();
    let a = analyze_file(&p).expect("byte UTF-8 hỏng không được làm hỏng cả phiên");
    assert!(a.calls.iter().all(|c| c.request_id != "rX"), "lượt gọi thiếu usage bị bỏ qua");
    let d = call_detail(&p, "rY").unwrap();
    assert!(d.tools[0].summary.chars().count() <= 161, "nhãn được cắt ngắn");
}
