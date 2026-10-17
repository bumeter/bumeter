//! Watch for file changes via OS events (inotify / FSEvents / ReadDirectoryChangesW).
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use crate::host::HostMsg;
use crate::source::WatchTarget;

/// Only forward `.jsonl`/`.json` files, `limits_file` and the files a target lists (ignore our own DB/WAL writes in the data dir).
/// Directories that cannot be watched (inotify watch limit reached, missing permissions) are returned so they can be reported, and
/// will only be updated by the periodic reconciliation scan.
pub fn spawn_watcher(targets: &[WatchTarget], limits_file: &Path, tx: Sender<HostMsg>) -> notify::Result<(RecommendedWatcher, Vec<PathBuf>)> {
    let limits_file = limits_file.to_path_buf();
    let listed: Vec<PathBuf> = targets.iter().flat_map(|t| t.files.iter().cloned()).collect();
    let mut w = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        let Ok(ev) = res else { return };
        if !matches!(ev.kind, EventKind::Create(_) | EventKind::Modify(_)) {
            return;
        }
        let paths: Vec<PathBuf> = ev
            .paths
            .into_iter()
            .filter(|p| p.extension().is_some_and(|e| e == "jsonl" || e == "json") || *p == limits_file || listed.contains(p))
            .collect();
        if !paths.is_empty() {
            let _ = tx.send(HostMsg::FsChanged(paths));
        }
    })?;
    let mut failed = Vec::new();
    for t in targets.iter().filter(|t| t.path.exists()) {
        let mode = if t.recursive { RecursiveMode::Recursive } else { RecursiveMode::NonRecursive };
        if let Err(e) = w.watch(&t.path, mode) {
            eprintln!("bumeter: cannot watch {}: {e} (will rely on periodic scan)", t.path.display());
            failed.push(t.path.clone());
        }
    }
    Ok((w, failed))
}
