//! Truy vấn đọc.
use rusqlite::{params, OptionalExtension, Row};
use serde::Serialize;

use super::Store;
use crate::model::{SkillKind, Usage};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PromptStats {
    pub key: String,
    /// Công cụ AI: `claude_code`, `kiro`…
    pub tool: String,
    /// Credit (công cụ tính phí theo credit, ví dụ Kiro).
    pub credits: f64,
    /// Các model đã dùng trong lần hỏi.
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

const USAGE_SUMS: &str = "COALESCE(SUM(input),0), COALESCE(SUM(cw5),0), COALESCE(SUM(cw1h),0), COALESCE(SUM(cache_read),0), COALESCE(SUM(output),0)";

/// SQLite chỉ có số nguyên có dấu; token luôn không âm.
pub(crate) fn u64_at(r: &Row, i: usize) -> rusqlite::Result<u64> {
    Ok(r.get::<_, i64>(i)?.max(0) as u64)
}

fn usage_at(r: &Row, i: usize) -> rusqlite::Result<Usage> {
    Ok(Usage { input: u64_at(r, i)?, cache_write_5m: u64_at(r, i + 1)?, cache_write_1h: u64_at(r, i + 2)?, cache_read: u64_at(r, i + 3)?, output: u64_at(r, i + 4)? })
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
            |r| Ok((usage_at(r, 0)?, r.get::<_, u32>(5)?, r.get::<_, Option<String>>(6)?, r.get::<_, f64>(7)?, split_models(&r.get::<_, String>(8)?))),
        )?;
        let context_at_start = self.conn().query_row(
            "SELECT input + cw5 + cw1h + cache_read FROM api_calls WHERE prompt_key = ?1 AND sidechain = 0 ORDER BY ts LIMIT 1",
            [&key],
            |r| u64_at(r, 0),
        ).optional()?.unwrap_or(0);
        Ok(Some(PromptStats { key, tool, credits, models, session_id, project, ts, text, interrupted, usage, api_calls, context_at_start, last_call_ts }))
    }

    /// Token theo model cho các lượt gọi thỏa `where_sql` (dùng để tính tiền).
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

    /// `ts` của lần hỏi mới nhất, None nếu chưa có.
    pub fn latest_prompt_ts(&self) -> rusqlite::Result<Option<String>> {
        self.conn().query_row("SELECT MAX(ts) FROM prompts", [], |r| r.get(0))
    }

    /// `ts` mới nhất trong `alerts_sent` của các key bắt đầu bằng `prefix`.
    pub fn latest_alert_ts(&self, prefix: &str) -> rusqlite::Result<Option<String>> {
        self.conn().query_row("SELECT MAX(ts) FROM alerts_sent WHERE substr(key, 1, length(?1)) = ?1", [prefix], |r| r.get(0))
    }

    /// Số ngày (theo múi giờ lệch `offset_secs`) có ít nhất một lần hỏi trong [from, to).
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

    /// Tổng token của từng lần hỏi từ `since` (để tính trung vị).
    pub fn prompt_totals_since(&self, since: &str) -> rusqlite::Result<Vec<u64>> {
        let mut st = self.conn().prepare(
            "SELECT COALESCE(SUM(a.input + a.cw5 + a.cw1h + a.cache_read + a.output), 0)
             FROM prompts p LEFT JOIN api_calls a ON a.prompt_key = p.key WHERE p.ts >= ?1 GROUP BY p.key",
        )?;
        let rows = st.query_map([since], |r| u64_at(r, 0))?;
        rows.collect()
    }

    /// Context hiện tại của phiên = context của lượt gọi chính (không phải subagent) mới nhất.
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

    /// Các lần hỏi của phiên theo thời gian.
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

