//! Read-only safety scan of installed skills: text that hijacks the model, commands that reach
//! outside the project, credentials, and skills that duplicate each other.
//!
//! Everything here is a heuristic over the text of `SKILL.md`. A match is a reason to look at that
//! line, not a verdict, so every finding carries the match and a one-line reason. Nothing is ever
//! changed or moved by this module.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::catalog::{Source, SkillEntry};

/// Ignore descriptions shorter than this when looking for duplicate skills: "Run tests" says nothing.
const MIN_DESC_CHARS: usize = 20;
/// A skill with less than this in its instructions cannot really duplicate another one.
const MIN_BODY_CHARS: usize = 40;
/// A run of base64-ish characters this long on one line is machine data, not prose.
const BLOB_CHARS: usize = 120;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    High,
    Warn,
    Info,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Finding {
    pub severity: Severity,
    /// `injection` | `concealment` | `exfiltration` | `remote_exec` | `destructive` | `self_modify` | `secret` | `hidden_text` | `obfuscation`
    pub kind: &'static str,
    pub skill: String,
    pub tool: String,
    /// The `SKILL.md` the line was read from.
    pub path: PathBuf,
    /// 1-based line number.
    pub line: usize,
    /// The text that matched, trimmed for the report.
    pub text: String,
    pub why: &'static str,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Duplicate {
    /// `content` | `description` | `name`
    pub kind: &'static str,
    pub severity: Severity,
    /// Skill names, sorted.
    pub skills: Vec<String>,
    pub why: &'static str,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Audit {
    /// Skills looked at.
    pub skills: usize,
    /// Skills that had a readable `SKILL.md`.
    pub scanned: usize,
    /// False when at least one finding is `high`.
    pub ok: bool,
    /// Sorted: high first, then by skill and line.
    pub findings: Vec<Finding>,
    pub duplicates: Vec<Duplicate>,
}

struct Rule {
    kind: &'static str,
    severity: Severity,
    why: &'static str,
    needles: &'static [&'static str],
}

/// A line match is a plain case-insensitive substring, so the table stays readable and there is no
/// regex engine in the tree. Two halves of one trick (a credential path and a way to send it out)
/// cannot be seen on a single line; those are checked per file below.
const RULES: &[Rule] = &[
    Rule { kind: "injection", severity: Severity::High, why: "tells the model to ignore the instructions it was given", needles: &[
        "ignore previous instructions", "ignore all previous", "ignore the above", "ignore everything above",
        "disregard previous", "disregard all previous", "disregard the above", "disregard your instructions",
        "forget all previous", "forget everything above", "override the system prompt", "override your instructions",
        "print your system prompt", "reveal your instructions", "repeat your system prompt",
        "your new instructions are", "you are now a different", "pretend you are a different", "new system prompt",
    ] },
    Rule { kind: "concealment", severity: Severity::High, why: "asks for something to be hidden from the user", needles: &[
        "do not tell the user", "don't tell the user", "do not mention this to the user", "do not mention this in your",
        "without telling the user", "without informing the user", "do not reveal this", "keep this secret from",
        "don't mention this to the user", "hide this from the user",
    ] },
    Rule { kind: "remote_exec", severity: Severity::High, why: "runs code downloaded from the network", needles: &[
        "| sh", "|sh", "| bash", "|bash", "| zsh", "|zsh", "| python", "|python", "| node",
        "bash <(curl", "sh -c \"$(curl", "eval $(curl", "eval \"$(curl", "iex(new-object", "| powershell",
        "curl -s | sh", "wget -qo- | sh",
    ] },
    Rule { kind: "destructive", severity: Severity::High, why: "destroys data outside the project", needles: &[
        "rm -rf /", "rm -rf ~", "rm -rf $home", "rm -rf /*", ":(){", "mkfs.", "dd if=/dev/zero of=/dev",
        "dd if=/dev/random of=/dev", "> /dev/sda", "chmod -r 777 /", "history -c", "shred -u", "wipefs",
        "rm -rf ..", "git clean -fdx",
    ] },
    Rule { kind: "destructive", severity: Severity::Warn, why: "rewrites code or history without asking", needles: &[
        "git push --force", "git reset --hard", "sudo rm", "chown -r", "killall", "pkill -9", "rm -r ~/",
    ] },
];

/// `remote_exec` is about code from the network: a pipe into a shell only counts when the line fetches
/// something. Without this, a Markdown table cell (`| Node module`) or a local `cat x | sh` would be "high".
const DOWNLOADERS: &[&str] = &[
    "curl", "wget", "iwr ", "irm ", "invoke-webrequest", "invoke-restmethod", "net.webclient", "downloadstring", "http://", "https://",
];

