//! Hợp đồng giữa lõi và các adapter đọc log.
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::model::RawEvent;

/// Trạng thái parse của một file, được lưu cùng offset để đọc tiếp sau khi khởi động lại.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParseCtx {
    pub current_prompt: Option<String>,
    pub session_emitted: bool,
}

/// Mô tả một công cụ AI cho UI: nhãn, ký hiệu, màu và những gì log của nó cho phép.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceInfo {
    pub id: String,
    pub label: String,
    /// Một chữ cái hiển thị trong icon/tag.
    pub letter: String,
    /// Màu nhận diện (hex), theo màu thương hiệu của công cụ.
    pub color: String,
    /// Tên file logo trong `app/ui/public/tool-icons/`; không có thì UI hiện ô chữ cái.
    #[serde(default)]
    pub icon: Option<String>,
    /// Log có số token thật (không chỉ ước lượng).
    pub real_tokens: bool,
    /// Có Phân tích context theo lượt gọi.
    pub composition: bool,
    /// Có quota 5 giờ/7 ngày.
    pub limits: bool,
    /// Chi phí tính bằng credit.
    pub credits: bool,
    /// Đọc được nội dung hỏi/đáp từ log.
    pub transcript: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("dòng log không hợp lệ: {0}")]
pub struct LineError(pub String);

pub trait LogSource: Send {
    fn id(&self) -> &'static str;
    fn info(&self) -> SourceInfo;
    fn watch_roots(&self) -> Vec<PathBuf>;
    fn accepts(&self, path: &Path) -> bool;
    fn parse_line(&self, path: &Path, ctx: &mut ParseCtx, line: &str) -> Result<Vec<RawEvent>, LineError>;

    /// Nguồn ghi đè cả file mỗi lần (không nối dòng): engine đọc lại toàn bộ bằng `parse_file`.
    fn whole_file(&self) -> bool {
        false
    }

    fn parse_file(&self, _path: &Path) -> Result<Vec<RawEvent>, LineError> {
        Err(LineError("nguồn này đọc theo dòng".into()))
    }
}
