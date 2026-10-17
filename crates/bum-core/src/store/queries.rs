//! Read queries.
use rusqlite::{params, OptionalExtension, Row};
use serde::Serialize;

use super::Store;
use crate::model::{SkillKind, Usage};

/// One skill use: (prompt_key, session_id, ts, skill, kind).
pub type SkillUseRow = (String, String, String, String, SkillKind);
/// One tool's totals: (tool, conversations with turns, turns, last activity).
pub type ToolRow = (String, u64, u64, Option<String>);

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PromptStats {
    pub key: String,
    /// AI tool: `claude_code`, `kiro`…
    pub tool: String,
    /// Credits (tools billed in credits, e.g. Kiro).
    pub credits: f64,
    /// Models used in the turn.
    pub models: Vec<String>,
    pub session_id: String,
    pub project: String,
    pub ts: String,
    pub text: String,
    pub interrupted: bool,
    pub usage: Usage,
    pub api_calls: u32,
    pub context_at_start: u64,
    pub last_call_ts: Option<String>,
}

fn split_models(s: &str) -> Vec<String> {
    s.split(',').filter(|m| !m.is_empty()).map(str::to_string).collect()
}

const USAGE_SUMS: &str = "COALESCE(SUM(input),0), COALESCE(SUM(cw5),0), COALESCE(SUM(cw1h),0), COALESCE(SUM(cache_read),0), COALESCE(SUM(output),0), COALESCE(SUM(reasoning),0)";

/// SQLite only has signed integers; tokens are always non-negative.
pub(crate) fn u64_at(r: &Row, i: usize) -> rusqlite::Result<u64> {
    Ok(r.get::<_, i64>(i)?.max(0) as u64)
}

/// Reads 6 consecutive columns: input, cw5, cw1h, cache_read, output, reasoning.
fn usage_at(r: &Row, i: usize) -> rusqlite::Result<Usage> {
    Ok(Usage { input: u64_at(r, i)?, cache_write_5m: u64_at(r, i + 1)?, cache_write_1h: u64_at(r, i + 2)?, cache_read: u64_at(r, i + 3)?, output: u64_at(r, i + 4)?, reasoning: u64_at(r, i + 5)? })
}

impl Store {
    pub fn usage_totals(&self) -> rusqlite::Result<Usage> {
        self.conn().query_row(&format!("SELECT {USAGE_SUMS} FROM api_calls"), [], |r| usage_at(r, 0))
    }

    pub fn api_call_count(&self) -> rusqlite::Result<u64> {
        self.conn().query_row("SELECT COUNT(*) FROM api_calls", [], |r| u64_at(r, 0))
    }

    pub fn skill_use_count(&self) -> rusqlite::Result<u64> {
        self.conn().query_row("SELECT COUNT(*) FROM skill_uses", [], |r| u64_at(r, 0))
    }

