//! Tìm skill trên skills.sh và chọn đúng thư mục skill trong repo đã clone.
//!
//! Cách làm theo skills-manager (MIT, © 2026 Tianliang Zhang, github.com/xingkongliang/skills-manager):
//! skills.sh chỉ là danh mục; cài = clone `github.com/<source>` rồi lấy thư mục của `skillId`.
//! Không gửi telemetry, không đọc HTML bảng xếp hạng (dễ hỏng) — chỉ dùng API tìm kiếm.
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::frontmatter;

pub const SEARCH_URL: &str = "https://skills.sh/api/search";
const MAX_LIMIT: u32 = 50;
const TIMEOUT_SECS: &str = "15";
/// Độ sâu tối đa khi tìm thư mục skill trong repo.
const MAX_DEPTH: usize = 6;
const SKIP_DIRS: [&str; 2] = [".git", "node_modules"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hit {
    /// `owner/repo/skillId`.
    pub id: String,
    /// `owner/repo` trên GitHub.
    pub source: String,
    pub skill_id: String,
    pub name: String,
    pub installs: u64,
}

/// `owner/repo`: hai đoạn chữ, số, `-`, `_`, `.`; không bắt đầu bằng `-` hay `.`.
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

/// Mã hoá phần trăm cho query (giữ nguyên ký tự không dành riêng theo RFC 3986).
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

/// Đọc kết quả `api/search`: mảng hoặc `{ "skills": [...] }`. Mục thiếu hoặc không an toàn bị bỏ.
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

/// Tìm trên skills.sh bằng `curl` có sẵn trên máy.
pub fn search(query: &str, limit: u32) -> anyhow::Result<Vec<Hit>> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(Vec::new());
    }
    let url = format!("{SEARCH_URL}?q={}&limit={}", encode_query(q), limit.clamp(1, MAX_LIMIT));
    let out = Command::new("curl")
        .args(["-fsS", "--max-time", TIMEOUT_SECS, "-H", "Accept: application/json", "--"])
        .arg(&url)
        .output()
        .context("không chạy được curl")?;
    if !out.status.success() {
        bail!("skills.sh không trả lời: {}", String::from_utf8_lossy(&out.stderr).trim());
    }
    let v: Value = serde_json::from_slice(&out.stdout).context("kết quả từ skills.sh không đọc được")?;
    Ok(parse_search(&v))
}

/// Thư mục thật (không phải symlink) có file SKILL.md thật.
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

/// Duyệt theo thứ tự tên (cố định giữa các máy), không đi theo symlink.
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

/// Thư mục của skill `id` trong repo: `<id>`, `skills/<id>`, `.agents/skills/<id>`, rồi tìm theo
/// tên thư mục, cuối cùng theo `name` trong SKILL.md. Không thấy thì lỗi — không bao giờ lấy cả repo.
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
    bail!("không thấy skill '{id}' trong repo")
}
