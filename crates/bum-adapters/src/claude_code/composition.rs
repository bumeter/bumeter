//! Analyze a session: estimate the context composition of each API call (spec section 6.1).
//! Reads the log file directly on open (nothing stored in the DB). Context that is not in the log
//! (system prompt, tool definitions, CLAUDE.md, hidden thinking) = real `context` − measured parts → the System group.
use std::collections::{HashMap, HashSet};
use std::path::Path;

use serde::Serialize;
use serde_json::Value;

const CHARS_PER_TOKEN: f64 = 3.5;
const RESULT_LIMIT: usize = 4000;
const TOP_BLOCKS: usize = 8;
const BIG_OUTPUT_TOKENS: u64 = 10_000;
const REPEATED_READS: u32 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Category { System, Skill, FileRead, ToolIo, Thinking, Output, User }

const ORDER: [Category; 7] = [Category::System, Category::Skill, Category::FileRead, Category::ToolIo, Category::Thinking, Category::Output, Category::User];

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CallPoint {
    pub request_id: String,
    pub ts: String,
    pub context: u64,
    pub output: u64,
    /// In `ORDER` order; the sum equals `context` exactly.
    pub parts: Vec<(Category, u64)>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PromptMark { pub call_index: usize, pub key: String, pub text: String }

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SessionAnalysis {
    pub calls: Vec<CallPoint>,
    pub prompts: Vec<PromptMark>,
    /// Index of the first call after each compaction.
    pub compactions: Vec<usize>,
    pub suggestions: Vec<Suggestion>,
}

/// Optimization suggestion as a code + parameters; the UI translates it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum Suggestion {
    /// A file read repeatedly.
    RepeatedRead { file: String, times: u32, tokens: u64 },
    /// Output of a tool call that is too long.
    LongOutput { label: String, tokens: u64 },
    /// A skill that loads many tokens per call.
    HeavySkill { skill: String, tokens: u64 },
}

