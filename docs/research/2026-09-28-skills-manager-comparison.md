# Comparison with skills-manager and applicable parts (2026-09-28)

- Reference project: `~/outsource/skills-manager` (github.com/xingkongliang/skills-manager), version v1.40.0.
- **MIT** license (Tianliang Zhang): code/data may be reused, just attribute the source.
- Stack: Tauri 2, React 19, Tailwind, Rust, SQLite (rusqlite), react-i18next. About 42,600 lines of Rust, 526 tests.

## 1. Difference in purpose

| | skills-manager | thongke |
|---|---|---|
| Question answered | Which skills are installed for which agent? | How much does AI use cost, where does it go, is it effective? |
| Data | Skill directories of 54 agents | Conversation logs (Claude Code, Kiro) and quota |
| Skills | Central library (`~/.skills-manager`); deploy to each agent via symlink/copy; presets, updates from git, backup to GitHub, skills.sh marketplace | List skills, **baseline cost (tok/session), usage count, where used**; disable/enable/remove with undo |
| Token, quota analysis | None | The main part |

The two apps **complement each other**: thongke knows which skills are *worth keeping*, skills-manager knows where a skill *lives* and how it is distributed.

## 2. Warning: possible conflict on this machine

- The machine has `~/.skills-manager` (the library currently only has `frontend-design`).
- Skills in `~/.claude/skills` are currently **not** symlinks from there, so there is no conflict yet.
- Risk: thongke disables a skill by moving the directory to `~/.claude/skills-disabled` (`crates/tk-skills/src/ops.rs`). If a skill was deployed by skills-manager, it will see an inconsistent state and may redeploy it.

## 3. Worth adopting (by order of benefit)

