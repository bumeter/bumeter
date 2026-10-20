//! `bum report`: the Overview for a terminal — the limits, what burned the quota, and what to do about
//! it. For machines with no tray or window (SSH, servers, WSL), and for agents (`--json`).
//!
//! Token amounts are `Usage::quota_weight()`, as in the app's Effectiveness panel: raw sums are almost
//! all cache reads, which weigh little against a rate limit.
use std::collections::HashMap;

use serde::Serialize;
use bum_core::effect::Effect;
use bum_core::handlers::{self, HandlerCtx};
use bum_core::i18n::Lang;
use bum_core::insight::fmt_tokens;
use bum_core::limits::LimitState;
use bum_core::source::SourceInfo;

/// Projects listed; the rest are left out.
const TOP_PROJECTS: usize = 5;
/// Turns listed under "costliest".
const COSTLIEST: usize = 3;
/// Characters of a question shown next to a costly turn.
const QUESTION_CHARS: usize = 60;

#[derive(Debug, Serialize)]
pub struct ToolShare {
    pub id: String,
    pub label: String,
    /// The tool logs no tokens (Kiro): the numbers are estimated from its context percentage.
    pub estimated: bool,
    pub turns: u64,
    pub weighted: u64,
    pub raw: u64,
}

#[derive(Debug, Serialize)]
pub struct ProjectShare {
    pub project: String,
    pub turns: u64,
    pub weighted: u64,
}

#[derive(Debug, Serialize)]
pub struct CostlyTurn {
    pub key: String,
    pub session_id: String,
    pub ts: String,
    pub tool: String,
    pub project: String,
    pub weighted: u64,
    pub question: String,
}

#[derive(Debug, Serialize)]
pub struct Report {
    pub days: u32,
    /// Start of the range: local midnight, as RFC3339.
    pub from: String,
    pub tool: Option<String>,
    pub limits: LimitState,
    pub turns: u64,
    pub sessions: u64,
    pub weighted: u64,
    pub raw: u64,
    /// Most weighted first.
    pub tools: Vec<ToolShare>,
    pub projects: Vec<ProjectShare>,
    pub costliest: Vec<CostlyTurn>,
    pub effect: Effect,
    /// Sessions named by the advice whose tool `bum session` can analyze (Claude Code today), so the
    /// text only suggests a command that works.
    pub analyzable: Vec<String>,
}

