//! Cấu hình người dùng (`config.toml` trong data dir), thiếu thì dùng mặc định.
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Notify {
    pub quota: bool,
    pub blocked: bool,
    pub anomaly: bool,
    pub context: bool,
}

impl Default for Notify {
    fn default() -> Self {
        Self { quota: true, blocked: true, anomaly: true, context: true }
    }
}

/// Giao diện sáng/tối; `System` theo hệ điều hành.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

/// Hạn mức do người dùng tự nhập cho một công cụ (vd Kiro không lưu hạn mức credit trên máy).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ToolLimit {
    /// Credit mỗi kỳ (tháng); `None` = không theo dõi.
    pub monthly_credits: Option<f64>,
    /// Ngày reset trong tháng, 1–28.
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
    /// Lần hỏi bị coi là bất thường khi tốn hơn N lần trung vị 30 ngày.
    pub anomaly_multiplier: f64,
    /// Ngưỡng context (token) cho cảnh báo "context lớn" và lọc 🧠.
    pub context_threshold: u64,
    /// Các mốc % quota sẽ thông báo.
    pub quota_thresholds: Vec<u8>,
    pub notify: Notify,
    pub claude_projects_dir: Option<PathBuf>,
    /// Thư mục phiên Kiro CLI (mặc định `~/.kiro/sessions/cli`).
    pub kiro_sessions_dir: Option<PathBuf>,
    /// Ngôn ngữ giao diện và thông báo.
    pub language: crate::i18n::Lang,
    /// Giao diện sáng/tối (mặc định theo hệ thống).
    pub theme: Theme,
    /// Hạn mức theo công cụ (id nguồn → hạn mức), vd `[tool_limits.kiro]`.
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
