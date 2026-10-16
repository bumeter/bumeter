//! Tắt/bật/cài/gỡ skill an toàn: plan → apply → nhật ký → undo (spec mục 8.2).
//! Symlink: chỉ di chuyển/xoá chính cái link, không bao giờ đụng thư mục nó trỏ tới.
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
    /// Thư mục hoặc git URL khi cài.
    #[serde(default)]
    pub source: Option<String>,
    /// Công cụ: `claude_code` (mặc định) hoặc `kiro`.
    #[serde(default)]
    pub tool: Option<String>,
    /// `install_market`: `skillId` trên skills.sh (`source` là `owner/repo`).
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
    /// Clone repo vào thư mục tạm, chỉ lấy thư mục của `skill` (skills.sh).
    Fetch { url: String, skill: String, to: PathBuf },
    Cli { args: Vec<String>, undo: Vec<String> },
}

impl Action {
    fn describe(&self) -> String {
        match self {
            Action::Move { from, to } => format!("chuyển {} → {}", from.display(), to.display()),
            Action::Link { target, link } => format!("tạo symlink {} → {}", link.display(), target.display()),
            Action::Copy { from, to } => format!("sao chép {} → {}", from.display(), to.display()),
            Action::Clone { url, to } => format!("git clone {url} → {}", to.display()),
            Action::Fetch { url, skill, to } => format!("tải skill {skill} từ {url} → {}", to.display()),
            Action::Cli { args, .. } => format!("chạy: claude {}", args.join(" ")),
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
    /// Gốc URL để clone `owner/repo` (test thay bằng `file://`).
    git_base: String,
}

pub const GITHUB: &str = "https://github.com/";

/// Tên thư mục skill hợp lệ: không có `/`, `\`, `..`, không bắt đầu bằng `.` hay `-`.
pub(crate) fn check_name(name: &str) -> anyhow::Result<()> {
    let ok = !name.is_empty()
        && name.len() <= 64
        && !name.starts_with(['.', '-'])
        && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        && !name.contains("..");
    if !ok {
        bail!("tên skill '{name}' không hợp lệ (chỉ dùng chữ, số, - _ .)");
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
        let entry = scan_personal(&self.claude_dir).into_iter().find(|s| s.name == name).with_context(|| format!("không tìm thấy skill '{name}'"))?;
        if !entry.managed {
            bail!("skill '{name}' được đồng bộ từ claude.ai, không quản lý được ở đây");
        }
        Ok(entry)
    }

    fn dir_name(entry: &SkillEntry) -> anyhow::Result<String> {
        Ok(entry.path.as_ref().and_then(|p| p.file_name()).context("skill không có thư mục")?.to_string_lossy().to_string())
    }

    fn check_install_name(&self, name: &str) -> anyhow::Result<PathBuf> {
        check_name(name)?;
        if scan_personal(&self.claude_dir).iter().any(|s| s.name == name) {
            bail!("đã có skill tên '{name}'");
        }
        let dst = self.skills_dir().join(name);
        if exists(&dst) {
            bail!("{} đã tồn tại", dst.display());
        }
        Ok(dst)
    }

    pub fn plan(&self, req: &OpRequest) -> anyhow::Result<Plan> {
        let mut actions = Vec::new();
        match req.op.as_str() {
            "disable" | "enable" | "uninstall" => {
                if req.targets.is_empty() {
                    bail!("chưa chọn skill");
                }
                for name in &req.targets {
                    let e = self.find(name)?;
                    let from = e.path.clone().context("skill không có thư mục")?;
                    let dir = Self::dir_name(&e)?;
                    let to = match (req.op.as_str(), e.state) {
                        ("disable", SkillState::Enabled) => self.disabled_dir().join(&dir),
                        ("enable", SkillState::Disabled) => self.skills_dir().join(&dir),
                        ("uninstall", _) => self.trash_slot(&dir),
                        ("disable", _) => bail!("'{name}' đang tắt rồi"),
                        _ => bail!("'{name}' đang bật rồi"),
                    };
                    if exists(&to) && req.op != "uninstall" {
                        bail!("{} đã tồn tại", to.display());
                    }
                    actions.push(Action::Move { from, to });
                }
            }
            "install_dir" => {
                let src = PathBuf::from(req.source.as_deref().context("thiếu thư mục nguồn")?);
                let src = std::fs::canonicalize(&src).with_context(|| format!("không mở được {}", src.display()))?;
                let body = std::fs::read_to_string(src.join("SKILL.md")).with_context(|| format!("{} không có SKILL.md", src.display()))?;
                let name = frontmatter::parse(&body).name.filter(|n| !n.is_empty()).context("SKILL.md thiếu `name` trong frontmatter")?;
                let dst = self.check_install_name(&name)?;
                actions.push(if cfg!(unix) { Action::Link { target: src, link: dst } } else { Action::Copy { from: src, to: dst } });
            }
            "install_git" => {
                let url = req.source.clone().context("thiếu git URL")?;
                if url.starts_with('-') {
                    bail!("git URL không hợp lệ");
                }
                let name = url.trim_end_matches('/').rsplit(['/', ':', '\\']).next().unwrap_or_default().trim_end_matches(".git").to_string();
                if name.is_empty() {
                    bail!("không đọc được tên từ URL");
                }
                let dst = self.check_install_name(&name)?;
                actions.push(Action::Clone { url, to: dst });
            }
            "install_market" => {
                let source = req.source.as_deref().context("thiếu repo nguồn (owner/repo)")?;
                if !crate::market::valid_source(source) {
                    bail!("repo nguồn '{source}' không hợp lệ");
                }
                let skill = req.skill.clone().context("thiếu tên skill")?;
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
                    bail!("chưa chọn plugin");
                }
            }
            other => bail!("không có thao tác '{other}'"),
        }
        let mut description = actions.iter().map(|a| format!("• {}", a.describe())).collect::<Vec<_>>().join("\n");
        description.push_str("\nCó hiệu lực từ phiên Claude Code mới. Hoàn tác được trong nhật ký.");
        Ok(Plan { id: new_id(), op: req.op.clone(), description, actions })
    }

    fn run(&self, a: &Action) -> anyhow::Result<()> {
        match a {
            Action::Move { from, to } => {
                if exists(to) {
                    bail!("{} đã tồn tại", to.display());
                }
                std::fs::create_dir_all(to.parent().context("đường dẫn đích không hợp lệ")?)?;
                // rename trên symlink chỉ đổi chỗ chính cái link.
                std::fs::rename(from, to).with_context(|| format!("không chuyển được {}", from.display()))
            }
            Action::Link { target, link } => {
                std::fs::create_dir_all(link.parent().context("đường dẫn không hợp lệ")?)?;
                #[cfg(unix)]
                std::os::unix::fs::symlink(target, link)?;
                #[cfg(not(unix))]
                bail!("symlink chỉ dùng trên Unix: {} → {}", link.display(), target.display());
                #[cfg(unix)]
                Ok(())
            }
            Action::Copy { from, to } => copy_dir(from, to),
            Action::Clone { url, to } => {
                let st = std::process::Command::new("git").args(["clone", "-q", "--depth", "1", "--", url]).arg(to).status().context("không chạy được git")?;
                if !st.success() {
                    bail!("git clone thất bại");
                }
                if !to.join("SKILL.md").exists() {
                    std::fs::remove_dir_all(to)?;
                    bail!("repo không có SKILL.md ở thư mục gốc");
                }
                Ok(())
            }
            Action::Fetch { url, skill, to } => {
                let tmp = self.data_dir.join("tmp").join(new_id());
                std::fs::create_dir_all(tmp.parent().context("thư mục tạm không hợp lệ")?)?;
                let res = (|| {
                    let st = std::process::Command::new("git").args(["clone", "-q", "--depth", "1", "--", url]).arg(&tmp).status().context("không chạy được git")?;
                    if !st.success() {
                        bail!("git clone {url} thất bại");
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

    /// Thực hiện plan; một bước lỗi thì trả lại các bước đã làm.
    pub fn apply(&self, plan: &Plan) -> anyhow::Result<JournalEntry> {
        for (i, a) in plan.actions.iter().enumerate() {
            if let Err(e) = self.run(a) {
                for done in plan.actions[..i].iter().rev() {
                    let _ = self.reverse(done);
                }
                return Err(e.context("đã trả lại các bước trước đó"));
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

    /// Nhật ký, mới nhất trước.
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
        let entry = entries.iter_mut().find(|e| e.id == id).context("không có thao tác này trong nhật ký")?;
        if entry.undone {
            bail!("thao tác này đã được hoàn tác");
        }
        // Bước nào đã về chỗ cũ (lần hoàn tác trước dở dang) thì bỏ qua; lỗi thì làm tiếp bước khác rồi báo.
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
            bail!("hoàn tác chưa xong, có thể thử lại sau khi xử lý:\n{}", failed.join("\n"));
        }
        entry.undone = true;
        let body: String = entries.iter().map(|e| serde_json::to_string(e).map(|s| s + "\n")).collect::<Result<_, _>>()?;
        std::fs::write(self.journal_path(), body)?;
        Ok(())
    }

    /// Xoá hẳn mục trong thùng rác đã quá 30 ngày. Trả về số mục đã xoá.
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

/// Sao chép thư mục; symlink bị bỏ qua (không đi theo link ra ngoài nguồn).
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

/// Chuyển thư mục; khác ổ đĩa (rename lỗi) thì sao chép.
fn move_dir(from: &Path, to: &Path) -> anyhow::Result<()> {
    if exists(to) {
        bail!("{} đã tồn tại", to.display());
    }
    std::fs::create_dir_all(to.parent().context("đường dẫn đích không hợp lệ")?)?;
    if std::fs::rename(from, to).is_err() {
        if let Err(e) = copy_dir(from, to) {
            let _ = std::fs::remove_dir_all(to);
            return Err(e.context(format!("không chép được skill vào {}", to.display())));
        }
    }
    Ok(())
}
