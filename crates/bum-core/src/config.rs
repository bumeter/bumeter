//! User configuration (`config.toml` in the data dir); missing values fall back to defaults.
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Notify {
    pub quota: bool,
    pub blocked: bool,
    pub anomaly: bool,
    pub context: bool,
    /// "Limit likely before reset" notifications.
    pub forecast: bool,
    /// 5-hour window: notify when the forecast limit is at most this many minutes away.
    pub forecast_lead_minutes: u32,
}

impl Default for Notify {
    fn default() -> Self {
        Self { quota: true, blocked: true, anomaly: true, context: true, forecast: true, forecast_lead_minutes: 30 }
    }
}

/// Light/dark theme; `System` follows the operating system.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

/// User-entered quota for a tool (e.g. Kiro does not store credit quota locally).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ToolLimit {
    /// Credits per period (month); `None` = not tracked.
    pub monthly_credits: Option<f64>,
    /// Reset day of the month, 1–28.
    pub reset_day: u32,
}

impl Default for ToolLimit {
    fn default() -> Self {
        Self { monthly_credits: None, reset_day: 1 }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// A turn is considered anomalous when it costs more than N times the 30-day median.
    pub anomaly_multiplier: f64,
    /// Context threshold (tokens) for the "large context" alert and the 🧠 filter.
    pub context_threshold: u64,
    /// Quota % thresholds that trigger notifications.
    pub quota_thresholds: Vec<u8>,
    pub notify: Notify,
    pub claude_projects_dir: Option<PathBuf>,
    /// Kiro CLI sessions directory (default `~/.kiro/sessions/cli`).
    pub kiro_sessions_dir: Option<PathBuf>,
    /// UI and notification language.
    pub language: crate::i18n::Lang,
    /// Light/dark theme (default follows the system).
    pub theme: Theme,
    /// Per-tool quota (source id → quota), e.g. `[tool_limits.kiro]`.
    pub tool_limits: std::collections::BTreeMap<String, ToolLimit>,
}

impl Default for Config {
    fn default() -> Self {
        Self { anomaly_multiplier: 3.0, context_threshold: 300_000, quota_thresholds: vec![70, 90], notify: Notify::default(), claude_projects_dir: None, kiro_sessions_dir: None, language: crate::i18n::Lang::Vi, theme: Theme::System, tool_limits: Default::default() }
    }
}

impl Config {
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path).ok().and_then(|s| toml::from_str(&s).ok()).unwrap_or_default()
    }
}

impl Config {
    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, toml::to_string_pretty(self)?)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }
}