    pub fn prompt_stats(&self, key: &str) -> rusqlite::Result<Option<PromptStats>> {
        let head = self.conn().query_row(
            "SELECT p.key, p.session_id, COALESCE(s.project, ''), p.ts, p.text, p.interrupted, COALESCE(s.tool, '')
             FROM prompts p LEFT JOIN sessions s ON s.id = p.session_id WHERE p.key = ?1",
            [key],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?, r.get::<_, String>(4)?, r.get::<_, bool>(5)?, r.get::<_, String>(6)?)),
        ).optional()?;
        let Some((key, session_id, project, ts, text, interrupted, tool)) = head else { return Ok(None) };
        let (usage, api_calls, last_call_ts, credits, models) = self.conn().query_row(
            &format!("SELECT {USAGE_SUMS}, COUNT(*), MAX(ts), COALESCE(SUM(credits), 0), COALESCE(GROUP_CONCAT(DISTINCT model), '') FROM api_calls WHERE prompt_key = ?1"),
            [&key],
            |r| Ok((usage_at(r, 0)?, r.get::<_, u32>(6)?, r.get::<_, Option<String>>(7)?, r.get::<_, f64>(8)?, split_models(&r.get::<_, String>(9)?))),
        )?;
        let context_at_start = self.conn().query_row(
            "SELECT input + cw5 + cw1h + cache_read FROM api_calls WHERE prompt_key = ?1 AND sidechain = 0 ORDER BY ts LIMIT 1",
            [&key],
            |r| u64_at(r, 0),
        ).optional()?.unwrap_or(0);
        Ok(Some(PromptStats { key, tool, credits, models, session_id, project, ts, text, interrupted, usage, api_calls, context_at_start, last_call_ts }))
    }

    /// Tokens per model for calls matching `where_sql` (used for costing).
    fn model_usage(&self, where_sql: &str, args: &[&dyn rusqlite::ToSql]) -> rusqlite::Result<Vec<(String, Usage)>> {
        let mut st = self.conn().prepare(&format!("SELECT model, {USAGE_SUMS} FROM api_calls WHERE {where_sql} GROUP BY model"))?;
        let rows = st.query_map(args, |r| Ok((r.get(0)?, usage_at(r, 1)?)))?;
        rows.collect()
    }

    pub fn model_usage_for_prompt(&self, key: &str) -> rusqlite::Result<Vec<(String, Usage)>> {
        self.model_usage("prompt_key = ?1", &[&key])
    }

    pub fn model_usage_between(&self, from: &str, to: &str) -> rusqlite::Result<Vec<(String, Usage)>> {
        self.model_usage("ts >= ?1 AND ts < ?2", &[&from, &to])
    }

    pub fn prompt_count_between(&self, from: &str, to: &str) -> rusqlite::Result<u64> {
        self.conn().query_row("SELECT COUNT(*) FROM prompts WHERE ts >= ?1 AND ts < ?2", params![from, to], |r| u64_at(r, 0))
    }

    /// `ts` of the most recent turn, None if none yet.
    pub fn latest_prompt_ts(&self) -> rusqlite::Result<Option<String>> {
        self.conn().query_row("SELECT MAX(ts) FROM prompts", [], |r| r.get(0))
    }

    /// Most recent `ts` in `alerts_sent` among keys starting with `prefix`.
    pub fn latest_alert_ts(&self, prefix: &str) -> rusqlite::Result<Option<String>> {
        self.conn().query_row("SELECT MAX(ts) FROM alerts_sent WHERE substr(key, 1, length(?1)) = ?1", [prefix], |r| r.get(0))
    }

    /// Most recent anomaly marker (`ts` of the `anomaly:<prompt_key>` key in `alerts_sent`), filtered by tool if given.
    /// Shared source for the Bum face in the tray and UI (spec 3.4).
    pub fn latest_anomaly_ts(&self, tool: Option<&str>) -> rusqlite::Result<Option<String>> {
        self.conn().query_row(
            "SELECT MAX(a.ts) FROM alerts_sent a
             LEFT JOIN prompts p ON p.key = substr(a.key, 9)
             LEFT JOIN sessions s ON s.id = p.session_id
             WHERE substr(a.key, 1, 8) = 'anomaly:' AND (?1 IS NULL OR s.tool = ?1)",
            [tool],
            |r| r.get(0),
        )
    }

    /// Number of days (in the timezone offset by `offset_secs`) with at least one turn in [from, to).
    pub fn active_days_between(&self, from: &str, to: &str, offset_secs: i32) -> rusqlite::Result<u64> {
        let off = format!("{offset_secs} seconds");
        self.conn().query_row(
            "SELECT COUNT(DISTINCT date(ts, ?3)) FROM prompts WHERE ts >= ?1 AND ts < ?2",
            params![from, to, off],
            |r| u64_at(r, 0),
        )
    }

    pub fn recent_prompt_keys(&self, limit: u32) -> rusqlite::Result<Vec<String>> {
        let mut st = self.conn().prepare("SELECT key FROM prompts ORDER BY ts DESC LIMIT ?1")?;
        let rows = st.query_map([limit], |r| r.get(0))?;
        rows.collect()
    }

    /// Total tokens per turn since `since` (to compute the median).
    pub fn prompt_totals_since(&self, since: &str) -> rusqlite::Result<Vec<u64>> {
        let mut st = self.conn().prepare(
            "SELECT COALESCE(SUM(a.input + a.cw5 + a.cw1h + a.cache_read + a.output + a.reasoning), 0)
             FROM prompts p LEFT JOIN api_calls a ON a.prompt_key = p.key WHERE p.ts >= ?1 GROUP BY p.key",
        )?;
        let rows = st.query_map([since], |r| u64_at(r, 0))?;
        rows.collect()
    }

    /// Current session context = context of the most recent main call (not a subagent).
    pub fn session_current_context(&self, session_id: &str) -> rusqlite::Result<Option<(u64, String)>> {
        self.conn().query_row(
            "SELECT input + cw5 + cw1h + cache_read, ts FROM api_calls WHERE session_id = ?1 AND sidechain = 0 ORDER BY ts DESC LIMIT 1",
            [session_id],
            |r| Ok((u64_at(r, 0)?, r.get(1)?)),
        ).optional()
    }

    pub fn session_project(&self, session_id: &str) -> rusqlite::Result<Option<String>> {
        self.conn().query_row("SELECT project FROM sessions WHERE id = ?1", [session_id], |r| r.get(0)).optional()
    }

    pub fn latest_rate_limit(&self) -> rusqlite::Result<Option<(String, i64)>> {
        self.conn().query_row(
            "SELECT limit_type, resets_at FROM rate_limit_events ORDER BY resets_at DESC LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        ).optional()
    }

    /// A session's turns in chronological order.
    pub fn session_prompts(&self, session_id: &str) -> rusqlite::Result<Vec<(String, String)>> {
        let mut st = self.conn().prepare("SELECT key, ts FROM prompts WHERE session_id = ?1 ORDER BY ts")?;
        let rows = st.query_map([session_id], |r| Ok((r.get(0)?, r.get(1)?)))?;
        rows.collect()
    }

    pub fn session_skill_uses(&self, session_id: &str) -> rusqlite::Result<Vec<(String, String, SkillKind)>> {
        let mut st = self.conn().prepare("SELECT prompt_key, skill, kind FROM skill_uses WHERE session_id = ?1 ORDER BY ts")?;
        let rows = st.query_map([session_id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)))?;
        Ok(rows.filter_map(Result::ok).filter_map(|(k, s, kind)| SkillKind::parse(&kind).map(|kind| (k, s, kind))).collect())
    }

    pub fn session_compactions(&self, session_id: &str) -> rusqlite::Result<Vec<String>> {
        let mut st = self.conn().prepare("SELECT ts FROM compactions WHERE session_id = ?1 ORDER BY ts")?;
        let rows = st.query_map([session_id], |r| r.get(0))?;
        rows.collect()
    }
}

