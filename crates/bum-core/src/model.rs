//! Mô hình dữ liệu chung, không phụ thuộc công cụ AI cụ thể.
use serde::{Deserialize, Serialize};

/// Token của một lượt gọi API (hoặc tổng của nhiều lượt).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    pub input: u64,
    pub cache_write_5m: u64,
    pub cache_write_1h: u64,
    pub cache_read: u64,
    pub output: u64,
}

impl Usage {
    pub fn cache_write(&self) -> u64 {
        self.cache_write_5m + self.cache_write_1h
    }

    /// Số token phải "đọc" ở lượt gọi này (mọi thứ trừ output).
    pub fn context(&self) -> u64 {
        self.input + self.cache_write() + self.cache_read
    }

    pub fn total(&self) -> u64 {
        self.context() + self.output
    }

    pub fn add(&mut self, other: &Usage) {
        self.input += other.input;
        self.cache_write_5m += other.cache_write_5m;
        self.cache_write_1h += other.cache_write_1h;
        self.cache_read += other.cache_read;
        self.output += other.output;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillKind {
    /// Skill được gọi trong chính lần hỏi (tool `Skill` hoặc `attributionSkill`).
    Direct,
    /// Slash command người dùng gõ (`/model`, `/pj-review`…).
    Command,
}

impl SkillKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            SkillKind::Direct => "direct",
            SkillKind::Command => "command",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "direct" => Some(SkillKind::Direct),
            "command" => Some(SkillKind::Command),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolUse {
    pub id: String,
    pub name: String,
}

/// Sự kiện mà adapter sinh ra từ một dòng log. Thời gian là RFC3339 UTC.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RawEvent {
    Session {
        session_id: String,
        tool: String,
        project: String,
        cwd: Option<String>,
        version: Option<String>,
        ts: String,
    },
    Prompt {
        key: String,
        session_id: String,
        ts: String,
        text: String,
    },
    ApiCall {
        request_id: String,
        session_id: String,
        prompt_key: Option<String>,
        ts: String,
        model: String,
        usage: Usage,
        tool_uses: Vec<ToolUse>,
        sidechain: bool,
    },
    Skill {
        prompt_key: Option<String>,
        session_id: String,
        ts: String,
        skill: String,
        kind: SkillKind,
    },
    Interrupted {
        prompt_key: Option<String>,
        session_id: String,
        ts: String,
    },
    Compacted {
        session_id: String,
        ts: String,
    },
    RateLimited {
        ts: String,
        limit_type: String,
        resets_at: i64,
    },
    /// Tiêu đề hội thoại (Claude `ai-title`, Kiro `title`); cái sau cùng thắng.
    Title {
        session_id: String,
        title: String,
    },
    /// Chi phí tính bằng credit của công cụ (ví dụ Kiro), gắn vào một lượt gọi.
    Metered {
        request_id: String,
        credits: f64,
    },
}
