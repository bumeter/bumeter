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
}

impl Default for Config {
    fn default() -> Self {
        Self { anomaly_multiplier: 3.0, context_threshold: 300_000, quota_thresholds: vec![70, 90], notify: Notify::default(), claude_projects_dir: None, kiro_sessions_dir: None, language: crate::i18n::Lang::Vi }
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
