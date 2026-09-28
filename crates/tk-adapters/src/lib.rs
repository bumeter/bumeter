pub mod claude_code;
pub mod kiro;

/// Mọi công cụ được hỗ trợ, theo thứ tự hiển thị, kèm thư mục log. Thêm công cụ mới: thêm vào đây.
pub fn registry(claude_root: &std::path::Path, kiro_root: &std::path::Path) -> Vec<(tk_core::source::SourceInfo, std::path::PathBuf)> {
    vec![(claude_code::info(), claude_root.to_path_buf()), (kiro::info(), kiro_root.to_path_buf())]
}
