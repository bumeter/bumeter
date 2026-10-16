use chrono::NaiveDate;
use tk_adapters::kiro::usage::{parse, run_with, KiroUsage};

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
    assert_eq!(parse("Estimated Usage | resets on 2026-13-45 | X\nCredits (1 of 10 covered)"), None, "ngày sai");
    assert_eq!(parse("Estimated Usage | resets on 2026-11-01 | X\nCredits (abc of 10)"), None, "số sai");
    assert_eq!(parse("Estimated Usage | resets on 2026-11-01 | X\nCredits (5 of 0 covered)"), None, "hạn mức 0");
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
    assert!(t.elapsed() < Duration::from_secs(3), "hết giờ thì bỏ, không chờ");
    assert_eq!(run_with(&dir.path().join("nope"), Duration::from_secs(1)), None);
    assert_eq!(run_with(&script("bad", "echo lỗi; exit 1"), Duration::from_secs(5)), None);
}
