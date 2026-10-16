//! SQLite: the single place where parsed data is stored (spec sections 4.6, 5).
mod schema;
pub mod notes;
pub mod queries;

use std::path::Path;
use std::time::Duration;

use rusqlite::{params, Connection, OpenFlags, OptionalExtension};

use crate::model::RawEvent;

pub use queries::{PromptStats, ToolActivity};

pub struct Store {
    conn: Connection,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileOffset {
    pub path: String,
    pub file_id: String,
    pub size: u64,
    pub offset: u64,
    pub mtime: i64,
    pub ctx_json: String,
}

/// What a batch of events just changed, used to decide alerts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ApplySummary {
    pub touched_prompts: Vec<String>,
    pub touched_sessions: Vec<String>,
    pub new_sessions: Vec<String>,
    pub new_rate_limits: Vec<(String, i64)>,
}

impl ApplySummary {
    pub fn merge(&mut self, other: ApplySummary) {
        for k in other.touched_prompts { push_unique(&mut self.touched_prompts, &k) }
        for k in other.touched_sessions { push_unique(&mut self.touched_sessions, &k) }
        for k in other.new_sessions { push_unique(&mut self.new_sessions, &k) }
        self.new_rate_limits.extend(other.new_rate_limits);
    }
}

fn push_unique(v: &mut Vec<String>, k: &str) {
    if !v.iter().any(|x| x == k) {
        v.push(k.to_string());
    }
}

