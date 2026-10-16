//! Adapter cho Claude Code: đọc `~/.claude/projects/**/*.jsonl`.
mod log;
pub mod composition;
pub mod statusline;
pub mod transcript;

use std::path::{Path, PathBuf};

use bum_core::model::RawEvent;
use bum_core::source::{LineError, LogSource, ParseCtx};

pub const TOOL_ID: &str = "claude_code";

pub struct ClaudeCode {
    root: PathBuf,
}

impl ClaudeCode {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// `BUM_CLAUDE_DIR` (hoặc `TK_CLAUDE_DIR`) nếu có, không thì `~/.claude/projects`.
    pub fn default_root() -> PathBuf {
        if let Some(dir) = std::env::var_os("BUM_CLAUDE_DIR").or_else(|| std::env::var_os("TK_CLAUDE_DIR")) {
            return PathBuf::from(dir);
        }
        dirs::home_dir().unwrap_or_default().join(".claude").join("projects")
    }
}

pub fn info() -> bum_core::source::SourceInfo {
    bum_core::source::SourceInfo {
        id: TOOL_ID.into(), label: "Claude Code".into(), letter: "C".into(), color: "#d77655".into(), icon: Some("claude_code.svg".into()),
        real_tokens: true, composition: true, limits: true, credits: false, transcript: true,
    }
}

impl LogSource for ClaudeCode {
    fn id(&self) -> &'static str {
        TOOL_ID
    }

    fn info(&self) -> bum_core::source::SourceInfo {
        info()
    }

    fn watch_roots(&self) -> Vec<PathBuf> {
        vec![self.root.clone()]
    }

    fn accepts(&self, path: &Path) -> bool {
        path.starts_with(&self.root) && path.extension().is_some_and(|e| e == "jsonl")
    }

    fn parse_line(&self, path: &Path, ctx: &mut ParseCtx, line: &str) -> Result<Vec<RawEvent>, LineError> {
        log::parse_line(&self.root, path, ctx, line)
    }
}
