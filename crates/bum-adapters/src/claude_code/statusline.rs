//! Set up the statusline to capture the quota % (spec section 7.1).
//! Does not modify the user's script: only changes `statusLine.command` in `~/.claude/settings.json`,
//! to `"<bum>" statusline --tee | { <old command>; }` (record the numbers, show the old line as it was)
//! or `"<bum>" statusline --around '<old command>'` (show Bumeter's part after the old line). Always
//! backs up first and can be undone.
use std::path::{Path, PathBuf};

use anyhow::{bail, Context};
use serde_json::{json, Value};

/// Matches both `"<path>" statusline --tee | …` and a hand-written `bum statusline --tee | …`.
const TEE: &str = " statusline --tee | ";
/// Matches both `"<path>" statusline --around '…'` and a hand-written `bum statusline --around '…'`.
const AROUND: &str = " statusline --around ";

/// Whether `Mode::Show` works here. Its hook runs the user's command through `sh`, and Windows has
/// no `sh` by default (Claude Code may also run the hook under `cmd.exe`, which does not read `'…'`):
/// there, `Show` falls back to `Record`.
pub const SHOW_SUPPORTED: bool = cfg!(not(windows));

/// What the hook does next to a status line the user already has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// Record the quota numbers; the user's line is shown unchanged.
    Record,
    /// Record them and show Bumeter's part (limits, context) after the user's line.
    Show,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Status {
    NotConfigured,
    Custom { command: String },
    /// `show`: Bumeter's part is on screen (the `--around` hook, or Bumeter's own line).
    Hooked { command: String, show: bool },
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

/// Recognize a statusline command written by Bumeter, whatever the `bum` path is.
fn is_hook(cmd: &str) -> bool {
    cmd.contains(TEE) || cmd.contains(AROUND) || cmd.ends_with("\" statusline") || cmd.contains("bum statusline")
}

/// Quote for `sh` as one word: `'…'`, with each `'` written as `'\''`.
fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// One `sh` word back to its value: what `sh_quote` writes, plus the `"…"` and bare words of a
/// hand-written hook. None for anything the shell would expand or split (`$`, `` ` ``, spaces, `;`…).
fn sh_unquote(word: &str) -> Option<String> {
    let mut out = String::new();
    let mut chars = word.trim().chars();
    while let Some(c) = chars.next() {
        match c {
            '\'' => loop {
                match chars.next()? {
                    '\'' => break,
                    c => out.push(c),
                }
            },
            '"' => loop {
                match chars.next()? {
                    '"' => break,
                    '$' | '`' => return None,
                    '\\' => match chars.next()? {
                        c @ ('$' | '`' | '"' | '\\') => out.push(c),
                        '\n' => {}
                        c => {
                            out.push('\\');
                            out.push(c);
                        }
                    },
                    c => out.push(c),
                }
            },
            '\\' => out.push(chars.next()?),
            c if c.is_whitespace() || "$`;&|<>()*?[]{}#~!".contains(c) => return None,
            c => out.push(c),
        }
    }
    Some(out)
}

/// What runs inside a hook: `Ok(Some(original))` for `"<bum>" statusline --tee | { <original>; }`
/// and `"<bum>" statusline --around '<original>'`, `Ok(None)` for Bumeter's own line, and `Err` for
/// any other use of `bum statusline` (piped into `jq`, `--quiet`, an unreadable `--around`), which
/// cannot be rebuilt without losing part of it.
fn unwrap_hook(cmd: &str) -> Result<Option<String>, ()> {
    if let Some(i) = cmd.find(AROUND) {
        return sh_unquote(&cmd[i + AROUND.len()..]).map(Some).ok_or(());
    }
    if let Some(i) = cmd.find(TEE) {
        let inner = &cmd[i + TEE.len()..];
        let inner = inner.strip_prefix("{ ").and_then(|s| s.strip_suffix("; }"))
            .or_else(|| inner.strip_prefix("( ").and_then(|s| s.strip_suffix(" )")))
            .unwrap_or(inner);
        return Ok(Some(inner.to_string()));
    }
    if cmd.trim_end().ends_with(" statusline") {
        Ok(None)
    } else {
        Err(())
    }
}

pub struct StatuslineSetup {
    claude_dir: PathBuf,
    backup_dir: PathBuf,
    bum_bin: String,
}

impl StatuslineSetup {
    pub fn new(claude_dir: PathBuf, backup_dir: PathBuf, bum_bin: String) -> Self {
        Self { claude_dir, backup_dir, bum_bin }
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
        format!("\"{}\" statusline", self.bum_bin)
    }

    /// Group the original command so `|` does not change the meaning of commands containing `;` / `&&`.
    fn wrap(&self, original: &str, mode: Mode) -> String {
        if mode == Mode::Show && SHOW_SUPPORTED {
            return format!("{} --around {}", self.marker(), sh_quote(original));
        }
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
            Some(c) if is_hook(&c) => {
                let show = c.contains(AROUND) || !(c.contains(TEE) || c.contains(" statusline --tee"));
                Status::Hooked { command: c, show }
            }
            Some(c) => Status::Custom { command: c },
        })
    }

    /// `plan_for(Mode::Record)`: the behaviour before `Mode` existed.
    pub fn plan(&self) -> anyhow::Result<Plan> {
        self.plan_for(Mode::Record)
    }

    pub fn plan_for(&self, mode: Mode) -> anyhow::Result<Plan> {
        let bum = Path::new(&self.bum_bin);
        if !bum.is_absolute() || !bum.is_file() {
            bail!("no bum program found at '{}'; build/install bum first", self.bum_bin);
        }
        let path = self.settings_path();
        let describe = |before: &str, after: &str| {
            format!("Will change statusLine.command in {}:\n  before: {before}\n  after:  {after}\nThe current statusline still displays as before. The original file is backed up.", path.display())
        };
        Ok(match self.status()? {
            Status::Hooked { command, .. } => {
                // Rebuild from the original command (never wrap a hook again): this handles a changed
                // bum path and a switch between recording and showing alike. The command inside the
                // current hook comes first, so edits the user made to it after enabling are kept. A hook
                // we cannot read back is only replaced when it is exactly what we wrote (the backup then
                // knows the original); otherwise rebuilding it would drop part of the user's command.
                let original = match unwrap_hook(&command) {
                    Ok(o) => o,
                    Err(()) => match self.read_backup()?.filter(|b| b.written_command == command) {
                        Some(b) => b.previous_status_line.as_ref().and_then(|p| p["command"].as_str()).map(str::to_string),
                        None => bail!("statusLine.command in {} runs bum in a form Bumeter cannot rebuild without losing part of it:\n  {command}\nChange it by hand, or remove it and enable again.", path.display()),
                    },
                };
                let new = original.as_deref().map(|o| self.wrap(o, mode)).unwrap_or_else(|| self.marker());
                if new == command {
                    Plan { description: "Limit tracking is already enabled; no change needed.".into(), new_command: None }
                } else {
                    Plan { description: describe(&command, &new), new_command: Some(new) }
                }
            }
            Status::NotConfigured => {
                let cmd = self.marker();
                Plan { description: format!("Will add statusLine to {}:\n  {cmd}\nThe original file is backed up and can be removed again.", path.display()), new_command: Some(cmd) }
            }
            Status::Custom { command } => {
                let new = self.wrap(&command, mode);
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
        // Back up only on the first time: later runs (bum path changes) keep the original.
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

        // Only the command changes: other statusLine fields (`padding`…) stay as they are.
        match v.get_mut("statusLine").and_then(Value::as_object_mut) {
            Some(sl) => {
                sl.insert("type".into(), json!("command"));
                sl.insert("command".into(), json!(cmd));
            }
            None => v["statusLine"] = json!({ "type": "command", "command": cmd }),
        }
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
                let prev = b.previous_status_line.clone().unwrap_or(Value::Null);
                // The command inside the hook wins over the backup's: switching modes keeps an edit
                // the user made to it, so it may be newer. The other fields stay as they are now.
                let restored = match unwrap_hook(&b.written_command) {
                    Ok(Some(inner)) => {
                        let mut sl = v["statusLine"].clone();
                        sl["command"] = json!(inner);
                        if let Some(t) = prev.get("type") {
                            sl["type"] = t.clone();
                        }
                        sl
                    }
                    _ => prev,
                };
                match restored {
                    Value::Null => { v.as_object_mut().map(|o| o.remove("statusLine")); }
                    sl => v["statusLine"] = sl,
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
    let tmp = path.with_extension("bum-tmp");
    std::fs::write(&tmp, bytes)?;
    if let Ok(meta) = std::fs::metadata(path) {
        std::fs::set_permissions(&tmp, meta.permissions())?;
    }
    std::fs::rename(&tmp, path)?;
    Ok(())
}
