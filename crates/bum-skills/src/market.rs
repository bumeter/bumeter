//! Find skills on skills.sh and pick the right skill directory inside the cloned repo.
//!
//! Approach follows skills-manager (MIT, © 2026 Tianliang Zhang, github.com/xingkongliang/skills-manager):
//! skills.sh is only a catalog; installing = clone `github.com/<source>` then take the directory for `skillId`.
//! No telemetry, no scraping the HTML: only the JSON APIs are used — `api/search` for a query and
//! `api/skills/<mode>/<page>` for the leaderboard (Hot / Trending 24h / All time).
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::frontmatter;

pub const SEARCH_URL: &str = "https://skills.sh/api/search";
/// Leaderboard base; a full page is `<LEADERBOARD_URL>/<mode>/<page>`.
pub const LEADERBOARD_URL: &str = "https://skills.sh/api/skills";
/// Leaderboard modes skills.sh understands, in the order the UI shows them.
pub const LEADERBOARD_MODES: [&str; 3] = ["hot", "trending", "all-time"];
const MAX_LIMIT: u32 = 50;
const TIMEOUT_SECS: &str = "15";
/// Maximum depth when searching for a skill directory inside the repo.
const MAX_DEPTH: usize = 6;
const SKIP_DIRS: [&str; 2] = [".git", "node_modules"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hit {
    /// `owner/repo/skillId`.
    pub id: String,
    /// `owner/repo` on GitHub.
    pub source: String,
    pub skill_id: String,
    pub name: String,
    pub installs: u64,
}

/// `owner/repo`: two segments of letters, digits, `-`, `_`, `.`; not starting with `-` or `.`.
pub fn valid_source(s: &str) -> bool {
    let parts: Vec<&str> = s.split('/').collect();
    parts.len() == 2
        && parts.iter().all(|p| {
            !p.is_empty()
                && p.len() <= 100
                && !p.starts_with(['-', '.'])
                && !p.contains("..")
                && p.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        })
}

/// Percent-encode the query (leave unreserved characters per RFC 3986 as-is).
pub fn encode_query(q: &str) -> String {
    let mut out = String::new();
    for b in q.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Read an `api/search` result: an array or `{ "skills": [...] }`. Missing or unsafe items are dropped.
pub fn parse_search(v: &Value) -> Vec<Hit> {
    let items = v.as_array().or_else(|| v["skills"].as_array());
    items
        .into_iter()
        .flatten()
        .filter_map(|it| {
            let source = it["source"].as_str()?.to_string();
            let skill_id = it["skillId"].as_str().or_else(|| it["skill_id"].as_str())?.to_string();
            if !valid_source(&source) || crate::ops::check_name(&skill_id).is_err() {
                return None;
            }
            Some(Hit {
                id: it["id"].as_str().map(str::to_string).unwrap_or_else(|| format!("{source}/{skill_id}")),
                name: it["name"].as_str().unwrap_or(&skill_id).to_string(),
                installs: it["installs"].as_u64().unwrap_or(0),
                source,
                skill_id,
            })
        })
        .collect()
}

/// Whether `mode` is a leaderboard mode skills.sh understands.
pub fn valid_mode(mode: &str) -> bool {
    LEADERBOARD_MODES.contains(&mode)
}

/// GET `url` and parse the body as JSON, using the `curl` available on the machine.
fn get_json(url: &str) -> anyhow::Result<Value> {
    let out = Command::new("curl")
        .args(["-fsS", "--max-time", TIMEOUT_SECS, "-H", "Accept: application/json", "--"])
        .arg(url)
        .output()
        .context("could not run curl")?;
    if !out.status.success() {
        bail!("skills.sh did not respond: {}", String::from_utf8_lossy(&out.stderr).trim());
    }
    serde_json::from_slice(&out.stdout).context("could not read the result from skills.sh")
}

/// Search skills.sh using the `curl` available on the machine.
pub fn search(query: &str, limit: u32) -> anyhow::Result<Vec<Hit>> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(Vec::new());
    }
    let url = format!("{SEARCH_URL}?q={}&limit={}", encode_query(q), limit.clamp(1, MAX_LIMIT));
    Ok(parse_search(&get_json(&url)?))
}

/// One page of a skills.sh leaderboard (up to 200 skills), plus how many more there are.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Page {
    pub hits: Vec<Hit>,
    pub has_more: bool,
    /// Total number of skills in this leaderboard.
    pub total: u64,
}

/// Read a leaderboard response: the skills plus `hasMore` / `total`.
pub fn parse_page(v: &Value) -> Page {
    Page { hits: parse_search(v), has_more: v["hasMore"].as_bool().unwrap_or(false), total: v["total"].as_u64().unwrap_or(0) }
}

/// One page of a skills.sh leaderboard (`hot`, `trending`, `all-time`); page 1 is the top of the list.
pub fn leaderboard(mode: &str, page: u32) -> anyhow::Result<Page> {
    if !valid_mode(mode) {
        bail!("unknown leaderboard mode '{mode}'");
    }
    let url = format!("{LEADERBOARD_URL}/{mode}/{}", page.max(1));
    Ok(parse_page(&get_json(&url)?))
}

/// A real directory (not a symlink) containing a real SKILL.md file.
fn is_skill_dir(p: &Path) -> bool {
    let real_dir = std::fs::symlink_metadata(p).map(|m| m.is_dir()).unwrap_or(false);
    real_dir
        && ["SKILL.md", "skill.md"]
            .iter()
            .any(|f| std::fs::symlink_metadata(p.join(f)).map(|m| m.is_file()).unwrap_or(false))
}

fn skill_name(dir: &Path) -> Option<String> {
    let body = std::fs::read_to_string(dir.join("SKILL.md")).or_else(|_| std::fs::read_to_string(dir.join("skill.md"))).ok()?;
    frontmatter::parse(&body).name
}

/// Walk in name order (stable across machines), not following symlinks.
fn walk(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > MAX_DEPTH {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut children: Vec<PathBuf> = rd
        .flatten()
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .filter(|e| !SKIP_DIRS.contains(&e.file_name().to_string_lossy().as_ref()))
        .map(|e| e.path())
        .collect();
    children.sort();
    for c in children {
        out.push(c.clone());
        walk(&c, depth + 1, out);
    }
}

/// The directory of skill `id` in the repo: `<id>`, `skills/<id>`, `.agents/skills/<id>`, then search by
/// directory name, and finally by `name` in SKILL.md. If not found it errors — never take the whole repo.
pub fn find_skill_dir(repo: &Path, id: &str) -> anyhow::Result<PathBuf> {
    for c in [repo.join(id), repo.join("skills").join(id), repo.join(".agents").join("skills").join(id)] {
        if is_skill_dir(&c) {
            return Ok(c);
        }
    }
    let mut dirs = Vec::new();
    walk(repo, 1, &mut dirs);
    if let Some(d) = dirs.iter().find(|d| d.file_name().is_some_and(|n| n == id) && is_skill_dir(d)) {
        return Ok(d.clone());
    }
    if let Some(d) = dirs.iter().find(|d| is_skill_dir(d) && skill_name(d).as_deref() == Some(id)) {
        return Ok(d.clone());
    }
    bail!("skill '{id}' not found in the repo")
}
