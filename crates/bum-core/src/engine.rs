//! Adapter → store pipeline: incrementally read each file from its saved offset.
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::source::{DbSource, LogSource, ParseCtx, WatchTarget};
use crate::store::{ApplySummary, FileOffset, Store};
use crate::tailer;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IngestReport {
    pub summary: ApplySummary,
    pub parse_errors: u64,
    /// File could not be read (permissions, deleted mid-read, …); it was skipped and scanning continues with other files.
    pub file_errors: u64,
    pub lines: u64,
}

impl IngestReport {
    pub fn merge(&mut self, other: IngestReport) {
        self.summary.merge(other.summary);
        self.parse_errors += other.parse_errors;
        self.file_errors += other.file_errors;
        self.lines += other.lines;
    }
}

/// Minimum time between two scans of the same database: a streaming tool writes its WAL many times per second.
const DB_MIN_INTERVAL: Duration = Duration::from_secs(1);

pub struct Engine {
    store: Store,
    sources: Vec<Box<dyn LogSource>>,
    db_sources: Vec<Box<dyn DbSource>>,
    db_min_interval: Duration,
    /// Per database source: when it was last scanned, and whether a change arrived since (deferred scan).
    db_last: Vec<Option<Instant>>,
    db_pending: Vec<bool>,
}

impl Engine {
    pub fn new(store: Store, sources: Vec<Box<dyn LogSource>>) -> Self {
        Self { store, sources, db_sources: Vec::new(), db_min_interval: DB_MIN_INTERVAL, db_last: Vec::new(), db_pending: Vec::new() }
    }

    /// Add database sources (read by watermark, never purged).
    pub fn with_db_sources(mut self, db_sources: Vec<Box<dyn DbSource>>) -> Self {
        self.db_last = vec![None; db_sources.len()];
        self.db_pending = vec![false; db_sources.len()];
        self.db_sources = db_sources;
        self
    }

    /// Change the minimum time between two scans of a database (tests).
    pub fn with_db_min_interval(mut self, d: Duration) -> Self {
        self.db_min_interval = d;
        self
    }

    /// Run the database scans deferred by throttling once their interval has passed. Call it periodically (host tick).
    pub fn scan_pending_db(&mut self) -> anyhow::Result<IngestReport> {
        let mut report = IngestReport::default();
        for i in 0..self.db_sources.len() {
            if self.db_pending[i] && self.db_last[i].is_none_or(|t| t.elapsed() >= self.db_min_interval) {
                report.merge(self.ingest_db(i)?);
            }
        }
        Ok(report)
    }

    pub fn store(&self) -> &Store {
        &self.store
    }

    pub fn store_mut(&mut self) -> &mut Store {
        &mut self.store
    }

    pub fn watch_roots(&self) -> Vec<PathBuf> {
        self.sources.iter().flat_map(|s| s.watch_roots()).collect()
    }

    pub fn accepts(&self, path: &Path) -> bool {
        self.sources.iter().any(|s| s.accepts(path))
    }

    /// What the host should watch: log roots recursively; each database's directory non-recursively,
    /// forwarding only the database and its WAL (the directory may hold busy folders such as snapshots).
    pub fn watch_targets(&self) -> Vec<WatchTarget> {
        let logs = self.watch_roots().into_iter().map(|path| WatchTarget { path, recursive: true, files: Vec::new() });
        let dbs = self.db_sources.iter().filter_map(|d| {
            let db = d.db_path();
            let dir = db.parent()?.to_path_buf();
            Some(WatchTarget { path: dir, recursive: false, files: vec![db.clone(), wal_path(&db)] })
        });
        logs.chain(dbs).collect()
    }

    pub fn ingest_path(&mut self, path: &Path) -> anyhow::Result<IngestReport> {
        if let Some(i) = self.db_sources.iter().position(|d| {
            let db = d.db_path();
            path == db || path == wal_path(&db)
        }) {
            if self.db_last[i].is_some_and(|t| t.elapsed() < self.db_min_interval) {
                self.db_pending[i] = true; // picked up by `scan_pending_db`
                return Ok(IngestReport::default());
            }
            return self.ingest_db(i);
        }
        let Some(src) = self.sources.iter().position(|s| s.accepts(path)) else { return Ok(IngestReport::default()) };
        let meta = match std::fs::metadata(path) {
            Ok(m) => m,
            // File deleted: keep the existing data.
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(IngestReport::default()),
            Err(e) => return Err(e.into()),
        };
        let key = path.to_string_lossy().to_string();
        let file_id = tailer::file_identity(&meta);
        let size = meta.len();
        if self.sources[src].whole_file() {
            return self.ingest_whole(src, path, key, file_id, size, tailer::mtime_secs(&meta));
        }

        let (start, mut ctx) = match self.store.get_offset(&key)? {
            Some(prev) if prev.file_id == file_id && size >= prev.offset => {
                if size == prev.offset {
                    return Ok(IngestReport::default());
                }
                (prev.offset, serde_json::from_str::<ParseCtx>(&prev.ctx_json).unwrap_or_default())
            }
            Some(_) => {
                // File overwritten to a smaller size or replaced: re-read from the start.
                self.store.purge_file(&key)?;
                (0, ParseCtx::default())
            }
            None => (0, ParseCtx::default()),
        };

        let (lines, offset) = match tailer::read_complete_lines(path, start) {
            Ok(r) => r,
            // Deleted between stat and open.
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(IngestReport::default()),
            Err(e) => return Err(e.into()),
        };
        let mut events = Vec::new();
        let mut parse_errors = 0;
        for line in &lines {
            match self.sources[src].parse_line(path, &mut ctx, line) {
                Ok(mut ev) => events.append(&mut ev),
                Err(_) => parse_errors += 1,
            }
        }
        let off = FileOffset { path: key, file_id, size, offset, mtime: tailer::mtime_secs(&meta), ctx_json: serde_json::to_string(&ctx)? };
        let summary = self.store.ingest_batch(&off, &events, parse_errors)?;
        Ok(IngestReport { summary, parse_errors, file_errors: 0, lines: lines.len() as u64 })
    }

