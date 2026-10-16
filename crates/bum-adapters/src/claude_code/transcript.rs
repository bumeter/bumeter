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
