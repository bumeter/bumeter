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

pub const CURRENT_VERSION: i64 = 8;

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
    if version < 6 {
        // 5h/7d percentage readings for the quota forecast (`win`, not `window`: WINDOW is an SQL keyword).
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS limit_samples(win TEXT NOT NULL, ts INTEGER NOT NULL, pct REAL NOT NULL,
               resets_at INTEGER NOT NULL, PRIMARY KEY(win, ts));",
        )?;
        conn.pragma_update(None, "user_version", 6)?;
    }
    if version < 7 {
        // Reasoning tokens (billed like output) and the USD cost a tool reports itself (OpenCode); NULL = not reported.
        for (col, ddl) in [("reasoning", "INTEGER NOT NULL DEFAULT 0"), ("cost_usd", "REAL")] {
            let has: bool = conn.prepare("SELECT 1 FROM pragma_table_info('api_calls') WHERE name = ?1")?.exists([col])?;
            if !has {
                conn.execute_batch(&format!("ALTER TABLE api_calls ADD COLUMN {col} {ddl};"))?;
            }
        }
        conn.pragma_update(None, "user_version", 7)?;
    }
    if version < 8 {
        // What each tool call acted on and whether it failed (repeated reads, retried errors).
        for (col, ddl) in [("target", "TEXT"), ("target_hash", "INTEGER"), ("error", "INTEGER"), ("result_chars", "INTEGER")] {
            let has: bool = conn.prepare("SELECT 1 FROM pragma_table_info('api_call_tools') WHERE name = ?1")?.exists([col])?;
            if !has {
                conn.execute_batch(&format!("ALTER TABLE api_call_tools ADD COLUMN {col} {ddl};"))?;
            }
        }
        // `purge_file` deletes by source file: without these, re-reading hundreds of files scans the
        // whole tables once per file (85 s instead of a few on a real 418 MB history).
        conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS api_call_tools_req ON api_call_tools(request_id);
             CREATE INDEX IF NOT EXISTS api_call_tools_file ON api_call_tools(source_file);
             CREATE INDEX IF NOT EXISTS prompts_file ON prompts(source_file);
             CREATE INDEX IF NOT EXISTS skill_uses_file ON skill_uses(source_file);
             CREATE INDEX IF NOT EXISTS compactions_file ON compactions(source_file);",
        )?;
        // Fill the new columns for the history too: make Claude Code's files look replaced, so the engine
        // deletes their old rows and reads them again from the start, once. (Unlike Kiro's whole-file
        // reads in v5, a JSONL tail is resumed by file identity and offset, not by size.) Notes are keyed
        // by prompt key, which a re-read gives back unchanged.
        conn.execute_batch(
            "UPDATE file_offsets SET file_id = 'reread-v8' WHERE path IN (
               SELECT a.source_file FROM api_calls a JOIN sessions s ON s.id = a.session_id WHERE s.tool = 'claude_code');",
        )?;
        conn.pragma_update(None, "user_version", 8)?;
    }
    Ok(())
}
