//! Ensure only one host writes the DB and watches the logs.
use std::fs::{File, OpenOptions, TryLockError};
use std::io;
use std::path::Path;

pub struct HostLock {
    _file: File,
}

impl HostLock {
    /// `Ok(None)` if another process already holds the lock.
    pub fn try_acquire(path: &Path) -> io::Result<Option<Self>> {
        let file = OpenOptions::new().create(true).write(true).truncate(false).open(path)?;
        match file.try_lock() {
            Ok(()) => Ok(Some(Self { _file: file })),
            Err(TryLockError::WouldBlock) => Ok(None),
            Err(TryLockError::Error(e)) => Err(e),
        }
    }
}
