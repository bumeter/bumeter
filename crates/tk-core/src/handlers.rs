//! Lớp xử lý dùng chung cho lệnh Tauri và tk-service (spec mục 14.4).
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::attribution::{attribute, AttributedSkill};
use crate::model::Usage;
use crate::pricing::Pricing;
use crate::store::{PromptStats, Store};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TodaySummary {
    pub prompts: u64,
    pub usage: Usage,
    pub total: u64,
    pub cost_usd: f64,
    pub unpriced_models: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PromptRow {
    pub stats: PromptStats,
    pub skills: Vec<AttributedSkill>,
    pub cost_usd: Option<f64>,
    pub starred: bool,
    pub anomalous: bool,
    /// Tổng token / trung vị 30 ngày.
    pub ratio: Option<f64>,
}

/// `from`/`to`: RFC3339 UTC, `to` không bao gồm.
pub fn today_summary(store: &Store, pricing: &Pricing, from: &str, to: &str) -> rusqlite::Result<TodaySummary> {
    let per_model = store.model_usage_between(from, to)?;
    let mut usage = Usage::default();
    for (_, u) in &per_model {
        usage.add(u);
    }
    let (cost_usd, unpriced_models) = pricing.cost_of(&per_model);
    Ok(TodaySummary { prompts: store.prompt_count_between(from, to)?, total: usage.total(), usage, cost_usd, unpriced_models })
}

pub fn recent_prompts(store: &Store, pricing: &Pricing, limit: u32) -> rusqlite::Result<Vec<PromptRow>> {
    let mut by_session: HashMap<String, HashMap<String, Vec<AttributedSkill>>> = HashMap::new();
    let mut rows = Vec::new();
    for key in store.recent_prompt_keys(limit)? {
        let Some(stats) = store.prompt_stats(&key)? else { continue };
        if !by_session.contains_key(&stats.session_id) {
            let sid = stats.session_id.clone();
            let map = attribute(&store.session_prompts(&sid)?, &store.session_skill_uses(&sid)?, &store.session_compactions(&sid)?);
            by_session.insert(sid, map);
        }
        let skills = by_session[&stats.session_id].get(&key).cloned().unwrap_or_default();
        let (cost, unpriced) = pricing.cost_of(&store.model_usage_for_prompt(&key)?);
        let starred = store.get_note(&key)?.is_some_and(|n| n.starred);
        rows.push(PromptRow { stats, skills, cost_usd: unpriced.is_empty().then_some(cost), starred, anomalous: false, ratio: None });
    }
    Ok(rows)
}

pub struct HandlerCtx<'a> {
    pub store: &'a Store,
    pub pricing: &'a Pricing,
    pub cfg: &'a crate::config::Config,
    pub paths: &'a crate::paths::Paths,
}

/// Ghi ⭐/📝 qua một kết nối ghi ngắn hạn rồi sao lưu `notes.json`.
pub fn set_note(paths: &crate::paths::Paths, key: &str, starred: bool, note: &str, now: chrono::DateTime<chrono::Utc>) -> anyhow::Result<()> {
    let mut store = Store::open(&paths.db())?;
    store.set_note(key, starred, note, &now.to_rfc3339_opts(chrono::SecondsFormat::Millis, true))?;
    crate::store::notes::export_notes(&store, &paths.notes())
}

/// Đầu ngày: nửa đêm, hoặc giờ hợp lệ đầu tiên nếu nửa đêm không tồn tại (DST lúc 00:00).
pub fn start_of_day<Tz: chrono::TimeZone>(
    midnight: chrono::LocalResult<chrono::DateTime<Tz>>,
    one_am: impl FnOnce() -> chrono::LocalResult<chrono::DateTime<Tz>>,
) -> chrono::DateTime<Tz> {
    match midnight.earliest() {
        Some(t) => t,
        None => one_am().earliest().expect("01:00 luôn tồn tại khi 00:00 bị bỏ qua"),
    }
}

/// Khoảng [0h hôm nay, 0h ngày mai) theo giờ máy, dạng RFC3339 UTC.
pub fn local_day_bounds(now: chrono::DateTime<chrono::Utc>) -> (String, String) {
    use chrono::{Local, NaiveTime, SecondsFormat, TimeZone};
    let day = now.with_timezone(&Local).date_naive();
    let at = |d: chrono::NaiveDate| {
        let local = |h| Local.from_local_datetime(&d.and_time(NaiveTime::from_hms_opt(h, 0, 0).unwrap()));
        start_of_day(local(0), || local(1)).with_timezone(&chrono::Utc).to_rfc3339_opts(SecondsFormat::Millis, true)
    };
    (at(day), at(day.succ_opt().unwrap_or(day)))
}

