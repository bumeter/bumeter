//! `bum statusline`: never fails, because an error here would break Claude Code's statusline.
use std::io::Read;

use bum_core::limits;
use bum_core::paths::Paths;

pub fn run(quiet: bool, tee: bool, around: Option<String>) {
    let mut raw = String::new();
    let _ = std::io::stdin().read_to_string(&mut raw);
    let now = chrono::Utc::now().timestamp();
    let paths = Paths::resolve();
    let input: Option<serde_json::Value> = serde_json::from_str(&raw).ok();

    if let Some(file) = input.as_ref().and_then(|v| limits::from_statusline(v, now)) {
        if paths.ensure().is_ok() {
            let _ = limits::write_atomic(&paths.limits(), &file);
        }
    }
    if tee {
        use std::io::Write;
        let _ = std::io::stdout().write_all(raw.as_bytes());
        return;
    }
    if quiet {
        return;
    }
    let mut st = limits::state(limits::read(&paths.limits()), None, now);
    if let Some(f) = host_forecasts(&paths) {
        (st.forecast_five_hour, st.forecast_seven_day) = f;
    }
    let cfg = bum_core::config::Config::load(&paths.config());
    let ctx = input
        .as_ref()
        .and_then(|v| v["transcript_path"].as_str())
        .and_then(|p| bum_adapters::claude_code::transcript::current_context(std::path::Path::new(p)));
    match around {
        // Next to someone else's status line, a "5h —" with no numbers is noise: leave it out.
        Some(cmd) => println!("{}", compose(original_line(&cmd, &raw).as_deref(), &segment(&st, ctx, cfg.context_threshold, cfg.language, false))),
        None => println!("{}", segment(&st, ctx, cfg.context_threshold, cfg.language, true)),
    }
}

/// Bumeter's part: the limits (as `bum status` and the tray show them), then the context the session
/// carries now, with a nudge to `/compact` past the threshold the app uses for "large context".
/// `keep_empty`: still show "5h —" when there are no numbers yet.
fn segment(st: &limits::LimitState, ctx: Option<u64>, threshold: u64, lang: bum_core::i18n::Lang, keep_empty: bool) -> String {
    let mut parts = Vec::new();
    if keep_empty || st.five_hour.is_some() || st.seven_day.is_some() || st.blocked.is_some() {
        parts.push(limits::render_line(st, None, lang));
    }
    if let Some(c) = ctx {
        let warn = if c > threshold { " ⚠ /compact" } else { "" };
        parts.push(format!("ctx {}{warn}", bum_core::insight::fmt_tokens(c)));
    }
    parts.join(" · ")
}

/// The user's own status line run with the same input, for `--around`. None when it fails or prints
/// nothing: Bumeter's part is then shown alone rather than an error.
fn original_line(cmd: &str, input: &str) -> Option<String> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let mut child = Command::new("sh").arg("-c").arg(cmd).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().ok()?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(input.as_bytes());
    }
    let out = child.wait_with_output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout).trim_end().to_string();
    (!text.is_empty()).then_some(text)
}

/// The user's line first, untouched, then Bumeter's part on the same (last) line.
fn compose(theirs: Option<&str>, ours: &str) -> String {
    match (theirs, ours.is_empty()) {
        (Some(t), false) => format!("{t} · {ours}"),
        (Some(t), true) => t.to_string(),
        (None, _) => ours.to_string(),
    }
}

type Forecasts = (Option<bum_core::forecast::Forecast>, Option<bum_core::forecast::Forecast>);

/// (5-hour, 7-day) forecasts from the running host (they need the reading history in the DB).
/// None when the host can't be reached; never fails.
pub fn host_forecasts(paths: &Paths) -> Option<Forecasts> {
    let v = bum_service::client::call(paths, "limit_state", serde_json::Value::Null).ok()?;
    let f = |k: &str| serde_json::from_value(v[k].clone()).ok();
    Some((f("forecast_five_hour"), f("forecast_seven_day")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bum_core::i18n::Lang;

    fn st(five: Option<f64>) -> limits::LimitState {
        let mut st = limits::state(None, None, 0);
        st.five_hour = five.map(|p| limits::Window { used_percentage: p, resets_at: 4_102_444_800 });
        st
    }

    #[test]
    fn context_is_shown_and_flagged_past_the_threshold() {
        assert!(segment(&st(Some(62.0)), Some(120_000), 300_000, Lang::En, true).ends_with(" · ctx 120K"));
        assert!(segment(&st(Some(62.0)), Some(312_000), 300_000, Lang::En, true).ends_with(" · ctx 312K ⚠ /compact"));
        // Unknown context (e.g. right after /compact): nothing about it.
        assert!(!segment(&st(Some(62.0)), None, 300_000, Lang::En, true).contains("ctx"));
    }

    #[test]
    fn next_to_another_status_line_an_empty_quota_is_left_out() {
        assert_eq!(segment(&st(None), Some(50_000), 300_000, Lang::En, false), "ctx 50K");
        assert!(segment(&st(None), Some(50_000), 300_000, Lang::En, true).contains("5h —"));
        assert_eq!(segment(&st(None), None, 300_000, Lang::En, false), "");
    }

    #[test]
    fn the_users_line_comes_first_and_survives_alone() {
        assert_eq!(compose(Some("\x1b[32mme@box:/p\x1b[0m"), "ctx 50K"), "\x1b[32mme@box:/p\x1b[0m · ctx 50K");
        assert_eq!(compose(Some("me@box"), ""), "me@box");
        assert_eq!(compose(None, "ctx 50K"), "ctx 50K");
    }

    #[test]
    fn the_original_command_gets_the_same_input() {
        assert_eq!(original_line("cat", "{\"a\":1}\n").as_deref(), Some("{\"a\":1}"));
        assert_eq!(original_line("exit 3", "x"), None);
        assert_eq!(original_line("true", "x"), None);
    }
}
