# thongke — AI usage statistics app design

- Date: 2026-09-24
- Status: Awaiting approval of the written version (content approved piece by piece)
- User: individual (an outsource developer working from tickets), using Claude Code daily

## 1. Goals

### 1.1 Problem

The user does not know how many tokens each AI prompt costs, which skills are used, or why some prompts are very expensive, so they do not know what to optimize. `/context` only shows the context of the current session at one point in time; it has no history and cannot be compared.

### 1.2 Desired outcome

A lightweight desktop app that **runs in the background continuously** and reads the logs of AI tools in real time. Opening the app answers 3 questions right away:

1. Which prompt is the most expensive, and why?
2. Which skills cost a lot or a little per use, and where are they used?
3. What habits should change?

The app can also:

- Warn when the usage limit (5 hours / 7 days) is nearly reached and when there is an anomalous prompt.
- Install, disable, enable, and remove skills and plugins right in the app.
- Deep-analyze each session: context composition, compact chart, view tool-call content.
- Have an architecture that allows adding other tools later (Codex, Kiro, Gemini…).
- Run on Windows, macOS, and Linux.

### 1.3 Success criteria

- Displayed token counts match an independent count (`tk audit`) 100%.
- From a new log line to the UI or tray updating: under 100ms.
- RAM while running in the background (window closed): under 20MB. CPU near 0% when there is no new log.
- First scan of about 34MB of logs (on the current machine): under 2 seconds.

### 1.4 Product principles

**A single app, centrally managed.** The user does not have to assemble multiple tools (ccusage, claude-devtools, TokenTracker…). Per-prompt analysis, context composition analysis, quota, notifications, and skill management all live in the same app.

### 1.5 Out of scope for v1

Codex, Kiro, and Gemini adapters (only design the trait); report export; syncing skills between tools; editing skill content in the app; browsing a public skill store; multi-machine or team use.

### 1.6 Survey of similar tools (09/2026)

Information taken from READMEs. ccusage was actually tried on the user's machine.

| Tool | Type | Has | Missing |
|---|---|---|---|
| ccusage | CLI | Stats by day, session, 5h block, about 18 tools, with dedup (matches the numbers: 726M) | Per-prompt stats, skills, tray, notifications |
| claude-devtools | Electron, 3 OSes | Splits each call's tokens into 7 groups (CLAUDE.md, skill, file, tool I/O, thinking…), compaction chart, notifications when costly | Tray and 5h/7d quota, skill management; heavy because it is Electron |
| TokenTracker | Tray, 3 OSes | Realtime limits, over 40 tools, enable/disable and sync skills with Undo | Per-prompt stats, skill cost vs. usage |
| Claude-Code-Usage-Monitor v4 | TUI | Reads `rate_limits` from statusline, predicts usage rate | OS notifications, tray |
| ccboard | TUI and web | Skills/Plugins tab, budget warnings | Unclear whether it has skill management |

**thongke's differentiators:**
1. Gathers everything into one lightweight app (Tauri).
2. Explains **"why it's costly and what to do"** right while working.
3. Quota notifications based on the real % from statusline, works on both Linux and Windows.
4. Skill management **based on data**: compare baseline cost with real usage, then disable/enable/remove.

## 2. Findings from real data

Survey of `~/.claude/projects` on the user's machine (24 `.jsonl` files, about 34MB, 595 prompts):