impl Suggestion {
    /// Suggestion sentence for the CLI (the UI translates it from the code).
    pub fn text(&self, lang: bum_core::i18n::Lang) -> String {
        use bum_core::i18n::Lang::*;
        use bum_core::insight::fmt_tokens as f;
        match (self, lang) {
            (Suggestion::RepeatedRead { file, times, tokens }, Vi) => format!("File {file} bị đọc {times} lần (~{} token). Nên đọc một lần, hoặc dùng offset/limit.", f(*tokens)),
            (Suggestion::RepeatedRead { file, times, tokens }, En) => format!("File {file} was read {times} times (~{} tokens). Read it once, or use offset/limit.", f(*tokens)),
            (Suggestion::LongOutput { label, tokens }, Vi) => format!("Output của `{label}` dài ~{} token. Nên lọc bớt bằng | head hoặc grep.", f(*tokens)),
            (Suggestion::LongOutput { label, tokens }, En) => format!("Output of `{label}` is ~{} tokens. Trim it with | head or grep.", f(*tokens)),
            (Suggestion::HeavySkill { skill, tokens }, Vi) => format!("Skill {skill} nạp ~{} token mỗi lần gọi.", f(*tokens)),
            (Suggestion::HeavySkill { skill, tokens }, En) => format!("Skill {skill} loads ~{} tokens per call.", f(*tokens)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Block { pub category: Category, pub label: String, pub tokens: u64 }

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ToolCall {
    pub name: String,
    pub summary: String,
    pub result: String,
    pub truncated: bool,
    pub old_text: Option<String>,
    pub new_text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CallDetail { pub top_blocks: Vec<Block>, pub tools: Vec<ToolCall> }

fn tokens(chars: usize) -> u64 {
    (chars as f64 / CHARS_PER_TOKEN).round() as u64
}

fn text_of(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Array(items) => items.iter().filter_map(|i| i["text"].as_str()).collect::<Vec<_>>().join("\n"),
        _ => String::new(),
    }
}

/// Total number of characters of every string inside a JSON value.
fn string_chars(v: &Value) -> usize {
    match v {
        Value::String(s) => s.chars().count(),
        Value::Array(items) => items.iter().map(string_chars).sum(),
        Value::Object(map) => map.values().map(string_chars).sum(),
        _ => 0,
    }
}

const LABEL_LIMIT: usize = 160;

fn summary(name: &str, input: &Value) -> String {
    let s = summary_full(name, input);
    if s.chars().count() > LABEL_LIMIT { format!("{}…", s.chars().take(LABEL_LIMIT).collect::<String>()) } else { s }
}

fn summary_full(name: &str, input: &Value) -> String {
    let key = match name { "Bash" => "command", "Read" | "Edit" | "Write" | "NotebookEdit" => "file_path", "Grep" | "Glob" => "pattern", "Skill" => "skill", _ => "" };
    input[key].as_str().map(str::to_string).unwrap_or_else(|| input.to_string().chars().take(80).collect())
}

fn is_file_read(name: &str, input: &Value) -> bool {
    match name {
        "Read" | "Grep" | "Glob" | "NotebookRead" => true,
        "Bash" => input["command"].as_str().is_some_and(|c| ["cat ", "head ", "tail ", "sed ", "less "].iter().any(|p| c.trim_start().starts_with(p))),
        _ => false,
    }
}

#[derive(Default)]
struct Walk {
    blocks: Vec<Block>,
    tools: HashMap<String, (String, Value)>,
    results: HashMap<String, String>,
    request_tools: HashMap<String, Vec<String>>,
    seen: HashSet<String>,
    out: Vec<CallPoint>,
    prompts: Vec<PromptMark>,
    compactions: Vec<usize>,
    pending_prompt: Option<(String, String)>,
    reads: HashMap<String, (u32, u64)>,
    big: Vec<(String, u64)>,
    skills: Vec<(String, u64)>,
    snapshot_for: Option<String>,
    snapshot: Option<Vec<Block>>,
}

impl Walk {
    fn push(&mut self, category: Category, label: String, chars: usize) {
        let t = tokens(chars);
        if t > 0 {
            self.blocks.push(Block { category, label, tokens: t });
        }
    }

    fn snapshot_call(&mut self, rid: &str, ts: &str, usage: &Value) {
        let n = |k: &str| usage[k].as_u64().unwrap_or(0);
        let context = n("input_tokens") + n("cache_creation_input_tokens") + n("cache_read_input_tokens");
        let mut sums: HashMap<Category, u64> = HashMap::new();
        for b in &self.blocks {
            *sums.entry(b.category).or_default() += b.tokens;
        }
        let measured: u64 = sums.values().sum();
        if measured > context && measured > 0 {
            // Estimate exceeds the real number: scale proportionally, then push the rounding remainder into the largest group.
            let mut scaled: HashMap<Category, u64> = sums.iter().map(|(c, v)| (*c, v * context / measured)).collect();
            let rest = context - scaled.values().sum::<u64>();
            if let Some(big) = sums.iter().max_by_key(|(_, v)| **v).map(|(c, _)| *c) {
                *scaled.entry(big).or_default() += rest;
            }
            sums = scaled;
        } else {
            *sums.entry(Category::System).or_default() += context - measured;
        }
        if let Some((key, text)) = self.pending_prompt.take() {
            self.prompts.push(PromptMark { call_index: self.out.len(), key, text });
        }
        if self.snapshot_for.as_deref() == Some(rid) {
            let mut top = self.blocks.clone();
            top.sort_by(|a, b| b.tokens.cmp(&a.tokens));
            top.truncate(TOP_BLOCKS);
            self.snapshot = Some(top);
        }
        self.out.push(CallPoint {
            request_id: rid.to_string(),
            ts: ts.to_string(),
            context,
            output: n("output_tokens"),
            parts: ORDER.iter().map(|c| (*c, sums.get(c).copied().unwrap_or(0))).collect(),
        });
    }

    fn line(&mut self, v: &Value) {
        if v["isSidechain"].as_bool() == Some(true) {
            return; // subagents have their own context
        }
        let ts = v["timestamp"].as_str().unwrap_or_default();
        match v["type"].as_str() {
            Some("attachment") => {
                let a = &v["attachment"];
                let kind = a["type"].as_str().unwrap_or("");
                // The system prompt snapshot repeats many times; it is not additional content.
                if kind == "prompt_snapshot" {
                    return;
                }
                let chars: usize = ["content", "stdout", "snippet", "text", "addedLines", "addedBlocks", "entries"].iter().map(|k| string_chars(&a[*k])).sum();
                self.push(Category::System, format!("attachment {kind}"), chars);
            }
            Some("user") => self.user(v),
            Some("assistant") => {
                let msg = &v["message"];
                if msg["model"].as_str() == Some("<synthetic>") {
                    return;
                }
                let Some(rid) = v["requestId"].as_str().or(msg["id"].as_str()).map(str::to_string) else { return };
                // Skip calls without `usage` (avoids a fake drop to 0).
                if !msg["usage"].is_object() {
                    return;
                }
                if self.seen.insert(rid.clone()) {
                    self.snapshot_call(&rid, ts, &msg["usage"]);
                }
                for c in msg["content"].as_array().into_iter().flatten() {
                    match c["type"].as_str() {
                        Some("text") => self.push(Category::Output, "response".into(), c["text"].as_str().unwrap_or("").chars().count()),
                        Some("thinking") => {
                            // The thinking content is usually hidden, but the encrypted signature is still sent back in the context.
                            let text = c["thinking"].as_str().unwrap_or("");
                            let chars = if text.is_empty() { c["signature"].as_str().unwrap_or("").chars().count() } else { text.chars().count() };
                            self.push(Category::Thinking, "thinking".into(), chars);
                        }
                        Some("tool_use") => {
                            let (id, name) = (c["id"].as_str().unwrap_or("").to_string(), c["name"].as_str().unwrap_or("").to_string());
                            let label = format!("{name} {}", summary(&name, &c["input"]));
                            self.push(Category::ToolIo, label, c["input"].to_string().chars().count());
                            self.request_tools.entry(rid.clone()).or_default().push(id.clone());
                            self.tools.insert(id, (name, c["input"].clone()));
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    fn user(&mut self, v: &Value) {
        let content = &v["message"]["content"];
        if v["isCompactSummary"].as_bool() == Some(true) {
            self.blocks.clear();
            self.compactions.push(self.out.len());
            self.push(Category::System, "compact summary".into(), text_of(content).chars().count());
            return;
        }
        let results: Vec<&Value> = content.as_array().into_iter().flatten().filter(|c| c["type"] == "tool_result").collect();
        if !results.is_empty() {
            for r in results {
                let id = r["tool_use_id"].as_str().unwrap_or("").to_string();
                let text = text_of(&r["content"]);
                let chars = text.chars().count();
                let (name, input) = self.tools.get(&id).cloned().unwrap_or_default();
                let label = format!("{name} {}", summary(&name, &input));
                let category = if name == "Skill" { Category::Skill } else if is_file_read(&name, &input) { Category::FileRead } else { Category::ToolIo };
                let t = tokens(chars);
                if name == "Read" {
                    let e = self.reads.entry(summary(&name, &input)).or_default();
                    e.0 += 1;
                    e.1 += t;
                }
                if name == "Skill" {
                    self.skills.push((summary(&name, &input), t));
                }
                if t > BIG_OUTPUT_TOKENS {
                    self.big.push((label.clone(), t));
                }
                self.push(category, label, chars);
                self.results.insert(id, text);
            }
            return;
        }
        let text = text_of(content);
        let is_prompt = v["isMeta"].as_bool() != Some(true) && v.get("toolUseResult").is_none() && !text.trim_start().starts_with('<') && !text.trim().is_empty();
        if is_prompt && !text.starts_with("[Request interrupted") {
            if let Some(key) = v["promptId"].as_str() {
                self.pending_prompt = Some((key.to_string(), text.chars().take(200).collect()));
            }
            self.push(Category::User, "question".into(), text.chars().count());
        } else {
            self.push(Category::System, "meta".into(), text.chars().count());
        }
    }
}

fn walk(path: &Path, snapshot_for: Option<&str>) -> std::io::Result<Walk> {
    // Read line by line as bytes: one bad UTF-8 byte (Claude is mid-write) only corrupts that line.
    let bytes = std::fs::read(path)?;
    let text = String::from_utf8_lossy(&bytes);
    let mut w = Walk { snapshot_for: snapshot_for.map(str::to_string), ..Default::default() };
    for line in text.lines() {
        if let Ok(v) = serde_json::from_str::<Value>(line) {
            w.line(&v);
        }
    }
    Ok(w)
}

pub fn analyze_file(path: &Path) -> std::io::Result<SessionAnalysis> {
    let w = walk(path, None)?;
    let mut suggestions = Vec::new();
    let mut reads: Vec<_> = w.reads.iter().filter(|(_, (n, _))| *n >= REPEATED_READS).collect();
    reads.sort_by(|a, b| b.1 .1.cmp(&a.1 .1));
    for (file, (n, t)) in reads {
        suggestions.push(Suggestion::RepeatedRead { file: file.clone(), times: *n, tokens: *t });
    }
    for (label, t) in &w.big {
        suggestions.push(Suggestion::LongOutput { label: label.clone(), tokens: *t });
    }
    for (skill, t) in w.skills.iter().filter(|(_, t)| *t > 5_000) {
        suggestions.push(Suggestion::HeavySkill { skill: skill.clone(), tokens: *t });
    }
    Ok(SessionAnalysis { calls: w.out, prompts: w.prompts, compactions: w.compactions, suggestions })
}

/// Heaviest context blocks at call `request_id` and that call's tool calls.
pub fn call_detail(path: &Path, request_id: &str) -> Option<CallDetail> {
    let w = walk(path, Some(request_id)).ok()?;
    let top_blocks = w.snapshot?;
    let tools = w.request_tools.get(request_id).into_iter().flatten().filter_map(|id| {
        let (name, input) = w.tools.get(id)?.clone();
        let full = w.results.get(id).cloned().unwrap_or_default();
        let truncated = full.chars().count() > RESULT_LIMIT;
        Some(ToolCall {
            summary: summary(&name, &input),
            result: full.chars().take(RESULT_LIMIT).collect(),
            truncated,
            old_text: input["old_string"].as_str().map(str::to_string),
            new_text: input["new_string"].as_str().or(input["content"].as_str()).map(str::to_string),
            name,
        })
    }).collect();
    Some(CallDetail { top_blocks, tools })
}
