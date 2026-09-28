//! Adapter cho Kiro CLI: `~/.kiro/sessions/cli/<id>.json` (metadata từng lượt, bị ghi đè toàn bộ)
//! + `<id>.jsonl` (câu hỏi, câu trả lời, tool). Kiro không ghi token: context =
//! `context_usage_percentage × context_window_tokens`; chi phí tính bằng credit.
use std::path::{Path, PathBuf};

use serde_json::Value;
use tk_core::model::{RawEvent, ToolUse, Usage};
use tk_core::source::{LineError, LogSource, ParseCtx};

pub const TOOL_ID: &str = "kiro";
const DEFAULT_WINDOW: f64 = 1_000_000.0;

pub struct Kiro {
    root: PathBuf,
}

impl Kiro {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// `TK_KIRO_DIR` nếu có, không thì `~/.kiro/sessions/cli`.
    pub fn default_root() -> PathBuf {
        if let Some(d) = std::env::var_os("TK_KIRO_DIR") {
            return PathBuf::from(d);
        }
        dirs::home_dir().unwrap_or_default().join(".kiro").join("sessions").join("cli")
    }
}

fn epoch_to_rfc3339(secs: i64) -> String {
    chrono::DateTime::from_timestamp(secs, 0).map(|t| t.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)).unwrap_or_default()
}

fn text_of(content: &Value) -> String {
    content.as_array().into_iter().flatten().filter(|c| c["kind"] == "text").filter_map(|c| c["data"].as_str()).collect::<Vec<_>>().join("\n")
}

/// Mỗi câu hỏi (Prompt) và tên các tool dùng sau nó, theo thứ tự.
fn read_prompts(jsonl: &Path) -> Vec<(String, String, Vec<ToolUse>)> {
    let bytes = std::fs::read(jsonl).unwrap_or_default();
    let mut out: Vec<(String, String, Vec<ToolUse>)> = Vec::new();
    for line in String::from_utf8_lossy(&bytes).lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
        let d = &v["data"];
        match v["kind"].as_str() {
            Some("Prompt") => {
                let ts = d["meta"]["timestamp"].as_i64().map(epoch_to_rfc3339).unwrap_or_default();
                out.push((ts, text_of(&d["content"]), Vec::new()));
            }
            Some("AssistantMessage") => {
                if let Some(last) = out.last_mut() {
                    for c in d["content"].as_array().into_iter().flatten().filter(|c| c["kind"] == "toolUse") {
                        let t = &c["data"];
                        last.2.push(ToolUse { id: t["toolUseId"].as_str().unwrap_or_default().into(), name: t["name"].as_str().unwrap_or_default().into() });
                    }
                }
            }
            _ => {}
        }
    }
    out
}

pub fn info() -> tk_core::source::SourceInfo {
    tk_core::source::SourceInfo {
        id: TOOL_ID.into(), label: "Kiro".into(), letter: "K".into(), color: "#9046ff".into(), icon: Some("kiro.svg".into()),
        real_tokens: false, composition: false, limits: false, credits: true, transcript: true,
    }
}

impl LogSource for Kiro {
    fn id(&self) -> &'static str {
        TOOL_ID
    }

    fn info(&self) -> tk_core::source::SourceInfo {
        info()
    }

    fn watch_roots(&self) -> Vec<PathBuf> {
        vec![self.root.clone()]
    }

    fn accepts(&self, path: &Path) -> bool {
        path.starts_with(&self.root) && path.extension().is_some_and(|e| e == "json")
    }

    fn parse_line(&self, _path: &Path, _ctx: &mut ParseCtx, _line: &str) -> Result<Vec<RawEvent>, LineError> {
        Err(LineError("Kiro đọc cả file".into()))
    }

    fn whole_file(&self) -> bool {
        true
    }

    fn parse_file(&self, path: &Path) -> Result<Vec<RawEvent>, LineError> {
        let raw = std::fs::read(path).map_err(|e| LineError(e.to_string()))?;
        let v: Value = serde_json::from_slice(&raw).map_err(|e| LineError(e.to_string()))?;
        let Some(sid) = v["session_id"].as_str() else { return Ok(Vec::new()) };
        let st = &v["session_state"];
        let model_info = &st["rts_model_state"]["model_info"];
        let model = format!("kiro:{}", model_info["model_id"].as_str().unwrap_or("auto"));
        let window = model_info["context_window_tokens"].as_f64().unwrap_or(DEFAULT_WINDOW);
        let subagent = v["session_created_reason"].as_str() == Some("subagent");
        let turns = st["conversation_metadata"]["user_turn_metadatas"].as_array().cloned().unwrap_or_default();
        let prompts = read_prompts(&path.with_extension("jsonl"));
        let cwd = v["cwd"].as_str().map(str::to_string);
        let project = cwd.as_deref().and_then(|c| Path::new(c).file_name()).map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "kiro".into());
        let first_ts = prompts.first().map(|p| p.0.clone()).filter(|t| !t.is_empty())
            .or_else(|| v["created_at"].as_str().map(str::to_string)).unwrap_or_default();

        let mut out = vec![RawEvent::Session { session_id: sid.into(), tool: TOOL_ID.into(), project, cwd, version: None, ts: first_ts.clone() }];
        if let Some(title) = v["title"].as_str().filter(|t| !t.trim().is_empty() && !subagent) {
            out.push(RawEvent::Title { session_id: sid.into(), title: title.chars().take(200).collect() });
        }
        for (i, t) in turns.iter().enumerate() {
            let (p_ts, text, tools) = prompts.get(i).cloned().unwrap_or_default();
            let key = format!("kiro:{sid}:{i}");
            let request_id = format!("kiro:{sid}:{i}");
            let ts = t["end_timestamp"].as_str().map(str::to_string).filter(|s| !s.is_empty()).unwrap_or_else(|| p_ts.clone());
            let prompt_ts = if p_ts.is_empty() { ts.clone() } else { p_ts };
            let is_prompt = !subagent && !text.trim().is_empty();
            if is_prompt {
                out.push(RawEvent::Prompt { key: key.clone(), session_id: sid.into(), ts: prompt_ts, text: text.chars().take(2000).collect() });
            }
            let context = (t["context_usage_percentage"].as_f64().unwrap_or(0.0) / 100.0 * window).round() as u64;
            out.push(RawEvent::ApiCall {
                request_id: request_id.clone(),
                session_id: sid.into(),
                prompt_key: is_prompt.then(|| key.clone()),
                ts: ts.clone(),
                model: model.clone(),
                usage: Usage { input: context, ..Default::default() },
                tool_uses: tools,
                sidechain: subagent,
            });
            let credits: f64 = t["metering_usage"].as_array().into_iter().flatten().filter_map(|m| m["value"].as_f64()).sum();
            if credits > 0.0 {
                out.push(RawEvent::Metered { request_id, credits });
            }
            if is_prompt && matches!(t["end_reason"].as_str(), Some("Cancelled" | "ToolUseRejected")) {
                out.push(RawEvent::Interrupted { prompt_key: Some(key), session_id: sid.into(), ts });
            }
        }
        Ok(out)
    }
}

