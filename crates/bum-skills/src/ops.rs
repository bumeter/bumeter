//! Safely disable/enable/install/remove skills: plan → apply → journal → undo (spec section 8.2).
//! Symlink: only the link itself is moved/deleted, never the directory it points to.
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::catalog::{scan_personal, SkillEntry, SkillState};
use crate::frontmatter;
use crate::plugins::ClaudeCli;

const TRASH_DAYS: i64 = 30;
const TRASH_TS: &str = "%Y%m%dT%H%M%S";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpRequest {
    /// `disable` | `enable` | `uninstall` | `install_dir` | `install_git` | `install_market` | `plugin_disable` | `plugin_enable` | `plugin_uninstall`
    pub op: String,
    #[serde(default)]
    pub targets: Vec<String>,
    /// Directory or git URL when installing.
    #[serde(default)]
    pub source: Option<String>,
    /// Tool: `claude_code` (default) or `kiro`.
    #[serde(default)]
    pub tool: Option<String>,
    /// `install_market`: `skillId` on skills.sh (`source` is `owner/repo`).
    #[serde(default)]
    pub skill: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Action {
    Move { from: PathBuf, to: PathBuf },
    Link { target: PathBuf, link: PathBuf },
    Copy { from: PathBuf, to: PathBuf },
    Clone { url: String, to: PathBuf },
    /// Clone the repo into a temp directory, take only the `skill` directory (skills.sh).
    Fetch { url: String, skill: String, to: PathBuf },
    Cli { args: Vec<String>, undo: Vec<String> },
}

impl Action {
    fn describe(&self) -> String {
        match self {
            Action::Move { from, to } => format!("move {} → {}", from.display(), to.display()),
            Action::Link { target, link } => format!("create symlink {} → {}", link.display(), target.display()),
            Action::Copy { from, to } => format!("copy {} → {}", from.display(), to.display()),
            Action::Clone { url, to } => format!("git clone {url} → {}", to.display()),
            Action::Fetch { url, skill, to } => format!("download skill {skill} from {url} → {}", to.display()),
            Action::Cli { args, .. } => format!("run: claude {}", args.join(" ")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    pub id: String,
    pub op: String,
    pub description: String,
    pub actions: Vec<Action>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalEntry {
    pub id: String,
    pub ts: String,
    pub op: String,
    pub description: String,
    pub actions: Vec<Action>,
    pub undone: bool,
}

pub struct Manager {
    claude_dir: PathBuf,
    data_dir: PathBuf,
    cli: ClaudeCli,
    /// URL base for cloning `owner/repo` (tests substitute `file://`).
    git_base: String,
}

pub const GITHUB: &str = "https://github.com/";

/// Valid skill directory name: no `/`, `\`, `..`, does not start with `.` or `-`.
pub(crate) fn check_name(name: &str) -> anyhow::Result<()> {
    let ok = !name.is_empty()
        && name.len() <= 64
        && !name.starts_with(['.', '-'])
        && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        && !name.contains("..");
    if !ok {
        bail!("skill name '{name}' is invalid (use only letters, digits, - _ .)");
    }
    Ok(())
}

fn exists(p: &Path) -> bool {
    std::fs::symlink_metadata(p).is_ok()
}

fn new_id() -> String {
    Utc::now().format("%Y%m%dT%H%M%S%.9f").to_string()
}

impl Manager {
    pub fn new(claude_dir: PathBuf, data_dir: PathBuf, cli: ClaudeCli) -> Self {
        Self { claude_dir, data_dir, cli, git_base: GITHUB.into() }
    }

    pub fn with_git_base(mut self, base: impl Into<String>) -> Self {
        self.git_base = base.into();
        self
    }

    fn skills_dir(&self) -> PathBuf { self.claude_dir.join("skills") }
    fn disabled_dir(&self) -> PathBuf { self.claude_dir.join("skills-disabled") }
    fn trash_dir(&self) -> PathBuf { self.data_dir.join("trash") }
    fn journal_path(&self) -> PathBuf { self.data_dir.join("skill_ops.jsonl") }

    fn trash_slot(&self, name: &str) -> PathBuf {
        let mut p = self.trash_dir().join(format!("{}-{name}", Utc::now().format(TRASH_TS)));
        let mut n = 1;
        while exists(&p) {
            p = self.trash_dir().join(format!("{}-{name}-{n}", Utc::now().format(TRASH_TS)));
            n += 1;
        }
        p
    }

    fn find(&self, name: &str) -> anyhow::Result<SkillEntry> {
        let entry = scan_personal(&self.claude_dir).into_iter().find(|s| s.name == name).with_context(|| format!("skill '{name}' not found"))?;
        if !entry.managed {
            bail!("skill '{name}' is synced from claude.ai, cannot be managed here");
        }
        Ok(entry)
    }

    fn dir_name(entry: &SkillEntry) -> anyhow::Result<String> {
        Ok(entry.path.as_ref().and_then(|p| p.file_name()).context("skill has no directory")?.to_string_lossy().to_string())
    }

    fn check_install_name(&self, name: &str) -> anyhow::Result<PathBuf> {
        check_name(name)?;
        if scan_personal(&self.claude_dir).iter().any(|s| s.name == name) {
            bail!("a skill named '{name}' already exists");
        }
        let dst = self.skills_dir().join(name);
        if exists(&dst) {
            bail!("{} already exists", dst.display());
        }
        Ok(dst)
    }

    pub fn plan(&self, req: &OpRequest) -> anyhow::Result<Plan> {
        let mut actions = Vec::new();
        match req.op.as_str() {
            "disable" | "enable" | "uninstall" => {
                if req.targets.is_empty() {
                    bail!("no skill selected");
                }
                for name in &req.targets {
                    let e = self.find(name)?;
                    let from = e.path.clone().context("skill has no directory")?;
                    let dir = Self::dir_name(&e)?;
                    let to = match (req.op.as_str(), e.state) {
                        ("disable", SkillState::Enabled) => self.disabled_dir().join(&dir),
                        ("enable", SkillState::Disabled) => self.skills_dir().join(&dir),
                        ("uninstall", _) => self.trash_slot(&dir),
                        ("disable", _) => bail!("'{name}' is already disabled"),
                        _ => bail!("'{name}' is already enabled"),
                    };
                    if exists(&to) && req.op != "uninstall" {
                        bail!("{} already exists", to.display());
                    }
                    actions.push(Action::Move { from, to });
                }
            }
            "install_dir" => {
                let src = PathBuf::from(req.source.as_deref().context("missing source directory")?);
                let src = std::fs::canonicalize(&src).with_context(|| format!("could not open {}", src.display()))?;
                let body = std::fs::read_to_string(src.join("SKILL.md")).with_context(|| format!("{} has no SKILL.md", src.display()))?;
                let name = frontmatter::parse(&body).name.filter(|n| !n.is_empty()).context("SKILL.md is missing `name` in frontmatter")?;
                let dst = self.check_install_name(&name)?;
                actions.push(if cfg!(unix) { Action::Link { target: src, link: dst } } else { Action::Copy { from: src, to: dst } });
            }
            "install_git" => {
                let url = req.source.clone().context("missing git URL")?;
                if url.starts_with('-') {
                    bail!("invalid git URL");
                }
                let name = url.trim_end_matches('/').rsplit(['/', ':', '\\']).next().unwrap_or_default().trim_end_matches(".git").to_string();
                if name.is_empty() {
                    bail!("could not read a name from the URL");
                }
                let dst = self.check_install_name(&name)?;
                actions.push(Action::Clone { url, to: dst });
            }
            "install_market" => {
                let source = req.source.as_deref().context("missing source repo (owner/repo)")?;
                if !crate::market::valid_source(source) {
                    bail!("source repo '{source}' is invalid");
                }
                let skill = req.skill.clone().context("missing skill name")?;
                let to = self.check_install_name(&skill)?;
                actions.push(Action::Fetch { url: format!("{}{source}.git", self.git_base), skill, to });
            }
            op @ ("plugin_disable" | "plugin_enable" | "plugin_uninstall") => {
                let (verb, undo) = match op {
                    "plugin_disable" => ("disable", "enable"),
                    "plugin_enable" => ("enable", "disable"),
                    _ => ("uninstall", "install"),
                };
                for id in &req.targets {
                    actions.push(Action::Cli { args: vec!["plugin".into(), verb.into(), id.clone()], undo: vec!["plugin".into(), undo.into(), id.clone()] });
                }
                if actions.is_empty() {
                    bail!("no plugin selected");
                }
            }
            other => bail!("no such operation '{other}'"),
        }
        let mut description = actions.iter().map(|a| format!("• {}", a.describe())).collect::<Vec<_>>().join("\n");
        description.push_str("\nTakes effect from a new Claude Code session. Can be undone from the journal.");
        Ok(Plan { id: new_id(), op: req.op.clone(), description, actions })
    }

    fn run(&self, a: &Action) -> anyhow::Result<()> {
        match a {
            Action::Move { from, to } => {
                if exists(to) {
                    bail!("{} already exists", to.display());
                }
                std::fs::create_dir_all(to.parent().context("invalid destination path")?)?;
                // rename on a symlink only relocates the link itself.
                std::fs::rename(from, to).with_context(|| format!("could not move {}", from.display()))
            }
            Action::Link { target, link } => {
                std::fs::create_dir_all(link.parent().context("invalid path")?)?;
                #[cfg(unix)]
                std::os::unix::fs::symlink(target, link)?;
                #[cfg(not(unix))]
                bail!("symlinks are only supported on Unix: {} → {}", link.display(), target.display());
                #[cfg(unix)]
                Ok(())
            }
            Action::Copy { from, to } => copy_dir(from, to),
            Action::Clone { url, to } => {
                let st = crate::process::command("git").args(["clone", "-q", "--depth", "1", "--", url]).arg(to).status().context("could not run git")?;
                if !st.success() {
                    bail!("git clone failed");
                }
                if !to.join("SKILL.md").exists() {
                    std::fs::remove_dir_all(to)?;
                    bail!("repo has no SKILL.md at its root");
                }
                Ok(())
            }
            Action::Fetch { url, skill, to } => {
                let tmp = self.data_dir.join("tmp").join(new_id());
                std::fs::create_dir_all(tmp.parent().context("invalid temp directory")?)?;
                let res = (|| {
                    let st = crate::process::command("git").args(["clone", "-q", "--depth", "1", "--", url]).arg(&tmp).status().context("could not run git")?;
                    if !st.success() {
                        bail!("git clone {url} failed");
                    }
                    let found = crate::market::find_skill_dir(&tmp, skill)?;
                    move_dir(&found, to)
                })();
                let _ = std::fs::remove_dir_all(&tmp);
                res
            }
            Action::Cli { args, .. } => self.cli.run(&args.iter().map(String::as_str).collect::<Vec<_>>()).map(|_| ()),
        }
    }

    fn reverse(&self, a: &Action) -> anyhow::Result<()> {
        match a {
            Action::Move { from, to } => self.run(&Action::Move { from: to.clone(), to: from.clone() }),
            Action::Link { link, .. } => {
                if std::fs::symlink_metadata(link).map(|m| m.file_type().is_symlink()).unwrap_or(false) {
                    std::fs::remove_file(link)?;
                }
                Ok(())
            }
            Action::Copy { to, .. } | Action::Clone { to, .. } | Action::Fetch { to, .. } => {
                let name = to.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                self.run(&Action::Move { from: to.clone(), to: self.trash_slot(&name) })
            }
            Action::Cli { undo, .. } => self.cli.run(&undo.iter().map(String::as_str).collect::<Vec<_>>()).map(|_| ()),
        }
    }

    /// Execute the plan; if one step fails, roll back the steps already done.
    pub fn apply(&self, plan: &Plan) -> anyhow::Result<JournalEntry> {
        for (i, a) in plan.actions.iter().enumerate() {
            if let Err(e) = self.run(a) {
                for done in plan.actions[..i].iter().rev() {
                    let _ = self.reverse(done);
                }
                return Err(e.context("rolled back the previous steps"));
            }
        }
        let entry = JournalEntry {
            id: plan.id.clone(),
            ts: Utc::now().to_rfc3339(),
            op: plan.op.clone(),
            description: plan.description.clone(),
            actions: plan.actions.clone(),
            undone: false,
        };
        std::fs::create_dir_all(&self.data_dir)?;
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(self.journal_path())?;
        writeln!(f, "{}", serde_json::to_string(&entry)?)?;
        Ok(entry)
    }

    /// Journal, newest first.
    pub fn log(&self) -> anyhow::Result<Vec<JournalEntry>> {
        let text = match std::fs::read_to_string(self.journal_path()) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e.into()),
        };
        let mut entries: Vec<JournalEntry> = text.lines().filter_map(|l| serde_json::from_str(l).ok()).collect();
        entries.reverse();
        Ok(entries)
    }

    pub fn undo(&self, id: &str) -> anyhow::Result<()> {
        let mut entries = self.log()?;
        entries.reverse();
        let entry = entries.iter_mut().find(|e| e.id == id).context("no such operation in the journal")?;
        if entry.undone {
            bail!("this operation has already been undone");
        }
        // Steps already back in place (a previously incomplete undo) are skipped; on error, continue with the other steps then report.
        let mut failed = Vec::new();
        for a in entry.actions.iter().rev() {
            if let Action::Move { from, to } = a {
                if exists(from) && !exists(to) {
                    continue;
                }
            }
            if let Err(e) = self.reverse(a) {
                failed.push(format!("{}: {e:#}", a.describe()));
            }
        }
        if !failed.is_empty() {
            bail!("undo is incomplete; you can retry after resolving:\n{}", failed.join("\n"));
        }
        entry.undone = true;
        let body: String = entries.iter().map(|e| serde_json::to_string(e).map(|s| s + "\n")).collect::<Result<_, _>>()?;
        std::fs::write(self.journal_path(), body)?;
        Ok(())
    }

    /// Permanently delete trash entries older than 30 days. Returns the number of entries deleted.
    pub fn purge_trash(&self, now: DateTime<Utc>) -> anyhow::Result<usize> {
        let Ok(rd) = std::fs::read_dir(self.trash_dir()) else { return Ok(0) };
        let mut removed = 0;
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            let Some(stamp) = name.split('-').next() else { continue };
            let Ok(at) = chrono::NaiveDateTime::parse_from_str(stamp, TRASH_TS) else { continue };
            if (now - at.and_utc()).num_days() > TRASH_DAYS {
                let p = e.path();
                if std::fs::symlink_metadata(&p)?.file_type().is_symlink() || p.is_file() {
                    std::fs::remove_file(&p)?;
                } else {
                    std::fs::remove_dir_all(&p)?;
                }
                removed += 1;
            }
        }
        Ok(removed)
    }
}

/// Copy a directory; symlinks are skipped (do not follow links outside the source).
fn copy_dir(from: &Path, to: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)? {
        let e = e?;
        let dst = to.join(e.file_name());
        let ft = e.file_type()?;
        if ft.is_dir() {
            copy_dir(&e.path(), &dst)?;
        } else if ft.is_file() {
            std::fs::copy(e.path(), dst)?;
        }
    }
    Ok(())
}

/// Move a directory; across filesystems (rename fails) fall back to copying.
fn move_dir(from: &Path, to: &Path) -> anyhow::Result<()> {
    if exists(to) {
        bail!("{} already exists", to.display());
    }
    std::fs::create_dir_all(to.parent().context("invalid destination path")?)?;
    if std::fs::rename(from, to).is_err() {
        if let Err(e) = copy_dir(from, to) {
            let _ = std::fs::remove_dir_all(to);
            return Err(e.context(format!("could not copy the skill to {}", to.display())));
        }
    }
    Ok(())
}
