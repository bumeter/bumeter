//! Vị trí file của app. `BUM_DATA_DIR` (hoặc `TK_DATA_DIR` cũ) ghi đè (dùng cho test).
use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

const MAX_SOCKET_PATH: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    pub data_dir: PathBuf,
    /// Đang dùng thư mục `thongke` cũ (không chuyển được): giữ tên file cũ để app vẫn chạy.
    legacy: bool,
}

const NEW_DIR: &str = "bumeter";
const OLD_DIR: &str = "thongke";

/// Kết quả chuyển dữ liệu `thongke` → `bumeter`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MigrateOutcome {
    AlreadyNew,
    Moved,
    NothingToMove,
    /// Không đổi tên được thư mục cũ (lý do kèm theo): tiếp tục dùng thư mục cũ.
    KeptLegacy(String),
}

/// Chuyển `base/thongke` → `base/bumeter` một lần. Không bao giờ đụng thư mục cũ nếu đã có thư mục mới.
/// Đổi tên thư mục giữ nguyên fd đang mở (app cũ còn chạy), nên không làm hỏng DB.
/// Nếu lần trước bị gián đoạn (thư mục đã đổi, file db chưa) thì hoàn tất ở lần gọi này.
pub fn migrate_legacy(base: &Path) -> anyhow::Result<MigrateOutcome> {
    migrate_with(base, &|a, b| std::fs::rename(a, b))
}

type RenameFn<'a> = &'a dyn Fn(&Path, &Path) -> io::Result<()>;

fn migrate_with(base: &Path, rename: RenameFn) -> anyhow::Result<MigrateOutcome> {
    let new = base.join(NEW_DIR);
    let old = base.join(OLD_DIR);
    if new.exists() {
        // Còn db cũ mà chưa có db mới: lần chuyển trước dở dang, làm nốt (không bao giờ mở db rỗng bên cạnh db cũ).
        if new.join("thongke.db").exists() && !new.join("bumeter.db").exists() {
            return Ok(match move_db_files(&new, rename) {
                Ok(()) => MigrateOutcome::Moved,
                Err(e) => MigrateOutcome::KeptLegacy(e.to_string()),
            });
        }
        return Ok(MigrateOutcome::AlreadyNew);
    }
    if !old.exists() {
        return Ok(MigrateOutcome::NothingToMove);
    }
    if let Err(e) = rename(&old, &new) {
        return Ok(MigrateOutcome::KeptLegacy(e.to_string()));
    }
    Ok(match move_db_files(&new, rename) {
        Ok(()) => MigrateOutcome::Moved,
        Err(e) => MigrateOutcome::KeptLegacy(e.to_string()),
    })
}

/// Đổi `thongke.db*` → `bumeter.db*`: `-wal`, `-shm` trước, db chính sau cùng (có `bumeter.db` thì chắc chắn đủ file đi kèm).
/// Lỗi thì trả lại các file đã đổi để còn nguyên bộ tên cũ.
fn move_db_files(dir: &Path, rename: RenameFn) -> io::Result<()> {
    let mut done: Vec<(PathBuf, PathBuf)> = Vec::new();
    for suffix in ["-wal", "-shm", ""] {
        let from = dir.join(format!("thongke.db{suffix}"));
        if !from.exists() {
            continue;
        }
        let to = dir.join(format!("bumeter.db{suffix}"));
        if let Err(e) = rename(&from, &to) {
            for (f, t) in done.iter().rev() {
                let _ = std::fs::rename(t, f);
            }
            return Err(e);
        }
        done.push((from, to));
    }
    Ok(())
}