- **The log repeats records:** each content block (text, tool_use…) of an API call is written as its own `type=assistant` line, and these lines carry the **same `usage`**. Dedup by `requestId` is mandatory (fallback: `message.id`). Without dedup the total is 1.40 billion tokens; after dedup it is 721 million.
- The median is about 678K tokens per prompt, p90 about 2.86M.
- Expensive prompts are mostly due to **large context**, not output. For example: one prompt cost 9.4M tokens across 11 API calls with a context of about 841K, and only 17K of output.
- Skills are recorded in `attributionSkill` (on the assistant line) and in the `Skill` tool_use (`input.skill`). Slash commands are inside `<command-name>` in the user message.
- The log only records limits **once blocked**: the `isApiErrorMessage` line has `error: "rate_limit"`, along with `quotaLimits.rateLimitType` (`five_hour`…) and `resetsAt`.
- The limit percentage **before being blocked** is only in the statusLine stdin JSON (officially documented): `rate_limits.five_hour.used_percentage`, `rate_limits.five_hour.resets_at`, `rate_limits.seven_day.*`. It is only available on Pro/Max plans and appears after the first API response. Hooks, `/usage`, and OpenTelemetry do not provide this number.
- `claude plugin details <name>` reports the baseline (always-on) tokens and the per-call (on-invoke) tokens of each sub-skill in a plugin.
- The machine already has `~/.codex/sessions`, `~/.kiro/{sessions,skills}`, `~/.gemini` for adapters later.

## 3. Architecture

### 3.1 Technology

- **Tauri 2 + Rust** for the core. The UI is **plain HTML/CSS/TypeScript** running in the OS WebView, with no framework, built with Vite.
- **Why chosen:** the lightest of the options that still has a full UI. File watching via OS events (the `notify` crate: inotify / FSEvents / ReadDirectoryChangesW). Tray, notifications, and autostart are available.
- **RAM savings:** closing the window **destroys the WebView**, leaving only the Rust core and the tray running in the background. Reopening recreates the WebView (about 0.3 seconds).
- Storage in **SQLite** (the `rusqlite` crate, bundled build). File location via the `dirs` crate: `data_dir()/thongke/thongke.db`, for example `~/.local/share/thongke/`.

### 3.2 Diagram

```
┌──────────── Tauri window (WebView) — plain HTML/TS ────────────┐
│   Prompts · Skill · Project · Day · Settings                   │
└──────────▲ event "data-changed"  │ invoke(query_*, skill_*, …) ┘
           │                       ▼
┌────────────────────────── Rust core (background) ──────────────┐
│ Watcher(notify) → Adapter.parse → Aggregator → Store(SQLite)   │
│                                          │                     │
│                         Insight/Alert ◄──┘ → Tray + Notification│
│  LimitWatcher (limits.json) ─────────────┘                     │
│                                                                │
│  trait LogSource    trait SkillSource                          │
│   └─ claude_code    └─ claude_code        (v1)                 │
│   └─ codex / kiro   └─ kiro               (later)              │
└────────────────────────────────────────────────────────────────┘
```

The UI and the core communicate through Tauri's internal IPC. **No network port is opened.**

### 3.2.1 Many UIs, one core (host + client)

```
                    ┌──────────── tk-core + tk-adapters + tk-skills ────────────┐
                    │  watcher → store(SQLite) → insight/alerts  (1 writer)     │
                    └──────────────────────────┬────────────────────────────────┘
                                 tk-service: local API (JSON-RPC)
                     Unix socket (Linux/macOS) · Named pipe (Windows)
            ┌──────────────┬───────────────┼────────────────┬──────────────────┐
            ▼              ▼               ▼                ▼                  ▼
     Desktop app      Terminal TUI     VS Code ext     Claude statusline   (later) Web
     (Tauri, tray)    `tk tui`         status bar +    `tk statusline`
                                      webview panel
```

- The **host** is the single process that watches logs and writes the DB. The host is the desktop app if it is running, otherwise it is `tk daemon` running without a UI. A file lock `data_dir()/thongke/host.lock` ensures there is only one host. The DB runs in WAL mode.
- **Clients** include the TUI, VS Code, and the statusline. A client calls `tk-service` over a socket or pipe. If there is no host yet, the client starts `tk daemon` itself.
- The socket is accessible only to the current user (mode `0600`, on Windows a per-user pipe ACL), and does not open a network port.
- `tk-service`'s command set is **identical** to the Tauri commands in section 14.4. Both call the same `handlers` layer, so each command's logic is written only once. The Tauri UI calls directly in-process, not through the socket.

