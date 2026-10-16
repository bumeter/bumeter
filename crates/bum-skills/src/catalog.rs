//! Danh mục skill: cá nhân, đã tắt, synced (spec mục 8.1). Plugin nằm ở `plugins.rs`.
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::frontmatter;

/// Ký tự trên mỗi token khi ước lượng.
const CHARS_PER_TOKEN: f64 = 3.5;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Source {
    Personal,
    Plugin { plugin: String },
    /// Đồng bộ từ claude.ai; chỉ đọc.
    Synced,
    /// Slash command chỉ thấy trong log.
    Command,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillState {
    Enabled,
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SkillEntry {
    /// Tên dùng trong log (skill plugin: `plugin:skill`).
    pub name: String,
    /// Công cụ sở hữu skill: `claude_code` hoặc `kiro`.
    pub tool: String,
    pub description: String,
    pub source: Source,
    pub state: SkillState,
    pub path: Option<PathBuf>,
    pub symlink: bool,
    /// Thời điểm cài (mtime của thư mục/link), để không gợi ý tắt skill vừa cài.
    pub installed_at: Option<String>,
    pub always_on_tokens: Option<u64>,
    pub on_invoke_tokens: Option<u64>,
    /// Chi phí là ước lượng (không lấy từ `claude plugin details`).
    pub estimated: bool,
    /// App được phép tắt/bật/gỡ.
    pub managed: bool,
}

pub fn estimate_tokens(text: &str) -> u64 {
    (text.chars().count() as f64 / CHARS_PER_TOKEN).ceil() as u64
}

fn read_entry(dir: &Path, source: Source, state: SkillState, tool: &str) -> Option<SkillEntry> {
    let body = std::fs::read_to_string(dir.join("SKILL.md")).ok()?;
    let fm = frontmatter::parse(&body);
    let dir_name = dir.file_name()?.to_string_lossy().to_string();
    let description = fm.description.unwrap_or_default();
    let managed = source == Source::Personal;
    Some(SkillEntry {
        name: fm.name.filter(|n| !n.is_empty()).unwrap_or(dir_name),
        tool: tool.to_string(),
        always_on_tokens: Some(estimate_tokens(&description)),
        on_invoke_tokens: Some(estimate_tokens(&body)),
        description,
        source,
        state,
        symlink: std::fs::symlink_metadata(dir).map(|m| m.file_type().is_symlink()).unwrap_or(false),
        installed_at: std::fs::symlink_metadata(dir).and_then(|m| m.modified()).ok().map(|t| chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339()),
        path: Some(dir.to_path_buf()),
        estimated: true,
        managed,
    })
}

fn subdirs(dir: &Path) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut out: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    out.sort();
    out
}

/// `claude_dir` = `~/.claude`.
pub fn scan_personal(claude_dir: &Path) -> Vec<SkillEntry> {
    scan_personal_for(claude_dir, "claude_code")
}

/// Cùng bố cục `skills/`, `skills-disabled/` cho công cụ khác (ví dụ `~/.kiro`).
pub fn scan_personal_for(claude_dir: &Path, tool: &str) -> Vec<SkillEntry> {
    let mut out = Vec::new();
    for d in subdirs(&claude_dir.join("skills")) {
        if d.file_name().is_some_and(|n| n == "synced") {
            // skills/synced/<uuid>/<skill>/SKILL.md
            for group in subdirs(&d) {
                out.extend(subdirs(&group).iter().filter_map(|s| read_entry(s, Source::Synced, SkillState::Enabled, tool)));
            }
            continue;
        }
        out.extend(read_entry(&d, Source::Personal, SkillState::Enabled, tool));
    }
    for d in subdirs(&claude_dir.join("skills-disabled")) {
        out.extend(read_entry(&d, Source::Personal, SkillState::Disabled, tool));
    }
    out
}
