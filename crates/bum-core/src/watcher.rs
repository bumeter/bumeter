//! Watch for file changes via OS events (inotify / FSEvents / ReadDirectoryChangesW).
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use crate::host::HostMsg;

/// Only forward `.jsonl` files and `limits_file` (ignore DB/WAL writes in the data dir).
/// Directories that cannot be watched (inotify watch limit reached, missing permissions) are returned so they can be reported, and
/// will only be updated by the periodic reconciliation scan.
pub fn spawn_watcher(dirs: &[PathBuf], limits_file: &Path, tx: Sender<HostMsg>) -> notify::Result<(RecommendedWatcher, Vec<PathBuf>)> {
    let limits_file = limits_file.to_path_buf();
    let mut w = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        let Ok(ev) = res else { return };
        if !matches!(ev.kind, EventKind::Create(_) | EventKind::Modify(_)) {
            return;
        }
        let paths: Vec<PathBuf> = ev
            .paths
            .into_iter()
            .filter(|p| p.extension().is_some_and(|e| e == "jsonl" || e == "json") || *p == limits_file)
            .collect();
        if !paths.is_empty() {
            let _ = tx.send(HostMsg::FsChanged(paths));
        }
    })?;
    let mut failed = Vec::new();
    for d in dirs.iter().filter(|d| d.exists()) {
        if let Err(e) = w.watch(d, RecursiveMode::Recursive) {
            eprintln!("bumeter: cannot watch {}: {e} (will rely on periodic scan)", d.display());
            failed.push(d.clone());
        }
    }
    Ok((w, failed))
}
