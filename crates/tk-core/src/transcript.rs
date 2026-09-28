//! Nội dung hỏi/đáp của một hội thoại, đọc thẳng từ log khi cần (không lưu DB).
use serde::Serialize;

/// Giới hạn độ dài để payload gửi lên UI không quá lớn.
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
    /// Trùng `prompts.key` trong DB.
    pub key: String,
    pub ts: String,
    pub question: String,
    pub items: Vec<TranscriptItem>,
    pub interrupted: bool,
}

/// Cắt chuỗi theo ký tự (không vỡ UTF-8), thêm `…` khi bị cắt.
pub fn clip(s: &str, max: usize) -> String {
    let mut out: String = s.chars().take(max).collect();
    if s.chars().nth(max).is_some() {
        out.push('…');
    }
    out
}

/// Tóm tắt input của tool: ưu tiên trường dễ đọc, không có thì JSON rút gọn.
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

/// Chỉ giữ lượt có `key` (nếu có).
pub fn only(turns: Vec<TranscriptTurn>, key: Option<&str>) -> Vec<TranscriptTurn> {
    match key {
        Some(k) => turns.into_iter().filter(|t| t.key == k).collect(),
        None => turns,
    }
}