/// One row for the list: turn stats + ⭐.
#[derive(Debug, Clone, PartialEq)]
pub struct PromptListItem {
    pub stats: PromptStats,
    pub starred: bool,
}

fn like_pattern(q: &str) -> String {
    let escaped = q.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
    format!("%{escaped}%")
}

impl Store {
    /// Every turn matching the SQL conditions (search, project, time), newest first.
    pub fn prompt_items(&self, q: Option<&str>, project: Option<&str>, from: Option<&str>, to: Option<&str>) -> rusqlite::Result<Vec<PromptListItem>> {
        self.prompt_items_for(q, project, from, to, None)
    }

    pub fn prompt_items_for(&self, q: Option<&str>, project: Option<&str>, from: Option<&str>, to: Option<&str>, tool: Option<&str>) -> rusqlite::Result<Vec<PromptListItem>> {
        let pattern = q.filter(|s| !s.is_empty()).map(like_pattern);
        let mut st = self.conn().prepare(
            "SELECT p.key, p.session_id, COALESCE(s.project, ''), p.ts, p.text, p.interrupted,
                    COALESCE(SUM(a.input),0), COALESCE(SUM(a.cw5),0), COALESCE(SUM(a.cw1h),0), COALESCE(SUM(a.cache_read),0), COALESCE(SUM(a.output),0), COALESCE(SUM(a.reasoning),0),
                    COUNT(a.request_id), MAX(a.ts),
                    COALESCE((SELECT c.input + c.cw5 + c.cw1h + c.cache_read FROM api_calls c
                              WHERE c.prompt_key = p.key AND c.sidechain = 0 ORDER BY c.ts LIMIT 1), 0),
                    COALESCE(n.starred, 0), COALESCE(s.tool, ''), COALESCE(SUM(a.credits), 0), COALESCE(GROUP_CONCAT(DISTINCT a.model), '')
             FROM prompts p
             LEFT JOIN sessions s ON s.id = p.session_id
             LEFT JOIN api_calls a ON a.prompt_key = p.key
             LEFT JOIN notes n ON n.prompt_key = p.key
             WHERE (?1 IS NULL OR p.text LIKE ?1 ESCAPE '\\')
               AND (?2 IS NULL OR s.project = ?2)
               AND (?3 IS NULL OR p.ts >= ?3)
               AND (?4 IS NULL OR p.ts < ?4)
               AND (?5 IS NULL OR s.tool = ?5)
             GROUP BY p.key ORDER BY p.ts DESC"
        )?;
        let rows = st.query_map(params![pattern, project, from, to, tool], |r| {
            Ok(PromptListItem {
                stats: PromptStats {
                    key: r.get(0)?,
                    tool: r.get(16)?,
                    credits: r.get(17)?,
                    models: split_models(&r.get::<_, String>(18)?),
                    session_id: r.get(1)?,
                    project: r.get(2)?,
                    ts: r.get(3)?,
                    text: r.get(4)?,
                    interrupted: r.get(5)?,
                    usage: usage_at(r, 6)?,
                    api_calls: r.get(12)?,
                    last_call_ts: r.get(13)?,
                    context_at_start: u64_at(r, 14)?,
                },
                starred: r.get(15)?,
            })
        })?;
        rows.collect()
    }