/// Words that turn a dangerous pattern into a warning about it: "never run …", "… will destroy the workspace".
const WARNING_CUES: &[&str] = &[
    "never", "don't", "do not", "must not", "should not", "avoid", "will destroy", "would destroy", "destroys", "dangerous",
    "warning", "beware", "careful",
];

/// Rules about wording rather than commands: there, a quoted phrase is being cited, not said.
const PROSE_KINDS: &[&str] = &["injection", "concealment"];

/// `needle` as a whole word in `hay`, so "whenever" does not count as "never".
fn has_word(hay: &str, needle: &str) -> bool {
    hay.match_indices(needle).any(|(i, _)| {
        let before = hay[..i].chars().next_back().is_none_or(|c| !c.is_alphanumeric());
        let after = hay[i + needle.len()..].chars().next().is_none_or(|c| !c.is_alphanumeric());
        before && after
    })
}

/// The line names the pattern to warn against it rather than to ask for it: a warning word outside the
/// match, or, for prose rules, the phrase is quoted. A malicious skill can dress a line up the same way,
/// so this only lowers a finding to `warn`; it is still reported with the line it matched.
fn mentioned_as_warning(lower: &str, needle: &str, kind: &str) -> bool {
    let rest = lower.replacen(needle, " ", 1);
    if WARNING_CUES.iter().any(|c| has_word(&rest, c)) {
        return true;
    }
    PROSE_KINDS.contains(&kind)
        && lower.match_indices(needle).any(|(i, _)| lower[..i].chars().next_back().is_some_and(|c| matches!(c, '"' | '\'' | '`' | '“' | '‘' | '«')))
}

/// Files that hold credentials. Reading one is fine; reading one *and* sending something out is not.
const SECRET_PATHS: &[&str] = &[
    ".env", "id_rsa", "id_ed25519", ".aws/credentials", ".ssh/", ".npmrc", ".netrc", ".git-credentials",
    "credentials.json", "login.keychain", ".p12", "private_key", "secrets.yaml",
];

/// Ways to get text off the machine.
const SEND_VERBS: &[&str] = &[
    "curl ", "wget ", "nc -", "netcat", "requests.post", "urllib", "webhook", "pastebin", "transfer.sh",
    "api.telegram.org", "discord.com/api/webhooks", "hooks.slack.com", "ngrok",
];

/// Files that change how the agent itself behaves.
const CONFIG_PATHS: &[&str] = &[
    ".claude/settings.json", "claude.md", "agents.md", ".bashrc", ".zshrc", ".bash_profile", ".profile",
    "crontab", "launchagents", ".claude.json",
];

/// Ways to write to a file.
const WRITE_VERBS: &[&str] = &["echo ", "cat >", "tee ", ">>", "sed -i", "chmod ", "install -m", "write_text", "truncate "];

