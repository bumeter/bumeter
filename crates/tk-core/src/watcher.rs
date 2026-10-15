//! Theo dõi thay đổi file bằng sự kiện hệ điều hành (inotify / FSEvents / ReadDirectoryChangesW).
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use crate::host::HostMsg;

/// Chỉ chuyển tiếp file `.jsonl` và `limits_file` (bỏ qua ghi DB/WAL trong data dir).
/// Thư mục không theo dõi được (hết inotify watch, thiếu quyền) được trả về để báo, và
/// sẽ chỉ được cập nhật qua lần quét đối chiếu định kỳ.
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
            eprintln!("bumeter: không theo dõi được {}: {e} (sẽ dựa vào quét định kỳ)", d.display());
            failed.push(d.clone());
        }
    }
    Ok((w, failed))
}
