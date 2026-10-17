//! OpenCode: reads sessions and model calls from its SQLite database (`session_v2` + `session_message`, OpenCode 2.x),
//! read-only and incrementally by `time_updated` (OpenCode phase-1 spec).
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context};
use bum_core::model::{RawEvent, ToolUse, Usage};
use bum_core::source::{DbSource, SourceInfo};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use serde_json::Value;

pub const TOOL_ID: &str = "opencode";

pub struct OpenCode {
    db: PathBuf,
}

impl OpenCode {
    pub fn new(db: impl Into<PathBuf>) -> Self {
        Self { db: db.into() }
    }

    /// `BUM_OPENCODE_DB` if set, otherwise `~/.local/share/opencode/opencode.db`.
    pub fn default_path() -> PathBuf {
        if let Some(p) = std::env::var_os("BUM_OPENCODE_DB") {
            return PathBuf::from(p);
        }
        dirs::home_dir().unwrap_or_default().join(".local").join("share").join("opencode").join("opencode.db")
    }
}

pub fn info() -> SourceInfo {
    SourceInfo {
        id: TOOL_ID.into(), label: "OpenCode".into(), letter: "O".into(), color: "#4b8bbe".into(), icon: None,
        real_tokens: true, composition: false, limits: false, credits: false, transcript: false,
    }
}

/// Read-only connection that still sees WAL commits (never `immutable=1`).
fn open_readonly(db: &Path) -> anyhow::Result<Connection> {
    let conn = Connection::open_with_flags(db, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)
        .with_context(|| format!("cannot open {}", db.display()))?;
    conn.busy_timeout(std::time::Duration::from_secs(2))?;
    Ok(conn)
}

fn has_table(conn: &Connection, name: &str) -> rusqlite::Result<bool> {
    conn.prepare("SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1")?.exists([name])
}

fn ms_to_rfc3339(ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ms).map(|t| t.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)).unwrap_or_default()
}

/// Last path component of the session directory, like Claude Code and Kiro projects.
fn project_of(directory: &str) -> String {
    Path::new(directory).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| directory.to_string())
}

struct SessionRow {
    id: String,
    parent: Option<String>,
    directory: String,
    title: String,
    version: Option<String>,
    created: i64,
    updated: i64,
}

const SESSION_COLS: &str = "id, parent_id, COALESCE(directory, ''), COALESCE(title, ''), version, time_created, time_updated";

fn session_row(r: &rusqlite::Row) -> rusqlite::Result<SessionRow> {
    Ok(SessionRow { id: r.get(0)?, parent: r.get(1)?, directory: r.get(2)?, title: r.get(3)?, version: r.get(4)?, created: r.get(5)?, updated: r.get(6)? })
}

fn session_events(s: &SessionRow, out: &mut Vec<RawEvent>) {
    out.push(RawEvent::Session {
        session_id: s.id.clone(),
        tool: TOOL_ID.into(),
        project: project_of(&s.directory),
        cwd: (!s.directory.is_empty()).then(|| s.directory.clone()),
        version: s.version.clone(),
        ts: ms_to_rfc3339(s.created),
    });
    if !s.title.trim().is_empty() {
        out.push(RawEvent::Title { session_id: s.id.clone(), title: s.title.clone() });
    }
}

fn usage_of(t: &Value) -> Usage {
    let n = |v: &Value| v.as_u64().unwrap_or(0);
    Usage {
        input: n(&t["input"]),
        output: n(&t["output"]),
        reasoning: n(&t["reasoning"]),
        cache_read: n(&t["cache"]["read"]),
        // OpenCode doesn't distinguish cache TTLs.
        cache_write_5m: n(&t["cache"]["write"]),
        cache_write_1h: 0,
    }
}

fn tool_uses(content: &Value) -> Vec<ToolUse> {
    content
        .as_array()
        .into_iter()
        .flatten()
        .filter(|p| p["type"] == "tool")
        .map(|p| ToolUse { id: p["id"].as_str().unwrap_or_default().into(), name: p["name"].as_str().unwrap_or_default().into() })
        .collect()
}

fn key(id: &str) -> String {
    format!("{TOOL_ID}:{id}")
}