    /// Whole-file source: skip if unchanged, otherwise delete the file's old data and re-ingest.
    fn ingest_whole(&mut self, src: usize, path: &Path, key: String, file_id: String, size: u64, mtime: i64) -> anyhow::Result<IngestReport> {
        if let Some(prev) = self.store.get_offset(&key)? {
            if prev.file_id == file_id && prev.size == size && prev.mtime == mtime {
                return Ok(IngestReport::default());
            }
            self.store.purge_file(&key)?;
        }
        let (events, parse_errors) = match self.sources[src].parse_file(path) {
            Ok(ev) => (ev, 0),
            Err(_) => (Vec::new(), 1),
        };
        let lines = events.len() as u64;
        let off = FileOffset { path: key, file_id, size, offset: size, mtime, ctx_json: "{}".into() };
        let summary = self.store.ingest_batch(&off, &events, parse_errors)?;
        Ok(IngestReport { summary, parse_errors, file_errors: 0, lines })
    }

    /// Database source: read rows changed since the saved watermark. The watermark is stored in `file_offsets`
    /// (key `db:<id>`) in the same transaction as the rows; the file itself is never compared or purged.
    fn ingest_db(&mut self, i: usize) -> anyhow::Result<IngestReport> {
        self.db_last[i] = Some(Instant::now());
        self.db_pending[i] = false;
        let db = self.db_sources[i].db_path();
        if !db.exists() {
            return Ok(IngestReport::default());
        }
        let key = format!("db:{}", self.db_sources[i].id());
        let path = db.to_string_lossy().to_string();
        // The watermark belongs to one database file: pointing the source at another file starts over.
        let saved = self.store.get_offset(&key)?.and_then(|o| serde_json::from_str::<serde_json::Value>(&o.ctx_json).ok());
        let watermark = saved.filter(|v| v["path"].as_str() == Some(path.as_str())).and_then(|v| v["watermark"].as_i64());
        let (events, next) = self.db_sources[i].scan(watermark)?;
        let off = FileOffset { path: key, file_id: "db".into(), size: 0, offset: 0, mtime: 0, ctx_json: serde_json::json!({ "watermark": next, "path": path }).to_string() };
        let summary = self.store.ingest_batch(&off, &events, 0)?;
        Ok(IngestReport { summary, parse_errors: 0, file_errors: 0, lines: events.len() as u64 })
    }

    /// Scan every file of every source; unchanged files are skipped thanks to the offset. Database sources are read by watermark.
    pub fn full_scan(&mut self) -> anyhow::Result<IngestReport> {
        let mut files = Vec::new();
        for root in self.watch_roots() {
            collect_files(&root, &mut files);
        }
        files.sort();
        let mut report = IngestReport::default();
        files.retain(|f| self.accepts(f));
        for f in &files {
            match self.ingest_path(f) {
                Ok(r) => report.merge(r),
                Err(e) => {
                    eprintln!("bumeter: skipping {}: {e}", f.display());
                    report.file_errors += 1;
                }
            }
        }
        for i in 0..self.db_sources.len() {
            match self.ingest_db(i) {
                Ok(r) => report.merge(r),
                Err(e) => {
                    eprintln!("bumeter: skipping {} database: {e}", self.db_sources[i].id());
                    report.file_errors += 1;
                }
            }
        }
        Ok(report)
    }
}

/// SQLite's write-ahead log next to `db`.
fn wal_path(db: &Path) -> PathBuf {
    let mut s = db.as_os_str().to_owned();
    s.push("-wal");
    PathBuf::from(s)
}

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        match e.file_type() {
            Ok(t) if t.is_dir() => collect_files(&p, out),
            Ok(t) if t.is_file() => out.push(p),
            _ => {}
        }
    }
}
