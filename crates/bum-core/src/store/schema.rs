use rusqlite::Connection;

const V1: &str = r#"
CREATE TABLE sessions(id TEXT PRIMARY KEY, tool TEXT NOT NULL, project TEXT NOT NULL, cwd TEXT, version TEXT,
  first_ts TEXT NOT NULL, deleted INTEGER NOT NULL DEFAULT 0);
CREATE TABLE prompts(key TEXT PRIMARY KEY, session_id TEXT NOT NULL, ts TEXT NOT NULL, text TEXT NOT NULL,
  interrupted INTEGER NOT NULL DEFAULT 0, source_file TEXT NOT NULL);
CREATE INDEX prompts_ts ON prompts(ts);
CREATE INDEX prompts_session ON prompts(session_id, ts);
CREATE TABLE api_calls(request_id TEXT PRIMARY KEY, session_id TEXT NOT NULL, prompt_key TEXT, ts TEXT NOT NULL,
  model TEXT NOT NULL, input INTEGER NOT NULL, cw5 INTEGER NOT NULL, cw1h INTEGER NOT NULL,
  cache_read INTEGER NOT NULL, output INTEGER NOT NULL, sidechain INTEGER NOT NULL, source_file TEXT NOT NULL);
CREATE INDEX api_calls_prompt ON api_calls(prompt_key);
CREATE INDEX api_calls_ts ON api_calls(ts);
CREATE INDEX api_calls_session ON api_calls(session_id, ts);
CREATE INDEX api_calls_file ON api_calls(source_file);
CREATE TABLE api_call_tools(tool_use_id TEXT PRIMARY KEY, request_id TEXT NOT NULL, name TEXT NOT NULL,
  source_file TEXT NOT NULL);
CREATE TABLE skill_uses(prompt_key TEXT NOT NULL, session_id TEXT NOT NULL, ts TEXT NOT NULL, skill TEXT NOT NULL,
  kind TEXT NOT NULL, source_file TEXT NOT NULL, UNIQUE(prompt_key, skill, kind));
CREATE TABLE compactions(session_id TEXT NOT NULL, ts TEXT NOT NULL, source_file TEXT NOT NULL, UNIQUE(session_id, ts));
CREATE TABLE rate_limit_events(limit_type TEXT NOT NULL, resets_at INTEGER NOT NULL, ts TEXT NOT NULL,
  UNIQUE(limit_type, resets_at));
CREATE TABLE file_offsets(path TEXT PRIMARY KEY, file_id TEXT NOT NULL, size INTEGER NOT NULL,
  offset INTEGER NOT NULL, mtime INTEGER NOT NULL, ctx_json TEXT NOT NULL);
CREATE TABLE alerts_sent(key TEXT PRIMARY KEY, ts TEXT NOT NULL);
CREATE TABLE counters(name TEXT PRIMARY KEY, value INTEGER NOT NULL);
"#;

const V2: &str = r#"
CREATE TABLE notes(prompt_key TEXT PRIMARY KEY, starred INTEGER NOT NULL DEFAULT 0, note TEXT NOT NULL DEFAULT '',
  updated_at TEXT NOT NULL);
"#;

pub const CURRENT_VERSION: i64 = 4;

pub fn version(conn: &Connection) -> rusqlite::Result<i64> {
    conn.query_row("PRAGMA user_version", [], |r| r.get(0))
}

pub fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if version < 1 {
        conn.execute_batch(V1)?;
        conn.pragma_update(None, "user_version", 1)?;
    }
    if version < 2 {
        conn.execute_batch(V2)?;
        conn.pragma_update(None, "user_version", 2)?;
    }
    if version < 3 {
        let has: bool = conn.prepare("SELECT 1 FROM pragma_table_info('api_calls') WHERE name = 'credits'")?.exists([])?;
        if !has {
            conn.execute_batch("ALTER TABLE api_calls ADD COLUMN credits REAL NOT NULL DEFAULT 0;")?;
        }
        conn.pragma_update(None, "user_version", 3)?;
    }
    if version < 4 {
        // Separate table: a title can arrive before the line that creates the session.
        conn.execute_batch("CREATE TABLE IF NOT EXISTS session_titles(session_id TEXT PRIMARY KEY, title TEXT NOT NULL, source_file TEXT NOT NULL);")?;
        conn.pragma_update(None, "user_version", 4)?;
    }
    if version < 5 {
        // The old Kiro parser dropped questions from Kiro CLI sessions tagged "subagent": skew the offset so
        // the engine deletes the file's old data and re-reads it (once).
        conn.execute_batch(
            "UPDATE file_offsets SET size = -1 WHERE path IN (
               SELECT a.source_file FROM api_calls a JOIN sessions s ON s.id = a.session_id WHERE s.tool = 'kiro');",
        )?;
        conn.pragma_update(None, "user_version", 5)?;
    }
    Ok(())
}