### 3.3 Modules (Rust)

| Module | Responsibility | Depends on |
|---|---|---|
| `adapters::LogSource` (trait) | `id()`, `watch_roots()`, `parse_line(ctx, line) -> Vec<RawEvent>` | — |
| `adapters::SkillSource` (trait) | `list()`, `enable()`, `disable()`, `install()`, `uninstall()`, `cost()` | — |
| `adapters::claude_code` | Implements both traits for Claude Code | `serde_json` |
| `watcher` | Receives events from `notify`, reads the file onward from its offset, rescans every 60 seconds | `notify`, `store` |
| `aggregator` | Turns `RawEvent` into Session / Prompt / ApiCall / SkillUse; dedups; attributes skills | `store` |
| `store` | SQLite schema, migrations, queries | `rusqlite` |
| `insight` | Flags anomalies, suggests optimizations, computes USD cost | `store`, `pricing` |
| `limits` | Reads `limits.json` and 429 events, computes limit state | `notify` |
| `alerts` | Decides when to send notifications (with anti-repeat), updates the tray | `tauri-plugin-notification` |
| `skills_ops` | Runs skill operations with confirmation, operation journal, undo, trash | `SkillSource`, `store` |
| `composition` | Estimates the context composition of each ApiCall, re-reading tool I/O content from the log by offset (section 6.1) | `adapters`, `store` |
| `handlers` | Shared handling layer for every command (used by both Tauri commands and `tk-service`) | all |
| `service` | JSON-RPC over Unix socket / named pipe, host lock | `handlers` |
| `commands` | Tauri `invoke` commands, just wrapping `handlers` | `handlers` |

The UI (`ui/`) has one file per screen: `prompts.ts`, `skills.ts`, `projects.ts`, `days.ts`, `settings.ts`, `session.ts` (Session analysis), and `detail.ts` for the detail panel. Charts are hand-written SVG, with no chart library, to keep the app light.

## 4. Common data model

### 4.1 Concepts

| Concept | Main fields | Claude Code |
|---|---|---|
| **Session** | id, tool, project, cwd, file_path, started_at, ended_at, deleted | One `.jsonl` file / `sessionId`. Project is taken from the parent directory name |
| **Prompt** | id, session_id, ts, text (up to 2,000 characters), interrupted, compacted_after | A `type=user` line with `promptId`, not `isMeta`, no `toolUseResult`, not starting with `<`, and not `[Request interrupted…]` |
| **ApiCall** | request_id (unique), prompt_id, ts, model, input, cache_write, cache_read, output, context, tools_json | A `type=assistant` line, **deduped by `requestId`**. `context = input + cache_write + cache_read`. Tools merged from every line with the same `requestId` |
| **SkillUse** | prompt_id, skill, kind (`direct` / `continued` / `command`), source | `Skill` tool_use, `attributionSkill`, `<command-name>` |

A `[Request interrupted by user]` line marks `interrupted=true` for the prompt immediately before it. An `isCompactSummary` line marks `compacted_after` and cuts the skill's "continued" chain.

### 4.2 Per-prompt metrics (computed at query time or cached in the `prompt_stats` table)

Totals for each token type, `total`, `context_at_start` (of the first ApiCall), `api_calls`, `tools` (counted by name), `duration` (from the prompt to the last ApiCall), `cost_usd`.

### 4.3 Skill attribution rules

- **direct:** the prompt has a `Skill` tool_use, has `attributionSkill`, or has a slash command (kind `command`).
- **continued:** later prompts in the same session inherit the nearest direct skill, until one of three cases: a different direct skill, a compact, or a prompt more than 30 minutes after the previous one.
- UI: a bold tag is direct, a light tag is continued.

### 4.4 Anomaly rules (adjustable thresholds)

`total > 3 × median (last 30 days)`, **or** `context_at_start > 300_000`.

