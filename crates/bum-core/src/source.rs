//! Contract between the core and the log-reading adapters.
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::model::RawEvent;

/// Parse state of a file, saved alongside the offset to resume after a restart.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParseCtx {
    pub current_prompt: Option<String>,
    pub session_emitted: bool,
}

/// Description of an AI tool for the UI: label, symbol, color, and what its logs support.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceInfo {
    pub id: String,
    pub label: String,
    /// A single letter shown in the icon/tag.
    pub letter: String,
    /// Identity color (hex), following the tool's brand color.
    pub color: String,
    /// Logo filename in `app/ui/public/tool-icons/`; if absent, the UI shows a letter box.
    #[serde(default)]
    pub icon: Option<String>,
    /// Logs have real token counts (not just estimates).
    pub real_tokens: bool,
    /// Has per-call context breakdown.
    pub composition: bool,
    /// Has 5-hour/7-day quota.
    pub limits: bool,
    /// Costs are billed in credits.
    pub credits: bool,
    /// Can read question/answer content from the logs.
    pub transcript: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid log line: {0}")]
pub struct LineError(pub String);

pub trait LogSource: Send {
    fn id(&self) -> &'static str;
    fn info(&self) -> SourceInfo;
    fn watch_roots(&self) -> Vec<PathBuf>;
    fn accepts(&self, path: &Path) -> bool;
    fn parse_line(&self, path: &Path, ctx: &mut ParseCtx, line: &str) -> Result<Vec<RawEvent>, LineError>;

    /// Source overwrites the whole file each time (no line appending): the engine re-reads everything via `parse_file`.
    fn whole_file(&self) -> bool {
        false
    }

    fn parse_file(&self, _path: &Path) -> Result<Vec<RawEvent>, LineError> {
        Err(LineError("this source is read line by line".into()))
    }
}

/// A tool that keeps its history in a database (e.g. OpenCode's SQLite) instead of append-only logs.
/// The engine stores a watermark per source and asks only for rows changed after it.
pub trait DbSource: Send {
    fn id(&self) -> &'static str;
    fn info(&self) -> SourceInfo;
    /// The database file; a change to it or to `<file>-wal` triggers a scan.
    fn db_path(&self) -> PathBuf;
    /// Rows changed after `watermark` (ms) → events and the new watermark. Must never write to the database.
    fn scan(&mut self, watermark: Option<i64>) -> anyhow::Result<(Vec<RawEvent>, i64)>;
}

/// A directory to watch. `files`: exact paths to forward besides `.jsonl`/`.json` files (e.g. a database and its WAL).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchTarget {
    pub path: PathBuf,
    pub recursive: bool,
    pub files: Vec<PathBuf>,
}
