//! Chuyển một dòng `.jsonl` của Claude Code thành `RawEvent` (spec mục 2, 4.1).
use std::path::Path;

use serde_json::Value;
use tk_core::model::{RawEvent, SkillKind, ToolUse, Usage};
use tk_core::source::{LineError, ParseCtx};

use super::TOOL_ID;

const MAX_PROMPT_CHARS: usize = 2000;

pub fn parse_line(root: &Path, path: &Path, ctx: &mut ParseCtx, line: &str) -> Result<Vec<RawEvent>, LineError> {
    let v: Value = serde_json::from_str(line).map_err(|e| LineError(e.to_string()))?;
    let mut out = Vec::new();

    let Some(session_id) = v["sessionId"].as_str() else { return Ok(out) };
    // `ai-title` không có timestamp: xử lý trước bước kiểm tra thời gian.
    if v["type"] == "ai-title" {
        if let Some(title) = v["aiTitle"].as_str().filter(|t| !t.trim().is_empty()) {
            out.push(RawEvent::Title { session_id: session_id.into(), title: title.chars().take(200).collect() });
        }
        return Ok(out);
    }
    let ts = v["timestamp"].as_str().unwrap_or_default().to_string();
    if ts.is_empty() {
        return Ok(out);
    }
    let sidechain = v["isSidechain"].as_bool().unwrap_or(false);

    if !ctx.session_emitted {
        let cwd = v["cwd"].as_str().map(str::to_string);
        out.push(RawEvent::Session {
            session_id: session_id.to_string(),
            tool: TOOL_ID.to_string(),
            project: project_label(root, path, cwd.as_deref()),
            cwd,
            version: v["version"].as_str().map(str::to_string),
            ts: ts.clone(),
        });
        ctx.session_emitted = true;
    }

    // Dòng user nào cũng mang promptId của lượt hỏi; dòng assistant thì không.
    let previous_prompt = ctx.current_prompt.clone();
    if let Some(pk) = v["promptId"].as_str() {
        ctx.current_prompt = Some(pk.to_string());
    }

    match v["type"].as_str() {
        Some("user") => parse_user(&v, session_id, &ts, sidechain, previous_prompt, ctx, &mut out),
        Some("assistant") => parse_assistant(&v, session_id, &ts, sidechain, ctx, &mut out),
        _ => {}
    }
    Ok(out)
}

fn parse_user(
    v: &Value,
    session_id: &str,
    ts: &str,
    sidechain: bool,
    previous_prompt: Option<String>,
    ctx: &ParseCtx,
    out: &mut Vec<RawEvent>,
) {
    if v["isCompactSummary"].as_bool() == Some(true) {
        out.push(RawEvent::Compacted { session_id: session_id.into(), ts: ts.into() });
        return;
    }
    let content = &v["message"]["content"];
    let text = extract_text(content);

    if let Some(cmd) = command_name(&text) {
        out.push(RawEvent::Skill {
            prompt_key: ctx.current_prompt.clone(),
            session_id: session_id.into(),
            ts: ts.into(),
            skill: cmd,
            kind: SkillKind::Command,
        });
        return;
    }
    if text.starts_with("[Request interrupted") {
        out.push(RawEvent::Interrupted { prompt_key: previous_prompt, session_id: session_id.into(), ts: ts.into() });
        return;
    }

    let is_prompt = !sidechain
        && v["isMeta"].as_bool() != Some(true)
        && v.get("toolUseResult").is_none()
        && !has_tool_result(content)
        && !text.trim().is_empty()
        && !text.trim_start().starts_with('<');
    if let (true, Some(key)) = (is_prompt, v["promptId"].as_str()) {
        out.push(RawEvent::Prompt {
            key: key.into(),
            session_id: session_id.into(),
            ts: ts.into(),
            text: text.chars().take(MAX_PROMPT_CHARS).collect(),
        });
    }
}

fn parse_assistant(v: &Value, session_id: &str, ts: &str, sidechain: bool, ctx: &ParseCtx, out: &mut Vec<RawEvent>) {
    if v["error"].as_str() == Some("rate_limit") {
        let q = &v["quotaLimits"];
        if let (Some(limit_type), Some(resets_at)) = (q["rateLimitType"].as_str(), q["resetsAt"].as_i64()) {
            out.push(RawEvent::RateLimited { ts: ts.into(), limit_type: limit_type.into(), resets_at });
        }
        return;
    }
    let msg = &v["message"];
    let model = msg["model"].as_str().unwrap_or_default();
    if model.is_empty() || model == "<synthetic>" {
        return;
    }
    let Some(request_id) = v["requestId"].as_str().or(msg["id"].as_str()) else { return };

    let mut tool_uses = Vec::new();
    if let Some(items) = msg["content"].as_array() {
        for item in items.iter().filter(|i| i["type"] == "tool_use") {
            let name = item["name"].as_str().unwrap_or_default();
            tool_uses.push(ToolUse { id: item["id"].as_str().unwrap_or_default().into(), name: name.into() });
            if name == "Skill" {
                if let Some(skill) = item["input"]["skill"].as_str() {
                    push_direct_skill(out, ctx, session_id, ts, skill);
                }
            }
        }
    }
    if let Some(skill) = v["attributionSkill"].as_str() {
        push_direct_skill(out, ctx, session_id, ts, skill);
    }

    out.push(RawEvent::ApiCall {
        request_id: request_id.into(),
        session_id: session_id.into(),
        prompt_key: ctx.current_prompt.clone(),
        ts: ts.into(),
        model: model.into(),
        usage: parse_usage(&msg["usage"]),
        tool_uses,
        sidechain,
    });
}

fn push_direct_skill(out: &mut Vec<RawEvent>, ctx: &ParseCtx, session_id: &str, ts: &str, skill: &str) {
    out.push(RawEvent::Skill {
        prompt_key: ctx.current_prompt.clone(),
        session_id: session_id.into(),
        ts: ts.into(),
        skill: skill.into(),
        kind: SkillKind::Direct,
    });
}

fn parse_usage(u: &Value) -> Usage {
    let n = |k: &str| u[k].as_u64().unwrap_or(0);
    let cache_creation = n("cache_creation_input_tokens");
    let cw1h = u["cache_creation"]["ephemeral_1h_input_tokens"].as_u64().unwrap_or(0).min(cache_creation);
    Usage {
        input: n("input_tokens"),
        cache_write_5m: cache_creation - cw1h,
        cache_write_1h: cw1h,
        cache_read: n("cache_read_input_tokens"),
        output: n("output_tokens"),
    }
}

pub(super) fn extract_text(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(items) => items
            .iter()
            .filter(|i| i["type"] == "text")
            .filter_map(|i| i["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

pub(super) fn has_tool_result(content: &Value) -> bool {
    content.as_array().is_some_and(|items| items.iter().any(|i| i["type"] == "tool_result"))
}

pub(super) fn command_name(text: &str) -> Option<String> {
    let start = text.find("<command-name>")? + "<command-name>".len();
    let end = text[start..].find("</command-name>")? + start;
    let name = text[start..end].trim();
    (!name.is_empty()).then(|| if name.starts_with('/') { name.to_string() } else { format!("/{name}") })
}

/// Tên project: thư mục cuối của `cwd`, không có thì tên thư mục ngay dưới root.
fn project_label(root: &Path, path: &Path, cwd: Option<&str>) -> String {
    if let Some(name) = cwd.and_then(|c| Path::new(c).file_name()).and_then(|n| n.to_str()) {
        return name.to_string();
    }
    path.strip_prefix(root)
        .ok()
        .and_then(|rel| rel.components().next())
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .unwrap_or_default()
}