impl Paths {
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        Self { data_dir: data_dir.into(), legacy: false }
    }

    pub fn resolve() -> Self {
        let default_base = || dirs::data_dir().unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(".local/share"));
        Self::resolve_with(std::env::var_os("BUM_DATA_DIR"), std::env::var_os("TK_DATA_DIR"), default_base)
    }

    /// `BUM_DATA_DIR` thắng `TK_DATA_DIR`; chỉ khi không đặt biến nào mới dùng thư mục mặc định (và chuyển dữ liệu cũ).
    fn resolve_with(bum: Option<OsString>, tk: Option<OsString>, default_base: impl FnOnce() -> PathBuf) -> Self {
        if let Some(dir) = bum.or(tk) {
            return Self::new(dir);
        }
        let base = default_base();
        match migrate_legacy(&base) {
            Ok(MigrateOutcome::KeptLegacy(reason)) => {
                eprintln!("bumeter: không chuyển được dữ liệu cũ ({reason}); tiếp tục dùng tên file cũ");
                // Thư mục đã đổi tên (chỉ file db chưa) thì ở thư mục mới, nếu không thì ở thư mục cũ.
                let new = base.join(NEW_DIR);
                Self { data_dir: if new.exists() { new } else { base.join(OLD_DIR) }, legacy: true }
            }
            Ok(_) => Self::new(base.join(NEW_DIR)),
            Err(e) => {
                eprintln!("bumeter: lỗi khi chuyển dữ liệu cũ: {e}");
                Self { data_dir: base.join(OLD_DIR), legacy: true }
            }
        }
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

    pub fn db(&self) -> PathBuf { self.data_dir.join(if self.legacy { "thongke.db" } else { "bumeter.db" }) }
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
        base.join(format!("{}-{:016x}.sock", if self.legacy { OLD_DIR } else { NEW_DIR }, h.finish()))
    }
    pub fn config(&self) -> PathBuf { self.data_dir.join("config.toml") }
    pub fn notes(&self) -> PathBuf { self.data_dir.join("notes.json") }
    pub fn pricing(&self) -> PathBuf { self.data_dir.join("pricing.toml") }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(p: &Path) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, b"x").unwrap();
    }

    #[test]
    fn only_legacy_is_moved_with_db_files_renamed() {
        let t = tempfile::tempdir().unwrap();
        let old = t.path().join("thongke");
        for f in ["thongke.db", "thongke.db-wal", "thongke.db-shm", "config.toml"] {
            touch(&old.join(f));
        }
        assert_eq!(migrate_legacy(t.path()).unwrap(), MigrateOutcome::Moved);
        let new = t.path().join("bumeter");
        assert!(!old.exists());
        for f in ["bumeter.db", "bumeter.db-wal", "bumeter.db-shm", "config.toml"] {
            assert!(new.join(f).exists(), "{f}");
        }
        assert!(!new.join("thongke.db").exists() && !new.join("thongke.db-wal").exists());
        // Chạy lại: đã có thư mục mới.
        assert_eq!(migrate_legacy(t.path()).unwrap(), MigrateOutcome::AlreadyNew);
    }

    #[test]
    fn both_present_leaves_legacy_untouched() {
        let t = tempfile::tempdir().unwrap();
        touch(&t.path().join("thongke/thongke.db"));
        touch(&t.path().join("bumeter/bumeter.db"));
        assert_eq!(migrate_legacy(t.path()).unwrap(), MigrateOutcome::AlreadyNew);
        assert!(t.path().join("thongke/thongke.db").exists());
        assert!(t.path().join("bumeter/bumeter.db").exists());
    }

    #[test]
    fn nothing_to_move() {
        let t = tempfile::tempdir().unwrap();
        assert_eq!(migrate_legacy(t.path()).unwrap(), MigrateOutcome::NothingToMove);
    }

    #[cfg(unix)]
    #[test]
    fn kept_legacy_when_rename_fails_and_paths_keep_working() {
        use std::os::unix::fs::PermissionsExt;
        let t = tempfile::tempdir().unwrap();
        let base = t.path().join("share");
        touch(&base.join("thongke/thongke.db"));
        std::fs::set_permissions(&base, std::fs::Permissions::from_mode(0o555)).unwrap();
        // root bỏ qua quyền ghi: không mô phỏng được.
        let can_write = std::fs::create_dir(base.join("probe")).is_ok();
        if can_write {
            std::fs::remove_dir(base.join("probe")).unwrap();
        } else {
            assert!(matches!(migrate_legacy(&base).unwrap(), MigrateOutcome::KeptLegacy(_)));
            let p = Paths::resolve_with(None, None, || base.clone());
            assert_eq!(p.data_dir, base.join("thongke"));
            assert_eq!(p.db(), base.join("thongke/thongke.db"));
            assert!(p.db().exists());
            assert!(p.socket().to_string_lossy().contains("host.sock") || p.socket().to_string_lossy().contains("thongke-"));
        }
        std::fs::set_permissions(&base, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[test]
    fn resolve_moves_default_dir_and_uses_new_names() {
        let t = tempfile::tempdir().unwrap();
        touch(&t.path().join("thongke/thongke.db"));
        let p = Paths::resolve_with(None, None, || t.path().to_path_buf());
        assert_eq!(p.data_dir, t.path().join("bumeter"));
        assert_eq!(p.db(), t.path().join("bumeter/bumeter.db"));
        assert!(p.db().exists());
    }

    #[test]
    fn bum_data_dir_beats_tk_data_dir_and_skips_migration() {
        let t = tempfile::tempdir().unwrap();
        touch(&t.path().join("thongke/thongke.db"));
        let p = Paths::resolve_with(Some("/b".into()), Some("/t".into()), || t.path().to_path_buf());
        assert_eq!(p.data_dir, PathBuf::from("/b"));
        let p = Paths::resolve_with(None, Some("/t".into()), || t.path().to_path_buf());
        assert_eq!(p.data_dir, PathBuf::from("/t"));
        assert!(t.path().join("thongke").exists(), "không chuyển khi đặt biến môi trường");
    }

    fn fail_on(name: &'static str) -> impl Fn(&Path, &Path) -> io::Result<()> {
        move |a, b| {
            if b.file_name().unwrap() == name {
                Err(io::Error::new(io::ErrorKind::PermissionDenied, "giả lập lỗi"))
            } else {
                std::fs::rename(a, b)
            }
        }
    }

    fn legacy_dir(base: &Path) {
        for f in ["thongke.db", "thongke.db-wal", "thongke.db-shm"] {
            touch(&base.join("thongke").join(f));
        }
    }

    #[test]
    fn main_db_rename_failure_still_finds_old_data_next_launch() {
        let t = tempfile::tempdir().unwrap();
        legacy_dir(t.path());
        let r = migrate_with(t.path(), &fail_on("bumeter.db")).unwrap();
        assert!(matches!(r, MigrateOutcome::KeptLegacy(_)));
        // Hoàn tác: đủ bộ tên cũ, không có bumeter.db* nào.
        let new = t.path().join("bumeter");
        for f in ["thongke.db", "thongke.db-wal", "thongke.db-shm"] {
            assert!(new.join(f).exists(), "{f}");
        }
        assert!(!new.join("bumeter.db").exists() && !new.join("bumeter.db-wal").exists());
        // Lần mở sau (lần này đổi tên được): hoàn tất, db thật được dùng, không mở db rỗng.
        let p = Paths::resolve_with(None, None, || t.path().to_path_buf());
        assert_eq!(p.db(), new.join("bumeter.db"));
        assert!(p.db().exists());
        assert!(new.join("bumeter.db-wal").exists() && !new.join("thongke.db").exists());
    }

    #[test]
    fn wal_or_shm_failure_leaves_consistent_set() {
        for failing in ["bumeter.db-wal", "bumeter.db-shm"] {
            let t = tempfile::tempdir().unwrap();
            legacy_dir(t.path());
            assert!(matches!(migrate_with(t.path(), &fail_on(failing)).unwrap(), MigrateOutcome::KeptLegacy(_)));
            let new = t.path().join("bumeter");
            for f in ["thongke.db", "thongke.db-wal", "thongke.db-shm"] {
                assert!(new.join(f).exists(), "{failing}: {f}");
            }
            assert!(!new.join("bumeter.db").exists() && !new.join("bumeter.db-wal").exists() && !new.join("bumeter.db-shm").exists());
        }
    }

    #[test]
    fn interrupted_migration_is_completed_next_resolve() {
        let t = tempfile::tempdir().unwrap();
        // Thư mục đã đổi, file chưa (và -wal đã sang tên mới: dở giữa chừng).
        touch(&t.path().join("bumeter/thongke.db"));
        touch(&t.path().join("bumeter/thongke.db-shm"));
        touch(&t.path().join("bumeter/bumeter.db-wal"));
        let p = Paths::resolve_with(None, None, || t.path().to_path_buf());
        let new = t.path().join("bumeter");
        assert_eq!(p.db(), new.join("bumeter.db"));
        for f in ["bumeter.db", "bumeter.db-wal", "bumeter.db-shm"] {
            assert!(new.join(f).exists(), "{f}");
        }
        assert!(!new.join("thongke.db").exists());
    }

    #[test]
    fn interrupted_and_failing_again_keeps_old_db_name() {
        let t = tempfile::tempdir().unwrap();
        touch(&t.path().join("bumeter/thongke.db"));
        assert!(matches!(migrate_with(t.path(), &fail_on("bumeter.db")).unwrap(), MigrateOutcome::KeptLegacy(_)));
        assert!(t.path().join("bumeter/thongke.db").exists());
    }

    #[test]
    fn long_socket_path_uses_bumeter_prefix() {
        let p = Paths::new(format!("/{}", "a".repeat(120)));
        assert!(p.socket().file_name().unwrap().to_string_lossy().starts_with("bumeter-"));
        assert_eq!(p.db().file_name().unwrap(), "bumeter.db");
    }
}
