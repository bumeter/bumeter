//! "Where did the quota go without a result?" — post-hoc analysis of the local history.
//!
//! Only signals the history records as facts are used: a turn the user interrupted (Esc), a turn
//! that started with a very large context, and context overflows (compactions). Guessing from the
//! wording of the next message ("no, …", "sửa lại …") was tried and dropped: on real history most
//! matches were new requests, not corrections. No model is called, so this costs no tokens, and
//! nothing leaves the machine.
//!
//! Token amounts are `Usage::quota_weight()`, not raw sums: raw sums are almost all cache reads,
//! which weigh little against a rate limit.
use std::collections::HashMap;

use chrono::DateTime;
use serde::Serialize;

use crate::model::Usage;
use crate::store::queries::{PromptListItem, RangeCall, ToolCallRow};

/// An interrupted turn counts as a *late* cancel when the model had already been working this long.
pub const LATE_CANCEL_SECS: i64 = 120;
/// A session that overflowed its context this many times in the range is worth splitting.
pub const SPLIT_AFTER_COMPACTIONS: u64 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    /// The tokens were spent: the number is a fact from the log.
    Sure,
    /// The tokens were spent, but whether acting on the advice saves them is an estimate.
    Likely,
    /// An upper bound: at most this much could be saved.
    Max,
}

/// One piece of advice. The UI turns `code` into a sentence; the numbers are its parameters.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Advice {
    /// `late_cancel` | `big_context` | `split_session` | `resend` | `startup` | `cold_cache` | `blocked`
    pub code: &'static str,
    pub confidence: Confidence,
    /// Turns (`late_cancel`, `big_context`) or sessions (`split_session`).
    pub count: u64,
    /// Quota-weighted tokens of those turns.
    pub weighted: u64,
    /// For `split_session`: the session that overflowed most, so the UI can open it.
    pub session_id: Option<String>,
    /// For `repeated_read` / `tool_retry`: the file read most / the tool retried most, to name it.
    pub target: Option<String>,
}

impl Advice {
    /// The finding, for the CLI (the UI translates it from the code; keep the two in step).
    pub fn text(&self, lang: crate::i18n::Lang) -> String {
        use crate::i18n::Lang::*;
        let (n, t) = (self.count, crate::insight::fmt_tokens(self.weighted));
        match (self.code, lang) {
            ("late_cancel", Vi) => format!("{n} lượt bị dừng khi model đã chạy hơn 2 phút — ≈ {t} token quy đổi không ra kết quả"),
            ("late_cancel", En) => format!("{n} turns stopped after the model had run for over 2 minutes — ≈ {t} quota-weighted tokens with no result"),
            ("big_context", Vi) => format!("{n} lượt bắt đầu với context rất lớn — ≈ {t} token quy đổi"),
            ("big_context", En) => format!("{n} turns started with a very large context — ≈ {t} quota-weighted tokens"),
            ("split_session", Vi) => format!("{n} phiên tràn context từ 2 lần trở lên — ≈ {t} token quy đổi"),
            ("split_session", En) => format!("{n} sessions overflowed their context 2 or more times — ≈ {t} quota-weighted tokens"),
            ("resend", Vi) => format!("{n} phiên gửi lại cùng context hơn {} lần — ≈ {t} token quy đổi", RESEND_FLAG as u64),
            ("resend", En) => format!("{n} sessions sent the same context again more than {} times — ≈ {t} quota-weighted tokens", RESEND_FLAG as u64),
            ("startup", Vi) => format!("Mỗi phiên khởi động với hơn {} token trước câu hỏi đầu tiên ({n} phiên) — ≈ {t} token quy đổi", crate::insight::fmt_tokens(STARTUP_FLAG)),
            ("startup", En) => format!("Every session starts with over {} tokens before the first question ({n} sessions) — ≈ {t} quota-weighted tokens", crate::insight::fmt_tokens(STARTUP_FLAG)),
            ("cold_cache", Vi) => format!("Cache nguội {n} lần: nghỉ quá lâu giữa hai lượt nên phải ghi lại cả context — ≈ {t} token quy đổi trả thêm"),
            ("cold_cache", En) => format!("The cache went cold {n} times: a long pause between turns made the whole context be written again — ≈ {t} extra quota-weighted tokens"),
            ("blocked", Vi) => format!("Bị chặn vì chạm hạn mức {n} lần"),
            ("blocked", En) => format!("Hit the rate limit {n} times"),
            ("repeated_read", Vi) => format!("{n} file bị đọc lại từ {REPEAT_READ} lần trở lên mà không sửa gì, nhiều nhất là {} — ≈ {t} token quy đổi", self.target.as_deref().unwrap_or("?")),
            ("repeated_read", En) => format!("{n} files were read {REPEAT_READ} or more times without an edit, most of all {} — ≈ {t} quota-weighted tokens", self.target.as_deref().unwrap_or("?")),
            ("tool_retry", Vi) => format!("{n} lần tool lỗi rồi bị gọi lại, nhiều nhất là {} — ≈ {t} token quy đổi của các call lỗi", self.target.as_deref().unwrap_or("?")),
            ("tool_retry", En) => format!("{n} times a tool failed and was called again, most of all {} — ≈ {t} quota-weighted tokens in the failing calls", self.target.as_deref().unwrap_or("?")),
            (code, _) => code.to_string(),
        }
    }

