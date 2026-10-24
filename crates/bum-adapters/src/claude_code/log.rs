//! Turn a Claude Code `.jsonl` line into `RawEvent`s (spec sections 2, 4.1).
use std::path::Path;

use serde_json::Value;
use bum_core::model::{RawEvent, SkillKind, ToolUse, Usage};
use bum_core::source::{LineError, ParseCtx};

use super::TOOL_ID;

const MAX_PROMPT_CHARS: usize = 2000;

pub fn parse_line(root: &Path, path: &Path, ctx: &mut ParseCtx, line: &str) -> Result<Vec<RawEvent>, LineError> {
    let v: Value = serde_json::from_str(line).map_err(|e| LineError(e.to_string()))?;
    let mut out = Vec::new();

    let Some(session_id) = v["sessionId"].as_str() else { return Ok(out) };
    // `ai-title` has no timestamp: handle it before the time check.
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

    // The project comes from `cwd`. Claude Code in VS Code starts a log with lines that carry a session
    // id and a time but no `cwd` (`queue-operation`): wait for a line with `cwd`, and only fall back to the
    // log directory's name when a question or an answer comes first.
    let has_cwd = v["cwd"].as_str().is_some_and(|c| !c.is_empty());
    let turn_line = matches!(v["type"].as_str(), Some("user" | "assistant"));
    if !ctx.session_emitted && (has_cwd || turn_line) {
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

    // Every user line carries the promptId of its turn; assistant lines do not.
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
    // Results of tool calls: they mark the call they answer (failed or not, and how long).
    for r in content.as_array().into_iter().flatten().filter(|r| r["type"] == "tool_result") {
        if let Some(id) = r["tool_use_id"].as_str() {
            out.push(RawEvent::ToolResult { tool_use_id: id.into(), is_error: r["is_error"].as_bool().unwrap_or(false), chars: result_chars(&r["content"]) });
        }
    }
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
            tool_uses.push(ToolUse {
                id: item["id"].as_str().unwrap_or_default().into(),
                name: name.into(),
                target: tool_target(name, &item["input"]),
                target_hash: Some(input_hash(&item["input"])),
            });
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
        cost_usd: None,
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
        reasoning: 0,
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

/// What a tool call acted on, short enough to store and show: a file path (with `/` separators), a
/// search pattern, the first two words of a shell command, a skill name. Never a whole command: those
/// can carry tokens. Same tools as `composition.rs` labels.
pub fn tool_target(name: &str, input: &Value) -> Option<String> {
    let field = |k: &str| input[k].as_str().map(str::trim).filter(|v| !v.is_empty());
    match name {
        "Read" | "Edit" | "MultiEdit" | "Write" | "NotebookEdit" => field("file_path").or_else(|| field("notebook_path")).map(|p| p.replace('\\', "/")),
        "Grep" | "Glob" => field("pattern").map(String::from),
        // Leading `NAME=value` assignments are skipped: they are where secrets go.
        "Bash" => field("command").map(|c| c.split_whitespace().filter(|w| !w.contains('=')).take(2).collect::<Vec<_>>().join(" ")).filter(|c| !c.is_empty()),
        "Skill" => field("skill").map(String::from),
        _ => None,
    }
}

/// A stable 64-bit hash (FNV-1a) of a tool input, keys sorted, to tell "the same call again" without
/// storing the input. Stable across Rust versions and runs, unlike `DefaultHasher`.
pub fn input_hash(input: &Value) -> i64 {
    fn canonical(v: &Value, out: &mut String) {
        match v {
            Value::Object(m) => {
                let mut keys: Vec<&String> = m.keys().collect();
                keys.sort();
                out.push('{');
                for k in keys {
                    out.push_str(&serde_json::to_string(k).unwrap_or_default());
                    out.push(':');
                    canonical(&m[k], out);
                    out.push(',');
                }
                out.push('}');
            }
            Value::Array(a) => {
                out.push('[');
                for x in a {
                    canonical(x, out);
                    out.push(',');
                }
                out.push(']');
            }
            other => out.push_str(&other.to_string()),
        }
    }
    let mut text = String::new();
    canonical(input, &mut text);
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in text.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h as i64
}

/// Characters of a tool result's text: a string, or the `text` parts of a list.
fn result_chars(content: &Value) -> u64 {
    match content {
        Value::String(s) => s.chars().count() as u64,
        Value::Array(parts) => parts.iter().filter_map(|p| p["text"].as_str()).map(|t| t.chars().count() as u64).sum(),
        _ => 0,
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

/// Project name: the last directory component of `cwd`; otherwise the directory directly under root.
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
