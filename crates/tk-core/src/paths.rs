//! Vị trí file của app. `TK_DATA_DIR` ghi đè (dùng cho test).
use std::io;
use std::path::PathBuf;

const MAX_SOCKET_PATH: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    pub data_dir: PathBuf,
}

impl Paths {
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        Self { data_dir: data_dir.into() }
    }

    pub fn resolve() -> Self {
        if let Some(dir) = std::env::var_os("TK_DATA_DIR") {
            return Self::new(dir);
        }
        let base = dirs::data_dir().unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(".local/share"));
        Self::new(base.join("thongke"))
    }

    /// Tạo data dir, chỉ user hiện tại được truy cập.
    pub fn ensure(&self) -> io::Result<()> {
        std::fs::create_dir_all(&self.data_dir)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&self.data_dir, std::fs::Permissions::from_mode(0o700))?;
        }
        Ok(())
    }

    pub fn db(&self) -> PathBuf { self.data_dir.join("thongke.db") }
    pub fn limits(&self) -> PathBuf { self.data_dir.join("limits.json") }
    pub fn lock(&self) -> PathBuf { self.data_dir.join("host.lock") }
    /// Socket Unix bị giới hạn ~108 byte (`sun_path`); data dir dài thì chuyển sang
    /// `$XDG_RUNTIME_DIR` (riêng từng user) với tên băm theo data dir.
    pub fn socket(&self) -> PathBuf {
        let preferred = self.data_dir.join("host.sock");
        if preferred.as_os_str().len() <= MAX_SOCKET_PATH {
            return preferred;
        }
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.data_dir.hash(&mut h);
        let base = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
        base.join(format!("thongke-{:016x}.sock", h.finish()))
    }
    pub fn config(&self) -> PathBuf { self.data_dir.join("config.toml") }
    pub fn notes(&self) -> PathBuf { self.data_dir.join("notes.json") }
    pub fn pricing(&self) -> PathBuf { self.data_dir.join("pricing.toml") }
}
