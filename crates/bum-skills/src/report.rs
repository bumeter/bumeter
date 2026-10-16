//! Ghép danh mục skill với mức dùng thật và đưa ra gợi ý (spec mục 8.1).
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::catalog::{SkillEntry, SkillState, Source};

const UNUSED_DAYS: i64 = 30;

/// Mức dùng một skill, lấy từ log (bum-core `skill_stats`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    pub name: String,
    pub uses: u64,
    pub projects: u64,
    pub tokens: u64,
    pub avg_tokens: Option<u64>,
    pub last_used: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SkillRow {
    pub entry: SkillEntry,
    pub uses: u64,
    pub projects: u64,
    pub tokens: u64,
    pub avg_tokens: Option<u64>,
    pub last_used: Option<String>,
    /// Đang bật, có chi phí nền, nhưng 30 ngày chưa dùng.
    pub unused_30d: bool,
    /// Có số liệu dùng không (log Kiro không ghi lần gọi skill).
    pub usage_known: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SkillReport {
    pub rows: Vec<SkillRow>,
    /// Slash command và skill chỉ còn thấy trong log (đã gỡ).
    pub commands: Vec<SkillRow>,
    /// Tổng chi phí nền của các skill đang bật (token mỗi phiên).
    pub total_always_on: u64,
    pub unused: Vec<String>,
    pub unused_always_on: u64,
    pub warnings: Vec<String>,
}

/// Skill cài trong 30 ngày gần đây thì chưa đủ lâu để coi là "không dùng".
fn recently_installed(installed_at: Option<&str>, now: DateTime<Utc>) -> bool {
    installed_at.and_then(|t| DateTime::parse_from_rfc3339(t).ok()).is_some_and(|t| (now - t.to_utc()).num_days() <= UNUSED_DAYS)
}

fn stale(last: Option<&str>, now: DateTime<Utc>) -> bool {
    match last.and_then(|l| DateTime::parse_from_rfc3339(l).ok()) {
        Some(t) => (now - t.to_utc()).num_days() > UNUSED_DAYS,
        None => true,
    }
}

/// `/tên` (slash command) của một skill đã cài được tính chung với skill đó.
fn merge_commands(entries: &[SkillEntry], usage: Vec<Usage>) -> Vec<Usage> {
    let installed: std::collections::HashSet<&str> = entries.iter().filter(|e| e.tool == "claude_code").map(|e| e.name.as_str()).collect();
    let mut out: Vec<Usage> = Vec::new();
    for u in usage {
        let name = match u.name.strip_prefix('/') {
            Some(bare) if installed.contains(bare) => bare.to_string(),
            _ => u.name.clone(),
        };
        match out.iter_mut().find(|x| x.name == name) {
            Some(x) => {
                x.uses += u.uses;
                x.tokens += u.tokens;
                x.projects = x.projects.max(u.projects);
                if u.last_used > x.last_used {
                    x.last_used = u.last_used;
                }
                x.avg_tokens = (x.uses > 0 && x.tokens > 0).then(|| x.tokens / x.uses);
            }
            None => out.push(Usage { name, ..u }),
        }
    }
    out
}

/// `usage = None`: chưa đọc được mức dùng (DB chưa có) → không gợi ý tắt skill nào.
pub fn build(entries: Vec<SkillEntry>, usage: Option<Vec<Usage>>, mut warnings: Vec<String>, now: DateTime<Utc>) -> SkillReport {
    let have_usage = usage.is_some();
    if !have_usage {
        warnings.push("Chưa đọc được mức dùng (dữ liệu chưa sẵn sàng), tạm không gợi ý tắt skill.".into());
    }
    let usage = merge_commands(&entries, usage.unwrap_or_default());
    let find = |name: &str| usage.iter().find(|u| u.name == name);
    let mut rows: Vec<SkillRow> = entries
        .into_iter()
        .map(|entry| {
            // Chỉ log Claude Code ghi lần gọi skill; skill Kiro trùng tên không mượn số liệu này.
            let usage_known = entry.tool == "claude_code";
            let u = if usage_known { find(&entry.name) } else { None };
            let last_used = u.and_then(|u| u.last_used.clone());
            let unused_30d = usage_known
                && have_usage
                && !recently_installed(entry.installed_at.as_deref(), now)
                && entry.state == SkillState::Enabled
                && entry.managed
                && entry.always_on_tokens.unwrap_or(0) > 0
                && stale(last_used.as_deref(), now);
            SkillRow {
                uses: u.map_or(0, |u| u.uses),
                projects: u.map_or(0, |u| u.projects),
                tokens: u.map_or(0, |u| u.tokens),
                avg_tokens: u.and_then(|u| u.avg_tokens),
                last_used,
                unused_30d,
                usage_known,
                entry,
            }
        })
        .collect();
    rows.sort_by(|a, b| b.uses.cmp(&a.uses).then(a.entry.name.cmp(&b.entry.name)));

    let known: std::collections::HashSet<&str> = rows.iter().filter(|r| r.usage_known).map(|r| r.entry.name.as_str()).collect();
    let mut commands: Vec<SkillRow> = usage
        .iter()
        .filter(|u| !known.contains(u.name.as_str()))
        .map(|u| SkillRow {
            entry: SkillEntry {
                name: u.name.clone(),
                tool: "claude_code".into(),
                description: String::new(),
                source: Source::Command,
                state: SkillState::Enabled,
                path: None,
                symlink: false,
                installed_at: None,
                always_on_tokens: None,
                on_invoke_tokens: None,
                estimated: false,
                managed: false,
            },
            uses: u.uses,
            projects: u.projects,
            tokens: u.tokens,
            avg_tokens: u.avg_tokens,
            last_used: u.last_used.clone(),
            unused_30d: false,
            usage_known: true,
        })
        .collect();
    commands.sort_by(|a, b| b.uses.cmp(&a.uses).then(a.entry.name.cmp(&b.entry.name)));

    let enabled = rows.iter().filter(|r| r.entry.state == SkillState::Enabled && r.entry.tool == "claude_code");
    let total_always_on = enabled.map(|r| r.entry.always_on_tokens.unwrap_or(0)).sum();
    let mut unused: Vec<&SkillRow> = rows.iter().filter(|r| r.unused_30d).collect();
    unused.sort_by(|a, b| a.entry.name.cmp(&b.entry.name));
    SkillReport {
        unused_always_on: unused.iter().map(|r| r.entry.always_on_tokens.unwrap_or(0)).sum(),
        unused: unused.iter().map(|r| r.entry.name.clone()).collect(),
        rows,
        commands,
        total_always_on,
        warnings,
    }
}