| # | Work | Reference in skills-manager | Apply to thongke |
|---|---|---|---|
| 1 | **Yield operations to skills-manager** when a skill is a symlink into `~/.skills-manager` | CLI `skills-manager-cli skills undeploy/deploy/status … --json`; the CLI is published at `~/.skills-manager/bin/skills-manager-cli` (with a `.version` file) | `tk-skills`: recognize the "Skills Manager" source, call its CLI instead of moving the directory itself; the UI shows "managed by Skills Manager". If there is no valid CLI (`.version` missing), read-only |
| 2 | **CI installer build** | `.github/workflows/release.yml`: matrix macOS arm64/x64, **ubuntu-22.04** x64, ubuntu-24.04-arm, Windows x64; `test.yml` runs `cargo test` | Build Linux on ubuntu-22.04 to lift the glibc 2.39 limitation (the installer currently only runs on Ubuntu 24.04+). Add Windows/macOS with the same matrix (the missing part of M6) |
| 3 | **Skill path table for 54 agents** | `src-tauri/src/core/tool_adapters.rs` (`default_tool_adapters`: key, `relative_skills_dir`, `relative_detect_dir`, `additional_scan_dirs`, per-project path; prefer `~/.config/...` or `dirs::config_dir()`) | Take only the **path data** so the Skill screen can list skills by tool. The log-reading code still has to be written separately for each tool (adapter in `tk-adapters`). Attribute the MIT source |
| 4 | **Guard against file-operation errors** | `core/path_guard.rs` (`sanitize_name`, `is_path_safe`: canonicalize, check the target is inside the base); `core/sync_engine.rs` blocks nested source/target (issue #61 infinite nesting, #199 deleting the source) | Add to `tk-skills/src/ops.rs` for `install_dir`, `uninstall`, `undo`, with tests |
| 5 | **Diagnostics for a background app** | `core/panic_log.rs` (hook that records crashes), `core/timing.rs` (log only the first call and slow calls), Settings → Export Logs, `core/log_sanitize.rs` (filter sensitive info) | Crash-recording hook for the tray app; time RPCs first-or-slow style; an "Export logs" button in Settings (zip, already filtered) |
| 6 | **CLI for agents + a bundled skill** | `core/cli_bridge.rs`, `skills/manage-skills/SKILL.md`; errors of the form `{"ok": false, "code": …, "message": …}` | Fits M7: `tk … --json` with error codes, plus a skill so Claude can ask itself "how much did I cost today, which conversation was the most expensive" |
| 7 | **Small UI conveniences** | `components/CommandPalette.tsx` (Ctrl+K), `SkillMarkdown.tsx` (view SKILL.md), settings `show_tray_icon` / `close_action`, `tauri-plugin-updater`, font size | Ctrl+K command palette; SKILL.md preview in the Skill screen; tray options (hide icon, close window to tray or quit); app auto-update |

## 4. Not worth taking

- **Backup/sync over Git, marketplace, presets**: outside thongke's purpose. If needed, just use skills-manager.
- **Agent logo set** (`public/agent-icons/`): brand logos, not covered by the MIT license. thongke keeps the colored letter box (`SourceInfo.letter`/`color`).
- **React UI code**: thongke uses plain TypeScript + `h()`; take only the ideas.

## 5. Suggested order of work

1. Item 1 (avoid a real conflict on this machine) and item 2 (resolves most of M6).
2. Item 4 (file safety) goes with item 1, since both change `tk-skills/ops.rs`.
3. Items 5 and 7 when doing M6.
4. Item 3 when expanding to new tools (OpenCode…); item 6 when doing M7.

## 6. Feature ideas worth bringing over (combined with thongke data)

Section 3 above is technical. This section is **user-visible features**. thongke's unique strength is usage data, so each idea is augmented with that data.

| # | In skills-manager | Bring to thongke | Value |
|---|---|---|---|
| A | **Presets**: group skills, one click to enable/disable a whole group for an agent | **Context-based skill sets with cost**: each set shows its total baseline cost (tok/session). Auto-**suggest sets by project** from logs: "project X only used pj-review, pj-estimate in 30 days → this set saves ~2K tok/session" | High: reduces baseline cost, right on the optimization goal |
| B | **Track updates + compare with upstream** | **Measure the impact when a skill changes**: record when SKILL.md changes (content hash), compare average tokens per use before and after. Example: "after editing pj-review on 20/09, the average dropped from 5.7M to 3.1M" | High: no other tool has it |
| C | **Activity journal** | **Mark events on the daily chart**: disable/enable a skill, change model, enable statusline. See immediately whether that change reduced tokens | High, quick to build (`skill_ops` already exists) |
| D | **Project Workspace** (a repo-local skill in `.claude/skills`) | Auto-scan project-level skills from the `cwd` already present in logs, no need for the user to add projects. Shows the **real** baseline cost of each project (global skills + project skills) | Medium–high |
| E | **Agent badges on skill cards** (which agents have the skill) | Each skill shows tool badges **with usage count per tool** | Medium |
| F | **Tag + "untagged" filter** | Tag **conversations** (bug, docs, review…), aggregate tokens by tag in the Overview: "review work costs 40% of tokens" | Medium–high |
| G | **⌘K command palette** | Jump quickly to a conversation, project, skill, model, settings; commands "filter anomalous cost", "switch Kiro scope" | Medium, quick to build |
| H | **Let agents self-manage** (a `manage-skills` skill + CLI) | A **`thongke`** skill for Claude: "how much did I cost today", "how much quota is left before a long task", "what context is this conversation at, should I /compact" | High, tied to M7 |
| I | **First launch** (start fresh or restore; auto-detect language from the OS) | Onboarding: detect tools present on the machine, choose language (default per OS), enable the quota statusline in one step | Medium (already in progress) |
| J | **Prompt on window close**: minimize to tray or quit, remember the choice | Do the same; currently closing the window always keeps it running in the background, so newcomers easily think the app has quit | Low, very quick to build |
| K | **Multi-select + batch operations** | Select multiple conversations to tag or ⭐; select multiple skills to disable/enable | Low–medium |
| L | **Quick guide** | A "how to read the numbers" box: real vs estimated tokens, baseline cost, why re-reading context is costly | Low |

**Do first:** C (quick, immediate payoff), A (right on the cost-reduction goal), B (the differentiator), H (when doing M7).

## 7. How skill management (install, disable, remove): should we follow it?

| | thongke (`tk-skills`) | skills-manager |
|---|---|---|
| Where skills live | Right in each tool's directory (`~/.claude/skills`, …) | **Central library** `~/.skills-manager/skills`; *deployed* to each agent via symlink or copy |
| Install | From a directory (symlink) or from git | Git (with subdirectory URL `…/tree/main/skills/x`), directory, `.zip`/`.skill`, marketplace, `owner/repo@skill` |
| Disable | Move to `skills-disabled`, with undo | None. Only "undeploy" (remove from the agent); the library still keeps it |
| Remove | 30-day trash, with undo | Delete from the library and every agent (asks for confirmation) |
| Update | None | Content hash vs upstream, view diff, report files that will be lost (`removals.rs`) |
| Safety | Plan → confirm → execute → journal → undo; a symlink only touches the link | `TargetState` (Absent / LinkToSource / ForeignLink / RealDir / RealFile, read via `symlink_metadata`) + `ReplacePolicy` (NoClobber / Recorded / UserConfirmed); `preflight_replace` checks the whole batch first (issue #363) |

**Conclusion:** **do not** copy the central-library model. That is the core of skills-manager and very heavy (`commands/skills.rs` ~4,200 lines, `git_fetcher.rs` ~2,700 lines). Rebuilding it would be like rewriting their app, and it easily conflicts on a machine using both apps. Keep thongke's current disable/remove with undo.

**Take in parts:**

1. A skill deployed by skills-manager → yield operations to its CLI (see item 3.1). To install for many agents, use skills-manager.
2. Target classification + default not to overwrite (`TargetState`/`ReplacePolicy`) for `tk-skills/ops.rs`: recognize foreign links and broken links; refuse when unsure the thing is ours.
3. Preflight the whole batch for "batch disable": if one skill can't be handled, stop everything before touching any skill.
4. Add install sources: a GitHub URL pointing to a subdirectory, `owner/repo@skill`, `.zip`.
5. Check for updates to git-installed skills via content hash (goes with idea 6B: tokens before and after the skill changes).
6. Report in advance which files will be lost when updating a skill.

**Not taking:** central library, synced presets, Git backup, marketplace.

## 8. UI: already brought over (2026-09-28)

Plan: `docs/superpowers/plans/2026-09-28-ui-from-skills-manager.md`.

- **Tool logos** Claude Code, Kiro (`app/ui/public/tool-icons/`, brand notice in `NOTICE.md`); if missing or broken, fall back to the letter box. Brand identity colors: Claude `#d77655`, Kiro `#9046ff`.
- **Skill screen**: pill-style counts, the "costly but unused" warning label (click to filter), status/source filter chips, dashed-border chip; switch **Table / Cards**; cards have an enable/disable toggle (still with confirmation + undo).
- **Search on skills.sh**: card-style results, "✓ Installed", a filter chip by source repo.
- **Conversations**: pill-style counts, conversation count per tool. **Overview**: 3 suggestions as cards with action buttons.
- Not doing: a row of logos for every agent on the card, a card grid for Conversations, changing the primary accent color, a collapsible sidebar (the sidebar currently has only one group).