    /// What to do about it.
    pub fn how(&self, lang: crate::i18n::Lang) -> &'static str {
        match self.code {
            "late_cancel" => lang.pick(
                "Nói rõ yêu cầu hoặc chốt kế hoạch trước khi để model chạy lâu; dừng sớm khi thấy lệch hướng.",
                "State the request or agree on a plan before letting the model run long; stop early when it drifts.",
            ),
            "big_context" => lang.pick(
                "Mỗi lượt gửi lại toàn bộ context. Dùng /compact hoặc mở phiên mới khi chuyển sang việc khác.",
                "Every turn sends the whole context again. Use /compact, or start a new session when you switch tasks.",
            ),
            "split_session" => lang.pick(
                "Chia việc lớn thành nhiều phiên ngắn; tràn context làm model quên phần đầu và phải đọc lại.",
                "Split big work into shorter sessions; an overflow makes the model forget the start and read it again.",
            ),
            "resend" => lang.pick(
                "Mỗi call gửi lại toàn bộ context. Phiên càng dài, mỗi câu hỏi càng đắt: mở phiên mới khi đổi việc.",
                "Every call sends the whole context again. The longer the session, the dearer each question: start a new one when you switch tasks.",
            ),
            "startup" => lang.pick(
                "Gồm system prompt, CLAUDE.md, skill luôn bật và định nghĩa tool. Xem phần nào lớn nhất trong phân tích phiên.",
                "System prompt, CLAUDE.md, always-on skills and tool definitions. See which part is largest in the session analysis.",
            ),
            "cold_cache" => lang.pick(
                "Cache giữ 5 phút (hoặc 1 giờ). Ghi lại cache tốn ×1,25–2, đọc cache chỉ ×0,1: hỏi tiếp sớm, hoặc /compact trước khi nghỉ lâu.",
                "The cache lasts 5 minutes (or 1 hour). Writing it again costs ×1.25–2, reading it only ×0.1: follow up sooner, or /compact before a long break.",
            ),
            "repeated_read" => lang.pick(
                "Đọc một lần, hoặc dùng offset/limit để chỉ đọc phần cần; mỗi lần đọc lại đưa cả file vào context.",
                "Read it once, or use offset/limit to read only the part you need; every read puts the whole file in the context again.",
            ),
            "tool_retry" => lang.pick(
                "Xem lỗi lặp ở đâu: thường là đường dẫn sai, lệnh sai cú pháp, hoặc test hỏng chưa sửa. Nói rõ cho model trước khi nó thử lại.",
                "See where the error repeats: often a wrong path, a malformed command, or a broken test. Tell the model before it tries again.",
            ),
            "blocked" => lang.pick(
                "Xem panel Hạn mức để biết khi nào mở lại, và dự báo để dừng trước khi chạm.",
                "See the Quota panel for when it reopens, and the forecast to stop before you hit it.",
            ),
            _ => "",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Effect {
    pub turns: u64,
    pub sessions: u64,
    /// Quota-weighted tokens of every turn in the range.
    pub weighted: u64,
    /// Turns the user cut off (Esc).
    pub interrupted_turns: u64,
    pub interrupted_weighted: u64,
    /// Turns that started with more context than `context_threshold`.
    pub big_context_turns: u64,
    pub big_context_weighted: u64,
    /// Context overflows (`/compact` and auto-compaction).
    pub compactions: u64,
    /// Most weighted first. Empty when there is nothing worth saying.
    pub advice: Vec<Advice>,
    /// Session economics; `None` when the range has no model calls.
    pub economics: Option<Economics>,
    /// Rate-limit hits; `None` for a tool without 5h/7d limits.
    pub blocked: Option<Blocked>,
    /// Repeated reads and retried tool errors; `None` when no tool result was recorded in the range.
    pub tools: Option<Tools>,
}

/// Seconds between the question and the last model call of the turn, when both are known.
fn run_secs(it: &PromptListItem) -> Option<i64> {
    let start = DateTime::parse_from_rfc3339(&it.stats.ts).ok()?;
    let end = DateTime::parse_from_rfc3339(it.stats.last_call_ts.as_deref()?).ok()?;
    Some((end - start).num_seconds())
}

/// `compactions` = (session id, overflows in the range) for the same tool and range as `items`.
pub fn summarize(items: &[PromptListItem], compactions: &[(String, u64)], context_threshold: u64) -> Effect {
    let mut weighted = 0u64;
    let mut interrupted = (0u64, 0u64);
    let mut late = (0u64, 0u64);
    let mut big = (0u64, 0u64);
    let mut by_session: HashMap<&str, u64> = HashMap::new();

    for it in items {
        let w = it.stats.usage.quota_weight();
        weighted += w;
        *by_session.entry(it.stats.session_id.as_str()).or_default() += w;
        if it.stats.interrupted {
            interrupted = (interrupted.0 + 1, interrupted.1 + w);
            if run_secs(it).is_some_and(|s| s >= LATE_CANCEL_SECS) {
                late = (late.0 + 1, late.1 + w);
            }
        }
        if it.stats.context_at_start > context_threshold {
            big = (big.0 + 1, big.1 + w);
        }
    }

    // Sessions that overflowed again and again; the worst one (most overflows, then most weight,
    // then id for a stable answer) is the one to open.
    let mut split: Vec<(&str, u64, u64)> = compactions
        .iter()
        .filter(|(_, n)| *n >= SPLIT_AFTER_COMPACTIONS)
        .map(|(sid, n)| (sid.as_str(), *n, by_session.get(sid.as_str()).copied().unwrap_or(0)))
        .collect();
    split.sort_by(|a, b| b.1.cmp(&a.1).then(b.2.cmp(&a.2)).then(a.0.cmp(b.0)));

    let mut advice = Vec::new();
    if late.0 > 0 {
        advice.push(Advice { code: "late_cancel", confidence: Confidence::Sure, count: late.0, weighted: late.1, session_id: None, target: None });
    }
    if big.0 > 0 {
        advice.push(Advice { code: "big_context", confidence: Confidence::Likely, count: big.0, weighted: big.1, session_id: None, target: None });
    }
    if let Some(worst) = split.first() {
        advice.push(Advice {
            code: "split_session",
            confidence: Confidence::Likely,
            count: split.len() as u64,
            weighted: split.iter().map(|s| s.2).sum(),
            session_id: Some(worst.0.to_string()),
            target: None,
        });
    }
    advice.sort_by(|a, b| b.weighted.cmp(&a.weighted).then(a.code.cmp(b.code)));

    Effect {
        turns: items.len() as u64,
        sessions: by_session.len() as u64,
        weighted,
        interrupted_turns: interrupted.0,
        interrupted_weighted: interrupted.1,
        big_context_turns: big.0,
        big_context_weighted: big.1,
        compactions: compactions.iter().map(|(_, n)| n).sum(),
        advice,
        economics: None,
        blocked: None,
        tools: None,
    }
}

// --- Session economics -----------------------------------------------------------------------------
//
// Why a session costs what it costs, from the calls alone: how much context it starts with, how many
// times the same context is sent again, how often the prompt cache went cold, and where the quota went
// (model, main chain or subagent, size of the context a turn started with). Thresholds were set on a
// real 30-day history (spec `2026-10-21-efficiency-2-design.md` §3b).

/// A session is flagged when it sent its largest context this many times over.
pub const RESEND_FLAG: f64 = 50.0;
/// Advise about the startup context only above this median (sessions here start at ~38K and barely vary).
pub const STARTUP_FLAG: u64 = 50_000;
/// A call is a cache rewrite when cache writes are at least this share of its context.
pub const COLD_SHARE: f64 = 0.5;
/// Prompt cache lifetimes, in seconds.
pub const COLD_GAP_5M: i64 = 300;
pub const COLD_GAP_1H: i64 = 3600;
/// Advise about cold caches from this many cold calls.
pub const COLD_MIN: u64 = 3;
/// Turns starting below this context fall in the smallest `by_start` bucket.
const SMALL_START: u64 = 100_000;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Startup {
    /// Sessions that started inside the range.
    pub sessions: u64,
    /// Median context of their first main-chain call.
    pub median: u64,
    /// Quota-weighted tokens of those first calls.
    pub weighted: u64,
    /// The session with the largest start, to open.
    pub max_session: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Resend {
    /// Sessions with at least 3 main-chain calls in the range.
    pub sessions: u64,
    /// Median of (sum of contexts ÷ largest context).
    pub median: f64,
    /// Sessions above `RESEND_FLAG`, and their quota-weighted tokens.
    pub flagged: u64,
    pub flagged_weighted: u64,
    /// The highest factor, to open.
    pub worst: Option<(String, f64)>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CacheStats {
    /// Cache reads ÷ all context tokens.
    pub read_share: f64,
    /// 1-hour cache writes ÷ all cache writes.
    pub write_1h_share: f64,
    /// Calls that rewrote the cache after it expired, and what that cost beyond reading it.
    pub cold_calls: u64,
    pub cold_extra: u64,
    pub worst_session: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BlockedType {
    pub limit_type: String,
    pub count: u64,
    /// Upper bound: the user may have switched model or done something else meanwhile.
    pub wait_secs: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Blocked {
    pub count: u64,
    /// One entry per limit type, waits never added across types (a 7-day wait dwarfs 5-hour ones).
    pub by_type: Vec<BlockedType>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Share {
    pub key: String,
    pub weighted: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Breakdown {
    /// Claude models by family (`opus`, `sonnet`, `haiku`), others as stored; top 3 then `other`.
    pub by_model: Vec<Share>,
    /// `main` and `subagent`.
    pub by_chain: Vec<Share>,
    /// Turns by the context they started with: `small`, `mid`, `large` (above `context_threshold`).
    pub by_start: Vec<Share>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Economics {
    /// Every model call in the range and their quota-weighted tokens. Larger than the turns' total:
    /// calls made for messages the user did not type (a subagent's hand-back, another session's
    /// message) belong to no turn.
    pub calls: u64,
    pub weighted: u64,
    pub startup: Startup,
    pub resend: Resend,
    /// `None` for a tool that reports no prompt cache.
    pub cache: Option<CacheStats>,
    pub breakdown: Breakdown,
}

/// Context sent with a call.
pub fn ctx(u: &Usage) -> u64 {
    u.input + u.cache_write_5m + u.cache_write_1h + u.cache_read
}

fn secs(ts: &str) -> Option<i64> {
    DateTime::parse_from_rfc3339(ts).ok().map(|t| t.timestamp())
}

fn median_u64(mut v: Vec<u64>) -> u64 {
    if v.is_empty() {
        return 0;
    }
    v.sort_unstable();
    let m = v.len() / 2;
    if v.len().is_multiple_of(2) { (v[m - 1] + v[m]) / 2 } else { v[m] }
}

fn median_f64(mut v: Vec<f64>) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(|a, b| a.total_cmp(b));
    let m = v.len() / 2;
    if v.len().is_multiple_of(2) { (v[m - 1] + v[m]) / 2.0 } else { v[m] }
}

/// `opus` / `sonnet` / `haiku` for Claude models; anything else as stored.
pub fn model_family(model: &str) -> String {
    let m = model.to_ascii_lowercase();
    ["opus", "sonnet", "haiku"].iter().find(|f| m.contains(**f)).map_or_else(|| model.to_string(), |f| f.to_string())
}

/// What rewriting the cache cost beyond reading it.
fn rewrite_extra(u: &Usage) -> u64 {
    let w = u.cache_write_5m * 125 + u.cache_write_1h * 200;
    w.saturating_sub((u.cache_write_5m + u.cache_write_1h) * 10) / 100
}

/// Indices of calls in `main` (one session's main chain, in time order) that rewrote an expired cache:
/// mostly cache writes, after a pause longer than the cache lasts, and not the first call after an
/// overflow (a compaction always writes a fresh context).
pub fn cold_calls(main: &[&RangeCall], compactions: &[String]) -> Vec<usize> {
    let mut out = Vec::new();
    let mut uses_1h = main.first().is_some_and(|c| c.usage.cache_write_1h > 0);
    for i in 1..main.len() {
        let (prev, c) = (main[i - 1], main[i]);
        let after_overflow = compactions.iter().any(|t| prev.ts.as_str() < t.as_str() && t.as_str() <= c.ts.as_str());
        let x = ctx(&c.usage);
        let writes = c.usage.cache_write_5m + c.usage.cache_write_1h;
        let gap = secs(&c.ts).zip(secs(&prev.ts)).map(|(a, b)| a - b);
        let limit = if uses_1h { COLD_GAP_1H } else { COLD_GAP_5M };
        if !after_overflow && x > 0 && writes as f64 >= COLD_SHARE * x as f64 && gap.is_some_and(|g| g > limit) {
            out.push(i);
        }
        uses_1h |= c.usage.cache_write_1h > 0;
    }
    out
}

fn shares(map: HashMap<String, u64>, top: usize) -> Vec<Share> {
    let mut v: Vec<Share> = map.into_iter().map(|(key, weighted)| Share { key, weighted }).collect();
    v.sort_by(|a, b| b.weighted.cmp(&a.weighted).then(a.key.cmp(&b.key)));
    if v.len() > top {
        let other: u64 = v.drain(top..).map(|s| s.weighted).sum();
        v.push(Share { key: "other".into(), weighted: other });
    }
    v
}

/// `calls` ordered by session then time (as `Store::session_calls_between` returns them); `first` = the
/// first main-chain call of each session; `compactions` = overflow times per session; `items` = the
/// turns of the same range; `from` = range start.
pub fn economics(
    calls: &[RangeCall],
    first: &HashMap<String, RangeCall>,
    compactions: &HashMap<String, Vec<String>>,
    items: &[PromptListItem],
    from: &str,
    context_threshold: u64,
) -> Economics {
    let mut by_model: HashMap<String, u64> = HashMap::new();
    let mut by_chain: HashMap<String, u64> = HashMap::new();
    let (mut reads, mut all_ctx, mut w5, mut w1h) = (0u64, 0u64, 0u64, 0u64);
    let mut starts: Vec<(&str, u64, u64)> = Vec::new();
    let mut factors: Vec<(&str, f64, u64)> = Vec::new();
    let (mut cold, mut cold_extra) = (0u64, 0u64);
    let mut cold_by_session: Vec<(&str, u64)> = Vec::new();

    for c in calls {
        let w = c.usage.quota_weight();
        *by_model.entry(model_family(&c.model)).or_default() += w;
        *by_chain.entry(if c.sidechain { "subagent" } else { "main" }.into()).or_default() += w;
        reads += c.usage.cache_read;
        all_ctx += ctx(&c.usage);
        w5 += c.usage.cache_write_5m;
        w1h += c.usage.cache_write_1h;
    }

    for session in calls.chunk_by(|a, b| a.session_id == b.session_id) {
        let sid = session[0].session_id.as_str();
        let weighted: u64 = session.iter().map(|c| c.usage.quota_weight()).sum();
        if let Some(f) = first.get(sid).filter(|f| f.ts.as_str() >= from) {
            starts.push((sid, ctx(&f.usage), f.usage.quota_weight()));
        }
        let main: Vec<&RangeCall> = session.iter().filter(|c| !c.sidechain).collect();
        if main.len() >= 3 {
            let max = main.iter().map(|c| ctx(&c.usage)).max().unwrap_or(0);
            if max > 0 {
                factors.push((sid, main.iter().map(|c| ctx(&c.usage)).sum::<u64>() as f64 / max as f64, weighted));
            }
        }
        let none = Vec::new();
        let idx = cold_calls(&main, compactions.get(sid).unwrap_or(&none));
        if !idx.is_empty() {
            cold += idx.len() as u64;
            cold_extra += idx.iter().map(|&i| rewrite_extra(&main[i].usage)).sum::<u64>();
            cold_by_session.push((sid, idx.len() as u64));
        }
    }

    let mut by_start: HashMap<String, u64> = HashMap::new();
    for it in items {
        let key = match it.stats.context_at_start {
            n if n < SMALL_START => "small",
            n if n <= context_threshold => "mid",
            _ => "large",
        };
        *by_start.entry(key.into()).or_default() += it.stats.usage.quota_weight();
    }
    let mut by_start: Vec<Share> = ["small", "mid", "large"]
        .iter()
        .filter_map(|k| by_start.get(*k).map(|w| Share { key: k.to_string(), weighted: *w }))
        .collect();
    by_start.retain(|s| s.weighted > 0);

    let flagged: Vec<&(&str, f64, u64)> = factors.iter().filter(|f| f.1 > RESEND_FLAG).collect();
    let worst = factors.iter().max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(a.0))).map(|f| (f.0.to_string(), f.1));
    cold_by_session.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));

    Economics {
        calls: calls.len() as u64,
        weighted: calls.iter().map(|c| c.usage.quota_weight()).sum(),
        startup: Startup {
            sessions: starts.len() as u64,
            median: median_u64(starts.iter().map(|s| s.1).collect()),
            weighted: starts.iter().map(|s| s.2).sum(),
            max_session: starts.iter().max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(a.0))).map(|s| s.0.to_string()),
        },
        resend: Resend {
            sessions: factors.len() as u64,
            median: median_f64(factors.iter().map(|f| f.1).collect()),
            flagged: flagged.len() as u64,
            flagged_weighted: flagged.iter().map(|f| f.2).sum(),
            worst,
        },
        cache: (reads + w5 + w1h > 0).then(|| CacheStats {
            read_share: if all_ctx > 0 { reads as f64 / all_ctx as f64 } else { 0.0 },
            write_1h_share: if w5 + w1h > 0 { w1h as f64 / (w5 + w1h) as f64 } else { 0.0 },
            cold_calls: cold,
            cold_extra,
            worst_session: cold_by_session.first().map(|s| s.0.to_string()),
        }),
        breakdown: Breakdown { by_model: shares(by_model, 3), by_chain: shares(by_chain, 2), by_start },
    }
}

/// Rate-limit hits `(limit_type, resets_at, ts)`, per type.
pub fn blocked(events: &[(String, i64, String)]) -> Blocked {
    let mut by: Vec<BlockedType> = Vec::new();
    for (kind, resets_at, ts) in events {
        let wait = secs(ts).map_or(0, |t| (resets_at - t).max(0));
        match by.iter_mut().find(|b| &b.limit_type == kind) {
            Some(b) => {
                b.count += 1;
                b.wait_secs += wait;
            }
            None => by.push(BlockedType { limit_type: kind.clone(), count: 1, wait_secs: wait }),
        }
    }
    by.sort_by(|a, b| a.limit_type.cmp(&b.limit_type));
    Blocked { count: events.len() as u64, by_type: by }
}

/// Add the advice that comes from `economics` and `blocked`, then order it: a rate-limit hit first (it is
/// what the user felt, and has no weight to rank by), the rest by quota-weighted tokens.
pub fn advise(e: &mut Effect) {
    if let Some(ec) = &e.economics {
        if ec.resend.flagged > 0 {
            e.advice.push(Advice { code: "resend", confidence: Confidence::Likely, count: ec.resend.flagged, weighted: ec.resend.flagged_weighted, session_id: ec.resend.worst.as_ref().map(|w| w.0.clone()), target: None });
        }
        if ec.startup.sessions > 0 && ec.startup.median >= STARTUP_FLAG {
            e.advice.push(Advice { code: "startup", confidence: Confidence::Sure, count: ec.startup.sessions, weighted: ec.startup.weighted, session_id: ec.startup.max_session.clone(), target: None });
        }
        if let Some(c) = ec.cache.as_ref().filter(|c| c.cold_calls >= COLD_MIN) {
            e.advice.push(Advice { code: "cold_cache", confidence: Confidence::Likely, count: c.cold_calls, weighted: c.cold_extra, session_id: c.worst_session.clone(), target: None });
        }
    }
    if let Some(t) = &e.tools {
        if let Some(top) = t.repeated.first() {
            e.advice.push(Advice { code: "repeated_read", confidence: Confidence::Likely, count: t.repeated.len() as u64, weighted: t.repeated_weighted, session_id: Some(top.session_id.clone()), target: Some(top.target.clone()) });
        }
        if t.retries >= RETRY_MIN {
            e.advice.push(Advice { code: "tool_retry", confidence: Confidence::Sure, count: t.retries, weighted: t.retry_weighted, session_id: t.retry_session.clone(), target: t.retry_tool.clone() });
        }
    }
    if let Some(b) = e.blocked.as_ref().filter(|b| b.count > 0) {
        e.advice.push(Advice { code: "blocked", confidence: Confidence::Sure, count: b.count, weighted: 0, session_id: None, target: None });
    }
    e.advice.sort_by(|a, b| (b.code == "blocked").cmp(&(a.code == "blocked")).then(b.weighted.cmp(&a.weighted)).then(a.code.cmp(b.code)));
}

// --- Tool signals (schema v8) -----------------------------------------------------------------------

/// A file read this many times in a session with no edit of it in between is a repeated read.
pub const REPEAT_READ: u64 = 3;
/// A failed tool call is retried when the same call comes again within this many tool calls.
pub const RETRY_WINDOW: usize = 3;
/// Advise about retries from this many.
pub const RETRY_MIN: u64 = 3;
/// Characters per token, as `composition.rs` estimates them.
const CHARS_PER_TOKEN_X100: u64 = 350;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RepeatedRead {
    pub session_id: String,
    pub target: String,
    pub times: u64,
    /// The reads after the first, put into the context again (×1.25, like a cache write).
    pub weighted: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ToolErrors {
    pub name: String,
    pub errors: u64,
    /// Calls of this tool whose result was seen.
    pub results: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Tools {
    /// Heaviest first.
    pub repeated: Vec<RepeatedRead>,
    pub repeated_weighted: u64,
    pub retries: u64,
    /// Quota-weighted tokens of the calls that made the failing tool calls (each call once).
    pub retry_weighted: u64,
    pub retry_session: Option<String>,
    pub retry_tool: Option<String>,
    pub errors: u64,
    pub calls_with_result: u64,
    /// Up to 3 tools with the most errors.
    pub by_tool_error: Vec<ToolErrors>,
}

const EDITS: [&str; 4] = ["Edit", "MultiEdit", "Write", "NotebookEdit"];

/// `rows` ordered by session, then time (as `Store::tool_calls_between` returns them); `call_weight` =
/// quota-weighted tokens per request id. `None` when no row has a known result: the tool's adapter does
/// not record results, or the history predates schema v8.
pub fn tool_signals(rows: &[ToolCallRow], call_weight: &HashMap<String, u64>) -> Option<Tools> {
    if !rows.iter().any(|r| r.error.is_some()) {
        return None;
    }
    let mut repeated: Vec<RepeatedRead> = Vec::new();
    let (mut retries, mut retry_weighted) = (0u64, 0u64);
    let mut retry_by_session: HashMap<&str, u64> = HashMap::new();
    let mut retry_by_tool: HashMap<&str, u64> = HashMap::new();
    let mut by_tool: HashMap<&str, (u64, u64)> = HashMap::new();
    let (mut errors, mut with_result) = (0u64, 0u64);

    for session in rows.chunk_by(|a, b| a.session_id == b.session_id) {
        let sid = session[0].session_id.as_str();
        // Reads of each file since its last edit: (count, characters of the reads after the first).
        let mut reads: HashMap<&str, (u64, u64)> = HashMap::new();
        let flush = |target: &str, (times, extra): (u64, u64), out: &mut Vec<RepeatedRead>| {
            if times >= REPEAT_READ {
                out.push(RepeatedRead { session_id: sid.to_string(), target: target.to_string(), times, weighted: extra * 125 / CHARS_PER_TOKEN_X100 });
            }
        };
        let mut failed_requests: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for (i, r) in session.iter().enumerate() {
            if let Some(e) = r.error {
                with_result += 1;
                let t = by_tool.entry(r.name.as_str()).or_default();
                t.1 += 1;
                if e {
                    errors += 1;
                    t.0 += 1;
                }
            }
            match (r.name.as_str(), r.target.as_deref()) {
                ("Read", Some(target)) => {
                    let e = reads.entry(target).or_default();
                    if e.0 > 0 {
                        e.1 += r.result_chars.unwrap_or(0);
                    }
                    e.0 += 1;
                }
                (name, Some(target)) if EDITS.contains(&name) => {
                    if let Some(group) = reads.remove(target) {
                        flush(target, group, &mut repeated);
                    }
                }
                _ => {}
            }
            if r.error == Some(true) {
                let again = session[i + 1..].iter().take(RETRY_WINDOW).any(|n| n.name == r.name && n.target_hash == r.target_hash);
                if again {
                    retries += 1;
                    *retry_by_session.entry(sid).or_default() += 1;
                    *retry_by_tool.entry(r.name.as_str()).or_default() += 1;
                    if failed_requests.insert(r.request_id.as_str()) {
                        retry_weighted += call_weight.get(&r.request_id).copied().unwrap_or(0);
                    }
                }
            }
        }
        let mut left: Vec<(&str, (u64, u64))> = reads.into_iter().collect();
        left.sort_by(|a, b| a.0.cmp(b.0));
        for (target, group) in left {
            flush(target, group, &mut repeated);
        }
    }

    repeated.sort_by(|a, b| b.weighted.cmp(&a.weighted).then(b.times.cmp(&a.times)).then(a.target.cmp(&b.target)));
    let top = |m: HashMap<&str, u64>| m.into_iter().max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(a.0))).map(|x| x.0.to_string());
    let mut by_tool_error: Vec<ToolErrors> = by_tool
        .into_iter()
        .filter(|(_, (e, _))| *e > 0)
        .map(|(name, (errors, results))| ToolErrors { name: name.to_string(), errors, results })
        .collect();
    by_tool_error.sort_by(|a, b| b.errors.cmp(&a.errors).then(a.name.cmp(&b.name)));
    by_tool_error.truncate(3);
    Some(Tools {
        repeated_weighted: repeated.iter().map(|r| r.weighted).sum(),
        repeated,
        retries,
        retry_weighted,
        retry_session: top(retry_by_session),
        retry_tool: top(retry_by_tool),
        errors,
        calls_with_result: with_result,
        by_tool_error,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Usage;
    use crate::store::queries::{PromptListItem, PromptStats};

    const THRESHOLD: u64 = 300_000;

    fn turn(session: &str, ts: &str, last: Option<&str>, usage: Usage, interrupted: bool, context_at_start: u64) -> PromptListItem {
        PromptListItem {
            stats: PromptStats {
                key: format!("{session}-{ts}"),
                tool: "claude_code".into(),
                credits: 0.0,
                models: vec!["claude".into()],
                session_id: session.into(),
                project: "p".into(),
                ts: ts.into(),
                text: "x".into(),
                interrupted,
                usage,
                api_calls: 1,
                context_at_start,
                last_call_ts: last.map(String::from),
            },
            starred: false,
        }
    }

    fn input(n: u64) -> Usage {
        Usage { input: n, ..Default::default() }
    }

    #[test]
    fn quota_weight_discounts_cache_reads_and_weighs_output() {
        let u = Usage { input: 100, cache_write_5m: 100, cache_write_1h: 100, cache_read: 1000, output: 10, reasoning: 10 };
        assert_eq!(u.quota_weight(), 100 + 125 + 200 + 100 + 100);
        // A turn that is 98% cache reads by raw count is not 98% of the quota.
        let reread = Usage { cache_read: 980_000, input: 20_000, ..Default::default() };
        assert_eq!(reread.quota_weight(), 98_000 + 20_000);
    }

    #[test]
    fn empty_history_is_all_zeros() {
        let e = summarize(&[], &[], THRESHOLD);
        assert_eq!((e.turns, e.sessions, e.weighted, e.compactions), (0, 0, 0, 0));
        assert!(e.advice.is_empty());
    }

    #[test]
    fn only_long_runs_are_late_cancels() {
        let items = vec![
            // Cut off after 5 minutes: late.
            turn("a", "2026-10-10T01:00:00.000Z", Some("2026-10-10T01:05:00.000Z"), input(1000), true, 0),
            // Cut off after 30 s: a quick redirect, counted as interrupted but not as late.
            turn("a", "2026-10-10T02:00:00.000Z", Some("2026-10-10T02:00:30.000Z"), input(100), true, 0),
            // No model call recorded: duration unknown, not late.
            turn("b", "2026-10-10T03:00:00.000Z", None, input(10), true, 0),
            turn("b", "2026-10-10T04:00:00.000Z", Some("2026-10-10T04:09:00.000Z"), input(5), false, 0),
        ];
        let e = summarize(&items, &[], THRESHOLD);
        assert_eq!((e.turns, e.sessions, e.weighted), (4, 2, 1115));
        assert_eq!((e.interrupted_turns, e.interrupted_weighted), (3, 1110));
        assert_eq!(e.advice, vec![Advice { code: "late_cancel", confidence: Confidence::Sure, count: 1, weighted: 1000, session_id: None, target: None }]);
    }

    #[test]
    fn big_context_turns_use_the_configured_threshold() {
        let items = vec![
            turn("a", "2026-10-10T01:00:00.000Z", None, input(700), false, THRESHOLD + 1),
            turn("a", "2026-10-10T02:00:00.000Z", None, input(50), false, THRESHOLD),
        ];
        let e = summarize(&items, &[], THRESHOLD);
        assert_eq!((e.big_context_turns, e.big_context_weighted), (1, 700));
        assert_eq!(e.advice[0].code, "big_context");
        assert_eq!(e.advice[0].confidence, Confidence::Likely);
    }

    #[test]
    fn repeated_overflows_point_at_the_worst_session() {
        let items = vec![
            turn("a", "2026-10-10T01:00:00.000Z", None, input(400), false, 0),
            turn("b", "2026-10-10T02:00:00.000Z", None, input(300), false, 0),
            turn("c", "2026-10-10T03:00:00.000Z", None, input(9000), false, 0),
        ];
        // c overflowed once: below the bar. b overflowed most, so it is the one to open.
        let compactions = vec![("a".to_string(), 2), ("b".to_string(), 3), ("c".to_string(), 1)];
        let e = summarize(&items, &compactions, THRESHOLD);
        assert_eq!(e.compactions, 6);
        assert_eq!(e.advice, vec![Advice { code: "split_session", confidence: Confidence::Likely, count: 2, weighted: 700, session_id: Some("b".into()), target: None }]);
    }

    #[test]
    fn advice_is_ordered_by_weight() {
        let items = vec![
            turn("a", "2026-10-10T01:00:00.000Z", Some("2026-10-10T01:10:00.000Z"), input(100), true, 0),
            turn("a", "2026-10-10T02:00:00.000Z", None, input(5000), false, THRESHOLD + 1),
        ];
        let e = summarize(&items, &[], THRESHOLD);
        let codes: Vec<&str> = e.advice.iter().map(|a| a.code).collect();
        assert_eq!(codes, vec!["big_context", "late_cancel"]);
    }

    // --- economics ---

    fn rc(session: &str, rid: &str, ts: &str, model: &str, sidechain: bool, usage: Usage) -> RangeCall {
        RangeCall { session_id: session.into(), request_id: rid.into(), ts: ts.into(), model: model.into(), sidechain, usage }
    }

    /// A call whose context is mostly cache reads (warm cache).
    fn warm(read: u64) -> Usage {
        Usage { input: 10, cache_read: read, cache_write_5m: 100, ..Default::default() }
    }

    fn firsts(calls: &[RangeCall]) -> HashMap<String, RangeCall> {
        let mut m = HashMap::new();
        for c in calls.iter().filter(|c| !c.sidechain) {
            m.entry(c.session_id.clone()).or_insert_with(|| c.clone());
        }
        m
    }

    const FROM: &str = "2026-10-01T00:00:00.000Z";

    #[test]
    fn a_single_call_session_has_a_startup_but_no_resend_factor() {
        let calls = vec![rc("a", "r1", "2026-10-10T01:00:00.000Z", "claude-opus-5", false, warm(40_000))];
        let e = economics(&calls, &firsts(&calls), &HashMap::new(), &[], FROM, THRESHOLD);
        assert_eq!((e.startup.sessions, e.startup.median), (1, 40_110));
        assert_eq!(e.startup.max_session.as_deref(), Some("a"));
        assert_eq!(e.resend.sessions, 0);
    }

    #[test]
    fn a_session_that_started_before_the_range_has_no_startup() {
        let calls = vec![rc("a", "r2", "2026-10-10T01:00:00.000Z", "claude-opus-5", false, warm(40_000))];
        let mut first = HashMap::new();
        first.insert("a".to_string(), rc("a", "r1", "2026-09-20T01:00:00.000Z", "claude-opus-5", false, warm(1)));
        let e = economics(&calls, &first, &HashMap::new(), &[], FROM, THRESHOLD);
        assert_eq!(e.startup.sessions, 0);
    }

    #[test]
    fn resend_divides_by_the_largest_context_not_the_last() {
        // Grows to 400, overflows, ends small: the factor is (100+200+400+50) / 400.
        let calls: Vec<RangeCall> = [100, 200, 400, 50].iter().enumerate()
            .map(|(i, n)| rc("a", &format!("r{i}"), &format!("2026-10-10T01:0{i}:00.000Z"), "claude-opus-5", false, Usage { cache_read: *n, ..Default::default() }))
            .collect();
        let e = economics(&calls, &firsts(&calls), &HashMap::new(), &[], FROM, THRESHOLD);
        assert_eq!(e.resend.sessions, 1);
        assert!((e.resend.median - 750.0 / 400.0).abs() < 1e-9);
        assert_eq!(e.resend.flagged, 0);
    }

    #[test]
    fn long_sessions_are_flagged_for_resending() {
        let calls: Vec<RangeCall> = (0..60)
            .map(|i| rc("a", &format!("r{i}"), &format!("2026-10-10T01:{:02}:00.000Z", i), "claude-opus-5", false, Usage { cache_read: 1000, ..Default::default() }))
            .collect();
        let e = economics(&calls, &firsts(&calls), &HashMap::new(), &[], FROM, THRESHOLD);
        assert_eq!(e.resend.flagged, 1);
        assert_eq!(e.resend.flagged_weighted, 60 * 100);
        assert_eq!(e.resend.worst.as_ref().map(|w| w.0.as_str()), Some("a"));
    }

    #[test]
    fn a_cache_rewrite_after_a_long_pause_is_cold() {
        let rewrite = Usage { input: 10, cache_write_5m: 8_000, cache_read: 2_000, ..Default::default() };
        let calls = vec![
            rc("a", "r1", "2026-10-10T01:00:00.000Z", "claude-opus-5", false, warm(10_000)),
            // 6 minutes later, mostly writes: the 5-minute cache had expired.
            rc("a", "r2", "2026-10-10T01:06:00.000Z", "claude-opus-5", false, rewrite.clone()),
            // 2 minutes later, mostly writes: a big new tool result, not an expired cache.
            rc("a", "r3", "2026-10-10T01:08:00.000Z", "claude-opus-5", false, rewrite),
        ];
        let main: Vec<&RangeCall> = calls.iter().collect();
        assert_eq!(cold_calls(&main, &[]), vec![1]);
        let e = economics(&calls, &firsts(&calls), &HashMap::new(), &[], FROM, THRESHOLD);
        let c = e.cache.unwrap();
        assert_eq!(c.cold_calls, 1);
        assert_eq!(c.cold_extra, 8_000 * 115 / 100);
        assert_eq!(c.worst_session.as_deref(), Some("a"));
    }

    #[test]
    fn a_one_hour_cache_survives_a_ten_minute_pause() {
        let calls = [
            rc("a", "r1", "2026-10-10T01:00:00.000Z", "claude-opus-5", false, Usage { cache_write_1h: 10_000, ..Default::default() }),
            rc("a", "r2", "2026-10-10T01:10:00.000Z", "claude-opus-5", false, Usage { cache_write_1h: 8_000, cache_read: 2_000, ..Default::default() }),
        ];
        let main: Vec<&RangeCall> = calls.iter().collect();
        assert!(cold_calls(&main, &[]).is_empty());
    }

    #[test]
    fn the_first_call_after_an_overflow_is_not_cold() {
        let calls = [
            rc("a", "r1", "2026-10-10T01:00:00.000Z", "claude-opus-5", false, warm(10_000)),
            rc("a", "r2", "2026-10-10T01:20:00.000Z", "claude-opus-5", false, Usage { cache_write_5m: 9_000, ..Default::default() }),
        ];
        let main: Vec<&RangeCall> = calls.iter().collect();
        assert!(cold_calls(&main, &["2026-10-10T01:19:00.000Z".to_string()]).is_empty());
        assert_eq!(cold_calls(&main, &[]), vec![1]);
    }

    #[test]
    fn subagents_count_in_the_breakdown_only() {
        let calls = vec![
            rc("a", "r1", "2026-10-10T01:00:00.000Z", "claude-opus-5", false, input(1000)),
            rc("a", "r2", "2026-10-10T01:01:00.000Z", "claude-sonnet-5", true, input(300)),
            rc("a", "r3", "2026-10-10T01:02:00.000Z", "kiro:auto", false, input(200)),
        ];
        let e = economics(&calls, &firsts(&calls), &HashMap::new(), &[], FROM, THRESHOLD);
        assert_eq!(e.startup.median, 1000);
        let chain: Vec<(&str, u64)> = e.breakdown.by_chain.iter().map(|s| (s.key.as_str(), s.weighted)).collect();
        assert_eq!(chain, vec![("main", 1200), ("subagent", 300)]);
        let models: Vec<&str> = e.breakdown.by_model.iter().map(|s| s.key.as_str()).collect();
        assert_eq!(models, vec!["opus", "sonnet", "kiro:auto"]);
        assert_eq!(e.breakdown.by_model.iter().map(|s| s.weighted).sum::<u64>(), 1500);
        assert_eq!((e.calls, e.weighted), (3, 1500));
    }

    #[test]
    fn by_start_buckets_turns_by_their_starting_context() {
        let items = vec![
            turn("a", "2026-10-10T01:00:00.000Z", None, input(10), false, 50_000),
            turn("a", "2026-10-10T02:00:00.000Z", None, input(20), false, 200_000),
            turn("a", "2026-10-10T03:00:00.000Z", None, input(30), false, THRESHOLD + 1),
        ];
        let e = economics(&[], &HashMap::new(), &HashMap::new(), &items, FROM, THRESHOLD);
        let b: Vec<(&str, u64)> = e.breakdown.by_start.iter().map(|s| (s.key.as_str(), s.weighted)).collect();
        assert_eq!(b, vec![("small", 10), ("mid", 20), ("large", 30)]);
    }

    #[test]
    fn a_tool_without_prompt_cache_has_no_cache_stats() {
        let calls = vec![rc("a", "r1", "2026-10-10T01:00:00.000Z", "kiro:auto", false, input(500))];
        assert!(economics(&calls, &firsts(&calls), &HashMap::new(), &[], FROM, THRESHOLD).cache.is_none());
    }

    #[test]
    fn rate_limit_waits_stay_per_type() {
        let ev = vec![
            ("five_hour".to_string(), 1_000 + 7_200, "1970-01-01T00:16:40.000Z".to_string()),
            ("seven_day".to_string(), 1_000 + 86_400, "1970-01-01T00:16:40.000Z".to_string()),
            // Reset already passed when the line was written: no wait.
            ("five_hour".to_string(), 0, "1970-01-01T00:16:40.000Z".to_string()),
        ];
        let b = blocked(&ev);
        assert_eq!(b.count, 3);
        assert_eq!(b.by_type, vec![
            BlockedType { limit_type: "five_hour".into(), count: 2, wait_secs: 7_200 },
            BlockedType { limit_type: "seven_day".into(), count: 1, wait_secs: 86_400 },
        ]);
    }

    #[test]
    fn a_rate_limit_hit_comes_first_then_the_heaviest() {
        let calls: Vec<RangeCall> = (0..60)
            .map(|i| rc("a", &format!("r{i}"), &format!("2026-10-10T01:{:02}:00.000Z", i), "claude-opus-5", false, Usage { cache_read: 1000, ..Default::default() }))
            .collect();
        let items = vec![turn("a", "2026-10-10T01:00:00.000Z", None, input(50_000), false, THRESHOLD + 1)];
        let mut e = summarize(&items, &[], THRESHOLD);
        e.economics = Some(economics(&calls, &firsts(&calls), &HashMap::new(), &items, FROM, THRESHOLD));
        e.blocked = Some(blocked(&[("five_hour".to_string(), 0, "2026-10-10T01:00:00.000Z".to_string())]));
        advise(&mut e);
        let codes: Vec<&str> = e.advice.iter().map(|a| a.code).collect();
        assert_eq!(codes, vec!["blocked", "big_context", "resend"]);
        assert_eq!(e.advice[2].session_id.as_deref(), Some("a"));
    }

    // --- tool signals ---

    fn tr(session: &str, rid: &str, name: &str, target: Option<&str>, hash: Option<i64>, error: Option<bool>, chars: u64) -> ToolCallRow {
        ToolCallRow { session_id: session.into(), request_id: rid.into(), ts: "2026-10-10T01:00:00.000Z".into(), name: name.into(), target: target.map(String::from), target_hash: hash, error, result_chars: Some(chars) }
    }

    fn read(session: &str, file: &str) -> ToolCallRow {
        tr(session, "r", "Read", Some(file), Some(1), Some(false), 3500)
    }

    #[test]
    fn three_reads_of_a_file_without_an_edit_are_repeated() {
        let rows = vec![read("a", "/x.rs"), read("a", "/x.rs"), read("a", "/x.rs")];
        let t = tool_signals(&rows, &HashMap::new()).unwrap();
        assert_eq!(t.repeated.len(), 1);
        // Two reads after the first: 7000 characters ≈ 2000 tokens, ×1.25.
        assert_eq!((t.repeated[0].times, t.repeated[0].weighted), (3, 2500));
    }

    #[test]
    fn an_edit_in_between_resets_the_count() {
        let edit = tr("a", "r", "Edit", Some("/x.rs"), Some(2), Some(false), 10);
        let rows = vec![read("a", "/x.rs"), edit, read("a", "/x.rs"), read("a", "/x.rs")];
        assert!(tool_signals(&rows, &HashMap::new()).unwrap().repeated.is_empty());
    }

    #[test]
    fn reads_in_different_sessions_do_not_add_up() {
        let rows = vec![read("a", "/x.rs"), read("b", "/x.rs"), read("c", "/x.rs")];
        assert!(tool_signals(&rows, &HashMap::new()).unwrap().repeated.is_empty());
    }

    #[test]
    fn a_failed_call_repeated_soon_is_a_retry() {
        let fail = |rid: &str| tr("a", rid, "Bash", Some("cargo test"), Some(7), Some(true), 10);
        let other = |rid: &str| tr("a", rid, "Read", Some("/y.rs"), Some(9), Some(false), 10);
        let again = |rid: &str| tr("a", rid, "Bash", Some("cargo test"), Some(7), Some(false), 10);
        let weights = HashMap::from([("r1".to_string(), 500), ("r9".to_string(), 50)]);
        // Retried as the second tool call after the failure: counted.
        let t = tool_signals(&[fail("r1"), other("r2"), again("r3")], &weights).unwrap();
        assert_eq!((t.retries, t.retry_weighted, t.errors, t.calls_with_result), (1, 500, 1, 3));
        assert_eq!(t.retry_tool.as_deref(), Some("Bash"));
        assert_eq!(t.by_tool_error, vec![ToolErrors { name: "Bash".into(), errors: 1, results: 2 }]);
        // Four tool calls later: not a retry.
        let late = tool_signals(&[fail("r9"), other("r2"), other("r3"), other("r4"), again("r5")], &weights).unwrap();
        assert_eq!(late.retries, 0);
        // Same tool, different input: not a retry.
        let diff = tr("a", "r3", "Bash", Some("cargo test"), Some(8), Some(false), 10);
        assert_eq!(tool_signals(&[fail("r1"), diff], &weights).unwrap().retries, 0);
    }

    #[test]
    fn no_known_result_means_no_tool_signals() {
        let rows = vec![tr("a", "r", "read", None, None, None, 0)];
        assert!(tool_signals(&rows, &HashMap::new()).is_none());
    }

    #[test]
    fn tool_advice_names_the_file_and_the_tool() {
        let fail = |rid: &str| tr("a", rid, "Bash", Some("npm test"), Some(7), Some(true), 10);
        let mut rows = vec![read("a", "/x.rs"), read("a", "/x.rs"), read("a", "/x.rs")];
        for i in 0..3 {
            rows.push(fail(&format!("f{i}")));
        }
        rows.push(tr("a", "ok", "Bash", Some("npm test"), Some(7), Some(false), 10));
        let mut e = summarize(&[], &[], THRESHOLD);
        e.tools = tool_signals(&rows, &HashMap::new());
        advise(&mut e);
        let a: Vec<(&str, Option<&str>)> = e.advice.iter().map(|a| (a.code, a.target.as_deref())).collect();
        assert_eq!(a, vec![("repeated_read", Some("/x.rs")), ("tool_retry", Some("Bash"))]);
        assert!(e.advice[0].text(crate::i18n::Lang::En).contains("/x.rs"));
    }

    #[test]
    fn startup_advice_only_for_a_heavy_setup() {
        let light = vec![rc("a", "r1", "2026-10-10T01:00:00.000Z", "claude-opus-5", false, warm(38_000))];
        let heavy = vec![rc("b", "r1", "2026-10-10T01:00:00.000Z", "claude-opus-5", false, warm(60_000))];
        for (calls, want) in [(light, false), (heavy, true)] {
            let mut e = summarize(&[], &[], THRESHOLD);
            e.economics = Some(economics(&calls, &firsts(&calls), &HashMap::new(), &[], FROM, THRESHOLD));
            advise(&mut e);
            assert_eq!(e.advice.iter().any(|a| a.code == "startup"), want);
        }
    }
}
