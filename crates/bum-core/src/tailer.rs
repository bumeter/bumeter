//! Read the new part of a log file, only up to the last newline (spec section 5).
use std::fs::{File, Metadata};
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;
use std::time::UNIX_EPOCH;

/// Return the complete lines from `from` and the new offset (right after the last `\n`).
pub fn read_complete_lines(path: &Path, from: u64) -> io::Result<(Vec<String>, u64)> {
    let mut f = File::open(path)?;
    f.seek(SeekFrom::Start(from))?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf)?;
    let Some(last_nl) = buf.iter().rposition(|b| *b == b'\n') else { return Ok((Vec::new(), from)) };
    let lines = buf[..last_nl]
        .split(|b| *b == b'\n')
        .map(|l| String::from_utf8_lossy(l).trim_end_matches('\r').to_string())
        .filter(|l| !l.is_empty())
        .collect();
    Ok((lines, from + last_nl as u64 + 1))
}

/// File identity to detect a replaced file.
#[cfg(unix)]
pub fn file_identity(meta: &Metadata) -> String {
    use std::os::unix::fs::MetadataExt;
    format!("{}:{}", meta.dev(), meta.ino())
}

#[cfg(not(unix))]
pub fn file_identity(meta: &Metadata) -> String {
    meta.created()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_nanos().to_string())
        .unwrap_or_default()
}

pub fn mtime_secs(meta: &Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
