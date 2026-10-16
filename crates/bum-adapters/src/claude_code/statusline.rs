//! Set up the statusline to capture the quota % (spec section 7.1).
//! Does not modify the user's script: only changes `statusLine.command` in `~/.claude/settings.json`
//! to `"<tk>" statusline --tee | { <old command>; }`. Always backs up first and can be undone.
use std::path::{Path, PathBuf};

use anyhow::{bail, Context};
use serde_json::{json, Value};

const TEE: &str = "\" statusline --tee | ";

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Status {
    NotConfigured,
    Custom { command: String },
    Hooked { command: String },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Plan {
    /// Description to show in the confirmation dialog.
    pub description: String,
    new_command: Option<String>,
}

impl Plan {
    pub fn is_noop(&self) -> bool {
        self.new_command.is_none()
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Backup {
    settings_path: PathBuf,
    existed: bool,
    /// Original content before the first enable.
    content: String,
    /// Original `statusLine` (null if absent). First-generation backups lack this field:
    /// `read_backup` recovers it from `content`.
    #[serde(default)]
    previous_status_line: Option<Value>,
    /// The command we wrote last time, to tell whether the user edited it afterwards.
    #[serde(default)]
    written_command: String,
}

/// Recognize a command written by tk, whatever tk's path is.
fn is_hook(cmd: &str) -> bool {
    cmd.contains(TEE) || cmd.ends_with("\" statusline") || cmd.contains("tk statusline") || cmd.contains("bum statusline")
}

/// Replace the `tk` program name immediately before ` statusline` with `bum` (directory unchanged).
/// Use the rewritten path only if that file exists; otherwise use `current_bum` (if it exists); if neither: `None`.
/// `None` if there is nothing to change. Accepts both `"/path/tk" statusline` and `/path/tk statusline`.
fn rewrite_legacy_bin(cmd: &str, current_bum: &str) -> Option<String> {
    let mut out = String::with_capacity(cmd.len());
    let mut rest = cmd;
    let mut changed = false;
    while let Some(i) = rest.find(" statusline") {
        let head = &rest[..i];
        let (head, quote) = match head.strip_suffix('"') {
            Some(h) => (h, "\""),
            None => (head, ""),
        };
        let stem = head.strip_suffix(".exe").unwrap_or(head);
        let ext = &head[stem.len()..];
        let is_tk = stem == "tk" || stem.ends_with("/tk") || stem.ends_with("\\tk") || stem.ends_with("\"tk");
        if is_tk {
            let sep: &[char] = if quote.is_empty() { &[' ', '"'] } else { &['"'] };
            let start = stem.rfind(sep).map_or(0, |i| i + 1);
            let candidate = format!("{}bum{ext}", &stem[start..stem.len() - 2]);
            let chosen = if Path::new(&candidate).is_file() {
                candidate
            } else if Path::new(current_bum).is_file() {
                current_bum.to_string()
            } else {
                return None;
            };
            out.push_str(&stem[..start]);
            if quote.is_empty() && chosen.contains(' ') {
                out.push_str(&format!("\"{chosen}\""));
            } else {
                out.push_str(&chosen);
            }
            changed = true;
        } else {
            out.push_str(head);
            out.push_str(ext);
        }
        out.push_str(quote);
        out.push_str(" statusline");
        rest = &rest[i + " statusline".len()..];
    }
    out.push_str(rest);
    changed.then_some(out)
}

/// The original command inside a `"<tk>" statusline --tee | { <original>; }` hook.
fn unwrap_hook(cmd: &str) -> Option<String> {
    let inner = &cmd[cmd.find(TEE)? + TEE.len()..];
    let inner = inner.strip_prefix("{ ").and_then(|s| s.strip_suffix("; }"))
        .or_else(|| inner.strip_prefix("( ").and_then(|s| s.strip_suffix(" )")))
        .unwrap_or(inner);
    Some(inner.to_string())
}

pub struct StatuslineSetup {
    claude_dir: PathBuf,
    backup_dir: PathBuf,
    tk_bin: String,
}

impl StatuslineSetup {
    pub fn new(claude_dir: PathBuf, backup_dir: PathBuf, tk_bin: String) -> Self {
        Self { claude_dir, backup_dir, tk_bin }
    }

    /// Defaults to `~/.claude`.
    pub fn default_claude_dir() -> PathBuf {
        dirs::home_dir().unwrap_or_default().join(".claude")
    }

    fn settings_path(&self) -> PathBuf {
        self.claude_dir.join("settings.json")
    }

    fn backup_path(&self) -> PathBuf {
        self.backup_dir.join("statusline-backup.json")
    }

    fn marker(&self) -> String {
        format!("\"{}\" statusline", self.tk_bin)
    }

    /// Group the original command so `|` does not change the meaning of commands containing `;` / `&&`.
    fn wrap(&self, original: &str) -> String {
        if cfg!(windows) {
            format!("{} --tee | ( {original} )", self.marker())
        } else {
            format!("{} --tee | {{ {original}; }}", self.marker())
        }
    }

    fn read_backup(&self) -> anyhow::Result<Option<Backup>> {
        match std::fs::read(self.backup_path()) {
            Ok(raw) => {
                let mut b: Backup = serde_json::from_slice(&raw)?;
                if b.previous_status_line.is_none() {
                    let original: Value = serde_json::from_str(&b.content).unwrap_or(Value::Null);
                    b.previous_status_line = Some(original.get("statusLine").cloned().unwrap_or(Value::Null));
                }
                Ok(Some(b))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn read_settings(&self) -> anyhow::Result<Option<(String, Value)>> {
        let p = self.settings_path();
        match std::fs::read_to_string(&p) {
            Ok(s) => {
                let v = serde_json::from_str(&s).with_context(|| format!("{} is not valid JSON", p.display()))?;
                Ok(Some((s, v)))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn status(&self) -> anyhow::Result<Status> {
        let command = self.read_settings()?.and_then(|(_, v)| v["statusLine"]["command"].as_str().map(str::to_string));
        Ok(match command {
            None => Status::NotConfigured,
            Some(c) if is_hook(&c) => Status::Hooked { command: c },
            Some(c) => Status::Custom { command: c },
        })
    }

    /// Legacy hook pointing at `tk` (renamed to `bum`): rewrite it to `bum`, with a backup (like `apply`).
    /// Returns `true` if it changed something; running again changes nothing.
    pub fn migrate_legacy_hook(&self) -> anyhow::Result<bool> {
        let Status::Hooked { command } = self.status()? else { return Ok(false) };
        let Some(new) = rewrite_legacy_bin(&command, &self.tk_bin) else {
            if command.contains("tk statusline") || command.contains("tk\" statusline") {
                eprintln!("bumeter: statusline hook still points at tk but no bum was found; leaving it unchanged");
            }
            return Ok(false);
        };
        self.apply(&Plan { description: String::new(), new_command: Some(new) })?;
        Ok(true)
    }

    pub fn plan(&self) -> anyhow::Result<Plan> {
        let tk = Path::new(&self.tk_bin);
        if !tk.is_absolute() || !tk.is_file() {
            bail!("no bum program found at '{}'; build/install bum first", self.tk_bin);
        }
        let path = self.settings_path();
        let describe = |before: &str, after: &str| {
            format!("Will change statusLine.command in {}:\n  before: {before}\n  after:  {after}\nThe current statusline still displays as before. The original file is backed up.", path.display())
        };
        Ok(match self.status()? {
            Status::Hooked { command } if command.starts_with(&self.marker()) => {
                Plan { description: "Limit tracking is already enabled; no change needed.".into(), new_command: None }
            }
            Status::Hooked { command } => {
                // The tk path changed: rebuild from the original command instead of wrapping again.
                let original = match self.read_backup()? {
                    Some(b) => b.previous_status_line.as_ref().and_then(|p| p["command"].as_str()).map(str::to_string),
                    None => unwrap_hook(&command),
                };
                let new = original.as_deref().map(|o| self.wrap(o)).unwrap_or_else(|| self.marker());
                Plan { description: describe(&command, &new), new_command: Some(new) }
            }
            Status::NotConfigured => {
                let cmd = self.marker();
                Plan { description: format!("Will add statusLine to {}:\n  {cmd}\nThe original file is backed up and can be removed again.", path.display()), new_command: Some(cmd) }
            }
            Status::Custom { command } => {
                let new = self.wrap(&command);
                Plan { description: describe(&command, &new), new_command: Some(new) }
            }
        })
    }

    pub fn apply(&self, plan: &Plan) -> anyhow::Result<()> {
        let Some(cmd) = &plan.new_command else { return Ok(()) };
        let path = self.settings_path();
        let current = self.read_settings()?;
        let mut v = current.as_ref().map(|(_, v)| v.clone()).unwrap_or_else(|| json!({}));
        if !v.is_object() {
            bail!("{} is not a JSON object", path.display());
        }
        std::fs::create_dir_all(&self.backup_dir)?;
        // Back up only on the first time: later runs (tk path changes) keep the original.
        let backup = match self.read_backup()? {
            Some(b) => Backup { written_command: cmd.clone(), ..b },
            None => Backup {
                settings_path: path.clone(),
                existed: current.is_some(),
                content: current.as_ref().map(|(s, _)| s.clone()).unwrap_or_default(),
                previous_status_line: Some(v.get("statusLine").cloned().unwrap_or(Value::Null)),
                written_command: cmd.clone(),
            },
        };
        write_atomic(&self.backup_path(), serde_json::to_string_pretty(&backup)?.as_bytes())?;

        v["statusLine"] = json!({ "type": "command", "command": cmd });
        std::fs::create_dir_all(&self.claude_dir)?;
        write_json(&path, &v)
    }

    /// Restore `statusLine` to before it was enabled, keeping other changes the user made afterwards.
    pub fn undo(&self) -> anyhow::Result<()> {
        let b = self.read_backup()?.context("no backup to undo")?;
        match self.read_settings()? {
            None => {}
            Some((_, mut v)) => {
                if v["statusLine"]["command"].as_str() != Some(b.written_command.as_str()) {
                    bail!("statusLine was edited after enabling; not undoing automatically to avoid losing your changes");
                }
                match b.previous_status_line.as_ref().unwrap_or(&Value::Null) {
                    Value::Null => { v.as_object_mut().map(|o| o.remove("statusLine")); }
                    prev => v["statusLine"] = prev.clone(),
                }
                let original: Option<Value> = b.existed.then(|| serde_json::from_str(&b.content).ok()).flatten();
                if original.as_ref() == Some(&v) {
                    write_atomic(&resolve(&b.settings_path), b.content.as_bytes())?; // restore byte-for-byte
                } else if !b.existed && v.as_object().is_some_and(|o| o.is_empty()) {
                    std::fs::remove_file(&b.settings_path)?;
                } else {
                    write_json(&b.settings_path, &v)?;
                }
            }
        }
        std::fs::remove_file(self.backup_path())?;
        Ok(())
    }
}

/// Write to the real file if `path` is a symlink (dotfiles managed by stow/home-manager).
fn resolve(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn write_json(path: &Path, v: &Value) -> anyhow::Result<()> {
    let mut out = serde_json::to_string_pretty(v)?;
    out.push('\n');
    write_atomic(&resolve(path), out.as_bytes())
}

/// Write a temp file then rename it; preserving the old file's permissions.
fn write_atomic(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let tmp = path.with_extension("tk-tmp");
    std::fs::write(&tmp, bytes)?;
    if let Ok(meta) = std::fs::metadata(path) {
        std::fs::set_permissions(&tmp, meta.permissions())?;
    }
    std::fs::rename(&tmp, path)?;
    Ok(())
}