/// `process.env` is not `~/.env`: a path has to start at a boundary, not inside a word.
fn mentions(lower: &str, needle: &str) -> bool {
    lower.match_indices(needle).any(|(i, _)| !lower[..i].chars().next_back().is_some_and(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_'))
}

/// The instructions after the frontmatter, which is where a duplicate really shows up: two skills
/// that share a body differ in their frontmatter by definition (the name is in it).
fn instructions(body: &str) -> String {
    let mut lines = body.lines();
    if lines.next().map(|l| l.trim_end()) == Some("---") {
        let rest = lines.skip_while(|l| l.trim_end() != "---").skip(1).collect::<Vec<_>>().join("\n");
        return normalize(&rest);
    }
    normalize(body)
}

/// Invisible or direction-changing characters: the text says something different from what it looks like.
const INVISIBLE: &[(char, &str)] = &[
    ('\u{200b}', "zero-width space"), ('\u{200c}', "zero-width non-joiner"), ('\u{200d}', "zero-width joiner"),
    ('\u{200e}', "left-to-right mark"), ('\u{200f}', "right-to-left mark"), ('\u{202a}', "left-to-right embedding"),
    ('\u{202b}', "right-to-left embedding"), ('\u{202c}', "pop directional formatting"), ('\u{202d}', "left-to-right override"),
    ('\u{202e}', "right-to-left override"), ('\u{2060}', "word joiner"), ('\u{2066}', "left-to-right isolate"),
    ('\u{2067}', "right-to-left isolate"), ('\u{2068}', "first strong isolate"), ('\u{2069}', "pop directional isolate"),
    ('\u{feff}', "zero-width no-break space"),
];

/// One line, trimmed and cut to something readable in a report.
fn snippet(line: &str) -> String {
    let s = line.trim();
    s.char_indices().take_while(|(i, _)| *i < 160).map(|(_, c)| c).collect()
}

fn is_base64ish(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '=')
}

/// Looks like a real credential: a known prefix followed by enough characters to be a key.
fn secret_token(text: &str) -> Option<&'static str> {
    let lower = text.to_lowercase();
    let long_enough = |prefix: &str, min: usize| {
        lower.match_indices(prefix).any(|(i, _)| text[i + prefix.len()..].chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-').count() >= min)
    };
    if lower.contains("-----begin") && lower.contains("private key") {
        return Some("a private key pasted into the file");
    }
    if long_enough("akia", 16) || long_enough("asia", 16) {
        return Some("an AWS access key id");
    }
    if long_enough("ghp_", 20) || long_enough("gho_", 20) || long_enough("ghs_", 20) || long_enough("github_pat_", 20) {
        return Some("a GitHub token");
    }
    if long_enough("sk-", 24) {
        return Some("an API key");
    }
    if long_enough("xoxb-", 10) || long_enough("xoxp-", 10) {
        return Some("a Slack token");
    }
    if long_enough("aiza", 30) {
        return Some("a Google API key");
    }
    None
}

/// Flags per file, so two halves of a trick on different lines are still caught together.
#[derive(Default)]
struct Flags {
    secret_line: Option<usize>,
    send_line: Option<usize>,
    config_line: Option<usize>,
    write_line: Option<usize>,
}

/// Scan one `SKILL.md`. Pure: same text and name always give the same findings.
pub fn scan_body(body: &str, skill: &str, tool: &str, path: &Path) -> Vec<Finding> {
    let mut out = Vec::new();
    let mut flags = Flags::default();
    let push = |kind, severity, line: usize, text: String, why: &'static str, out: &mut Vec<Finding>| {
        out.push(Finding { severity, kind, skill: skill.to_string(), tool: tool.to_string(), path: path.to_path_buf(), line, text, why });
    };

    for (i, raw) in body.lines().enumerate() {
        let line = i + 1;
        let lower = raw.to_lowercase();

        for r in RULES {
            if r.kind == "remote_exec" && !DOWNLOADERS.iter().any(|d| lower.contains(d)) {
                continue;
            }
            if let Some(n) = r.needles.iter().find(|n| lower.contains(**n)) {
                // Never for `remote_exec`: the line already fetches code and pipes it into a shell, and
                // "Careful: curl … | sh" is still an instruction to run it.
                let (severity, why) = if r.severity == Severity::High && r.kind != "remote_exec" && mentioned_as_warning(&lower, n, r.kind) {
                    (Severity::Warn, "names a dangerous pattern, apparently to warn against it: check that the line really is a warning")
                } else {
                    (r.severity, r.why)
                };
                push(r.kind, severity, line, format!("{n} · {}", snippet(raw)), why, &mut out);
            }
        }
        if let Some((c, name)) = INVISIBLE.iter().find(|(c, _)| raw.contains(*c)) {
            let count = raw.matches(*c).count();
            push("hidden_text", Severity::High, line, format!("{count}× {name} (U+{:04X})", *c as u32), "invisible characters: the text is not what it looks like", &mut out);
        }
        let mut run = 0usize;
        let mut longest = 0usize;
        for c in raw.chars() {
            run = if is_base64ish(c) { run + 1 } else { 0 };
            longest = longest.max(run);
        }
        if longest >= BLOB_CHARS {
            push("obfuscation", Severity::Warn, line, format!("{longest} base64-like characters on one line"), "encoded data the model would read but a human skims past", &mut out);
        }
        if let Some(w) = secret_token(raw) {
            push("secret", Severity::Warn, line, format!("{w} · {}", snippet(raw)), "a credential in the file: it is now in every session that loads it", &mut out);
        }

        flags.secret_line = flags.secret_line.or_else(|| SECRET_PATHS.iter().find(|p| mentions(&lower, p)).map(|_| line));
        flags.send_line = flags.send_line.or_else(|| SEND_VERBS.iter().find(|p| lower.contains(**p)).map(|_| line));
        flags.config_line = flags.config_line.or_else(|| CONFIG_PATHS.iter().find(|p| mentions(&lower, p)).map(|_| line));
        flags.write_line = flags.write_line.or_else(|| WRITE_VERBS.iter().find(|p| lower.contains(**p)).map(|_| line));
    }

    if let (Some(secret), Some(send)) = (flags.secret_line, flags.send_line) {
        push("exfiltration", Severity::High, secret, format!("line {secret} reads a credential file, line {send} can send data out"),
            "reads a credential file and has a way to send it somewhere", &mut out);
    }
    if let (Some(config), Some(write)) = (flags.config_line, flags.write_line) {
        push("self_modify", Severity::Warn, config, format!("line {config} names an agent config file, line {write} writes to a file"),
            "writes to a file that changes how the agent behaves", &mut out);
    }

    out.sort_by(|a, b| a.line.cmp(&b.line).then(a.kind.cmp(b.kind)));
    out.dedup_by(|b, a| a.line == b.line && a.kind == b.kind);
    out
}

