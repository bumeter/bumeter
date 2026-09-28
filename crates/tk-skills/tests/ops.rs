#![cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use tk_skills::ops::{Manager, OpRequest};
use tk_skills::plugins::ClaudeCli;

struct Env {
    _dir: tempfile::TempDir,
    root: PathBuf,
    m: Manager,
}

fn skill(dir: &Path, name: &str) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join("SKILL.md"), format!("---\nname: {name}\ndescription: mô tả {name}\n---\n")).unwrap();
}

fn env() -> Env {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    let claude = root.join(".claude");
    skill(&claude.join("skills/pj-review"), "pj-review");
    skill(&claude.join("skills/pj-debug"), "pj-debug");
    skill(&root.join("repo/pj-clarify"), "pj-clarify");
    std::os::unix::fs::symlink(root.join("repo/pj-clarify"), claude.join("skills/pj-clarify")).unwrap();
    skill(&claude.join("skills/synced/uuid/docs"), "docs");
    let bin = root.join("claude");
    std::fs::write(&bin, "#!/bin/sh\necho \"$@\" >> \"$(dirname \"$0\")/calls.log\"\n").unwrap();
    std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
    let m = Manager::new(claude, root.join("data"), ClaudeCli::new(bin));
    Env { _dir: dir, root, m }
}

fn req(op: &str, targets: &[&str]) -> OpRequest {
    OpRequest { op: op.into(), targets: targets.iter().map(|s| s.to_string()).collect(), source: None, tool: None, skill: None }
}

fn exists(p: PathBuf) -> bool {
    std::fs::symlink_metadata(p).is_ok()
}

#[test]
fn disable_moves_to_skills_disabled_and_undo_moves_back() {
    let e = env();
    let plan = e.m.plan(&req("disable", &["pj-debug"])).unwrap();
    assert!(plan.description.contains("skills-disabled"), "{}", plan.description);
    let entry = e.m.apply(&plan).unwrap();
    assert!(!exists(e.root.join(".claude/skills/pj-debug")));
    assert!(exists(e.root.join(".claude/skills-disabled/pj-debug/SKILL.md")));
    e.m.undo(&entry.id).unwrap();
    assert!(exists(e.root.join(".claude/skills/pj-debug/SKILL.md")));
    assert!(e.m.undo(&entry.id).is_err(), "không hoàn tác hai lần");
}

#[test]
fn enable_reverses_disable() {
    let e = env();
    e.m.apply(&e.m.plan(&req("disable", &["pj-debug"])).unwrap()).unwrap();
    e.m.apply(&e.m.plan(&req("enable", &["pj-debug"])).unwrap()).unwrap();
    assert!(exists(e.root.join(".claude/skills/pj-debug/SKILL.md")));
}

#[test]
fn symlinked_skill_only_the_link_moves_never_the_source() {
    let e = env();
    let entry = e.m.apply(&e.m.plan(&req("uninstall", &["pj-clarify"])).unwrap()).unwrap();
    assert!(!exists(e.root.join(".claude/skills/pj-clarify")));
    assert!(exists(e.root.join("repo/pj-clarify/SKILL.md")), "thư mục gốc phải còn nguyên");
    e.m.undo(&entry.id).unwrap();
    let link = e.root.join(".claude/skills/pj-clarify");
    assert!(std::fs::symlink_metadata(&link).unwrap().file_type().is_symlink());
    assert_eq!(std::fs::read_link(&link).unwrap(), e.root.join("repo/pj-clarify"));
}

#[test]
fn uninstall_goes_to_trash_and_can_be_undone() {
    let e = env();
    let entry = e.m.apply(&e.m.plan(&req("uninstall", &["pj-review"])).unwrap()).unwrap();
    assert!(!exists(e.root.join(".claude/skills/pj-review")));
    let trash: Vec<_> = std::fs::read_dir(e.root.join("data/trash")).unwrap().collect();
    assert_eq!(trash.len(), 1);
    e.m.undo(&entry.id).unwrap();
    assert!(exists(e.root.join(".claude/skills/pj-review/SKILL.md")));
}

#[test]
fn install_from_folder_links_it_and_validates() {
    let e = env();
    let src = e.root.join("new/pj-new");
    skill(&src, "pj-new");
    let r = OpRequest { op: "install_dir".into(), targets: vec![], source: Some(src.to_string_lossy().into()), tool: None, skill: None };
    let entry = e.m.apply(&e.m.plan(&r).unwrap()).unwrap();
    assert_eq!(std::fs::read_link(e.root.join(".claude/skills/pj-new")).unwrap(), src);
    e.m.undo(&entry.id).unwrap();
    assert!(!exists(e.root.join(".claude/skills/pj-new")));
    assert!(exists(src.join("SKILL.md")));

    let bad = e.root.join("new/empty");
    std::fs::create_dir_all(&bad).unwrap();
    assert!(e.m.plan(&OpRequest { op: "install_dir".into(), targets: vec![], source: Some(bad.to_string_lossy().into()), tool: None, skill: None }).is_err());
    let dup = e.root.join("new/dup");
    skill(&dup, "pj-review");
    assert!(e.m.plan(&OpRequest { op: "install_dir".into(), targets: vec![], source: Some(dup.to_string_lossy().into()), tool: None, skill: None }).is_err(), "trùng tên");
}

