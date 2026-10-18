//! Shared data model, independent of any specific AI tool.
use serde::{Deserialize, Serialize};

/// Tokens of one API call (or the total across several).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    pub input: u64,
    pub cache_write_5m: u64,
    pub cache_write_1h: u64,
    pub cache_read: u64,
    pub output: u64,
    /// Reasoning tokens reported separately (OpenCode); billed like output. 0 for tools that don't report it.
    #[serde(default)]
    pub reasoning: u64,
}

impl Usage {
    pub fn cache_write(&self) -> u64 {
        self.cache_write_5m + self.cache_write_1h
    }

    /// Number of tokens that had to be "read" in this call (everything except output and reasoning).
    pub fn context(&self) -> u64 {
        self.input + self.cache_write() + self.cache_read
    }

    pub fn total(&self) -> u64 {
        self.context() + self.output + self.reasoning
    }

    /// What these tokens weigh against a rate limit, in "input token" units. Every Claude model is
    /// priced with the same ratios (cache write 1.25× / 2×, cache read 0.1×, output 5×) and the limits
    /// are very likely weighted the same way, so this ranks what burns the quota far better than
    /// `total()`, where cache reads are almost everything. A relative unit, not money.
    pub fn quota_weight(&self) -> u64 {
        let w = self.input as u128 * 100
            + self.cache_write_5m as u128 * 125
            + self.cache_write_1h as u128 * 200
            + self.cache_read as u128 * 10
            + (self.output + self.reasoning) as u128 * 500;
        (w / 100) as u64
    }

    pub fn add(&mut self, other: &Usage) {
        self.input += other.input;
        self.cache_write_5m += other.cache_write_5m;
        self.cache_write_1h += other.cache_write_1h;
        self.cache_read += other.cache_read;
        self.output += other.output;
        self.reasoning += other.reasoning;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillKind {
    /// Skill invoked within the turn itself (tool `Skill` or `attributionSkill`).
    Direct,
    /// Slash command typed by the user (`/model`, `/pj-review`…).
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

/// Event produced by an adapter from a log line. Times are RFC3339 UTC.
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
        /// Cost in USD as reported by the tool (OpenCode); None when the tool doesn't report it.
        #[serde(default)]
        cost_usd: Option<f64>,
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
    /// Conversation title (Claude `ai-title`, Kiro `title`); the last one wins.
    Title {
        session_id: String,
        title: String,
    },
    /// Cost in the tool's credits (e.g. Kiro), attached to a call.
    Metered {
        request_id: String,
        credits: f64,
    },
}