fn normalize(body: &str) -> String {
    body.trim().replace("\r\n", "\n").lines().map(str::trim_end).collect::<Vec<_>>().join("\n")
}

fn normalize_desc(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

/// `foo-bar`, `foo_bar` and `FooBar` are the same name spelled differently.
fn slug(name: &str) -> String {
    name.to_lowercase().replace(['-', '_'], "")
}

/// Scan every installed skill that has a `SKILL.md`. Read-only; the caller decides what to do.
pub fn audit(entries: &[SkillEntry]) -> Audit {
    let mut findings: Vec<Finding> = Vec::new();
    let mut scanned = 0usize;
    let mut by_content: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut by_desc: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut by_slug: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();

    for e in entries {
        if e.source == Source::Command {
            continue; // slash commands come from the log, not from a file
        }
        let Some(dir) = entry_dir(e) else { continue };
        let Ok(body) = std::fs::read_to_string(dir.join("SKILL.md")) else { continue };
        scanned += 1;
        findings.extend(scan_body(&body, &e.name, &e.tool, &dir.join("SKILL.md")));
        let text = instructions(&body);
        if text.chars().count() >= MIN_BODY_CHARS {
            by_content.entry(text).or_default().push(e.name.clone());
        }
        if e.description.chars().count() >= MIN_DESC_CHARS {
            by_desc.entry(normalize_desc(&e.description)).or_default().push(e.name.clone());
        }
        by_slug.entry((e.tool.clone(), slug(&e.name))).or_default().push(e.name.clone());
    }

    findings.sort_by(|a, b| a.severity.cmp(&b.severity).then(a.skill.cmp(&b.skill)).then(a.line.cmp(&b.line)));

    let mut duplicates = Vec::new();
    let mut identical_pairs: Vec<(String, String)> = Vec::new();
    for names in groups(&by_content) {
        identical_pairs.extend(pairs(&names));
        duplicates.push(Duplicate { kind: "content", severity: Severity::Warn, skills: names, why: "the same SKILL.md in two places: editing one does nothing for the other" });
    }
    for names in groups(&by_desc) {
        // Two skills with the same file are already reported; the same description alone is the news.
        if couples(&names).all(|(a, b)| identical_pairs.contains(&(a.clone(), b.clone())) || identical_pairs.contains(&(b.clone(), a.clone()))) {
            continue;
        }
        duplicates.push(Duplicate { kind: "description", severity: Severity::Warn, skills: names, why: "two skills describing the same work: one of them loads into every session for nothing" });
    }
    for (_, names) in by_slug.into_iter().filter(|(_, n)| n.len() > 1) {
        let names = sorted(&names);
        if names.iter().collect::<std::collections::BTreeSet<_>>().len() == 1 {
            continue; // the same skill object twice, not a duplicate to report
        }
        duplicates.push(Duplicate { kind: "name", severity: Severity::Info, skills: names, why: "the same name spelled differently: usage is counted separately for each" });
    }

    let ok = !findings.iter().any(|f| f.severity == Severity::High);
    Audit { skills: entries.iter().filter(|e| e.source != Source::Command).count(), scanned, ok, findings, duplicates }
}

/// The directory holding `SKILL.md`; plugin skills without a local directory are skipped by the caller.
fn entry_dir(e: &SkillEntry) -> Option<&PathBuf> {
    e.path.as_ref()
}

fn sorted(names: &[String]) -> Vec<String> {
    let mut v = names.to_vec();
    v.sort();
    v.dedup();
    v
}

/// Groups with two or more members, names sorted.
fn groups(map: &BTreeMap<String, Vec<String>>) -> Vec<Vec<String>> {
    map.values().filter(|v| sorted(v).len() > 1).map(|v| sorted(v)).collect()
}

fn pairs(names: &[String]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for (i, a) in names.iter().enumerate() {
        for b in &names[i + 1..] {
            out.push((a.clone(), b.clone()));
        }
    }
    out
}

fn couples(names: &[String]) -> impl Iterator<Item = (String, String)> + '_ {
    names
        .iter()
        .enumerate()
        .flat_map(move |(i, a)| names[i + 1..].iter().map(move |b| (a.clone(), b.clone())))
}
