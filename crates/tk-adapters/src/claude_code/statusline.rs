//! Thiết lập statusline để lấy % quota (spec mục 7.1).
//! Không sửa script của người dùng: chỉ đổi `statusLine.command` trong `~/.claude/settings.json`
//! thành `"<tk>" statusline --tee | { <lệnh cũ>; }`. Luôn sao lưu trước, hoàn tác được.
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
    /// Mô tả để hiện trong hộp xác nhận.
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
    /// Nội dung gốc trước lần bật đầu tiên.
    content: String,
    /// `statusLine` gốc (null nếu chưa có).
    previous_status_line: Value,
    /// Lệnh ta đã ghi lần gần nhất, để biết người dùng có sửa sau đó không.
    written_command: String,
}

/// Nhận ra lệnh do tk ghi, bất kể đường dẫn tk là gì.
fn is_hook(cmd: &str) -> bool {
    cmd.contains(TEE) || cmd.ends_with("\" statusline") || cmd.contains("tk statusline")
}

/// Lệnh gốc bên trong một hook `"<tk>" statusline --tee | { <gốc>; }`.
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

    /// `~/.claude` mặc định.
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

    /// Nhóm lệnh gốc để `|` không làm đổi nghĩa lệnh có `;` / `&&`.
    fn wrap(&self, original: &str) -> String {
        if cfg!(windows) {
            format!("{} --tee | ( {original} )", self.marker())
        } else {
            format!("{} --tee | {{ {original}; }}", self.marker())
        }
    }

    fn read_backup(&self) -> anyhow::Result<Option<Backup>> {
        match std::fs::read(self.backup_path()) {
            Ok(raw) => Ok(Some(serde_json::from_slice(&raw)?)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn read_settings(&self) -> anyhow::Result<Option<(String, Value)>> {
        let p = self.settings_path();
        match std::fs::read_to_string(&p) {
            Ok(s) => {
                let v = serde_json::from_str(&s).with_context(|| format!("{} không phải JSON hợp lệ", p.display()))?;
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

    pub fn plan(&self) -> anyhow::Result<Plan> {
        let tk = Path::new(&self.tk_bin);
        if !tk.is_absolute() || !tk.is_file() {
            bail!("không tìm thấy chương trình tk ở '{}'; hãy build/cài tk trước", self.tk_bin);
        }
        let path = self.settings_path();
        let describe = |before: &str, after: &str| {
            format!("Sẽ đổi statusLine.command trong {}:\n  trước: {before}\n  sau:   {after}\nStatusline hiện tại vẫn hiển thị như cũ. File gốc được sao lưu.", path.display())
        };
        Ok(match self.status()? {
            Status::Hooked { command } if command.starts_with(&self.marker()) => {
                Plan { description: "Đã bật theo dõi giới hạn, không cần thay đổi.".into(), new_command: None }
            }
            Status::Hooked { command } => {
                // Đường dẫn tk đã đổi: dựng lại từ lệnh gốc, không bọc thêm lớp nữa.
                let original = match self.read_backup()? {
                    Some(b) => b.previous_status_line["command"].as_str().map(str::to_string),
                    None => unwrap_hook(&command),
                };
                let new = original.as_deref().map(|o| self.wrap(o)).unwrap_or_else(|| self.marker());
                Plan { description: describe(&command, &new), new_command: Some(new) }
            }
            Status::NotConfigured => {
                let cmd = self.marker();
                Plan { description: format!("Sẽ thêm statusLine vào {}:\n  {cmd}\nFile gốc được sao lưu, có thể gỡ lại.", path.display()), new_command: Some(cmd) }
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
            bail!("{} không phải object JSON", path.display());
        }
        std::fs::create_dir_all(&self.backup_dir)?;
        // Chỉ sao lưu lần đầu: các lần sau (đổi đường dẫn tk) giữ nguyên bản gốc.
        let backup = match self.read_backup()? {
            Some(b) => Backup { written_command: cmd.clone(), ..b },
            None => Backup {
                settings_path: path.clone(),
                existed: current.is_some(),
                content: current.as_ref().map(|(s, _)| s.clone()).unwrap_or_default(),
                previous_status_line: v.get("statusLine").cloned().unwrap_or(Value::Null),
                written_command: cmd.clone(),
            },
        };
        write_atomic(&self.backup_path(), serde_json::to_string_pretty(&backup)?.as_bytes())?;

        v["statusLine"] = json!({ "type": "command", "command": cmd });
        std::fs::create_dir_all(&self.claude_dir)?;
        write_json(&path, &v)
    }

    /// Trả `statusLine` về như trước khi bật, giữ các thay đổi khác người dùng làm sau đó.
    pub fn undo(&self) -> anyhow::Result<()> {
        let b = self.read_backup()?.context("chưa có bản sao lưu để hoàn tác")?;
        match self.read_settings()? {
            None => {}
            Some((_, mut v)) => {
                if v["statusLine"]["command"].as_str() != Some(b.written_command.as_str()) {
                    bail!("statusLine đã bị sửa sau khi bật, không tự hoàn tác để tránh mất thay đổi của bạn");
                }
                match &b.previous_status_line {
                    Value::Null => { v.as_object_mut().map(|o| o.remove("statusLine")); }
                    prev => v["statusLine"] = prev.clone(),
                }
                let original: Option<Value> = b.existed.then(|| serde_json::from_str(&b.content).ok()).flatten();
                if original.as_ref() == Some(&v) {
                    write_atomic(&resolve(&b.settings_path), b.content.as_bytes())?; // khôi phục đúng từng byte
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

/// Ghi vào file thật nếu `path` là symlink (dotfiles quản lý bằng stow/home-manager).
fn resolve(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn write_json(path: &Path, v: &Value) -> anyhow::Result<()> {
    let mut out = serde_json::to_string_pretty(v)?;
    out.push('\n');
    write_atomic(&resolve(path), out.as_bytes())
}

/// Ghi file tạm rồi đổi tên; giữ nguyên quyền của file cũ.
fn write_atomic(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let tmp = path.with_extension("tk-tmp");
    std::fs::write(&tmp, bytes)?;
    if let Ok(meta) = std::fs::metadata(path) {
        std::fs::set_permissions(&tmp, meta.permissions())?;
    }
    std::fs::rename(&tmp, path)?;
    Ok(())
}
