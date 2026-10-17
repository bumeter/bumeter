pub mod claude_code;
pub mod kiro;
pub mod opencode;
pub mod process;

/// All supported tools, in display order, with their log directory. Add a new tool here.
pub fn registry(claude_root: &std::path::Path, kiro_root: &std::path::Path) -> Vec<(bum_core::source::SourceInfo, std::path::PathBuf)> {
    vec![(claude_code::info(), claude_root.to_path_buf()), (kiro::info(), kiro_root.to_path_buf())]
}

/// Tools whose log directory exists on this machine (so the tray hides tools that are not installed).
pub fn installed(claude_root: &std::path::Path, kiro_root: &std::path::Path) -> Vec<bum_core::source::SourceInfo> {
    registry(claude_root, kiro_root).into_iter().filter(|(_, root)| root.exists()).map(|(i, _)| i).collect()
}

/// Limits that tools self-report (runs the tool's command; may take a few seconds — call it on a background thread).
/// Add a tool with a similar command here.
pub fn fetch_quotas(now: i64) -> Vec<(String, bum_core::meter::ToolQuota)> {
    kiro::usage::fetch().map(|u| (kiro::TOOL_ID.to_string(), u.into_quota(now))).into_iter().collect()
}

/// Merge fresh results into the quota file (keeping the old numbers for tools not fetched this time). Returns whether there is anything new.
pub fn store_quotas(path: &std::path::Path, fresh: Vec<(String, bum_core::meter::ToolQuota)>) -> std::io::Result<bool> {
    if fresh.is_empty() {
        return Ok(false);
    }
    let mut all = bum_core::meter::read_quotas(path);
    all.extend(fresh);
    bum_core::meter::write_quotas(path, &all)?;
    Ok(true)
}