impl Store {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        Self::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> rusqlite::Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    /// Read-only connection for clients (service, Tauri commands). The host is still the only writer.
    pub fn open_readonly(path: &Path) -> rusqlite::Result<Self> {
        let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)?;
        conn.busy_timeout(Duration::from_secs(2))?;
        // DB created by the old version (old host still running): upgrade the schema once, then reopen read-only.
        // Migrations only add tables, safe while another host is writing (SQLite locks by itself).
        if schema::version(&conn)? < schema::CURRENT_VERSION {
            drop(conn);
            drop(Self::open(path)?);
            return Self::open_readonly(path);
        }
        Ok(Self { conn })
    }

    fn init(conn: Connection) -> rusqlite::Result<Self> {
        conn.busy_timeout(Duration::from_secs(2))?;
        conn.pragma_update_and_check(None, "journal_mode", "WAL", |r| r.get::<_, String>(0))?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        schema::migrate(&conn)?;
        Ok(Self { conn })
    }

    pub(crate) fn conn(&self) -> &Connection {
        &self.conn
    }

    pub fn get_offset(&self, path: &str) -> rusqlite::Result<Option<FileOffset>> {
        self.conn
            .query_row(
                "SELECT path, file_id, size, offset, mtime, ctx_json FROM file_offsets WHERE path = ?1",
                [path],
                |r| Ok(FileOffset { path: r.get(0)?, file_id: r.get(1)?, size: queries::u64_at(r, 2)?, offset: queries::u64_at(r, 3)?, mtime: r.get(4)?, ctx_json: r.get(5)? }),
            )
            .optional()
    }

    /// Delete all data produced from a file (when the file is overwritten / replaced).
    pub fn purge_file(&mut self, path: &str) -> rusqlite::Result<()> {
        let tx = self.conn.transaction()?;
        for table in ["prompts", "api_calls", "api_call_tools", "skill_uses", "compactions", "session_titles"] {
            tx.execute(&format!("DELETE FROM {table} WHERE source_file = ?1"), [path])?;
        }
        tx.execute("DELETE FROM file_offsets WHERE path = ?1", [path])?;
        tx.commit()
    }

    /// Write a batch of events and the new offset in the same transaction.
    pub fn ingest_batch(&mut self, off: &FileOffset, events: &[RawEvent], parse_errors: u64) -> rusqlite::Result<ApplySummary> {
        let tx = self.conn.transaction()?;
        let mut sum = ApplySummary::default();
        let src = off.path.as_str();
        for ev in events {
            match ev {
                RawEvent::Session { session_id, tool, project, cwd, version, ts } => {
                    let n = tx.execute(
                        "INSERT OR IGNORE INTO sessions(id, tool, project, cwd, version, first_ts) VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
                        params![session_id, tool, project, cwd, version, ts],
                    )?;
                    if n == 1 {
                        push_unique(&mut sum.new_sessions, session_id);
                    }
                }
                RawEvent::Prompt { key, session_id, ts, text } => {
                    tx.execute(
                        "INSERT OR IGNORE INTO prompts(key, session_id, ts, text, source_file) VALUES(?1, ?2, ?3, ?4, ?5)",
                        params![key, session_id, ts, text, src],
                    )?;
                    push_unique(&mut sum.touched_prompts, key);
                }
                RawEvent::ApiCall { request_id, session_id, prompt_key, ts, model, usage, tool_uses, sidechain } => {
                    // Same requestId appears on multiple lines: keep the largest value (spec section 2).
                    tx.execute(
                        "INSERT INTO api_calls(request_id, session_id, prompt_key, ts, model, input, cw5, cw1h, cache_read, output, sidechain, source_file)
                         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
                         ON CONFLICT(request_id) DO UPDATE SET
                           input = max(input, excluded.input), cw5 = max(cw5, excluded.cw5), cw1h = max(cw1h, excluded.cw1h),
                           cache_read = max(cache_read, excluded.cache_read), output = max(output, excluded.output),
                           prompt_key = coalesce(api_calls.prompt_key, excluded.prompt_key)",
                        params![request_id, session_id, prompt_key, ts, model, usage.input as i64, usage.cache_write_5m as i64,
                                usage.cache_write_1h as i64, usage.cache_read as i64, usage.output as i64, sidechain, src],
                    )?;
                    for t in tool_uses {
                        tx.execute(
                            "INSERT OR IGNORE INTO api_call_tools(tool_use_id, request_id, name, source_file) VALUES(?1, ?2, ?3, ?4)",
                            params![t.id, request_id, t.name, src],
                        )?;
                    }
                    if let Some(k) = prompt_key {
                        push_unique(&mut sum.touched_prompts, k);
                    }
                    push_unique(&mut sum.touched_sessions, session_id);
                }
                RawEvent::Skill { prompt_key, session_id, ts, skill, kind } => {
                    tx.execute(
                        "INSERT OR IGNORE INTO skill_uses(prompt_key, session_id, ts, skill, kind, source_file) VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
                        params![prompt_key.as_deref().unwrap_or(""), session_id, ts, skill, kind.as_str(), src],
                    )?;
                }
                RawEvent::Interrupted { prompt_key: Some(k), .. } => {
                    tx.execute("UPDATE prompts SET interrupted = 1 WHERE key = ?1", [k])?;
                }
                RawEvent::Interrupted { prompt_key: None, .. } => {}
                RawEvent::Compacted { session_id, ts } => {
                    tx.execute(
                        "INSERT OR IGNORE INTO compactions(session_id, ts, source_file) VALUES(?1, ?2, ?3)",
                        params![session_id, ts, src],
                    )?;
                }
                RawEvent::Title { session_id, title } => {
                    tx.execute(
                        "INSERT INTO session_titles(session_id, title, source_file) VALUES(?1, ?2, ?3)
                         ON CONFLICT(session_id) DO UPDATE SET title = excluded.title",
                        params![session_id, title, src],
                    )?;
                }
                RawEvent::Metered { request_id, credits } => {
                    tx.execute("UPDATE api_calls SET credits = ?2 WHERE request_id = ?1", params![request_id, credits])?;
                }
                RawEvent::RateLimited { ts, limit_type, resets_at } => {
                    let n = tx.execute(
                        "INSERT OR IGNORE INTO rate_limit_events(limit_type, resets_at, ts) VALUES(?1, ?2, ?3)",
                        params![limit_type, resets_at, ts],
                    )?;
                    if n == 1 {
                        sum.new_rate_limits.push((limit_type.clone(), *resets_at));
                    }
                }
            }
        }
        tx.execute(
            "INSERT INTO file_offsets(path, file_id, size, offset, mtime, ctx_json) VALUES(?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(path) DO UPDATE SET file_id = excluded.file_id, size = excluded.size, offset = excluded.offset,
               mtime = excluded.mtime, ctx_json = excluded.ctx_json",
            params![off.path, off.file_id, off.size as i64, off.offset as i64, off.mtime, off.ctx_json],
        )?;
        if parse_errors > 0 {
            tx.execute(
                "INSERT INTO counters(name, value) VALUES('parse_errors', ?1)
                 ON CONFLICT(name) DO UPDATE SET value = value + excluded.value",
                [parse_errors as i64],
            )?;
        }
        tx.commit()?;
        Ok(sum)
    }

    /// Return `true` if `key` has never been sent (and mark it as sent).
    pub fn alert_once(&mut self, key: &str, ts: &str) -> rusqlite::Result<bool> {
        Ok(self.conn.execute("INSERT OR IGNORE INTO alerts_sent(key, ts) VALUES(?1, ?2)", params![key, ts])? == 1)
    }

    pub fn parse_errors(&self) -> rusqlite::Result<u64> {
        Ok(self.conn
            .query_row("SELECT value FROM counters WHERE name = 'parse_errors'", [], |r| queries::u64_at(r, 0))
            .optional()?
            .unwrap_or(0))
    }
}
