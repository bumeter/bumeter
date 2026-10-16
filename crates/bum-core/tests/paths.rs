use bum_core::paths::Paths;

#[test]
fn all_files_live_under_the_data_dir() {
    let p = Paths::new("/tmp/tk-test");
    for f in [p.db(), p.limits(), p.lock(), p.socket(), p.config(), p.pricing(), p.notes()] {
        assert!(f.starts_with("/tmp/tk-test"), "{f:?}");
    }
}

#[test]
fn ensure_creates_private_dir() {
    let dir = tempfile::tempdir().unwrap();
    let p = Paths::new(dir.path().join("d"));
    p.ensure().unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(&p.data_dir).unwrap().permissions().mode() & 0o777, 0o700);
    }
}

#[test]
fn socket_path_fits_unix_sun_path_even_for_long_data_dirs() {
    let long = format!("/tmp/{}", "d".repeat(150));
    let p = Paths::new(&long);
    assert!(p.socket().as_os_str().len() <= 100, "{:?}", p.socket());
    let other = Paths::new(format!("/tmp/{}", "e".repeat(150)));
    assert_ne!(p.socket(), other.socket(), "each data dir gets its own socket");
    assert_eq!(Paths::new("/tmp/short").socket(), std::path::PathBuf::from("/tmp/short/host.sock"));
}