Color bar display: green when below 2× the median, yellow from 2× to below 5×, red from 5× up.

### 4.5 Cost

The model pricing table is stored in `pricing.toml` (in the data dir, with a default bundled with the app, editable by the user). Pricing is separate for input, cache_write (5 minutes / 1 hour), cache_read, and output. A model not in the table shows "—", without guessing a price. The default table is taken from the official pricing page at deployment time.

### 4.6 Auxiliary SQLite tables

- `file_offsets(path, inode/file_id, size, offset, mtime)`
- `notes(prompt_id, starred, note, updated_at)`, backed up in parallel to `notes.json`
- `skill_ops(id, ts, op, target, before_json, after_json, status)` used for undo
- `alerts_sent(key, ts)` to prevent duplicate notifications
- `schema_version`

## 5. Realtime log reading

1. On startup: list the files in `watch_roots()`. For each file, compare with `file_offsets`, then read from `offset` to the end.
2. When `notify` reports a file change, read onward from `offset`. **Only process up to the last `\n` character.** The remainder is kept for the next read.
3. If `size < offset` or the inode/file_id changes, re-read the file from the start. Delete the old ApiCalls of that file before re-reading (dedup by `request_id` helps avoid double counting).
4. A deleted file: keep the data and set `Session.deleted = true`.
5. Rescan every 60 seconds (by size and mtime) to catch missed events, for example when the machine sleeps or inotify overflows.
6. Each batch of data is written in one transaction, then a `data-changed { session_ids, prompt_ids }` event is sent to the UI (if open) and to the `alerts` module.

## 6. UI

3-column layout (per the approved mockup v4):

- **Left sidebar:**
  - Screens: Prompts, Skill, Project, Day, Settings.
  - Quick filters: 🔥 anomalous cost, 🧠 context above 300K, ✋ interrupted, ⭐ needs review.
  - Today's metrics.
  - A "Tools" filter (hidden when there is only one adapter).
- **Middle column, prompt list:**
  - Grouped by session.
  - A search box over question content. Chip-style filters: project, skill, time range, sort.
  - Each row has: time, question, color bar, skill tags, tokens.
  - New prompts are appended automatically while running.
- **Right column, one prompt's detail:**
  - Total tokens, number of API calls, context at prompt time, actual output, cost.
  - A bar chart per API call (the context-reading part and the output part).
  - The tools used.
  - A **"Why it's costly / What to do"** section generated by `insight`.
  - ⭐, 📝 buttons and "Open log file location".
  - A **"Deep analysis"** button to open the Session analysis screen (section 6.1).
- **Skill:** a table of personal skills, skills inside plugins, slash commands, and skills that are **installed but unused**. There are columns for source, status, baseline cost, cost per call, usage count, average tokens per use, last use, and the tools that have the skill. Clicking a skill filters the middle column by that skill. See section 8 for the operation buttons.
- **Project:** each project has total tokens, session count, skills used, and the most expensive prompt. Clicking filters by project.
- **Day:** a bar chart of tokens by day. Clicking a bar filters by that day.
- **Settings:** thresholds, per-adapter paths, autostart, the pricing table, per-type notification on/off, statusline setup (section 7), the count of error log lines, the skill operation journal.

The display time zone follows the system. The UI follows the system light/dark setting.

### 6.0 Design language (approved 2026-09-24)

The app is **cross-platform** (Linux, Windows, macOS). The UI follows a **macOS style**, but is not a macOS-only app. Reference mockup: `2026-09-24-thongke-mockup-macos.html`.

