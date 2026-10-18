//! Re-read the Q&A of a Claude Code session from its main `.jsonl` file (skipping subagents).
use std::path::Path;

use serde_json::Value;
use bum_core::transcript::{clip, tool_input_summary, TranscriptItem, TranscriptTurn, MAX_TEXT, MAX_TOOL_OUTPUT};

use super::log::{command_name, extract_text, has_tool_result};

pub fn read(path: &Path) -> std::io::Result<Vec<TranscriptTurn>> {
    let bytes = std::fs::read(path)?;
    let mut turns: Vec<TranscriptTurn> = Vec::new();
    for line in String::from_utf8_lossy(&bytes).lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
        if v["isSidechain"].as_bool() == Some(true) {
            continue;
        }
        let content = &v["message"]["content"];
        match v["type"].as_str() {
            Some("user") => on_user(&v, content, &mut turns),
            Some("assistant") => {
                let Some(turn) = turns.last_mut() else { continue };
                if v["error"].is_string() || v["message"]["model"] == "<synthetic>" {
                    continue;
                }
                for c in content.as_array().into_iter().flatten() {
                    match c["type"].as_str() {
                        Some("text") => {
                            let text = c["text"].as_str().unwrap_or_default();
                            if !text.trim().is_empty() {
                                turn.items.push(TranscriptItem::Text { text: clip(text, MAX_TEXT) });
                            }
                        }
                        Some("tool_use") => turn.items.push(TranscriptItem::Tool {
                            id: c["id"].as_str().unwrap_or_default().into(),
                            name: c["name"].as_str().unwrap_or_default().into(),
                            input: tool_input_summary(&c["input"]),
                            output: None,
                            error: false,
                        }),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    Ok(turns)
}

fn on_user(v: &Value, content: &Value, turns: &mut Vec<TranscriptTurn>) {
    if has_tool_result(content) {
        let Some(turn) = turns.last_mut() else { return };
        for r in content.as_array().into_iter().flatten().filter(|r| r["type"] == "tool_result") {
            let id = r["tool_use_id"].as_str().unwrap_or_default();
            let out = match &r["content"] {
                Value::String(s) => s.clone(),
                other => extract_text(other),
            };
            for item in turn.items.iter_mut().rev() {
                if let TranscriptItem::Tool { id: tid, output, error, .. } = item {
                    if tid == id {
                        *output = Some(clip(&out, MAX_TOOL_OUTPUT));
                        *error = r["is_error"].as_bool().unwrap_or(false);
                        break;
                    }
                }
            }
        }
        return;
    }
    let text = extract_text(content);
    if text.starts_with("[Request interrupted") {
        if let Some(t) = turns.last_mut() {
            t.interrupted = true;
        }
        return;
    }
    let is_prompt = v["isMeta"].as_bool() != Some(true)
        && v["isCompactSummary"].as_bool() != Some(true)
        && v.get("toolUseResult").is_none()
        && command_name(&text).is_none()
        && !text.trim().is_empty()
        && !text.trim_start().starts_with('<');
    if let (true, Some(key)) = (is_prompt, v["promptId"].as_str()) {
        turns.push(TranscriptTurn {
            key: key.into(),
            ts: v["timestamp"].as_str().unwrap_or_default().into(),
            question: clip(&text, MAX_TEXT),
            items: Vec::new(),
            interrupted: false,
        });
    }
}

/// Bytes read from the end of a transcript to find the latest call: enough for many calls, cheap for a
/// status line that runs after every message.
const TAIL_BYTES: u64 = 512 * 1024;

/// The context the session is carrying now: `input + cache writes + cache reads` of the latest main-chain
/// call. None when the tail holds no call, or when the latest event is a compaction (the next call will
/// tell the new size; showing the old one would say "/compact" right after compacting).
pub fn current_context(path: &Path) -> Option<u64> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(path).ok()?;
    let len = f.metadata().ok()?.len();
    f.seek(SeekFrom::Start(len.saturating_sub(TAIL_BYTES))).ok()?;
    let mut bytes = Vec::new();
    f.read_to_end(&mut bytes).ok()?;
    for line in String::from_utf8_lossy(&bytes).lines().rev() {
        let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
        if v["isSidechain"].as_bool() == Some(true) {
            continue;
        }
        if v["isCompactSummary"].as_bool() == Some(true) {
            return None;
        }
        let u = &v["message"]["usage"];
        if v["type"].as_str() == Some("assistant") && u.is_object() {
            let n = |k: &str| u[k].as_u64().unwrap_or(0);
            return Some(n("input_tokens") + n("cache_creation_input_tokens") + n("cache_read_input_tokens"));
        }
    }
    None
}

#[cfg(test)]
mod context_tests {
    use super::current_context;

    fn file(lines: &[&str]) -> tempfile::NamedTempFile {
        let f = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(f.path(), lines.join("\n")).unwrap();
        f
    }

    const CALL_1: &str = r#"{"type":"assistant","message":{"usage":{"input_tokens":10,"cache_creation_input_tokens":100,"cache_read_input_tokens":1000,"output_tokens":5}}}"#;
    const CALL_2: &str = r#"{"type":"assistant","message":{"usage":{"input_tokens":20,"cache_creation_input_tokens":200,"cache_read_input_tokens":300000,"output_tokens":5}}}"#;
    const SUB: &str = r#"{"type":"assistant","isSidechain":true,"message":{"usage":{"input_tokens":1,"cache_read_input_tokens":5}}}"#;
    const USER: &str = r#"{"type":"user","message":{"content":"next question"}}"#;
    const COMPACT: &str = r#"{"type":"user","isCompactSummary":true,"message":{"content":"summary"}}"#;

    #[test]
    fn the_latest_main_chain_call_wins() {
        let f = file(&[CALL_1, CALL_2, SUB, USER, "not json"]);
        assert_eq!(current_context(f.path()), Some(300_220));
    }

    #[test]
    fn right_after_a_compaction_the_size_is_unknown() {
        assert_eq!(current_context(file(&[CALL_2, COMPACT]).path()), None);
        assert_eq!(current_context(file(&[CALL_2, COMPACT, CALL_1]).path()), Some(1110));
    }

    #[test]
    fn no_call_or_no_file_is_none() {
        assert_eq!(current_context(file(&[USER]).path()), None);
        assert_eq!(current_context(std::path::Path::new("/nonexistent/x.jsonl")), None);
    }
}