#[test]
fn bulk_disable_rolls_back_everything_if_one_step_fails() {
    let e = env();
    let plan = e.m.plan(&req("disable", &["pj-review", "pj-debug"])).unwrap();
    skill(&e.root.join(".claude/skills-disabled/pj-debug"), "pj-debug"); // xung đột xuất hiện sau khi plan
    assert!(e.m.apply(&plan).is_err());
    assert!(exists(e.root.join(".claude/skills/pj-review/SKILL.md")), "bước đã làm phải được trả lại");
    assert!(exists(e.root.join(".claude/skills/pj-debug/SKILL.md")));
}

#[test]
fn synced_and_unknown_skills_are_refused() {
    let e = env();
    assert!(e.m.plan(&req("disable", &["docs"])).is_err());
    assert!(e.m.plan(&req("disable", &["nope"])).is_err());
}

#[test]
fn plugin_ops_go_through_the_cli_and_undo_runs_the_inverse() {
    let e = env();
    let entry = e.m.apply(&e.m.plan(&req("plugin_disable", &["superpowers@claude-plugins-official"])).unwrap()).unwrap();
    e.m.undo(&entry.id).unwrap();
    let log = std::fs::read_to_string(e.root.join("calls.log")).unwrap();
    assert_eq!(log.lines().collect::<Vec<_>>(), vec![
        "plugin disable superpowers@claude-plugins-official",
        "plugin enable superpowers@claude-plugins-official",
    ]);
}

#[test]
fn journal_lists_operations_newest_first() {
    let e = env();
    e.m.apply(&e.m.plan(&req("disable", &["pj-debug"])).unwrap()).unwrap();
    e.m.apply(&e.m.plan(&req("disable", &["pj-review"])).unwrap()).unwrap();
    let log = e.m.log().unwrap();
    assert_eq!(log.len(), 2);
    assert!(log[0].description.contains("pj-review"));
}

#[test]
fn purge_trash_removes_only_entries_older_than_30_days() {
    let e = env();
    let trash = e.root.join("data/trash");
    std::fs::create_dir_all(trash.join("20260801T000000-old")).unwrap();
    std::fs::create_dir_all(trash.join("20260920T000000-new")).unwrap();
    let now = chrono::DateTime::parse_from_rfc3339("2026-09-24T00:00:00Z").unwrap().to_utc();
    assert_eq!(e.m.purge_trash(now).unwrap(), 1);
    assert!(exists(trash.join("20260920T000000-new")));
    assert!(!exists(trash.join("20260801T000000-old")));
}

#[test]
fn install_from_git_clones_and_rejects_repos_without_skill_md() {
    if std::process::Command::new("git").arg("--version").output().is_err() {
        return;
    }
    let e = env();
    let git = |dir: &Path, args: &[&str]| {
        assert!(std::process::Command::new("git").args(args).current_dir(dir).env("GIT_AUTHOR_NAME", "t").env("GIT_AUTHOR_EMAIL", "t@t").env("GIT_COMMITTER_NAME", "t").env("GIT_COMMITTER_EMAIL", "t@t").output().unwrap().status.success());
    };
    let repo = e.root.join("remote/pj-git");
    skill(&repo, "pj-git");
    git(&repo, &["init", "-q"]);
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "init"]);
    let url = format!("file://{}", repo.display());
    let entry = e.m.apply(&e.m.plan(&OpRequest { op: "install_git".into(), targets: vec![], source: Some(url), tool: None, skill: None }).unwrap()).unwrap();
    assert!(exists(e.root.join(".claude/skills/pj-git/SKILL.md")));
    e.m.undo(&entry.id).unwrap();
    assert!(!exists(e.root.join(".claude/skills/pj-git")));

    let empty = e.root.join("remote/not-a-skill");
    std::fs::create_dir_all(&empty).unwrap();
    std::fs::write(empty.join("README.md"), "x").unwrap();
    git(&empty, &["init", "-q"]);
    git(&empty, &["add", "."]);
    git(&empty, &["commit", "-qm", "init"]);
    let plan = e.m.plan(&OpRequest { op: "install_git".into(), targets: vec![], source: Some(format!("file://{}", empty.display())), tool: None, skill: None }).unwrap();
    assert!(e.m.apply(&plan).is_err());
    assert!(!exists(e.root.join(".claude/skills/not-a-skill")), "clone lỗi phải được dọn");
}

#[test]
fn install_rejects_names_that_escape_the_skills_dir() {
    let e = env();
    for bad in ["../../evil", "/tmp/abs", "a/b", ".hidden", "x\\..\\y"] {
        let src = e.root.join(format!("bad-{}", bad.len()));
        skill(&src, bad);
        let r = OpRequest { op: "install_dir".into(), targets: vec![], source: Some(src.to_string_lossy().into()), tool: None, skill: None };
        assert!(e.m.plan(&r).is_err(), "{bad}");
    }
    for url in ["--upload-pack=touch /tmp/pwned", "-c", "git@host:x/..", "https://host/.."] {
        let r = OpRequest { op: "install_git".into(), targets: vec![], source: Some(url.into()), tool: None, skill: None };
        assert!(e.m.plan(&r).is_err(), "{url}");
    }
}

