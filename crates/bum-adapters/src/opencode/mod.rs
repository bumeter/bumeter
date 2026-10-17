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
/// Re-read this much before the watermark on each scan (see `scan`).
const LOOKBACK_MS: i64 = 5_000;
/// Same cap as Claude Code prompts.
const MAX_PROMPT_CHARS: usize = 2000;
/// SQL condition: the message's `text` is not blank.
const HAS_TEXT: &str = "trim(COALESCE(json_extract(data, '$.text'), '')) <> ''";

pub struct OpenCode {
    db: PathBuf,
    /// Rows returned recently (key → `time_updated`): the lookback re-reads them, and unchanged ones are not sent again,
    /// so an idle database yields no events. Kept only for the lookback window; a restart re-sends that window once.
    returned: HashMap<String, i64>,
}

impl OpenCode {
    pub fn new(db: impl Into<PathBuf>) -> Self {
        Self { db: db.into(), returned: HashMap::new() }
    }

    /// Whether this version of a row was already returned; records it otherwise.
    fn is_new(&mut self, key: String, updated: i64) -> bool {
        self.returned.insert(key, updated) != Some(updated)
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
        id: TOOL_ID.into(), label: "OpenCode".into(), letter: "O".into(), color: "#4b8bbe".into(), icon: Some("opencode.png".into()),
        real_tokens: true, composition: false, limits: false, credits: false, transcript: false,
    }
}

/// Read-only connection that still sees WAL commits (never `immutable=1`). The main database file is never modified; when
/// OpenCode isn't running, SQLite may still create its `-wal`/`-shm` side files next to it, as any reader of a WAL database does.
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

