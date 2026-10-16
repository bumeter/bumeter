//! App file locations. `BUM_DATA_DIR` overrides them (used for tests).
use std::ffi::OsString;
use std::io;
use std::path::PathBuf;

const MAX_SOCKET_PATH: usize = 100;
const DIR_NAME: &str = "bumeter";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    pub data_dir: PathBuf,
}

impl Paths {
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        Self { data_dir: data_dir.into() }
    }

    pub fn resolve() -> Self {
        let default_base = || dirs::data_dir().unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(".local/share"));
        Self::resolve_with(std::env::var_os("BUM_DATA_DIR"), default_base)
    }

    /// `BUM_DATA_DIR` wins; otherwise the default directory (`<data>/bumeter`).
    fn resolve_with(bum: Option<OsString>, default_base: impl FnOnce() -> PathBuf) -> Self {
        if let Some(dir) = bum {
            return Self::new(dir);
        }
        Self::new(default_base().join(DIR_NAME))
    }

    /// Create the data dir, accessible only to the current user.
    pub fn ensure(&self) -> io::Result<()> {
        std::fs::create_dir_all(&self.data_dir)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&self.data_dir, std::fs::Permissions::from_mode(0o700))?;
        }
        Ok(())
    }

    pub fn db(&self) -> PathBuf { self.data_dir.join("bumeter.db") }
    pub fn limits(&self) -> PathBuf { self.data_dir.join("limits.json") }
    /// Tool self-reported quota (e.g. Kiro `/usage`), most recent fetch.
    pub fn tool_quotas(&self) -> PathBuf { self.data_dir.join("tool_quotas.json") }
    pub fn lock(&self) -> PathBuf { self.data_dir.join("host.lock") }
    /// Unix sockets are limited to ~108 bytes (`sun_path`); if the data dir is long, switch to
    /// `$XDG_RUNTIME_DIR` (per-user) with a name hashed from the data dir.
    pub fn socket(&self) -> PathBuf {
        let preferred = self.data_dir.join("host.sock");
        if preferred.as_os_str().len() <= MAX_SOCKET_PATH {
            return preferred;
        }
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.data_dir.hash(&mut h);
        let base = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
        base.join(format!("{DIR_NAME}-{:016x}.sock", h.finish()))
    }
    pub fn config(&self) -> PathBuf { self.data_dir.join("config.toml") }
    pub fn notes(&self) -> PathBuf { self.data_dir.join("notes.json") }
    pub fn pricing(&self) -> PathBuf { self.data_dir.join("pricing.toml") }
}
