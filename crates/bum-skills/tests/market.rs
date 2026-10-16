use std::fs;
use std::path::Path;

use serde_json::json;
use bum_skills::market::{encode_query, find_skill_dir, parse_page, parse_search, valid_mode, valid_source, Hit};

fn skill(dir: &Path, name: &str) {
    fs::create_dir_all(dir).unwrap();
    fs::write(dir.join("SKILL.md"), format!("---\nname: {name}\ndescription: d\n---\nbody")).unwrap();
}

#[test]
fn parses_the_real_search_response() {
    // Trimmed from `GET https://skills.sh/api/search?q=review&limit=3` (2026-09-28).
    let v = json!({"query":"review","searchType":"fuzzy","skills":[
        {"id":"mattpocock/skills/code-review","source":"mattpocock/skills","skillId":"code-review","name":"code-review","installs":625917},
        {"id":"obra/superpowers/requesting-code-review","source":"obra/superpowers","skillId":"requesting-code-review","name":"requesting-code-review","installs":239630}
    ],"count":2});
    let hits = parse_search(&v);
    assert_eq!(hits[0], Hit { id: "mattpocock/skills/code-review".into(), source: "mattpocock/skills".into(), skill_id: "code-review".into(), name: "code-review".into(), installs: 625917 });
    assert_eq!(hits.len(), 2);
    assert_eq!(parse_search(&json!([{"source":"a/b","skillId":"x","name":"x","installs":1}])).len(), 1, "also recognizes the array form");
    assert!(parse_search(&json!({"error":"rate limited"})).is_empty());
}

#[test]
fn leaderboard_modes_and_response_shape() {
    assert!(valid_mode("hot") && valid_mode("trending") && valid_mode("all-time"));
    for bad in ["", "daily", "24h", "../hot", "all_time"] {
        assert!(!valid_mode(bad), "{bad}");
    }
    // Trimmed from `GET https://skills.sh/api/skills/trending/1` (2026-10-17).
    let v = json!({"skills":[
        {"source":"emilkowalski/skills","skillId":"write-swift","name":"write-swift","installs":2066},
        {"source":"cloudflare/skills","skillId":"web-perf","name":"web-perf","installs":1002,"isOfficial":true}
    ],"total":9970,"hasMore":true,"page":1});
    let hits = parse_search(&v);
    assert_eq!(hits.iter().map(|h| h.skill_id.as_str()).collect::<Vec<_>>(), vec!["write-swift", "web-perf"]);
    assert_eq!(hits[0].installs, 2066);
}

#[test]
fn parses_a_leaderboard_page() {
    let v = json!({"skills":[{"source":"emilkowalski/skills","skillId":"write-swift","name":"write-swift","installs":2066}],"total":9970,"hasMore":true,"page":1});
    let p = parse_page(&v);
    assert_eq!(p.hits.len(), 1);
    assert!(p.has_more);
    assert_eq!(p.total, 9970);
    let empty = parse_page(&json!({"error":"rate limited"}));
    assert!(empty.hits.is_empty() && !empty.has_more && empty.total == 0, "a bad response yields an empty page");
}

#[test]
fn drops_entries_with_unsafe_source_or_id() {
    let v = json!({"skills":[
        {"source":"a/b","skillId":"../../etc","name":"x"},
        {"source":"a/b; rm -rf ~","skillId":"ok","name":"x"},
        {"source":"-oProxy/x","skillId":"ok","name":"x"},
        {"skillId":"ok","name":"no-source"},
        {"source":"acme/skills","skillId":"good_one.v2","name":"<b>good</b>"}
    ]});
    let hits = parse_search(&v);
    assert_eq!(hits.iter().map(|h| h.skill_id.as_str()).collect::<Vec<_>>(), vec!["good_one.v2"]);
    assert_eq!(hits[0].name, "<b>good</b>", "name kept as-is, UI renders it as text");
    assert_eq!(hits[0].id, "acme/skills/good_one.v2", "when id is missing, compose it from source + skillId");
}

#[test]
fn source_and_query_validation() {
    assert!(valid_source("vercel-labs/agent-skills"));
    assert!(valid_source("a.b/c_d"));
    for bad in ["a", "a/b/c", "/b", "a/", "-a/b", "a/-b", "a b/c", "a/b.git?x", "../x"] {
        assert!(!valid_source(bad), "{bad}");
    }
    assert_eq!(encode_query("code review & progress"), "code%20review%20%26%20progress");
}

#[test]
fn finds_the_skill_directory_like_skills_manager() {
    let t = tempfile::tempdir().unwrap();
    let r = t.path();
    skill(&r.join("direct"), "direct");
    skill(&r.join("skills/in-skills"), "in-skills");
    skill(&r.join(".agents/skills/agents"), "agents");
    skill(&r.join(".cursor/skills/agents"), "agents");
    skill(&r.join("cat/deep/nested"), "nested");
    skill(&r.join("cat/renamed-dir"), "by-name");
    assert_eq!(find_skill_dir(r, "direct").unwrap(), r.join("direct"));
    assert_eq!(find_skill_dir(r, "in-skills").unwrap(), r.join("skills/in-skills"));
    assert_eq!(find_skill_dir(r, "agents").unwrap(), r.join(".agents/skills/agents"), "prefer .agents/skills over each tool's own copy");
    assert_eq!(find_skill_dir(r, "nested").unwrap(), r.join("cat/deep/nested"));
    assert_eq!(find_skill_dir(r, "by-name").unwrap(), r.join("cat/renamed-dir"), "match by name in SKILL.md");
}

#[test]
fn missing_skill_is_an_error_not_the_whole_repo() {
    let t = tempfile::tempdir().unwrap();
    skill(t.path(), "root-skill");
    fs::create_dir_all(t.path().join("skills/empty")).unwrap();
    let e = find_skill_dir(t.path(), "wanted").unwrap_err().to_string();
    assert!(e.contains("wanted"), "{e}");
    assert!(find_skill_dir(t.path(), "empty").is_err(), "directory with the same name but no SKILL.md");
}

#[cfg(unix)]
#[test]
fn does_not_follow_symlinks_out_of_the_repo() {
    let outside = tempfile::tempdir().unwrap();
    skill(&outside.path().join("secret"), "secret");
    let t = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(outside.path().join("secret"), t.path().join("secret")).unwrap();
    fs::create_dir_all(t.path().join("skills")).unwrap();
    std::os::unix::fs::symlink(outside.path(), t.path().join("skills/link")).unwrap();
    assert!(find_skill_dir(t.path(), "secret").is_err());
}