- **Title bar:** use the **native title bar of each OS**, don't draw your own close/minimize buttons. Drop the 3 colored macOS buttons from the mockup.
- **Font:** `-apple-system, "SF Pro Text"` on macOS. Other OSes use **Inter**, embedded in the app (woff2, works offline). Numbers use `font-variant-numeric: tabular-nums`.
- **Colors:** declared as tokens on `:root`, with full light and dark modes following the system (`prefers-color-scheme`). The accent is the system blue (`#007aff` in light, `#0a84ff` in dark). Status colors: green, orange, red, purple (skills).
- **Details:** 0.5px hairlines; 6–12px corner radii; the selected row filled with the accent color and white text; 11px gray group labels; buttons with a thin border; stat cards on a light gray background; no heavy borders or harsh colors.
- **Translucent sidebar:** use `backdrop-filter` if the WebView supports it, otherwise a solid background color.
- **Interaction:** ↑/↓ to select a prompt, `⌘/Ctrl+F` to focus the search box, `Esc` to clear filters.

### 6.1 Session analysis (devtools-style deep analysis)

A full-page screen for one session, opened from the detail panel or from the session list.

- **Context chart over time:** the X axis is each API call, the Y axis is context. Overlapping areas show the **context composition**. Marks show the compact points (where context drops) and the user's prompts.
- **Context composition of each call (estimated):**

  | Group | How it is identified in the log |
  |---|---|
  | System / CLAUDE.md / memory | `attachment`s and `isMeta` messages at the start of the session or after a compact |
  | Skill | Content loaded when the `Skill` tool is called (the Skill tool_result) |
  | Files read | tool_result of Read, Grep, Glob, and Bash commands that read files |
  | Other tool I/O | tool_use input and the remaining tool_results |
  | Thinking | The assistant's `thinking` blocks |
  | Assistant output | The assistant's text and tool_use blocks |
  | User questions | User messages with a `promptId` |

  **How the estimate works:**
  - Count the characters of each block and convert to tokens with a factor (default 1 token ≈ 3.5 characters; adjustable).
  - Accumulate in conversation order. After each compact, reset, and the summary total is then counted into the "System" group.
  - Correct against the real `usage`: multiply every group by the same ratio so the total equals the real `context` of that call.
  - The UI always labels it clearly as an **"estimate"**.
- **Top heaviest blocks** in the current context, for example: "Bash tool_result at 11:30, about 120K tokens". Click to view the content.
- **View tool calls:** the input/output content of each tool. Edit and Write are shown as a diff, Bash shows the command and output (auto-truncated, with a "view all" button), Read shows the path and line count.
- **Suggestions from the analysis:**
  - "File X was read 4 times, costing about 200K."
  - "The Bash output is too long; use `| head`."
  - "Skill Y loads 9.7K tokens per call."

Tool I/O content is **not stored in SQLite**. When this screen opens, the app re-reads directly from the log file at the saved offset, so the DB stays small. The DB only stores the aggregate per-group numbers for each ApiCall, in the `call_composition(request_id, category, est_tokens)` table.

## 7. Usage limits, tray, and notifications

### 7.1 Sources of limit data

- **Before being blocked:** taken from statusLine via the **`tk statusline`** command. This command reads JSON from stdin, writes `rate_limits` to `data_dir()/thongke/limits.json` (temp-file-then-rename), then prints **one concise status line**, for example `🟡5h 78% ↻14:30 · this call 620K · ctx 450K · pj-review`. No `jq` needed; works on Windows too.
  - If the user already has a statusLine script (on this machine it is `~/.claude/statusline-command.sh`): the Settings screen has an "Enable limit tracking" button. When clicked, the app shows exactly the snippet that will be added to the script, then **backs up** the script. The snippet is `echo "$input" | tk statusline --quiet >/dev/null` (`--quiet` only writes the file, prints nothing), so the script's current display stays as it is.
  - If the user wants to use the app's status line directly: set `statusLine.command = "tk statusline"`. When there is no statusLine yet: the app asks first before creating a new script and adding `statusLine` to `~/.claude/settings.json`. Editing settings is an operation with confirmation, backup, and undo. There is a remove button to restore the backup.
- **Already blocked:** taken from a log line with `error: "rate_limit"`, using `quotaLimits.resetsAt` and `rateLimitType`.
- If `limits.json` is older than 10 minutes, the tray shows "updated X minutes ago". No alarm when the data is stale.

