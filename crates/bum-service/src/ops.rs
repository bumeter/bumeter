//! The Skills and Settings screens' commands (`skills:*`, `settings:*`), shared by the app and `bum rpc`
//! so those screens work the same in the desktop app and in an editor. A skill operation is always
//! planned by the core first; the UI may only apply a plan it was given (spec section 8).
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use serde_json::{json, Value};
use bum_adapters::claude_code::statusline::{Mode, StatuslineSetup, SHOW_SUPPORTED};
use bum_core::config::Config;
use bum_core::handlers::skill_stats;
use bum_core::paths::Paths;
use bum_core::store::Store;
use bum_skills::catalog::scan_personal;
use bum_skills::ops::{Manager, OpRequest, Plan};
use bum_skills::plugins::{scan_plugins, ClaudeCli};
use bum_skills::report::{build, Usage};

pub struct Ops {
    paths: Paths,
    /// The `bum` the statusline hook should call.
    bum_bin: String,
    version: String,
    /// Plans made but not applied yet: id → (tool, plan).
    pending: Mutex<HashMap<String, (String, Plan)>>,
}

impl Ops {
    pub fn new(paths: Paths, bum_bin: impl Into<String>, version: impl Into<String>) -> Self {
        Ops { paths, bum_bin: bum_bin.into(), version: version.into(), pending: Mutex::new(HashMap::new()) }
    }

    /// `None` when `method` is not `skills:*` or `settings:*`.
    pub fn call(&self, method: &str, params: &Value) -> Option<Result<Value, String>> {
        if let Some(action) = method.strip_prefix("skills:") {
            return Some(self.skills(action, params));
        }
        method.strip_prefix("settings:").map(|action| self.settings(action, params))
    }

    fn kiro_dir() -> PathBuf {
        bum_core::paths::home().join(".kiro")
    }

    /// One Manager per tool, with the same `skills/` + `skills-disabled/` layout; trash/log kept separate.
    fn manager(&self, tool: &str) -> Manager {
        if tool == "kiro" {
            Manager::new(Self::kiro_dir(), self.paths.data_dir.join("kiro"), ClaudeCli::system())
        } else {
            Manager::new(StatuslineSetup::default_claude_dir(), self.paths.data_dir.clone(), ClaudeCli::system())
        }
    }

    fn usage(&self) -> Option<Vec<Usage>> {
        let store = Store::open_readonly(&self.paths.db()).ok()?;
        Some(skill_stats(&store)
            .ok()?
            .into_iter()
            .map(|s| Usage { name: s.name, uses: s.uses, projects: s.projects, tokens: s.tokens, avg_tokens: s.avg_tokens, last_used: s.last_used })
            .collect())
    }

