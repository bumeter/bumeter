//! Question/answer content of a conversation, read straight from the logs on demand (not stored in the DB).
use serde::Serialize;

/// Length limits so the payload sent to the UI does not get too large.
pub const MAX_TEXT: usize = 20_000;
pub const MAX_TOOL_INPUT: usize = 300;
pub const MAX_TOOL_OUTPUT: usize = 1_500;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TranscriptItem {
    Text { text: String },
    Tool { id: String, name: String, input: String, output: Option<String>, error: bool },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TranscriptTurn {
    /// Matches `prompts.key` in the DB.
    pub key: String,
    pub ts: String,
    pub question: String,
    pub items: Vec<TranscriptItem>,
    pub interrupted: bool,
}

/// Trim the string by characters (without breaking UTF-8), appending `…` when trimmed.
pub fn clip(s: &str, max: usize) -> String {
    let mut out: String = s.chars().take(max).collect();
    if s.chars().nth(max).is_some() {
        out.push('…');
    }
    out
}

/// Summarize tool input: prefer a readable field; otherwise a compact JSON.
pub fn tool_input_summary(input: &serde_json::Value) -> String {
    const KEYS: [&str; 10] = ["command", "file_path", "path", "pattern", "skill", "description", "url", "query", "prompt", "__tool_use_purpose"];
    for k in KEYS {
        if let Some(s) = input[k].as_str().filter(|s| !s.is_empty()) {
            return clip(s, MAX_TOOL_INPUT);
        }
    }
    match input {
        serde_json::Value::Null => String::new(),
        serde_json::Value::Object(m) if m.is_empty() => String::new(),
        v => clip(&v.to_string(), MAX_TOOL_INPUT),
    }
}

/// Keep only the turn with `key` (if any).
pub fn only(turns: Vec<TranscriptTurn>, key: Option<&str>) -> Vec<TranscriptTurn> {
    match key {
        Some(k) => turns.into_iter().filter(|t| t.key == k).collect(),
        None => turns,
    }
}