### 7.2 Tray

- The icon changes color by the highest limit level: green below 70%, yellow from 70%, red from 90% or when already blocked.
- The menu contains: 5 hours and 7 days (% and reset time), today's tokens and cost, the latest prompt, Open dashboard, Pause notifications for 1 hour, Quit.

### 7.3 Notifications (each type can be turned on/off and have its threshold adjusted)

| Type | Condition | Anti-repeat |
|---|---|---|
| Limit | 5h or 7 days exceed 70% / 90% | Each threshold is reported only once per reset cycle |
| Blocked | There is a 429 error, re-reported at `resetsAt` | Each `resetsAt` is reported only once |
| Anomalous prompt | A prompt satisfying section 4.4 | Each prompt is reported once, when the prompt ends (no new ApiCall for 5 seconds) |
| Large context | `context` exceeds 300K in a running session | Each session is reported only once per threshold |

Clicking a notification opens the dashboard at that exact prompt or session.

## 8. Skill management

### 8.1 Listing

- Personal skills: subdirectories of `~/.claude/skills/` that have a `SKILL.md` (read the `name` and `description` frontmatter). Disabled skills are in `~/.claude/skills-disabled/`.
- Plugins: `claude plugin list` and `claude plugin details <name>` (get the list of sub-skills and the baseline cost / cost per call).
- The cost of a personal skill is an estimate: the baseline cost is based on the length of `description`, the cost per call on the length of `SKILL.md`. The character-to-token formula is labeled clearly as an estimate.
- Suggestion: a skill unused for 30 days with a baseline cost greater than 0 is suggested for disabling.

### 8.2 Operations

| Operation | Personal skills | Plugin |
|---|---|---|
| Disable | Move `skills/<n>` to `skills-disabled/<n>`. If it is a symlink, only move the link | `claude plugin disable` |
| Enable | Move back | `claude plugin enable` |
| Install | From a directory (Linux/macOS: symlink; Windows: copy) or a git URL (`git clone`). Check for a `SKILL.md` with valid frontmatter | `claude plugin install <plugin@marketplace>` |
| Remove | Move to `data_dir()/thongke/trash/<ts>-<n>`, kept for 30 days. If it is a symlink, only delete the link | `claude plugin uninstall` |

**Principles:**
- Plugins are only operated through the official CLI; the app never edits the plugin config files itself.
- Every operation goes through a confirmation dialog listing the affected files, and is written to `skill_ops` so it can be **undone**.
- On a duplicate skill name, ask the user whether to overwrite or rename.
- If an operation fails, undo what was done and show the original error message.
- After each operation, the app notes: "Takes effect from the next Claude Code session."

## 9. Extending to other tools

Adding a new tool consists of:
1. Create `adapters/<tool>.rs` implementing `LogSource` (and `SkillSource` if the tool has an equivalent concept).
2. Register the adapter in the registry.
3. Add default paths for each OS.

Core, store, and UI do not need changes. The `Session.tool` field allows filtering by tool. Usage limits are optional per adapter (`fn limits() -> Option<LimitState>`).

## 10. Cross-platform

| | Windows | macOS | Linux |
|---|---|---|---|
| Installer | `.msi` / NSIS `.exe` | `.dmg` | `.deb` / `.AppImage` |
| Claude logs | `%USERPROFILE%\.claude\projects` | `~/.claude/projects` | `~/.claude/projects` |
| Tray | Yes | Menu bar | Needs AppIndicator. If there is no tray, it still runs via a window and notifications |
| Autostart | `tauri-plugin-autostart` | like Windows | like Windows |

- Only one app instance: use `tauri-plugin-single-instance`.
- Build with GitHub Actions using a 3-OS matrix.
- Develop and test for real on Linux. Manual testing on Windows and macOS needs a real machine or a VM.

## 11. Error handling