/// Một dòng cho danh sách: số liệu lần hỏi + ⭐.
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
    /// Mọi lần hỏi thỏa điều kiện SQL (tìm kiếm, project, thời gian), mới nhất trước.
    pub fn prompt_items(&self, q: Option<&str>, project: Option<&str>, from: Option<&str>, to: Option<&str>) -> rusqlite::Result<Vec<PromptListItem>> {
        self.prompt_items_for(q, project, from, to, None)
    }

    pub fn prompt_items_for(&self, q: Option<&str>, project: Option<&str>, from: Option<&str>, to: Option<&str>, tool: Option<&str>) -> rusqlite::Result<Vec<PromptListItem>> {
        let pattern = q.filter(|s| !s.is_empty()).map(like_pattern);
        let mut st = self.conn().prepare(&format!(
            "SELECT p.key, p.session_id, COALESCE(s.project, ''), p.ts, p.text, p.interrupted,
                    COALESCE(SUM(a.input),0), COALESCE(SUM(a.cw5),0), COALESCE(SUM(a.cw1h),0), COALESCE(SUM(a.cache_read),0), COALESCE(SUM(a.output),0),
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
        ))?;
        let rows = st.query_map(params![pattern, project, from, to, tool], |r| {
            Ok(PromptListItem {
                stats: PromptStats {
                    key: r.get(0)?,
                    tool: r.get(15)?,
                    credits: r.get(16)?,
                    models: split_models(&r.get::<_, String>(17)?),
                    session_id: r.get(1)?,
                    project: r.get(2)?,
                    ts: r.get(3)?,
                    text: r.get(4)?,
                    interrupted: r.get(5)?,
                    usage: usage_at(r, 6)?,
                    api_calls: r.get(11)?,
                    last_call_ts: r.get(12)?,
                    context_at_start: u64_at(r, 13)?,
                },
                starred: r.get(14)?,
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
    /// Các lượt gọi API của một lần hỏi theo thời gian, kèm tên tool.
    pub fn prompt_calls(&self, key: &str) -> rusqlite::Result<Vec<CallRow>> {
        let mut st = self.conn().prepare(
            "SELECT a.request_id, a.ts, a.model, a.input, a.cw5, a.cw1h, a.cache_read, a.output, a.sidechain,
                    COALESCE((SELECT GROUP_CONCAT(t.name, char(31)) FROM api_call_tools t WHERE t.request_id = a.request_id), '')
             FROM api_calls a WHERE a.prompt_key = ?1 ORDER BY a.ts",
        )?;
        let rows = st.query_map([key], |r| {
            let usage = usage_at(r, 3)?;
            let tools: String = r.get(9)?;
            Ok(CallRow {
                request_id: r.get(0)?,
                ts: r.get(1)?,
                model: r.get(2)?,
                context: usage.context(),
                output: usage.output,
                usage,
                sidechain: r.get(8)?,
                tools: tools.split('\u{1f}').filter(|s| !s.is_empty()).map(str::to_string).collect(),
            })
        })?;
        rows.collect()
    }
}

impl Store {
    /// Mọi lần dùng skill: (prompt_key, session_id, ts, skill, kind).
    pub fn all_skill_uses(&self) -> rusqlite::Result<Vec<(String, String, String, String, SkillKind)>> {
        let mut st = self.conn().prepare("SELECT prompt_key, session_id, ts, skill, kind FROM skill_uses ORDER BY ts")?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get::<_, String>(4)?)))?;
        Ok(rows.filter_map(Result::ok).filter_map(|(a, b, c, d, k)| SkillKind::parse(&k).map(|k| (a, b, c, d, k))).collect())
    }
}

impl Store {
    /// File log chính của phiên (nơi có lần hỏi của người dùng, không phải file subagent).
    pub fn session_file(&self, session_id: &str) -> rusqlite::Result<Option<String>> {
        self.conn()
            .query_row("SELECT source_file FROM prompts WHERE session_id = ?1 ORDER BY ts LIMIT 1", [session_id], |r| r.get(0))
            .optional()
    }
}

impl Store {
    /// Mọi lượt gọi: (ts, model, usage, session_id).
    pub fn calls_brief(&self) -> rusqlite::Result<Vec<(String, String, Usage, String)>> {
        let mut st = self.conn().prepare("SELECT ts, model, input, cw5, cw1h, cache_read, output, session_id FROM api_calls")?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, usage_at(r, 2)?, r.get(7)?)))?;
        rows.collect()
    }

    pub fn session_projects(&self) -> rusqlite::Result<std::collections::HashMap<String, String>> {
        let mut st = self.conn().prepare("SELECT id, project FROM sessions")?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        rows.collect()
    }
}

/// Một dòng thống kê theo model.
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
        let rows = st.query_map([], |r| Ok(ModelRow { model: r.get(0)?, calls: u64_at(r, 1)?, prompts: u64_at(r, 2)?, usage: usage_at(r, 3)?, credits: r.get(8)?, last_used: r.get(9)? }))?;
        rows.collect()
    }
}

impl Store {
    pub fn session_title(&self, session_id: &str) -> rusqlite::Result<Option<String>> {
        self.conn().query_row("SELECT title FROM session_titles WHERE session_id = ?1", [session_id], |r| r.get(0)).optional()
    }
}

impl Store {
    /// (tool, số hội thoại có lượt hỏi, số lượt, lần hoạt động cuối).
    pub fn tool_rows(&self) -> rusqlite::Result<Vec<(String, u64, u64, Option<String>)>> {
        let mut st = self.conn().prepare(
            "SELECT s.tool, COUNT(DISTINCT p.session_id), COUNT(p.key), MAX(p.ts) FROM sessions s LEFT JOIN prompts p ON p.session_id = s.id GROUP BY s.tool",
        )?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, u64_at(r, 1)?, u64_at(r, 2)?, r.get(3)?)))?;
        rows.collect()
    }
}

impl Store {
    /// Toàn bộ tiêu đề hội thoại: session_id → tiêu đề.
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
    /// session_id → công cụ.
    pub fn session_tools(&self) -> rusqlite::Result<std::collections::HashMap<String, String>> {
        let mut st = self.conn().prepare("SELECT id, tool FROM sessions")?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        rows.collect()
    }
}