pub fn limit_state(ctx: &HandlerCtx, now: chrono::DateTime<chrono::Utc>) -> rusqlite::Result<crate::limits::LimitState> {
    let file = crate::limits::read(&ctx.paths.limits());
    Ok(crate::limits::state(file, ctx.store.latest_rate_limit()?, now.timestamp()))
}

pub fn dispatch(ctx: &HandlerCtx, method: &str, params: &serde_json::Value, now: chrono::DateTime<chrono::Utc>) -> Result<serde_json::Value, String> {
    let to_value = |r: Result<serde_json::Value, rusqlite::Error>| r.map_err(|e| e.to_string());
    match method {
        "ping" => Ok(serde_json::json!("pong")),
        "today_summary" => {
            let (from, to) = local_day_bounds(now);
            to_value(today_summary(ctx.store, ctx.pricing, &from, &to).map(|t| serde_json::to_value(t).unwrap_or_default()))
        }
        "recent_prompts" => {
            let limit = params["limit"].as_u64().unwrap_or(50).min(500) as u32;
            to_value(recent_prompts(ctx.store, ctx.pricing, limit).map(|r| serde_json::to_value(r).unwrap_or_default()))
        }
        "limit_state" => to_value(limit_state(ctx, now).map(|s| serde_json::to_value(s).unwrap_or_default())),
        "list_prompts" => {
            let filter: PromptFilter = if params.is_null() {
                PromptFilter::default()
            } else {
                serde_json::from_value(params.clone()).map_err(|e| format!("bộ lọc không hợp lệ: {e}"))?
            };
            to_value(list_prompts(ctx.store, ctx.pricing, ctx.cfg, &filter).map(|l| serde_json::to_value(l).unwrap_or_default()))
        }
        "prompt_detail" => {
            let key = params["key"].as_str().ok_or("thiếu 'key'")?;
            to_value(prompt_detail(ctx.store, ctx.pricing, ctx.cfg, key).map(|d| serde_json::to_value(d).unwrap_or_default()))
        }
        "sidebar_counts" => to_value(sidebar_counts(ctx.store, ctx.cfg).map(|c| serde_json::to_value(c).unwrap_or_default())),
        "tool_stats" => to_value(tool_stats(ctx.store).map(|c| serde_json::to_value(c).unwrap_or_default())),
        "model_stats" => to_value(model_stats(ctx.store, ctx.pricing).map(|c| serde_json::to_value(c).unwrap_or_default())),
        "project_stats" => to_value(project_stats_for(ctx.store, ctx.pricing, params["tool"].as_str()).map(|c| serde_json::to_value(c).unwrap_or_default())),
        "day_stats" => {
            let offset = chrono::Local::now().offset().local_minus_utc();
            to_value(day_stats_for(ctx.store, ctx.pricing, offset, params["tool"].as_str()).map(|c| serde_json::to_value(c).unwrap_or_default()))
        }
        "list_conversations" => {
            let filter: ConversationFilter = if params.is_null() {
                ConversationFilter::default()
            } else {
                serde_json::from_value(params.clone()).map_err(|e| format!("bộ lọc không hợp lệ: {e}"))?
            };
            let offset = chrono::Local::now().offset().local_minus_utc();
            to_value(list_conversations(ctx.store, ctx.pricing, ctx.cfg, &filter, offset).map(|l| serde_json::to_value(l).unwrap_or_default()))
        }
        "conversation_turns" => {
            let sid = params["session_id"].as_str().ok_or("thiếu 'session_id'")?;
            to_value(conversation_turns(ctx.store, ctx.pricing, ctx.cfg, sid).map(|t| serde_json::to_value(t).unwrap_or_default()))
        }
        "skill_stats" => to_value(skill_stats(ctx.store).map(|c| serde_json::to_value(c).unwrap_or_default())),
        "set_note" => {
            let key = params["key"].as_str().ok_or("thiếu 'key'")?;
            let starred = params["starred"].as_bool().unwrap_or(false);
            let note = params["note"].as_str().unwrap_or_default();
            set_note(ctx.paths, key, starred, note, now).map(|_| serde_json::json!(true)).map_err(|e| e.to_string())
        }
        other => Err(format!("không có lệnh '{other}'")),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Quick {
    Anomalous,
    BigContext,
    Interrupted,
    Starred,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Sort {
    #[default]
    Time,
    Tokens,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct PromptFilter {
    pub q: Option<String>,
    pub project: Option<String>,
    pub skill: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub quick: Option<Quick>,
    /// Chỉ một công cụ (`claude_code`, `kiro`).
    pub tool: Option<String>,
    pub sort: Sort,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SessionGroup {
    pub session_id: String,
    pub project: String,
    pub first_ts: String,
    pub last_ts: String,
    pub total: u64,
    pub prompts: u32,
    pub rows: Vec<PromptRow>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PromptList {
    pub groups: Vec<SessionGroup>,
    pub total_prompts: u64,
    pub total_tokens: u64,
    pub median: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SidebarCounts {
    pub prompts: u64,
    pub skills: u64,
    pub projects: u64,
    pub anomalous: u64,
    pub big_context: u64,
    pub interrupted: u64,
    pub starred: u64,
    /// Số lần hỏi theo từng công cụ (claude_code, kiro, …) để UI lọc và hiển thị.
    pub by_tool: HashMap<String, u64>,
}

const DEFAULT_LIMIT: u32 = 200;
const MAX_LIMIT: u32 = 1000;

/// Trung vị trong 30 ngày tính ngược từ lần hỏi mới nhất (không phụ thuộc giờ hiện tại).
fn median_30d(items: &[crate::store::queries::PromptListItem]) -> Option<u64> {
    let newest = items.iter().map(|i| i.stats.ts.as_str()).max()?;
    let newest = chrono::DateTime::parse_from_rfc3339(newest).ok()?;
    let since = (newest - chrono::Duration::days(30)).to_utc().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let mut totals: Vec<u64> = items.iter().filter(|i| i.stats.ts >= since).map(|i| i.stats.usage.total()).collect();
    crate::insight::median(&mut totals)
}

fn attribution_for(store: &Store, items: &[crate::store::queries::PromptListItem]) -> rusqlite::Result<HashMap<String, Vec<AttributedSkill>>> {
    let mut out = HashMap::new();
    let mut seen = std::collections::HashSet::new();
    for it in items {
        if seen.insert(it.stats.session_id.clone()) {
            let sid = &it.stats.session_id;
            out.extend(attribute(&store.session_prompts(sid)?, &store.session_skill_uses(sid)?, &store.session_compactions(sid)?));
        }
    }
    Ok(out)
}

pub fn list_prompts(store: &Store, pricing: &Pricing, cfg: &crate::config::Config, f: &PromptFilter) -> rusqlite::Result<PromptList> {
    let all = store.prompt_items(None, None, None, None)?;
    let median = median_30d(&all);
    let items = store.prompt_items_for(f.q.as_deref(), f.project.as_deref(), f.from.as_deref(), f.to.as_deref(), f.tool.as_deref())?;
    let mut rows: Vec<PromptRow> = turn_rows(store, cfg, items, median)?
        .into_iter()
        .filter(|r| f.skill.as_ref().is_none_or(|s| r.skills.iter().any(|a| &a.skill == s)))
        .filter(|r| match f.quick {
            None => true,
            Some(Quick::Anomalous) => r.anomalous,
            Some(Quick::BigContext) => r.stats.context_at_start > cfg.context_threshold,
            Some(Quick::Interrupted) => r.stats.interrupted,
            Some(Quick::Starred) => r.starred,
        })
        .collect();

    if f.sort == Sort::Tokens {
        rows.sort_by(|a, b| b.stats.usage.total().cmp(&a.stats.usage.total()));
    }
    let total_prompts = rows.len() as u64;
    let total_tokens = rows.iter().map(|r| r.stats.usage.total()).sum();
    let mut session_totals: HashMap<String, (u64, u32)> = HashMap::new();
    for r in &rows {
        let e = session_totals.entry(r.stats.session_id.clone()).or_default();
        e.0 += r.stats.usage.total();
        e.1 += 1;
    }

    let offset = f.offset.unwrap_or(0) as usize;
    let limit = f.limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let mut page: Vec<PromptRow> = rows.into_iter().skip(offset).take(limit).collect();
    for r in &mut page {
        let (cost, unpriced) = pricing.cost_of(&store.model_usage_for_prompt(&r.stats.key)?);
        r.cost_usd = unpriced.is_empty().then_some(cost);
    }

    let mut groups: Vec<SessionGroup> = Vec::new();
    for r in page {
        let group_key = if f.sort == Sort::Time { r.stats.session_id.clone() } else { String::new() };
        let start_new = groups.last().is_none_or(|g| g.session_id != group_key);
        if start_new {
            let (total, prompts) = if f.sort == Sort::Time { session_totals[&group_key] } else { (total_tokens, total_prompts as u32) };
            let project = if f.sort == Sort::Time { r.stats.project.clone() } else { String::new() };
            groups.push(SessionGroup { session_id: group_key, project, first_ts: r.stats.ts.clone(), last_ts: r.stats.ts.clone(), total, prompts, rows: Vec::new() });
        }
        let g = groups.last_mut().expect("vừa thêm");
        if r.stats.ts < g.first_ts { g.first_ts = r.stats.ts.clone(); }
        if r.stats.ts > g.last_ts { g.last_ts = r.stats.ts.clone(); }
        g.rows.push(r);
    }
    Ok(PromptList { groups, total_prompts, total_tokens, median })
}

pub fn sidebar_counts(store: &Store, cfg: &crate::config::Config) -> rusqlite::Result<SidebarCounts> {
    let all = store.prompt_items(None, None, None, None)?;
    let median = median_30d(&all);
    let count = |pred: &dyn Fn(&crate::store::queries::PromptListItem) -> bool| all.iter().filter(|i| pred(i)).count() as u64;
    let mut by_tool: HashMap<String, u64> = HashMap::new();
    for i in &all {
        *by_tool.entry(i.stats.tool.clone()).or_default() += 1;
    }
    Ok(SidebarCounts {
        prompts: all.len() as u64,
        skills: store.distinct_skill_count()?,
        projects: store.distinct_project_count()?,
        anomalous: count(&|i| crate::insight::anomaly(i.stats.usage.total(), i.stats.context_at_start, median, cfg).is_some()),
        big_context: count(&|i| i.stats.context_at_start > cfg.context_threshold),
        interrupted: count(&|i| i.stats.interrupted),
        starred: count(&|i| i.starred),
        by_tool,
    })
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PromptDetail {
    pub row: PromptRow,
    pub calls: Vec<crate::store::queries::CallRow>,
    /// Tỉ lệ token là đọc lại cache (context cũ).
    pub reread_share: f64,
    pub explanation: crate::insight::Explain,
    pub duration_secs: Option<i64>,
    pub note: Option<crate::store::notes::Note>,
}

pub fn prompt_detail(store: &Store, pricing: &Pricing, cfg: &crate::config::Config, key: &str) -> rusqlite::Result<Option<PromptDetail>> {
    let Some(stats) = store.prompt_stats(key)? else { return Ok(None) };
    let all = store.prompt_items(None, None, None, None)?;
    let median = median_30d(&all);
    let sid = stats.session_id.clone();
    let skills = attribute(&store.session_prompts(&sid)?, &store.session_skill_uses(&sid)?, &store.session_compactions(&sid)?)
        .remove(key)
        .unwrap_or_default();
    let note = store.get_note(key)?;
    let total = stats.usage.total();
    let (cost, unpriced) = pricing.cost_of(&store.model_usage_for_prompt(key)?);
    let parse = |s: &str| chrono::DateTime::parse_from_rfc3339(s).ok();
    let duration_secs = match (parse(&stats.ts), stats.last_call_ts.as_deref().and_then(parse)) {
        (Some(a), Some(b)) => Some((b - a).num_seconds()),
        _ => None,
    };
    let reread_share = if total == 0 { 0.0 } else { stats.usage.cache_read as f64 / total as f64 };
    let explanation = crate::insight::explain(&stats.usage, stats.context_at_start);
    let row = PromptRow {
        anomalous: crate::insight::anomaly(total, stats.context_at_start, median, cfg).is_some(),
        ratio: median.filter(|m| *m > 0).map(|m| total as f64 / m as f64),
        starred: note.as_ref().is_some_and(|n| n.starred),
        cost_usd: unpriced.is_empty().then_some(cost),
        skills,
        stats,
    };
    Ok(Some(PromptDetail { calls: store.prompt_calls(key)?, row, reread_share, explanation, duration_secs, note }))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SkillStat {
    pub name: String,
    /// Số lần gọi trực tiếp (tool Skill / slash command).
    pub uses: u64,
    pub projects: u64,
    /// Token của các lần hỏi gọi trực tiếp và làm tiếp.
    pub tokens: u64,
    pub avg_tokens: Option<u64>,
    pub last_used: Option<String>,
}

pub fn skill_stats(store: &Store) -> rusqlite::Result<Vec<SkillStat>> {
    #[derive(Default)]
    struct Acc { uses: u64, projects: std::collections::HashSet<String>, tokens: u64, last: Option<String> }
    let items = store.prompt_items(None, None, None, None)?;
    let by_key: HashMap<&str, &crate::store::queries::PromptListItem> = items.iter().map(|i| (i.stats.key.as_str(), i)).collect();
    let skills = attribution_for(store, &items)?;
    let mut acc: std::collections::BTreeMap<String, Acc> = std::collections::BTreeMap::new();

    for (key, list) in &skills {
        let Some(item) = by_key.get(key.as_str()) else { continue };
        for a in list {
            let e = acc.entry(a.skill.clone()).or_default();
            e.tokens += item.stats.usage.total();
            if a.how != crate::attribution::How::Continued {
                e.uses += 1;
                e.projects.insert(item.stats.project.clone());
                if e.last.as_deref().is_none_or(|l| l < item.stats.ts.as_str()) {
                    e.last = Some(item.stats.ts.clone());
                }
            }
        }
    }
    // Lần dùng không gắn với lần hỏi nào (slash command gõ riêng, ví dụ /model).
    let orphans: Vec<_> = store.all_skill_uses()?.into_iter().filter(|u| !by_key.contains_key(u.0.as_str())).collect();
    for (prompt_key, session_id, ts, skill, kind) in &orphans {
        // Gõ `/X` để gọi skill X: log ghi cả lệnh lẫn skill cùng promptId → chỉ đếm skill X.
        if *kind == crate::model::SkillKind::Command {
            let bare = skill.trim_start_matches('/');
            if orphans.iter().any(|o| &o.0 == prompt_key && o.4 == crate::model::SkillKind::Direct && o.3 == bare) {
                continue;
            }
        }
        let (session_id, ts, skill) = (session_id.clone(), ts.clone(), skill.clone());
        let e = acc.entry(skill).or_default();
        e.uses += 1;
        e.projects.insert(store.session_project(&session_id)?.unwrap_or_default());
        if e.last.as_deref().is_none_or(|l| l < ts.as_str()) {
            e.last = Some(ts);
        }
    }
    Ok(acc
        .into_iter()
        .map(|(name, a)| SkillStat {
            avg_tokens: (a.uses > 0 && a.tokens > 0).then(|| a.tokens / a.uses),
            name,
            uses: a.uses,
            projects: a.projects.len() as u64,
            tokens: a.tokens,
            last_used: a.last,
        })
        .collect())
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PriciestPrompt {
    pub key: String,
    pub text: String,
    pub total: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProjectStat {
    pub project: String,
    pub sessions: u64,
    pub prompts: u64,
    pub tokens: u64,
    pub cost_usd: f64,
    pub top_skills: Vec<String>,
    pub priciest: Option<PriciestPrompt>,
    pub last_used: String,
}

/// Tổng hợp theo project/ticket, nhiều token nhất trước (spec mục 6, wireframe màn 5).
pub fn project_stats(store: &Store, pricing: &Pricing) -> rusqlite::Result<Vec<ProjectStat>> {
    project_stats_for(store, pricing, None)
}

/// Như `project_stats` nhưng chỉ tính một công cụ (phạm vi đang chọn trên UI).
pub fn project_stats_for(store: &Store, pricing: &Pricing, tool: Option<&str>) -> rusqlite::Result<Vec<ProjectStat>> {
    #[derive(Default)]
    struct Acc { sessions: std::collections::HashSet<String>, prompts: u64, tokens: u64, per_model: HashMap<String, Usage>, skills: HashMap<String, u64>, priciest: Option<PriciestPrompt>, last: String }
    let projects = store.session_projects()?;
    let tools = store.session_tools()?;
    let items = store.prompt_items_for(None, None, None, None, tool)?;
    let skills = attribution_for(store, &items)?;
    let mut acc: HashMap<String, Acc> = HashMap::new();
    for (_, model, u, sid) in store.calls_brief()? {
        if tool.is_some_and(|t| tools.get(&sid).map(String::as_str) != Some(t)) { continue }
        let Some(p) = projects.get(&sid) else { continue };
        let a = acc.entry(p.clone()).or_default();
        a.tokens += u.total();
        a.per_model.entry(model).or_default().add(&u);
    }
    for it in &items {
        let a = acc.entry(it.stats.project.clone()).or_default();
        a.sessions.insert(it.stats.session_id.clone());
        a.prompts += 1;
        if it.stats.ts > a.last { a.last = it.stats.ts.clone(); }
        let total = it.stats.usage.total();
        if a.priciest.as_ref().is_none_or(|p| total > p.total) {
            a.priciest = Some(PriciestPrompt { key: it.stats.key.clone(), text: it.stats.text.chars().take(120).collect(), total });
        }
        for s in skills.get(&it.stats.key).into_iter().flatten().filter(|s| s.how != crate::attribution::How::Continued) {
            *a.skills.entry(s.skill.clone()).or_default() += 1;
        }
    }
    let mut out: Vec<ProjectStat> = acc
        .into_iter()
        .filter(|(_, a)| a.prompts > 0)
        .map(|(project, a)| {
            let mut top: Vec<(String, u64)> = a.skills.into_iter().collect();
            top.sort_by(|x, y| y.1.cmp(&x.1).then(x.0.cmp(&y.0)));
            let per_model: Vec<(String, Usage)> = a.per_model.into_iter().collect();
            ProjectStat {
                project,
                sessions: a.sessions.len() as u64,
                prompts: a.prompts,
                tokens: a.tokens,
                cost_usd: pricing.cost_of(&per_model).0,
                top_skills: top.into_iter().take(3).map(|t| t.0).collect(),
                priciest: a.priciest,
                last_used: a.last,
            }
        })
        .collect();
    out.sort_by(|a, b| b.tokens.cmp(&a.tokens).then(a.project.cmp(&b.project)));
    Ok(out)
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DayStat {
    /// `YYYY-MM-DD` theo giờ địa phương.
    pub day: String,
    /// Khoảng RFC3339 UTC của ngày đó, để lọc danh sách lần hỏi.
    pub from: String,
    pub to: String,
    pub tokens: u64,
    pub prompts: u64,
    pub cost_usd: f64,
}

/// Token/lần hỏi/chi phí theo ngày; `offset_secs` = lệch múi giờ so với UTC.
pub fn day_stats(store: &Store, pricing: &Pricing, offset_secs: i32) -> rusqlite::Result<Vec<DayStat>> {
    day_stats_for(store, pricing, offset_secs, None)
}

pub fn day_stats_for(store: &Store, pricing: &Pricing, offset_secs: i32, tool: Option<&str>) -> rusqlite::Result<Vec<DayStat>> {
    use chrono::{DateTime, FixedOffset, SecondsFormat};
    let tools = store.session_tools()?;
    let tz = FixedOffset::east_opt(offset_secs).unwrap_or(FixedOffset::east_opt(0).expect("UTC"));
    let day_of = |ts: &str| DateTime::parse_from_rfc3339(ts).ok().map(|t| t.with_timezone(&tz).date_naive());
    let mut days: std::collections::BTreeMap<chrono::NaiveDate, (u64, u64, HashMap<String, Usage>)> = Default::default();
    for (ts, model, u, sid) in store.calls_brief()? {
        if tool.is_some_and(|t| tools.get(&sid).map(String::as_str) != Some(t)) { continue }
        let Some(d) = day_of(&ts) else { continue };
        let e = days.entry(d).or_default();
        e.0 += u.total();
        e.2.entry(model).or_default().add(&u);
    }
    for it in store.prompt_items_for(None, None, None, None, tool)? {
        if let Some(d) = day_of(&it.stats.ts) {
            days.entry(d).or_default().1 += 1;
        }
    }
    let bound = |d: chrono::NaiveDate| {
        d.and_hms_opt(0, 0, 0).and_then(|n| n.and_local_timezone(tz).single()).map(|t| t.to_utc().to_rfc3339_opts(SecondsFormat::Millis, true)).unwrap_or_default()
    };
    Ok(days
        .into_iter()
        .map(|(d, (tokens, prompts, per_model))| DayStat {
            day: d.format("%Y-%m-%d").to_string(),
            from: bound(d),
            to: bound(d.succ_opt().unwrap_or(d)),
            tokens,
            prompts,
            cost_usd: pricing.cost_of(&per_model.into_iter().collect::<Vec<_>>()).0,
        })
        .collect())
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ModelStat {
    pub model: String,
    /// `claude_code` hoặc `kiro` (model Kiro có tiền tố `kiro:`).
    pub tool: String,
    pub calls: u64,
    pub prompts: u64,
    pub tokens: u64,
    pub output: u64,
    /// `None` nếu model không có trong bảng giá (ví dụ Kiro tính bằng credit).
    pub cost_usd: Option<f64>,
    pub credits: f64,
    pub last_used: String,
}

pub fn model_stats(store: &Store, pricing: &Pricing) -> rusqlite::Result<Vec<ModelStat>> {
    let mut out: Vec<ModelStat> = store
        .model_rows()?
        .into_iter()
        .map(|r| ModelStat {
            tool: if r.model.starts_with("kiro:") { "kiro".into() } else { "claude_code".into() },
            cost_usd: pricing.cost(&r.model, &r.usage),
            tokens: r.usage.total(),
            output: r.usage.output,
            model: r.model,
            calls: r.calls,
            prompts: r.prompts,
            credits: r.credits,
            last_used: r.last_used,
        })
        .collect();
    out.sort_by(|a, b| b.tokens.cmp(&a.tokens).then(a.model.cmp(&b.model)));
    Ok(out)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ToolStat {
    pub tool: String,
    pub sessions: u64,
    pub prompts: u64,
    pub last_ts: Option<String>,
}

pub fn tool_stats(store: &Store) -> rusqlite::Result<Vec<ToolStat>> {
    Ok(store.tool_rows()?.into_iter().map(|(tool, sessions, prompts, last_ts)| ToolStat { tool, sessions, prompts, last_ts }).collect())
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct ConversationFilter {
    /// Tìm trong tiêu đề và nội dung các lượt.
    pub q: Option<String>,
    pub project: Option<String>,
    /// Hội thoại có dùng ít nhất một model trong danh sách.
    pub models: Vec<String>,
    pub tool: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub quick: Option<Quick>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Conversation {
    pub session_id: String,
    pub tool: String,
    pub project: String,
    pub title: String,
    pub first_ts: String,
    pub last_ts: String,
    pub turns: u32,
    pub tokens: u64,
    pub credits: f64,
    pub models: Vec<String>,
    /// Context lớn nhất lúc bắt đầu một lượt.
    pub peak_context: u64,
    pub interrupted: u32,
    pub anomalous: u32,
    pub starred: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ConversationDay {
    /// `YYYY-MM-DD` theo giờ địa phương, tính theo lượt cuối của hội thoại.
    pub day: String,
    pub tokens: u64,
    pub conversations: Vec<Conversation>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ConversationList {
    pub days: Vec<ConversationDay>,
    pub total_conversations: u64,
    pub total_tokens: u64,
}

const TITLE_MAX: usize = 120;

fn fallback_title(text: &str) -> String {
    let line = text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or_default();
    let mut t: String = line.chars().take(TITLE_MAX).collect();
    if line.chars().count() > TITLE_MAX {
        t.push('…');
    }
    t
}

fn turn_rows(store: &Store, cfg: &crate::config::Config, items: Vec<crate::store::queries::PromptListItem>, median: Option<u64>) -> rusqlite::Result<Vec<PromptRow>> {
    let skills = attribution_for(store, &items)?;
    Ok(items
        .into_iter()
        .map(|it| {
            let total = it.stats.usage.total();
            let anomalous = crate::insight::anomaly(total, it.stats.context_at_start, median, cfg).is_some();
            let ratio = median.filter(|m| *m > 0).map(|m| total as f64 / m as f64);
            let row_skills = skills.get(&it.stats.key).cloned().unwrap_or_default();
            PromptRow { stats: it.stats, skills: row_skills, cost_usd: None, starred: it.starred, anomalous, ratio }
        })
        .collect())
}

/// Hội thoại (phiên) gom theo ngày, mới nhất trước. Số liệu luôn là của cả hội thoại;
/// bộ lọc chỉ quyết định hội thoại nào được hiện.
pub fn list_conversations(
    store: &Store,
    _pricing: &Pricing,
    cfg: &crate::config::Config,
    f: &ConversationFilter,
    offset_secs: i32,
) -> rusqlite::Result<ConversationList> {
    use chrono::{DateTime, FixedOffset};
    let all = store.prompt_items(None, None, None, None)?;
    let median = median_30d(&all);
    let items = store.prompt_items_for(None, f.project.as_deref(), None, None, f.tool.as_deref())?;
    let titles = store.session_titles()?;
    let q = f.q.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(str::to_lowercase);

    // Giữ thứ tự xuất hiện đầu tiên (items đã sắp mới → cũ).
    let mut order: Vec<String> = Vec::new();
    let mut by_session: HashMap<String, Vec<crate::store::queries::PromptListItem>> = HashMap::new();
    for it in items {
        let sid = it.stats.session_id.clone();
        by_session.entry(sid.clone()).or_insert_with(|| {
            order.push(sid);
            Vec::new()
        }).push(it);
    }

    let mut convs: Vec<Conversation> = Vec::new();
    for sid in order {
        let turns = &by_session[&sid];
        let in_range = |ts: &str| f.from.as_deref().is_none_or(|a| ts >= a) && f.to.as_deref().is_none_or(|b| ts < b);
        if !turns.iter().any(|t| in_range(&t.stats.ts)) {
            continue;
        }
        let first = turns.last().expect("không rỗng");
        let title = titles.get(&sid).cloned().unwrap_or_else(|| fallback_title(&first.stats.text));
        if let Some(q) = &q {
            let hit = title.to_lowercase().contains(q.as_str()) || turns.iter().any(|t| t.stats.text.to_lowercase().contains(q.as_str()));
            if !hit {
                continue;
            }
        }
        let mut models: Vec<String> = Vec::new();
        for t in turns {
            for m in &t.stats.models {
                if !models.contains(m) {
                    models.push(m.clone());
                }
            }
        }
        if !f.models.is_empty() && !models.iter().any(|m| f.models.contains(m)) {
            continue;
        }
        let is_anomalous = |t: &crate::store::queries::PromptListItem| crate::insight::anomaly(t.stats.usage.total(), t.stats.context_at_start, median, cfg).is_some();
        let c = Conversation {
            session_id: sid.clone(),
            tool: first.stats.tool.clone(),
            project: first.stats.project.clone(),
            title,
            first_ts: first.stats.ts.clone(),
            last_ts: turns[0].stats.ts.clone(),
            turns: turns.len() as u32,
            tokens: turns.iter().map(|t| t.stats.usage.total()).sum(),
            credits: turns.iter().map(|t| t.stats.credits).sum(),
            models,
            peak_context: turns.iter().map(|t| t.stats.context_at_start).max().unwrap_or(0),
            interrupted: turns.iter().filter(|t| t.stats.interrupted).count() as u32,
            anomalous: turns.iter().filter(|t| is_anomalous(t)).count() as u32,
            starred: turns.iter().filter(|t| t.starred).count() as u32,
        };
        let keep = match f.quick {
            None => true,
            Some(Quick::Anomalous) => c.anomalous > 0,
            Some(Quick::BigContext) => c.peak_context > cfg.context_threshold,
            Some(Quick::Interrupted) => c.interrupted > 0,
            Some(Quick::Starred) => c.starred > 0,
        };
        if keep {
            convs.push(c);
        }
    }
    convs.sort_by(|a, b| b.last_ts.cmp(&a.last_ts));
    let total_conversations = convs.len() as u64;
    let total_tokens = convs.iter().map(|c| c.tokens).sum();
    convs.truncate(f.limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize);

    let tz = FixedOffset::east_opt(offset_secs).unwrap_or(FixedOffset::east_opt(0).expect("UTC"));
    let mut days: Vec<ConversationDay> = Vec::new();
    for c in convs {
        let day = DateTime::parse_from_rfc3339(&c.last_ts).map(|t| t.with_timezone(&tz).format("%Y-%m-%d").to_string()).unwrap_or_default();
        if days.last().is_none_or(|d| d.day != day) {
            days.push(ConversationDay { day, tokens: 0, conversations: Vec::new() });
        }
        let d = days.last_mut().expect("vừa thêm");
        d.tokens += c.tokens;
        d.conversations.push(c);
    }
    Ok(ConversationList { days, total_conversations, total_tokens })
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ConversationTurns {
    pub title: String,
    pub turns: Vec<PromptRow>,
}

/// Tiêu đề và các lượt của một hội thoại, theo thứ tự thời gian.
pub fn conversation_turns(store: &Store, pricing: &Pricing, cfg: &crate::config::Config, session_id: &str) -> rusqlite::Result<ConversationTurns> {
    let all = store.prompt_items(None, None, None, None)?;
    let median = median_30d(&all);
    let mut items: Vec<_> = all.into_iter().filter(|i| i.stats.session_id == session_id).collect();
    items.reverse();
    let mut rows = turn_rows(store, cfg, items, median)?;
    for r in &mut rows {
        let (cost, unpriced) = pricing.cost_of(&store.model_usage_for_prompt(&r.stats.key)?);
        r.cost_usd = unpriced.is_empty().then_some(cost);
    }
    let title = match store.session_title(session_id)? {
        Some(t) => t,
        None => rows.first().map(|r| fallback_title(&r.stats.text)).unwrap_or_default(),
    };
    Ok(ConversationTurns { title, turns: rows })
}