| Situation | Handling |
|---|---|
| A broken or half-written JSON line | Only read up to the last `\n`. A broken line is skipped and counted in `parse_errors` |
| The log format changes | Ignore unknown fields; missing optional fields are left empty. On an unseen `version`, show a mild warning |
| A file rewritten smaller or deleted | See section 5 |
| The watcher misses an event | Rescan every 60 seconds |
| The DB is corrupt or a migration fails | Rename the old DB, recreate from logs, restore notes from `notes.json` |
| `limits.json` missing or stale | Show "no data yet" or "X minutes ago", no alarm |
| A skill operation fails | Undo and show the original error |
| No tray | Run via a window and notifications |

## 12. Testing

- **Claude Code adapter unit tests** on the sample files in `tests/fixtures/` (cut from real logs, **content removed**, only the structure kept). They include: duplicate `requestId` lines, compact, interrupt, 429 error, slash command, meta line, half-written line.
- **Aggregator:** direct and continued skill attribution, anomaly rules, cost, context at prompt time.
- **Watcher:** append a line to a file while watching, write a line in two parts, a file rewritten smaller, a deleted file. Check for no duplicates and no misses.
- **composition:** on the fixture set, the per-group totals after correction must equal the real `context`; a compact must reset correctly; Read, Bash, and Skill tool_results must be classified correctly.
- **skills_ops:** run on a temporary `HOME` directory. Do disable, enable, remove, undo, duplicate name, symlink in turn, then check the real files on disk. The plugin part uses a fake CLI (a fake `claude` script at the front of `PATH`).
- **`tk audit`:** compare the DB's total tokens with an independent count directly from the logs. It must match 100%.
- **Performance:** measure the first-scan time, realtime latency, and RAM with the window hidden, against the criteria in section 1.3.
- **CI:** run `cargo test` on 3 OSes. UI tests are done manually on Linux.

## 13. v1 scope and milestones

Each completed milestone is **immediately usable**. M1 to M6 take about 4 weeks of work. M7 and M8 add about 1.5 weeks.

| Milestone | Content | Estimate |
|---|---|---|
| **M1: Core + Tray** | Claude Code adapter (log part), watcher, dedup, skill attribution, SQLite, `tk audit`; `tk-service` + host lock; `tk statusline`; tray; 5h/7d quota (section 7); quota, large-context, and anomalous-prompt notifications | 5–6 days |
| **M2: Prompts** | 3-column frame, prompt list, search and filters, a detail panel with "Why it's costly / What to do", ⭐ and 📝, basic Settings | 4–5 days |
| **M3: Skill** | List and cost computation (section 8.1), "where used", suggestions; disable/enable/install/remove with undo (section 8.2) | 4–5 days |
| **M4: Project, Day** | Two summary screens, click to filter the prompt list | 2 days |
| **M5: Session analysis** | Section 6.1: context composition, context and compact chart, view tool calls, suggestions | 4–5 days |
| **M6: Packaging** | Installers for 3 OSes, autostart, single-instance, CI | 2 days |
| **M7: Terminal** | `tk tui` (ratatui: Prompts, detail, Skill, quota; keys j/k, /, Tab), `tk daemon`, `tk prompts --json` | 3–4 days |
| **M8: VS Code** | Extension: a quota and token status bar, a webview panel reusing `app/ui` (switching `api.ts` to a socket), notifications, auto-filtering by workspace | 4–5 days |

- ⏭️ Deferred: Codex, Kiro, and Gemini adapters; report export; syncing skills between tools.
- When doing M5: read how claude-devtools (open source) classifies context for reference. **Do not copy code** without checking the license.

## 14. Project structure

Since there is no official name yet, the crates temporarily use the `tk-` prefix. Rename later with find and replace.

### 14.1 Directories