fn result_text(content: &Value) -> String {
    content
        .as_array()
        .into_iter()
        .flatten()
        .map(|c| match &c["data"] {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Hỏi/đáp của một phiên Kiro: `path` là file `.json`, nội dung nằm ở `.jsonl` cùng tên.
/// Lượt thứ i ứng với prompt key `kiro:<session_id>:<i>`.
pub fn transcript(path: &Path) -> std::io::Result<Vec<tk_core::transcript::TranscriptTurn>> {
    use tk_core::transcript::{clip, tool_input_summary, TranscriptItem, TranscriptTurn, MAX_TEXT, MAX_TOOL_OUTPUT};
    let meta: Value = std::fs::read(path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    let sid = meta["session_id"].as_str().map(str::to_string).unwrap_or_else(|| path.file_stem().unwrap_or_default().to_string_lossy().to_string());
    let ends = meta["session_state"]["conversation_metadata"]["user_turn_metadatas"].as_array().cloned().unwrap_or_default();
    let bytes = std::fs::read(path.with_extension("jsonl"))?;
    let mut turns: Vec<TranscriptTurn> = Vec::new();
    for line in String::from_utf8_lossy(&bytes).lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
        let d = &v["data"];
        match v["kind"].as_str() {
            Some("Prompt") => {
                let i = turns.len();
                turns.push(TranscriptTurn {
                    key: format!("kiro:{sid}:{i}"),
                    ts: d["meta"]["timestamp"].as_i64().map(epoch_to_rfc3339).unwrap_or_default(),
                    question: clip(&text_of(&d["content"]), MAX_TEXT),
                    items: Vec::new(),
                    interrupted: matches!(ends.get(i).and_then(|t| t["end_reason"].as_str()), Some("Cancelled" | "ToolUseRejected")),
                });
            }
            Some("AssistantMessage") => {
                let Some(turn) = turns.last_mut() else { continue };
                for c in d["content"].as_array().into_iter().flatten() {
                    match c["kind"].as_str() {
                        Some("text") => {
                            let text = c["data"].as_str().unwrap_or_default();
                            if !text.trim().is_empty() {
                                turn.items.push(TranscriptItem::Text { text: clip(text, MAX_TEXT) });
                            }
                        }
                        Some("toolUse") => {
                            let t = &c["data"];
                            turn.items.push(TranscriptItem::Tool {
                                id: t["toolUseId"].as_str().unwrap_or_default().into(),
                                name: t["name"].as_str().unwrap_or_default().into(),
                                input: tool_input_summary(&t["input"]),
                                output: None,
                                error: false,
                            });
                        }
                        _ => {}
                    }
                }
            }
            Some("ToolResults") => {
                let Some(turn) = turns.last_mut() else { continue };
                for r in d["content"].as_array().into_iter().flatten().filter(|r| r["kind"] == "toolResult") {
                    let r = &r["data"];
                    let id = r["toolUseId"].as_str().unwrap_or_default();
                    for item in turn.items.iter_mut().rev() {
                        if let TranscriptItem::Tool { id: tid, output, error, .. } = item {
                            if tid == id {
                                *output = Some(clip(&result_text(&r["content"]), MAX_TOOL_OUTPUT));
                                *error = r["status"].as_str().is_some_and(|s| s != "success");
                                break;
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    Ok(turns)
}
