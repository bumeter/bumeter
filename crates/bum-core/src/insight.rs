//! Evaluate a turn: whether it is anomalous, and why it was costly (spec section 4.4).
use serde::Serialize;

use crate::config::Config;
use crate::model::Usage;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Anomaly {
    pub ratio: Option<f64>,
    pub big_context: bool,
}

pub fn median(values: &mut [u64]) -> Option<u64> {
    if values.is_empty() {
        return None;
    }
    values.sort_unstable();
    Some(values[values.len() / 2])
}

pub fn anomaly(total: u64, context_at_start: u64, median: Option<u64>, cfg: &Config) -> Option<Anomaly> {
    let ratio = median.filter(|m| *m > 0).map(|m| total as f64 / m as f64);
    let over = ratio.is_some_and(|r| r > cfg.anomaly_multiplier);
    let big_context = context_at_start > cfg.context_threshold;
    (over || big_context).then_some(Anomaly { ratio, big_context })
}

/// "Why costly" as a code plus parameters; the UI translates it, notifications use `text()`.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum Explain {
    None,
    /// Most tokens are re-reads of old context.
    Reread { pct: f64, context: u64 },
    /// Most tokens are output.
    Output,
    Context { context: u64 },
}

pub fn explain(u: &Usage, context_at_start: u64) -> Explain {
    let total = u.total();
    if total == 0 {
        return Explain::None;
    }
    let reread = u.cache_read as f64 / total as f64 * 100.0;
    if reread >= 80.0 && context_at_start >= 100_000 {
        Explain::Reread { pct: (reread * 10.0).round() / 10.0, context: context_at_start }
    } else if u.output as f64 / total as f64 >= 0.2 {
        Explain::Output
    } else {
        Explain::Context { context: context_at_start }
    }
}

impl Explain {
    pub fn text(&self, lang: crate::i18n::Lang) -> String {
        use crate::i18n::Lang::*;
        match (self, lang) {
            (Explain::None, _) => String::new(),
            (Explain::Reread { pct, context }, Vi) => format!("{pct:.1}% token là đọc lại context cũ ({}). Nên /compact hoặc mở phiên mới.", fmt_tokens(*context)),
            (Explain::Reread { pct, context }, En) => format!("{pct:.1}% of tokens re-read old context ({}). Consider /compact or a new session.", fmt_tokens(*context)),
            (Explain::Output, _) => lang.pick("Phần lớn token là output (câu trả lời dài).", "Most tokens are output (long answers).").into(),
            (Explain::Context { context }, Vi) => format!("Context lúc hỏi {}.", fmt_tokens(*context)),
            (Explain::Context { context }, En) => format!("Context at start {}.", fmt_tokens(*context)),
        }
    }
}

pub fn fmt_tokens(n: u64) -> String {
    match n {
        0..=999 => n.to_string(),
        1_000..=999_999 => format!("{}K", n / 1_000),
        _ => format!("{:.1}M", n as f64 / 1_000_000.0),
    }
}