```
thongke/
├── Cargo.toml                  # workspace
├── crates/
│   ├── tk-core/                # Core: NO dependency on Tauri/Claude
│   │   └── src/ lib.rs · model.rs · aggregator.rs · watcher.rs · insight.rs
│   │           composition.rs · limits.rs · pricing.rs · config.rs · handlers.rs
│   │           store/{mod,schema,queries}.rs
│   ├── tk-adapters/            # Each AI tool is a module
│   │   └── src/ lib.rs (traits LogSource, SkillSource, registry)
│   │           claude_code/{mod,log,skills,statusline}.rs
│   ├── tk-skills/              # Safe skill operations
│   │   └── src/ ops.rs (plan/apply) · journal.rs (undo) · trash.rs
│   ├── tk-service/             # JSON-RPC over socket/pipe, host lock
│   └── tk-cli/                 # tk audit|scan|statusline|daemon|tui|prompts
├── app/
│   ├── src-tauri/src/ main.rs · runtime.rs · commands.rs · tray.rs · alerts.rs · window.rs
│   └── ui/src/ main.ts · api.ts · state.ts
│              views/{prompts,detail,session,skills,projects,days,settings,onboarding}.ts
│              components/ · charts/ (hand-written SVG) · styles/{tokens,layout}.css
├── extensions/vscode/          # M8
├── tests/fixtures/claude_code/ # sample logs with content removed
├── pricing.default.toml
├── docs/superpowers/specs/
└── .github/workflows/ ci.yml · release.yml
```

### 14.2 Dependencies between components

```
 app/ui ─invoke─► app/src-tauri ─┐        tk-cli / TUI / VS Code / statusline
                                 ▼                     │ socket
                           tk-core::handlers ◄── tk-service
                              ▲        ▲
                    tk-adapters    tk-skills
```

`tk-core` only knows `RawEvent` and the traits, so it can be tested with `cargo test` without a UI. `tk-adapters` is the only place that knows each tool's log format.

### 14.3 Realtime flow

```
Claude writes .jsonl ─► notify ─► watcher (offset → last '\n') ─► adapter.parse_line
  ─► aggregator (dedup, attribute skill) ─► store (1 transaction) ─► runtime:
        ├─► alerts → OS notification   ├─► tray (today's numbers, color)
        └─► event "data-changed" → Tauri UI / socket client (subscribe)
statusline ─► tk statusline ─► limits.json ─► notify ─► limits ─► tray/alerts
```

### 14.4 Command set (Tauri invoke = tk-service JSON-RPC)

| Group | Commands |
|---|---|
| Prompts | `list_prompts(filter)`, `prompt_detail(id)`, `session_analysis(id)`, `set_note(id, starred, note)` |
| Skill | `list_skills()`, `skill_usage(name)`, `plan_skill_op(op)`, `apply_skill_op(plan_id)`, `undo_skill_op(id)`, `skill_ops_log()` |
| Aggregation | `project_stats()`, `day_stats(range)`, `today_summary()`, `limit_state()` |
| Settings | `get_settings()`, `save_settings(s)`, `setup_statusline()`, `remove_statusline()`, `run_audit()` |
| Events | `data-changed`, `limits-changed`, `scan-progress` (a socket client receives them via `subscribe`) |

Skill operations always go through 2 steps: **`plan`** returns the specific list of changes to show in the confirmation dialog, then **`apply`** actually modifies files.

### 14.5 Main libraries

| Part | Libraries |
|---|---|
| Shared Rust | `serde`, `serde_json`, `thiserror`, `anyhow`, `tracing`, `chrono`, `dirs`, `toml` |
| Core | `rusqlite` (bundled), `notify` |
| Service | `interprocess` (cross-platform local socket / named pipe), `fs4` (file lock) |
| Tauri | `tauri` 2, the plugins `notification`, `autostart`, `single-instance`, `dialog`, `opener` |
| TUI | `ratatui`, `crossterm` |
| UI | Vite, TypeScript, `@tauri-apps/api`. No framework, no chart library |
| Test | `tempfile`, `insta` |