    pub fn distinct_skill_count(&self) -> rusqlite::Result<u64> {
        self.conn().query_row("SELECT COUNT(DISTINCT skill) FROM skill_uses", [], |r| u64_at(r, 0))
    }

    pub fn distinct_project_count(&self) -> rusqlite::Result<u64> {
        self.conn().query_row(
            "SELECT COUNT(DISTINCT s.project) FROM sessions s WHERE EXISTS (SELECT 1 FROM prompts p WHERE p.session_id = s.id)",
            [],
            |r| u64_at(r, 0),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CallRow {
    pub request_id: String,
    pub ts: String,
    pub model: String,
    pub usage: Usage,
    pub context: u64,
    pub output: u64,
    pub sidechain: bool,
    pub tools: Vec<String>,
}

impl Store {
    /// A turn's API calls in chronological order, with tool names.
    pub fn prompt_calls(&self, key: &str) -> rusqlite::Result<Vec<CallRow>> {
        let mut st = self.conn().prepare(
            "SELECT a.request_id, a.ts, a.model, a.input, a.cw5, a.cw1h, a.cache_read, a.output, a.reasoning, a.sidechain,
                    COALESCE((SELECT GROUP_CONCAT(t.name, char(31)) FROM api_call_tools t WHERE t.request_id = a.request_id), '')
             FROM api_calls a WHERE a.prompt_key = ?1 ORDER BY a.ts",
        )?;
        let rows = st.query_map([key], |r| {
            let usage = usage_at(r, 3)?;
            let tools: String = r.get(10)?;
            Ok(CallRow {
                request_id: r.get(0)?,
                ts: r.get(1)?,
                model: r.get(2)?,
                context: usage.context(),
                output: usage.output,
                usage,
                sidechain: r.get(9)?,
                tools: tools.split('\u{1f}').filter(|s| !s.is_empty()).map(str::to_string).collect(),
            })
        })?;
        rows.collect()
    }
}

impl Store {
    /// Every skill use: (prompt_key, session_id, ts, skill, kind).
    pub fn all_skill_uses(&self) -> rusqlite::Result<Vec<SkillUseRow>> {
        let mut st = self.conn().prepare("SELECT prompt_key, session_id, ts, skill, kind FROM skill_uses ORDER BY ts")?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get::<_, String>(4)?)))?;
        Ok(rows.filter_map(Result::ok).filter_map(|(a, b, c, d, k)| SkillKind::parse(&k).map(|k| (a, b, c, d, k))).collect())
    }
}

impl Store {
    /// The session's main log file (the one containing the user's turns, not a subagent file).
    pub fn session_file(&self, session_id: &str) -> rusqlite::Result<Option<String>> {
        self.conn()
            .query_row("SELECT source_file FROM prompts WHERE session_id = ?1 ORDER BY ts LIMIT 1", [session_id], |r| r.get(0))
            .optional()
    }
}

impl Store {
    /// Every call: (ts, model, usage, session_id).
    pub fn calls_brief(&self) -> rusqlite::Result<Vec<(String, String, Usage, String)>> {
        let mut st = self.conn().prepare("SELECT ts, model, input, cw5, cw1h, cache_read, output, reasoning, session_id FROM api_calls")?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, usage_at(r, 2)?, r.get(8)?)))?;
        rows.collect()
    }

    pub fn session_projects(&self) -> rusqlite::Result<std::collections::HashMap<String, String>> {
        let mut st = self.conn().prepare("SELECT id, project FROM sessions")?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        rows.collect()
    }
}

/// One per-model statistics row.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelRow {
    pub model: String,
    pub calls: u64,
    pub prompts: u64,
    pub usage: Usage,
    pub credits: f64,
    pub last_used: String,
}