#[test]
fn partial_undo_restores_what_it_can_and_can_be_finished_later() {
    let e = env();
    let entry = e.m.apply(&e.m.plan(&req("disable", &["pj-review", "pj-debug"])).unwrap()).unwrap();
    skill(&e.root.join(".claude/skills/pj-debug"), "pj-debug"); // người dùng tự tạo lại
    let err = e.m.undo(&entry.id).unwrap_err().to_string();
    assert!(err.contains("pj-debug"), "{err}");
    assert!(exists(e.root.join(".claude/skills/pj-review/SKILL.md")), "skill còn lại vẫn được trả về");
    std::fs::remove_dir_all(e.root.join(".claude/skills/pj-debug")).unwrap();
    e.m.undo(&entry.id).unwrap();
    assert!(exists(e.root.join(".claude/skills/pj-debug/SKILL.md")));
}

/// Repo giả kiểu GitHub: `<base>/acme/skills.git` chứa nhiều skill trong `skills/`.
fn market_repo(root: &Path) -> String {
    let git = |dir: &Path, args: &[&str]| {
        assert!(std::process::Command::new("git").args(args).current_dir(dir).env("GIT_AUTHOR_NAME", "t").env("GIT_AUTHOR_EMAIL", "t@t").env("GIT_COMMITTER_NAME", "t").env("GIT_COMMITTER_EMAIL", "t@t").output().unwrap().status.success());
    };
    let repo = root.join("gh/acme/skills.git");
    skill(&repo.join("skills/code-review"), "code-review");
    std::fs::write(repo.join("skills/code-review/extra.md"), "tài liệu kèm").unwrap();
    skill(&repo.join("skills/other"), "other");
    std::fs::write(repo.join("README.md"), "repo").unwrap();
    git(&repo, &["init", "-q"]);
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "init"]);
    format!("file://{}/", root.join("gh").display())
}

fn market(source: &str, skill: &str) -> OpRequest {
    OpRequest { op: "install_market".into(), targets: vec![], source: Some(source.into()), tool: None, skill: Some(skill.into()) }
}

#[test]
fn install_from_skills_sh_takes_only_that_skill_and_undo_trashes_it() {
    if std::process::Command::new("git").arg("--version").output().is_err() {
        return;
    }
    let e = env();
    let m = e.m.with_git_base(market_repo(&e.root));
    let plan = m.plan(&market("acme/skills", "code-review")).unwrap();
    assert!(plan.description.contains("acme/skills"), "{}", plan.description);
    let entry = m.apply(&plan).unwrap();
    let dst = e.root.join(".claude/skills/code-review");
    assert!(exists(dst.join("SKILL.md")));
    assert!(exists(dst.join("extra.md")), "lấy cả file kèm theo của skill");
    assert!(!exists(dst.join("README.md")) && !exists(dst.join("skills")), "không cài cả repo");
    assert!(!exists(e.root.join(".claude/skills/other")));
    assert_eq!(std::fs::read_dir(e.root.join("data/tmp")).map(|d| d.count()).unwrap_or(0), 0, "dọn thư mục tạm");
    m.undo(&entry.id).unwrap();
    assert!(!exists(dst.clone()));
    assert!(std::fs::read_dir(e.root.join("data/trash")).unwrap().count() >= 1, "gỡ vào thùng rác");
}

#[test]
fn install_from_skills_sh_fails_cleanly_when_skill_is_not_in_repo() {
    if std::process::Command::new("git").arg("--version").output().is_err() {
        return;
    }
    let e = env();
    let m = e.m.with_git_base(market_repo(&e.root));
    let err = m.apply(&m.plan(&market("acme/skills", "missing")).unwrap()).unwrap_err();
    assert!(format!("{err:#}").contains("missing"), "{err:#}");
    assert!(!exists(e.root.join(".claude/skills/missing")));
    assert_eq!(std::fs::read_dir(e.root.join("data/tmp")).map(|d| d.count()).unwrap_or(0), 0);
}

#[test]
fn install_from_skills_sh_validates_before_touching_anything() {
    let e = env();
    for (source, id) in [("acme", "x"), ("acme/skills/../x", "x"), ("-oProxy/x", "x"), ("acme/skills", "../evil"), ("acme/skills", ".hidden")] {
        assert!(e.m.plan(&market(source, id)).is_err(), "{source} {id}");
    }
    let dup = e.m.plan(&market("acme/skills", "pj-review")).unwrap_err();
    assert!(format!("{dup:#}").contains("pj-review"), "trùng tên skill đã có");
    assert!(e.m.plan(&OpRequest { skill: None, ..market("acme/skills", "x") }).is_err(), "thiếu skill id");
}
