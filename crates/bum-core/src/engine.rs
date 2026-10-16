//! Adapter → store pipeline: incrementally read each file from its saved offset.
use std::io;
use std::path::{Path, PathBuf};

use crate::source::{LogSource, ParseCtx};
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

pub struct Engine {
    store: Store,
    sources: Vec<Box<dyn LogSource>>,
}

impl Engine {
    pub fn new(store: Store, sources: Vec<Box<dyn LogSource>>) -> Self {
        Self { store, sources }
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

    pub fn ingest_path(&mut self, path: &Path) -> anyhow::Result<IngestReport> {
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

    /// Scan every file of every source; unchanged files are skipped thanks to the offset.
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
        Ok(report)
    }
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