impl Store {
    pub fn model_rows(&self) -> rusqlite::Result<Vec<ModelRow>> {
        let mut st = self.conn().prepare(&format!(
            "SELECT model, COUNT(*), COUNT(DISTINCT prompt_key), {USAGE_SUMS}, COALESCE(SUM(credits), 0), MAX(ts) FROM api_calls GROUP BY model"
        ))?;
        let rows = st.query_map([], |r| Ok(ModelRow { model: r.get(0)?, calls: u64_at(r, 1)?, prompts: u64_at(r, 2)?, usage: usage_at(r, 3)?, credits: r.get(9)?, last_used: r.get(10)? }))?;
        rows.collect()
    }
}

impl Store {
    pub fn session_title(&self, session_id: &str) -> rusqlite::Result<Option<String>> {
        self.conn().query_row("SELECT title FROM session_titles WHERE session_id = ?1", [session_id], |r| r.get(0)).optional()
    }
}

impl Store {
    /// (tool, number of conversations with turns, number of turns, last activity).
    pub fn tool_rows(&self) -> rusqlite::Result<Vec<ToolRow>> {
        let mut st = self.conn().prepare(
            "SELECT s.tool, COUNT(DISTINCT p.session_id), COUNT(p.key), MAX(p.ts) FROM sessions s LEFT JOIN prompts p ON p.session_id = s.id GROUP BY s.tool",
        )?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, u64_at(r, 1)?, u64_at(r, 2)?, r.get(3)?)))?;
        rows.collect()
    }
}

impl Store {
    /// All conversation titles: session_id → title.
    pub fn session_titles(&self) -> rusqlite::Result<std::collections::HashMap<String, String>> {
        let mut st = self.conn().prepare("SELECT session_id, title FROM session_titles")?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        rows.collect()
    }
}

impl Store {
    pub fn session_tool(&self, session_id: &str) -> rusqlite::Result<Option<String>> {
        self.conn().query_row("SELECT tool FROM sessions WHERE id = ?1", [session_id], |r| r.get(0)).optional()
    }
}

impl Store {
    /// session_id → tool.
    pub fn session_tools(&self) -> rusqlite::Result<std::collections::HashMap<String, String>> {
        let mut st = self.conn().prepare("SELECT id, tool FROM sessions")?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        rows.collect()
    }
}

/// A tool's activity over a time range.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct ToolActivity {
    pub tool: String,
    pub tokens: u64,
    pub credits: f64,
    pub prompts: u64,
}

impl Store {
    /// Total credits for tool `tool` in [from, to).
    pub fn tool_credits_between(&self, tool: &str, from: &str, to: &str) -> rusqlite::Result<f64> {
        self.conn().query_row(
            "SELECT COALESCE(SUM(a.credits), 0) FROM api_calls a JOIN sessions s ON s.id = a.session_id
             WHERE s.tool = ?1 AND a.ts >= ?2 AND a.ts < ?3",
            params![tool, from, to],
            |r| r.get(0),
        )
    }

    /// Tokens, credits, and turn counts per tool in [from, to); a tool with no activity has no row.
    pub fn tool_activity_between(&self, from: &str, to: &str) -> rusqlite::Result<Vec<ToolActivity>> {
        let mut out: Vec<ToolActivity> = Vec::new();
        let mut st = self.conn().prepare(&format!(
            "SELECT s.tool, {USAGE_SUMS}, COALESCE(SUM(a.credits), 0) FROM api_calls a JOIN sessions s ON s.id = a.session_id
             WHERE a.ts >= ?1 AND a.ts < ?2 GROUP BY s.tool"
        ))?;
        for row in st.query_map(params![from, to], |r| Ok((r.get::<_, String>(0)?, usage_at(r, 1)?, r.get::<_, f64>(7)?)))? {
            let (tool, usage, credits) = row?;
            out.push(ToolActivity { tool, tokens: usage.total(), credits, prompts: 0 });
        }
        let mut st = self.conn().prepare(
            "SELECT s.tool, COUNT(*) FROM prompts p JOIN sessions s ON s.id = p.session_id WHERE p.ts >= ?1 AND p.ts < ?2 GROUP BY s.tool",
        )?;
        for row in st.query_map(params![from, to], |r| Ok((r.get::<_, String>(0)?, u64_at(r, 1)?)))? {
            let (tool, n) = row?;
            match out.iter_mut().find(|a| a.tool == tool) {
                Some(a) => a.prompts = n,
                None => out.push(ToolActivity { tool, prompts: n, ..Default::default() }),
            }
        }
        Ok(out)
    }
}