    pub fn skills(&self, action: &str, params: &Value) -> Result<Value, String> {
        let tool = params["tool"].as_str().unwrap_or("claude_code").to_string();
        let m = self.manager(&tool);
        let err = |e: anyhow::Error| format!("{e:#}");
        match action {
            "list" => {
                let _ = m.purge_trash(chrono::Utc::now());
                let _ = self.manager("kiro").purge_trash(chrono::Utc::now());
                let mut entries = scan_personal(&StatuslineSetup::default_claude_dir());
                entries.extend(bum_skills::catalog::scan_personal_for(&Self::kiro_dir(), "kiro"));
                // `plugins: false`: return personal skills immediately; the UI makes a second call to fetch plugins (the CLI can be slow).
                let mut warnings = Vec::new();
                if params["plugins"].as_bool().unwrap_or(true) {
                    let (plugins, w) = scan_plugins(&ClaudeCli::system());
                    entries.extend(plugins);
                    warnings = w;
                }
                Ok(serde_json::to_value(build(entries, self.usage(), warnings, chrono::Utc::now())).unwrap_or_default())
            }
            "plan" => {
                let req: OpRequest = serde_json::from_value(params.clone()).map_err(|e| e.to_string())?;
                let plan = m.plan(&req).map_err(err)?;
                let out = json!({ "id": plan.id, "description": plan.description });
                self.pending.lock().map_err(|e| e.to_string())?.insert(plan.id.clone(), (tool, plan));
                Ok(out)
            }
            "apply" => {
                let id = params["id"].as_str().ok_or("missing 'id'")?;
                let (plan_tool, plan) = self.pending.lock().map_err(|e| e.to_string())?.remove(id).ok_or("plan does not exist or was already used")?;
                self.manager(&plan_tool).apply(&plan).map(|e| serde_json::to_value(e).unwrap_or_default()).map_err(err)
            }
            "undo" => {
                let id = params["id"].as_str().ok_or("missing 'id'")?;
                m.undo(id).map(|_| json!(true)).map_err(err)
            }
            "search" => {
                // skills.sh is only a catalog; installing clones directly from GitHub (op `install_market`).
                let q = params["q"].as_str().unwrap_or_default();
                let limit = params["limit"].as_u64().unwrap_or(20) as u32;
                bum_skills::market::search(q, limit).map(|h| serde_json::to_value(h).unwrap_or_default()).map_err(err)
            }
            "top" => {
                // Leaderboard shown as soon as the install dialog opens: Hot / Trending (24h) / All time.
                let mode = params["mode"].as_str().unwrap_or("trending");
                let page = params["page"].as_u64().unwrap_or(1) as u32;
                bum_skills::market::leaderboard(mode, page).map(|p| serde_json::to_value(p).unwrap_or_default()).map_err(err)
            }
            "count" => {
                // Installed personal skills for the sidebar badge. Plugins need the `claude` CLI (slow),
                // and usage counters need the DB, so neither is included here.
                let claude = scan_personal(&StatuslineSetup::default_claude_dir()).len();
                let kiro = bum_skills::catalog::scan_personal_for(&Self::kiro_dir(), "kiro").len();
                Ok(json!({ "claude_code": claude, "kiro": kiro, "total": claude + kiro }))
            }
            "audit" => {
                // Read-only scan of every SKILL.md it can find, across both tools.
                let mut entries = scan_personal(&StatuslineSetup::default_claude_dir());
                entries.extend(bum_skills::catalog::scan_personal_for(&Self::kiro_dir(), "kiro"));
                let (plugins, _) = scan_plugins(&ClaudeCli::system());
                entries.extend(plugins);
                Ok(serde_json::to_value(bum_skills::audit::audit(&entries)).unwrap_or_default())
            }
            "log" => m.log().map(|l| serde_json::to_value(l).unwrap_or_default()).map_err(err),
            other => Err(format!("no such action '{other}'")),
        }
    }

    fn statusline(&self) -> StatuslineSetup {
        StatuslineSetup::new(StatuslineSetup::default_claude_dir(), self.paths.data_dir.join("backups"), self.bum_bin.clone())
    }

    /// `save_config` returns the saved config; the app also switches its own language from it.
    pub fn settings(&self, action: &str, params: &Value) -> Result<Value, String> {
        let err = |e: anyhow::Error| e.to_string();
        let s = self.statusline();
        // `params.mode`: `"show"` puts Bumeter's part on the status line, `"record"` (the default) only records the numbers.
        let mode = || serde_json::from_value::<Mode>(params["mode"].clone()).unwrap_or(Mode::Record);
        match action {
            "get" => {
                let parse_errors = Store::open_readonly(&self.paths.db()).ok().and_then(|st| st.parse_errors().ok()).unwrap_or(0);
                Ok(json!({
                    "config": Config::load(&self.paths.config()),
                    "statusline": s.status().map_err(err)?,
                    "statusline_show": SHOW_SUPPORTED,
                    "data_dir": self.paths.data_dir,
                    "parse_errors": parse_errors,
                    "tool_quotas": bum_core::meter::read_quotas(&self.paths.tool_quotas()),
                    "version": self.version,
                }))
            }
            "statusline_plan" => {
                let plan = s.plan_for(mode()).map_err(err)?;
                Ok(json!({ "description": plan.description, "noop": plan.is_noop() }))
            }
            "statusline_apply" => s.plan_for(mode()).and_then(|p| s.apply(&p)).map(|_| json!(true)).map_err(err),
            "statusline_undo" => s.undo().map(|_| json!(true)).map_err(err),
            "save_config" => {
                let cfg: Config = serde_json::from_value(params["config"].clone()).map_err(|e| e.to_string())?;
                cfg.save(&self.paths.config()).map_err(err)?;
                Ok(serde_json::to_value(cfg).unwrap_or_default())
            }
            other => Err(format!("no such action '{other}'")),
        }
    }
}