/// Local midnight that starts a range of `days` calendar days (today included), as RFC3339 UTC: the
/// days the app's Overview counts.
pub fn range_start(days: u32, now: chrono::DateTime<chrono::Local>) -> String {
    let start = now.date_naive() - chrono::Days::new(u64::from(days.max(1) - 1));
    let midnight = start.and_hms_opt(0, 0, 0).and_then(|t| t.and_local_timezone(chrono::Local).earliest()).unwrap_or(now);
    midnight.with_timezone(&chrono::Utc).to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

/// First line of a question, cut to something that fits on one terminal line.
fn question(text: &str) -> String {
    let line = text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    let mut out: String = line.chars().take(QUESTION_CHARS).collect();
    if line.chars().count() > QUESTION_CHARS {
        out.push('…');
    }
    out
}

/// Everything in `[from, to)`, optionally for one tool. `sources` gives tool labels.
pub fn build(ctx: &HandlerCtx, sources: &[SourceInfo], days: u32, tool: Option<&str>, from: &str, now: chrono::DateTime<chrono::Utc>) -> anyhow::Result<Report> {
    let to = now.to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let items = ctx.store.prompt_items_for(None, None, Some(from), Some(&to), tool)?;
    let effect = handlers::effectiveness(ctx.store, ctx.cfg, tool, from, &to)?;
    let limits = handlers::limit_state(ctx, now)?;

    let mut tools: HashMap<&str, (u64, u64, u64)> = HashMap::new();
    let mut projects: HashMap<&str, (u64, u64)> = HashMap::new();
    let (mut weighted, mut raw) = (0u64, 0u64);
    for it in &items {
        let (w, r) = (it.stats.usage.quota_weight(), it.stats.usage.total());
        weighted += w;
        raw += r;
        let t = tools.entry(it.stats.tool.as_str()).or_default();
        *t = (t.0 + 1, t.1 + w, t.2 + r);
        if !it.stats.project.is_empty() {
            let p = projects.entry(it.stats.project.as_str()).or_default();
            *p = (p.0 + 1, p.1 + w);
        }
    }

    let info = |id: &str| sources.iter().find(|s| s.id == id);
    let session_tools = ctx.store.session_tools()?;
    let analyzable = effect
        .advice
        .iter()
        .filter_map(|a| a.session_id.clone())
        .filter(|sid| session_tools.get(sid).and_then(|t| info(t)).is_some_and(|s| s.composition))
        .collect();
    let mut tools: Vec<ToolShare> = tools
        .into_iter()
        .map(|(id, (turns, weighted, raw))| ToolShare {
            id: id.to_string(),
            label: info(id).map_or_else(|| id.to_string(), |s| s.label.clone()),
            estimated: info(id).is_some_and(|s| !s.real_tokens),
            turns,
            weighted,
            raw,
        })
        .collect();
    tools.sort_by(|a, b| b.weighted.cmp(&a.weighted).then(a.id.cmp(&b.id)));

    let mut projects: Vec<ProjectShare> =
        projects.into_iter().map(|(p, (turns, weighted))| ProjectShare { project: p.to_string(), turns, weighted }).collect();
    projects.sort_by(|a, b| b.weighted.cmp(&a.weighted).then(a.project.cmp(&b.project)));
    projects.truncate(TOP_PROJECTS);

    let mut costliest: Vec<&bum_core::store::queries::PromptListItem> = items.iter().collect();
    costliest.sort_by(|a, b| b.stats.usage.quota_weight().cmp(&a.stats.usage.quota_weight()).then(a.stats.key.cmp(&b.stats.key)));
    let costliest = costliest
        .into_iter()
        .take(COSTLIEST)
        .filter(|it| it.stats.usage.quota_weight() > 0)
        .map(|it| CostlyTurn {
            key: it.stats.key.clone(),
            session_id: it.stats.session_id.clone(),
            ts: it.stats.ts.clone(),
            tool: it.stats.tool.clone(),
            project: it.stats.project.clone(),
            weighted: it.stats.usage.quota_weight(),
            question: question(&it.stats.text),
        })
        .collect();

    Ok(Report {
        days,
        from: from.to_string(),
        tool: tool.map(String::from),
        limits,
        turns: items.len() as u64,
        sessions: effect.sessions,
        weighted,
        raw,
        tools,
        projects,
        costliest,
        effect,
        analyzable,
    })
}

fn pct(part: u64, whole: u64) -> u64 {
    if whole == 0 { 0 } else { (part as f64 * 100.0 / whole as f64).round() as u64 }
}

/// `MM-DD HH:MM` in local time; the stored value when it does not parse.
fn local_time(ts: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(ts).map(|t| t.with_timezone(&chrono::Local).format("%m-%d %H:%M").to_string()).unwrap_or_else(|_| ts.to_string())
}

/// The report as text. `tool_lines` are the per-tool quota lines of the tray (`bum status`).
pub fn render(r: &Report, tool_lines: &[String], lang: Lang) -> String {
    let mut out = Vec::new();
    let scope = r.tools.iter().find(|t| Some(&t.id) == r.tool.as_ref()).map_or_else(|| lang.pick("mọi tool", "all tools").to_string(), |t| t.label.clone());
    let range = match r.days {
        1 => lang.pick("hôm nay", "today").to_string(),
        n => format!("{n} {}", lang.pick("ngày", "days")),
    };
    out.push(format!("Bumeter · {range} · {scope}"));
    out.push(String::new());

    out.push(format!("{}  {}", lang.pick("Hạn mức  ", "Limits   "), bum_core::limits::render_line(&r.limits, None, lang)));
    for line in tool_lines {
        out.push(format!("           {line}"));
    }
    if r.limits.five_hour.is_none() && r.limits.seven_day.is_none() {
        out.push(format!("           {}", lang.pick(
            "(chưa có số 5h/7d: bật theo dõi hạn mức trong app, hoặc dùng `bum statusline` làm status line của Claude Code)",
            "(no 5h/7d numbers yet: turn on quota tracking in the app, or use `bum statusline` as Claude Code's status line)",
        )));
    }
    out.push(String::new());

    if r.turns == 0 {
        out.push(lang.pick("Chưa có lượt nào trong khoảng này.", "No turns in this range yet.").to_string());
        return out.join("\n") + "\n";
    }

    out.push(format!(
        "{}  {} {} ({} {}) · {} {} · {} {}",
        lang.pick("Đã dùng  ", "Usage    "),
        fmt_tokens(r.weighted),
        lang.pick("token quy đổi", "quota-weighted tokens"),
        fmt_tokens(r.raw),
        lang.pick("thô", "raw"),
        r.turns,
        lang.pick("lượt", "turns"),
        r.sessions,
        lang.pick("phiên", "sessions"),
    ));
    if r.tools.len() > 1 || r.tool.is_none() {
        let parts: Vec<String> = r
            .tools
            .iter()
            .map(|t| {
                let est = if t.estimated { format!(", {}", lang.pick("ước lượng", "estimated")) } else { String::new() };
                format!("{} {} ({}%{est})", t.label, fmt_tokens(t.weighted), pct(t.weighted, r.weighted))
            })
            .collect();
        out.push(format!("{}  {}", lang.pick("Theo tool", "By tool  "), parts.join(" · ")));
    }
    if !r.projects.is_empty() {
        let parts: Vec<String> = r.projects.iter().map(|p| format!("{} {} ({}%)", p.project, fmt_tokens(p.weighted), pct(p.weighted, r.weighted))).collect();
        out.push(format!("{}  {}", lang.pick("Project  ", "Projects "), parts.join(" · ")));
    }
    out.push(String::new());

    if !r.costliest.is_empty() {
        out.push(lang.pick("Lượt tốn nhất", "Costliest turns").to_string());
        for c in &r.costliest {
            out.push(format!("  {}  {:<16} {:>7}  {}", local_time(&c.ts), c.project, fmt_tokens(c.weighted), c.question));
        }
        out.push(String::new());
    }

    let e = &r.effect;
    out.push(format!(
        "{}  {} {} (≈ {}) · {} {} (≈ {}) · {} {}",
        lang.pick("Hiệu quả ", "Effect   "),
        e.interrupted_turns,
        lang.pick("lượt bị hủy", "cancelled"),
        fmt_tokens(e.interrupted_weighted),
        e.big_context_turns,
        lang.pick("lượt context rất lớn", "very large contexts"),
        fmt_tokens(e.big_context_weighted),
        e.compactions,
        lang.pick("lần tràn context", "overflows"),
    ));
    if e.advice.is_empty() {
        out.push(format!("           {}", lang.pick("Không có gì đáng lo trong khoảng này.", "Nothing worth worrying about in this range.")));
    }
    for a in &e.advice {
        let label = match a.confidence {
            bum_core::effect::Confidence::Sure => lang.pick("chắc chắn", "sure"),
            bum_core::effect::Confidence::Likely => lang.pick("có thể", "likely"),
        };
        out.push(format!("  [{label}] {}", a.text(lang)));
        out.push(format!("      {}", a.how(lang)));
        if let Some(sid) = a.session_id.as_ref().filter(|sid| r.analyzable.contains(sid)) {
            out.push(format!("      bum session {sid}"));
        }
    }
    out.push(String::new());
    out.push(lang.pick(
        "Token quy đổi theo hạn mức: cache read ×0,1, ghi cache ×1,25–2, output ×5 — ước lượng, không phải tiền.",
        "Quota-weighted tokens: cache reads ×0.1, cache writes ×1.25–2, output ×5 — an estimate, not money.",
    ).to_string());
    out.join("\n") + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;
    use bum_core::model::{RawEvent, Usage};
    use bum_core::paths::Paths;
    use bum_core::pricing::Pricing;
    use bum_core::store::{FileOffset, Store};

    fn info(id: &str, label: &str, real_tokens: bool) -> SourceInfo {
        SourceInfo { id: id.into(), label: label.into(), letter: "x".into(), color: "#000".into(), icon: None, real_tokens, composition: false, limits: false, credits: false, transcript: false }
    }

    fn call(rid: &str, pk: &str, sid: &str, ts: &str, usage: Usage) -> RawEvent {
        RawEvent::ApiCall { request_id: rid.into(), session_id: sid.into(), prompt_key: Some(pk.into()), ts: ts.into(), model: "claude-x".into(), usage, tool_uses: vec![], sidechain: false, cost_usd: None }
    }

    fn seeded(dir: &std::path::Path) -> (Paths, Store) {
        let paths = Paths::new(dir);
        let mut s = Store::open(&paths.db()).unwrap();
        let cache = |cr: u64, out: u64| Usage { cache_read: cr, output: out, ..Default::default() };
        let events = vec![
            RawEvent::Session { session_id: "a".into(), tool: "claude_code".into(), project: "alpha".into(), cwd: None, version: None, ts: "2026-10-18T01:00:00.000Z".into() },
            RawEvent::Session { session_id: "b".into(), tool: "kiro".into(), project: "beta".into(), cwd: None, version: None, ts: "2026-10-18T02:00:00.000Z".into() },
            RawEvent::Prompt { key: "p1".into(), session_id: "a".into(), ts: "2026-10-18T01:00:00.000Z".into(), text: "refactor the parser\nand more".into() },
            RawEvent::Prompt { key: "p2".into(), session_id: "b".into(), ts: "2026-10-18T02:00:00.000Z".into(), text: "write tests".into() },
            // p1: huge on raw count (cache reads), p2: small raw but all input.
            call("r1", "p1", "a", "2026-10-18T01:00:05.000Z", cache(1_000_000, 0)),
            call("r2", "p2", "b", "2026-10-18T02:00:05.000Z", Usage { input: 200_000, ..Default::default() }),
            // Both sessions overflowed twice; only Claude Code's can be opened with `bum session`.
            RawEvent::Compacted { session_id: "a".into(), ts: "2026-10-18T01:10:00.000Z".into() },
            RawEvent::Compacted { session_id: "a".into(), ts: "2026-10-18T01:20:00.000Z".into() },
            RawEvent::Compacted { session_id: "b".into(), ts: "2026-10-18T02:10:00.000Z".into() },
            RawEvent::Compacted { session_id: "b".into(), ts: "2026-10-18T02:20:00.000Z".into() },
        ];
        let off = FileOffset { path: "/x.jsonl".into(), file_id: "1".into(), size: 1, offset: 1, mtime: 0, ctx_json: "{}".into() };
        s.ingest_batch(&off, &events, 0).unwrap();
        (paths, s)
    }

    fn report(dir: &std::path::Path, tool: Option<&str>) -> Report {
        let (paths, store) = seeded(dir);
        let (pricing, cfg) = (Pricing::default_table(), bum_core::config::Config::default());
        let ctx = HandlerCtx { store: &store, pricing: &pricing, cfg: &cfg, paths: &paths };
        let sources = [SourceInfo { composition: true, ..info("claude_code", "Claude Code", true) }, info("kiro", "Kiro", false)];
        let now = "2026-10-18T12:00:00Z".parse().unwrap();
        build(&ctx, &sources, 7, tool, "2026-10-12T00:00:00.000Z", now).unwrap()
    }

    #[test]
    fn ranks_by_quota_weight_not_raw_tokens() {
        let dir = tempfile::tempdir().unwrap();
        let r = report(dir.path(), None);
        assert_eq!((r.turns, r.sessions, r.raw), (2, 2, 1_200_000));
        // 1M cache reads weigh 100K; 200K input weighs 200K: Kiro's turn is the costlier one.
        assert_eq!(r.weighted, 300_000);
        assert_eq!(r.tools.iter().map(|t| (t.id.as_str(), t.weighted)).collect::<Vec<_>>(), vec![("kiro", 200_000), ("claude_code", 100_000)]);
        assert!(r.tools[0].estimated && !r.tools[1].estimated);
        assert_eq!(r.projects[0].project, "beta");
        assert_eq!(r.costliest[0].key, "p2");
        assert_eq!(r.costliest[1].question, "refactor the parser");
    }

    #[test]
    fn only_suggests_bum_session_where_it_works() {
        let dir = tempfile::tempdir().unwrap();
        let r = report(dir.path(), Some("kiro"));
        assert_eq!(r.effect.advice[0].session_id.as_deref(), Some("b"));
        assert!(r.analyzable.is_empty());
        assert!(!render(&r, &[], Lang::En).contains("bum session"));
        let r = report(dir.path(), Some("claude_code"));
        assert_eq!(r.analyzable, vec!["a".to_string()]);
        assert!(render(&r, &[], Lang::En).contains("bum session a"));
    }

    #[test]
    fn one_tool_only() {
        let dir = tempfile::tempdir().unwrap();
        let r = report(dir.path(), Some("claude_code"));
        assert_eq!((r.turns, r.weighted), (1, 100_000));
        assert_eq!(r.tools.len(), 1);
    }

    #[test]
    fn text_names_the_tools_the_projects_and_the_unit() {
        let dir = tempfile::tempdir().unwrap();
        let r = report(dir.path(), None);
        let en = render(&r, &[], Lang::En);
        assert!(en.starts_with("Bumeter · 7 days · all tools"), "{en}");
        assert!(en.contains("300K quota-weighted tokens (1.2M raw) · 2 turns · 2 sessions"), "{en}");
        assert!(en.contains("Kiro 200K (67%, estimated) · Claude Code 100K (33%)"), "{en}");
        assert!(en.contains("beta 200K (67%)"), "{en}");
        assert!(en.contains("write tests"), "{en}");
        // No 5h/7d numbers in a fresh data directory: say how to get them.
        assert!(en.contains("`bum statusline`"), "{en}");
        assert!(en.trim_end().ends_with("an estimate, not money."), "{en}");
        let vi = render(&r, &[], Lang::Vi);
        assert!(vi.contains("token quy đổi") && vi.contains("ước lượng"), "{vi}");
    }

    #[test]
    fn json_carries_the_same_numbers() {
        let dir = tempfile::tempdir().unwrap();
        let v = serde_json::to_value(report(dir.path(), None)).unwrap();
        assert_eq!(v["weighted"], 300_000);
        assert_eq!(v["tools"][0]["label"], "Kiro");
        assert!(v["effect"]["advice"].is_array());
        assert!(v["limits"].is_object());
    }

    #[test]
    fn range_start_is_local_midnight() {
        let now = chrono::Local::now();
        let from: chrono::DateTime<chrono::Local> = range_start(7, now).parse::<chrono::DateTime<chrono::Utc>>().unwrap().with_timezone(&chrono::Local);
        assert_eq!(from.format("%H:%M").to_string(), "00:00");
        assert_eq!((now.date_naive() - from.date_naive()).num_days(), 6);
        assert_eq!(range_start(1, now).parse::<chrono::DateTime<chrono::Utc>>().unwrap().with_timezone(&chrono::Local).date_naive(), now.date_naive());
    }

    #[test]
    fn questions_are_cut_to_one_line() {
        assert_eq!(question("\n  hello\nworld"), "hello");
        assert_eq!(question(&"x".repeat(70)).chars().count(), QUESTION_CHARS + 1);
    }
}
