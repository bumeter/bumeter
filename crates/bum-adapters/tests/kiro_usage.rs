use chrono::NaiveDate;
use bum_adapters::kiro::usage::{fetch_with, logged_in_with, parse, run_with, KiroUsage};

const SAMPLE: &str = "\u{1b}[1mEstimated Usage\u{1b}[0m | resets on 2026-11-01 | \u{1b}[38;5;141mKIRO PRO\u{1b}[0m\n\
Credits (270.69 of 1000 covered in plan)\n\
\u{1b}[32m████████████████████\u{1b}[0m 27%\n\
\n\
Overages: Disabled (managed by your organization)\n";

fn expected() -> KiroUsage {
    KiroUsage { used: 270.69, limit: 1000.0, resets_on: NaiveDate::from_ymd_opt(2026, 11, 1).unwrap(), plan: "KIRO PRO".into() }
}

#[test]
fn parses_usage_output_with_ansi_codes() {
    assert_eq!(parse(SAMPLE), Some(expected()));
}

#[test]
fn parse_is_tolerant_of_case_spacing_and_thousands_separators() {
    let s = "estimated usage|Resets On  2026-11-01|KIRO POWER\n  credits ( 1,250.5  of  2,000 covered in plan )\n";
    assert_eq!(parse(s), Some(KiroUsage { used: 1250.5, limit: 2000.0, resets_on: NaiveDate::from_ymd_opt(2026, 11, 1).unwrap(), plan: "KIRO POWER".into() }));
}

#[test]
fn broken_output_is_none() {
    assert_eq!(parse(""), None);
    assert_eq!(parse("error: not logged in"), None);
    assert_eq!(parse("Estimated Usage | resets on 2026-13-45 | X\nCredits (1 of 10 covered)"), None, "bad date");
    assert_eq!(parse("Estimated Usage | resets on 2026-11-01 | X\nCredits (abc of 10)"), None, "bad number");
    assert_eq!(parse("Estimated Usage | resets on 2026-11-01 | X\nCredits (5 of 0 covered)"), None, "zero limit");
}

#[cfg(unix)]
#[test]
fn run_with_reads_stdout_and_handles_missing_or_slow_command() {
    use std::os::unix::fs::PermissionsExt;
    use std::time::Duration;
    let dir = tempfile::tempdir().unwrap();
    let script = |name: &str, body: &str| {
        let p = dir.path().join(name);
        std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p
    };
    let data = dir.path().join("out.txt");
    std::fs::write(&data, SAMPLE).unwrap();
    let ok = script("ok", &format!("[ \"$1 $2 $3\" = 'chat --no-interactive /usage' ] && cat '{}'", data.display()));
    assert_eq!(run_with(&ok, Duration::from_secs(5)), Some(expected()));
    let slow = script("slow", "sleep 5");
    let t = std::time::Instant::now();
    assert_eq!(run_with(&slow, Duration::from_millis(300)), None);
    assert!(t.elapsed() < Duration::from_secs(3), "give up on timeout, do not wait");
    assert_eq!(run_with(&dir.path().join("nope"), Duration::from_secs(1)), None);
    assert_eq!(run_with(&script("bad", "echo error; exit 1"), Duration::from_secs(5)), None);
}

#[test]
fn store_quotas_merges_and_skips_empty() {
    use bum_core::meter::read_quotas;
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("tool_quotas.json");
    assert!(!bum_adapters::store_quotas(&p, vec![]).unwrap());
    assert!(!p.exists());
    let q = |n: i64| expected().into_quota(n);
    assert!(bum_adapters::store_quotas(&p, vec![("other".into(), q(1)), ("kiro".into(), q(1))]).unwrap());
    assert!(bum_adapters::store_quotas(&p, vec![("kiro".into(), q(2))]).unwrap());
    let all = read_quotas(&p);
    assert_eq!((all["other"].fetched_at, all["kiro"].fetched_at), (1, 2));
}

/// Ask for `/usage` only when `kiro-cli whoami` reports a login; otherwise run nothing else.
#[cfg(unix)]
#[test]
fn usage_is_fetched_only_when_logged_in() {
    use std::os::unix::fs::PermissionsExt;
    use std::time::Duration;
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("out.txt");
    std::fs::write(&data, SAMPLE).unwrap();
    let asked = dir.path().join("asked-usage");
    // Fake kiro-cli: `whoami` follows the scenario; `chat … /usage` drops a marker then prints SAMPLE.
    let fake = |name: &str, whoami: &str| {
        let p = dir.path().join(name);
        std::fs::write(&p, format!(
            "#!/bin/sh\ncase \"$1\" in\n  whoami) {whoami} ;;\n  chat) touch '{}'; cat '{}' ;;\nesac\n",
            asked.display(), data.display())).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p
    };
    let t = Duration::from_secs(5);

    let ok = fake("in", "echo 'Logged in with IAM Identity Center (https://example.awsapps.com/start)'; echo 'Email: a@b.c'");
    assert!(logged_in_with(&ok, t));
    assert_eq!(fetch_with(&ok, t), Some(expected()));
    assert!(asked.exists());
    std::fs::remove_file(&asked).unwrap();

    for (name, whoami) in [("out", "echo 'Not logged in'; exit 1"), ("out0", "echo 'You are not logged in, run kiro-cli login'"), ("err", "exit 2"), ("slow", "sleep 5")] {
        let bin = fake(name, whoami);
        assert!(!logged_in_with(&bin, Duration::from_millis(500)), "{name}");
        assert_eq!(fetch_with(&bin, Duration::from_millis(500)), None, "{name}");
        assert!(!asked.exists(), "{name}: /usage must not run when not logged in");
    }
    assert!(!logged_in_with(&dir.path().join("nope"), t), "kiro-cli not installed");
}

/// The tray shows only tools whose log directory exists on this machine (no Kiro row when Kiro is not installed).
#[test]
fn installed_keeps_only_tools_whose_log_dir_exists() {
    let dir = tempfile::tempdir().unwrap();
    let claude = dir.path().join("claude/projects");
    std::fs::create_dir_all(&claude).unwrap();
    let kiro = dir.path().join("kiro/sessions/cli");
    let ids = |v: Vec<bum_core::source::SourceInfo>| v.into_iter().map(|i| i.id).collect::<Vec<_>>();
    assert_eq!(ids(bum_adapters::installed(&claude, &kiro)), ["claude_code"]);
    std::fs::create_dir_all(&kiro).unwrap();
    assert_eq!(ids(bum_adapters::installed(&claude, &kiro)), ["claude_code", "kiro"]);
}