impl DbSource for OpenCode {
    fn id(&self) -> &'static str {
        TOOL_ID
    }

    fn info(&self) -> SourceInfo {
        info()
    }

    fn db_path(&self) -> PathBuf {
        self.db.clone()
    }

    fn scan(&mut self, watermark: Option<i64>) -> anyhow::Result<(Vec<RawEvent>, i64)> {
        let conn = open_readonly(&self.db)?;
        if !has_table(&conn, "session_v2")? || !has_table(&conn, "session_message")? {
            bail!("unsupported OpenCode schema (needs session_v2 and session_message, OpenCode 2.x)");
        }
        let since = watermark.unwrap_or(i64::MIN);
        let mut next = watermark.unwrap_or(0);
        let mut out = Vec::new();

        // Changed sessions first, so every message below has its session row.
        let mut sessions: HashMap<String, Option<String>> = HashMap::new();
        let mut st = conn.prepare(&format!("SELECT {SESSION_COLS} FROM session_v2 WHERE time_updated > ?1 ORDER BY time_created"))?;
        for s in st.query_map([since], session_row)? {
            let s = s?;
            next = next.max(s.updated);
            session_events(&s, &mut out);
            sessions.insert(s.id.clone(), s.parent.clone());
        }

        let mut st = conn.prepare(
            "SELECT id, session_id, type, seq, time_created, time_updated, data FROM session_message WHERE time_updated > ?1 ORDER BY session_id, seq",
        )?;
        let rows: Vec<(String, String, String, i64, i64, i64, String)> =
            st.query_map([since], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)))?.collect::<rusqlite::Result<_>>()?;

        // Sessions referenced by changed messages but not changed themselves.
        let missing: HashSet<&String> = rows.iter().map(|r| &r.1).filter(|sid| !sessions.contains_key(*sid)).collect();
        let mut by_id = conn.prepare(&format!("SELECT {SESSION_COLS} FROM session_v2 WHERE id = ?1"))?;
        for sid in missing {
            if let Some(s) = by_id.query_row([sid], session_row).optional()? {
                session_events(&s, &mut out);
                sessions.insert(s.id.clone(), s.parent.clone());
            }
        }

        // The turn an assistant row belongs to: the last user row before it in the same session.
        let mut turn: HashMap<String, String> = HashMap::new();
        let mut prev_user = conn.prepare("SELECT id FROM session_message WHERE session_id = ?1 AND type = 'user' AND seq < ?2 ORDER BY seq DESC LIMIT 1")?;
        for (id, sid, kind, seq, created, updated, data) in &rows {
            next = next.max(*updated);
            let d: Value = serde_json::from_str(data).unwrap_or(Value::Null);
            let ts = ms_to_rfc3339(*created);
            match kind.as_str() {
                "user" => {
                    out.push(RawEvent::Prompt { key: key(id), session_id: sid.clone(), ts, text: d["text"].as_str().unwrap_or_default().chars().take(2000).collect() });
                    turn.insert(sid.clone(), key(id));
                }
                "assistant" => {
                    let prompt_key = match turn.get(sid) {
                        Some(k) => Some(k.clone()),
                        None => prev_user.query_row(params![sid, seq], |r| r.get::<_, String>(0)).optional()?.map(|u| key(&u)),
                    };
                    let model = format!("{TOOL_ID}:{}/{}", d["model"]["providerID"].as_str().unwrap_or("?"), d["model"]["id"].as_str().unwrap_or("?"));
                    out.push(RawEvent::ApiCall {
                        request_id: key(id),
                        session_id: sid.clone(),
                        prompt_key,
                        ts,
                        model,
                        usage: usage_of(&d["tokens"]),
                        tool_uses: tool_uses(&d["content"]),
                        sidechain: sessions.get(sid).is_some_and(|p| p.is_some()),
                        cost_usd: d["cost"].as_f64(),
                    });
                }
                "compaction" => {
                    let at = d["time"]["created"].as_i64().unwrap_or(*created);
                    out.push(RawEvent::Compacted { session_id: sid.clone(), ts: ms_to_rfc3339(at) });
                }
                // synthetic (injected reminders), system (tool notices), idle, and future types: not turns, no tokens.
                _ => {}
            }
        }
        Ok((out, next))
    }
}
