#![cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

#[test]
fn tk_skills_lists_personal_and_plugin_skills_read_only() {
    let home = tempfile::tempdir().unwrap();
    let s = home.path().join(".claude/skills/pj-review");
    std::fs::create_dir_all(&s).unwrap();
    std::fs::write(s.join("SKILL.md"), "---\nname: pj-review\ndescription: Review <code>\n---\n").unwrap();
    let bin = home.path().join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::write(bin.join("claude"), "#!/bin/sh\necho '[]'\n").unwrap();
    std::fs::set_permissions(bin.join("claude"), std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap());
    let out = Command::new(env!("CARGO_BIN_EXE_tk"))
        .args(["skills"])
        .env("HOME", home.path())
        .env("PATH", path)
        .env("TK_DATA_DIR", home.path().join("data"))
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(text.contains("pj-review"), "{text}");
    assert!(text.contains("Chi phí nền"), "{text}");
    assert!(home.path().join(".claude/skills/pj-review/SKILL.md").exists(), "không được đụng vào skill");
}
