//! Nối adapter → store: đọc tiếp từng file theo offset đã lưu.
use std::io;
use std::path::{Path, PathBuf};

use crate::source::{LogSource, ParseCtx};
use crate::store::{ApplySummary, FileOffset, Store};
use crate::tailer;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IngestReport {
    pub summary: ApplySummary,
    pub parse_errors: u64,
    /// File không đọc được (quyền, bị xóa giữa chừng…); đã bỏ qua, quét tiếp file khác.
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
            // File bị xóa: giữ dữ liệu đã có.
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
                // File bị ghi đè nhỏ hơn hoặc bị thay thế: đọc lại từ đầu.
                self.store.purge_file(&key)?;
                (0, ParseCtx::default())
            }
            None => (0, ParseCtx::default()),
        };

        let (lines, offset) = match tailer::read_complete_lines(path, start) {
            Ok(r) => r,
            // Bị xóa giữa lúc stat và lúc mở.
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

    /// Nguồn ghi đè cả file: bỏ qua nếu không đổi, không thì xoá dữ liệu cũ của file rồi nạp lại.
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

    /// Quét mọi file của mọi nguồn; file không đổi sẽ bị bỏ qua nhờ offset.
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
                    eprintln!("bumeter: bỏ qua {}: {e}", f.display());
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
