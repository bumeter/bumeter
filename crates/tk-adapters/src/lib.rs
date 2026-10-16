pub mod claude_code;
pub mod kiro;

/// Mọi công cụ được hỗ trợ, theo thứ tự hiển thị, kèm thư mục log. Thêm công cụ mới: thêm vào đây.
pub fn registry(claude_root: &std::path::Path, kiro_root: &std::path::Path) -> Vec<(tk_core::source::SourceInfo, std::path::PathBuf)> {
    vec![(claude_code::info(), claude_root.to_path_buf()), (kiro::info(), kiro_root.to_path_buf())]
}

/// Hạn mức các công cụ tự báo được (chạy lệnh của công cụ, có thể mất vài giây — gọi ở luồng nền).
/// Thêm công cụ có lệnh tương tự: thêm vào đây.
pub fn fetch_quotas(now: i64) -> Vec<(String, tk_core::meter::ToolQuota)> {
    kiro::usage::fetch().map(|u| (kiro::TOOL_ID.to_string(), u.into_quota(now))).into_iter().collect()
}

/// Gộp kết quả mới vào file hạn mức (giữ số cũ của công cụ lần này không lấy được). Trả về có gì mới không.
pub fn store_quotas(path: &std::path::Path, fresh: Vec<(String, tk_core::meter::ToolQuota)>) -> std::io::Result<bool> {
    if fresh.is_empty() {
        return Ok(false);
    }
    let mut all = tk_core::meter::read_quotas(path);
    all.extend(fresh);
    tk_core::meter::write_quotas(path, &all)?;
    Ok(true)
}