#[derive(Clone)]
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
        // One read transaction: both queries see the same snapshot.
        let tx = conn.unchecked_transaction()?;
        // OpenCode 2.x calls it `session_v2`, but some builds (and the migrated SQLite schema) call it `session`.
        let sessions_table = if has_table(&tx, "session_v2")? {
            "session_v2"
        } else if has_table(&tx, "session")? {
            "session"
        } else {
            bail!("unsupported OpenCode schema (no sessions table)");
        };
        if !has_table(&tx, "session_message")? {
            bail!("unsupported OpenCode schema (needs session_message)");
        }
        // Re-read a few seconds before the watermark: rows stamped in the same millisecond, or committed late by another
        // OpenCode process, would otherwise be skipped for good. Re-reading is harmless (every write is an upsert).
        let since = watermark.map_or(i64::MIN, |w| w - LOOKBACK_MS);
        let mut next = watermark.unwrap_or(0);
        let mut out = Vec::new();
        let mut meta = Sessions::new(&tx, sessions_table)?;

        // Changed conversations first (subagent sessions are folded into their parent, not shown on their own).
        let mut emitted: HashSet<String> = HashSet::new();
        let mut st = tx.prepare(&format!("SELECT {SESSION_COLS} FROM {sessions_table} WHERE time_updated > ?1 ORDER BY time_created"))?;
        for s in st.query_map([since], session_row)? {
            let s = s?;
            next = next.max(s.updated);
            if s.parent.is_none() && self.is_new(format!("s:{}", s.id), s.updated) {
                session_events(&s, &mut out);
                emitted.insert(s.id.clone());
            }
        }

        let mut st = tx.prepare(
            "SELECT id, session_id, type, seq, time_created, time_updated, data FROM session_message WHERE time_updated > ?1 ORDER BY session_id, seq",
        )?;
        let all: Vec<MessageRow> = st.query_map([since], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)))?.collect::<rusqlite::Result<_>>()?;
        next = all.iter().map(|r| r.5).fold(next, i64::max);
        let rows: Vec<MessageRow> = all.into_iter().filter(|r| self.is_new(format!("m:{}", r.0), r.5)).collect();

        // Conversations referenced by changed messages but not changed themselves.
        for (_, sid, ..) in &rows {
            let root = meta.root(sid)?;
            if !emitted.contains(&root) {
                if let Some(s) = meta.row(&root)? {
                    session_events(&s, &mut out);
                }
                emitted.insert(root);
            }
        }

        // Always look the turn up in the DB: a batch can hold an old question re-read after a late update, so the previous
        // row of the batch is not necessarily the question an answer belongs to.
        // Only questions with text: empty ones are not Prompts, so they can't be a turn either.
        let mut prev_user = tx.prepare(&format!("SELECT id FROM session_message WHERE session_id = ?1 AND type = 'user' AND {HAS_TEXT} AND seq < ?2 ORDER BY seq DESC LIMIT 1"))?;
        let mut user_before = tx.prepare(&format!(
            "SELECT id FROM session_message WHERE session_id = ?1 AND type = 'user' AND {HAS_TEXT} AND time_created <= ?2 ORDER BY time_created DESC, seq DESC LIMIT 1"
        ))?;
        for (id, sid, kind, seq, created, _updated, data) in &rows {
            let root = meta.root(sid)?;
            let subagent = root != *sid;
            let d: Value = serde_json::from_str(data).unwrap_or(Value::Null);
            let ts = ms_to_rfc3339(*created);
            match kind.as_str() {
                // A subagent's task prompt is not a turn of the user (same as Claude Code subagents).
                "user" if !subagent => {
                    let text = d["text"].as_str().unwrap_or_default();
                    if !text.trim().is_empty() {
                        out.push(RawEvent::Prompt { key: key(id), session_id: sid.clone(), ts, text: text.chars().take(MAX_PROMPT_CHARS).collect() });
                    }
                }
                "assistant" => {
                    // Subagent calls count toward the parent turn that spawned the subagent session.
                    let prompt_key = if subagent {
                        let spawned = meta.created(sid)?.unwrap_or(*created);
                        user_before.query_row(params![root, spawned], |r| r.get::<_, String>(0)).optional()?
                    } else {
                        prev_user.query_row(params![sid, seq], |r| r.get::<_, String>(0)).optional()?
                    };
                    let model = format!("{TOOL_ID}:{}/{}", d["model"]["providerID"].as_str().unwrap_or("?"), d["model"]["id"].as_str().unwrap_or("?"));
                    out.push(RawEvent::ApiCall {
                        request_id: key(id),
                        session_id: root.clone(),
                        prompt_key: prompt_key.map(|u| key(&u)),
                        ts,
                        model,
                        usage: usage_of(&d["tokens"]),
                        tool_uses: tool_uses(&d["content"]),
                        sidechain: subagent,
                        cost_usd: d["cost"].as_f64(),
                    });
                }
                "compaction" if !subagent => {
                    let at = d["time"]["created"].as_i64().unwrap_or(*created);
                    out.push(RawEvent::Compacted { session_id: sid.clone(), ts: ms_to_rfc3339(at) });
                }
                // synthetic (injected reminders), system (tool notices), idle, a subagent's own turns, and future types.
                _ => {}
            }
        }
        // Rows older than the next lookback window won't be read again.
        let floor = next - LOOKBACK_MS;
        self.returned.retain(|_, updated| *updated > floor);
        Ok((out, next))
    }
}

/// (id, session_id, type, seq, time_created, time_updated, data)
type MessageRow = (String, String, String, i64, i64, i64, String);

/// Session rows looked up by id, cached for one scan; resolves a subagent session to its top-level conversation.
struct Sessions<'c> {
    by_id: rusqlite::Statement<'c>,
    cache: HashMap<String, Option<SessionRow>>,
}

impl<'c> Sessions<'c> {
    fn new(conn: &'c Connection, table: &str) -> rusqlite::Result<Self> {
        Ok(Self { by_id: conn.prepare(&format!("SELECT {SESSION_COLS} FROM {table} WHERE id = ?1"))?, cache: HashMap::new() })
    }

    fn row(&mut self, id: &str) -> rusqlite::Result<Option<SessionRow>> {
        if !self.cache.contains_key(id) {
            let r = self.by_id.query_row([id], session_row).optional()?;
            self.cache.insert(id.to_string(), r);
        }
        Ok(self.cache[id].clone())
    }

    fn created(&mut self, id: &str) -> rusqlite::Result<Option<i64>> {
        Ok(self.row(id)?.map(|s| s.created))
    }

    /// The top-level conversation of `id` (itself when it has no parent); bounded against cycles.
    fn root(&mut self, id: &str) -> rusqlite::Result<String> {
        let mut cur = id.to_string();
        for _ in 0..16 {
            match self.row(&cur)?.and_then(|s| s.parent) {
                Some(p) => cur = p,
                None => break,
            }
        }
        Ok(cur)
    }
}
