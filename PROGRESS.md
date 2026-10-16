# PROGRESS — Bumeter (formerly thongke) (updated 2026-09-28, afternoon)

> This file lets a person/AI **pick up the work right away** when opening the repo. Reading the whole file tells you where the project stands, what's done, what's left unfinished. Update it whenever a large chunk is finished.

---

## 1. What the project is

**Bumeter** (old name: thongke; CLI `bum`, data directory `~/.local/share/bumeter`, migrated from `thongke` on startup; `BUM_*` variables win over `TK_*`) — a desktop app (Tauri 2) that runs in the background, reads the logs of AI coding tools and answers: which conversations/turns cost tokens, which skills are used, why something is expensive, how much of the 5-hour/7-day quota is left.

**Terminology:** Conversation (a session) → Turn (a question + answer) → Call turn (a request to the model). Bilingual UI **vi** (default) / **en**.

- Users: individuals (developers) on a **subscription plan** — the UI **shows no $** (only tokens, and Kiro credits).
- Main OS: Ubuntu Linux (developed on this machine). Windows/macOS are per spec but not built yet.
- Original spec: `docs/superpowers/specs/2026-09-24-thongke-design.md` (458 lines, section 13 = table of milestones M1–M8).
- Wireframe/mockup: `docs/superpowers/specs/2026-09-24-thongke-wireframes.md`, `...-mockup-macos.html`.
- Milestone plan: `docs/superpowers/plans/2026-09-24-m1…m5*.md`, `...-kiro.md`.

**Stack:** Rust workspace (edition 2021, rustc 1.94) + Tauri 2 + plain TypeScript UI (Vite 8, Vitest 5, **no framework**, rendered with the `h()` helper).

## 2. Repo structure (important paths)

```
Cargo.toml                  # workspace: bum-core, bum-adapters, bum-service, bum-cli, bum-skills, app/src-tauri
crates/bum-core/             # core: log-reading engine, SQLite, handlers (all RPC queries), models, limits
crates/bum-adapters/         # log sources: claude_code/ (JSONL ~/.claude/projects), kiro/ (whole-file ~/.kiro/sessions/cli)
crates/bum-service/          # bum-service JSON-RPC over a socket (used later by TUI/VS Code clients)
crates/bum-cli/              # bum commands: scan, audit, session, skills, statusline, prompts…
crates/bum-skills/           # list/disable/enable/remove skills with undo (Claude + Kiro); market.rs = search skills.sh (via `curl`)
app/src-tauri/              # Tauri app: commands (call `call {method, params}` → bum-core handlers), tray, window
app/ui/src/                 # the entire UI
  api.ts                    # THE ONLY CALL LAYER → Tauri IPC; outside Tauri it falls back to mock.ts
  mock.ts                   # fake data (browser dev) — ADDING A NEW METHOD MUST BE UPDATED HERE
  tools.ts                  # ⭐ tool detection — loaded from the `sources` RPC (backend registry), toolDef/toolTag/glyph/modelTag
  i18n.ts, locales/{vi,en}.ts  # t('key', {params}); en.ts is typed Record<vi key> so a missing key = typecheck error
  views/{sidebar,conversations,conversation,overview,skills,session,settings}.ts
  tests/*.test.ts           # Vitest (fake.ts = test data), 64 tests
docs/build-ubuntu.md        # Linux bundle build process
docs/m{1,2,3,5}-verification.md, docs/kiro-verification.md, docs/conversations-verification.md   # verification records
docs/research/              # research, comparison with other projects
docs/screens/*.png          # screenshots with fake data
```

**Data flow:** watcher reads logs → engine (`bum-core/src/engine.rs`) writes SQLite → UI calls RPC `list_conversations`, `conversation_turns`, `prompt_detail`, `sources`… → `onDataChanged` pushes when there is new log data. Question/answer content (`conversation_transcript`) is read straight from the log file on demand, not stored in the DB.

**Real data sources on this machine:**

| Source | Path | Scale |
|---|---|---|
| Claude Code | `~/.claude/projects` (JSONL) | 38MB, 16,482 lines |
| Kiro | `~/.kiro/sessions/cli` (whole-file) | 113MB |
| Quota | `~/.claude/settings.json` statusline → `rate_limits` 5h/7d | Pro/Max only |
| OpenCode (not integrated) | `~/.local/share/opencode/opencode.db` (SQLite WAL, has `cost`) | 522MB |

## 3. Git status

- Current branch: **`feat/skill-search`** (branched from `feat/conversations`, contains all of M1–M5 + Kiro + Conversations). Working tree clean, not merged into `main` yet.
- The **Skill search on skills.sh** batch (plan `docs/superpowers/plans/2026-09-28-skill-search.md`): `8e0d629` market.rs (parse + find skill directory) · `d4d5b3d` op `install_market` · UI skills.sh tab.
- The 7 commits of the Conversations batch (old → new): `aa82a72` titles · `6e93cd3` registry + tool_stats · `a06f3ce` list_conversations/turns/sources · `8567e8a` transcript · `b9054e2` backend i18n + per-tool stats · `177be5f` Conversations/Overview UI/vi-en/drop $ · `94aac58` docs.
- No TODO/FIXME in the code.

## 4. Progress by milestone (spec section 13)

| Milestone | Content | Status |
|---|---|---|
| **M1** Core + Tray | read logs, dedupe, assign skills, SQLite, `bum audit`, tray, quota, notifications | ✅ `docs/m1-verification.md` (tokens match 100%) |
| **M2** Turns | 3-column layout, list, search/filter, detail, ⭐/📝, settings | ✅ `docs/m2-verification.md` — this screen has been replaced by **Conversations** |
| **M3** Skills | list, baseline cost, where used, disable/enable/size/remove with undo | ✅ `docs/m3-verification.md` |
| **M4** Project + Day | 2 summary screens, click to filter | ✅ — merged into **Overview** (together with the Model screen) |
| **M5** Session analysis | context components, compact chart, tool calls | ✅ `docs/m5-verification.md` (analyzing 224 turns = 48ms) |
| **Kiro** | whole-file adapter, estimate context from %, credits, own skill view, Model screen | ✅ `docs/kiro-verification.md` |
| **Conversations + i18n** | conversation titles, backend tool registry (`sources`), Conversations screen grouped by day + transcript, tool scope, Overview (merges Project/Day/Model), vi/en, drop $ | ✅ `docs/conversations-verification.md` |
| **Skill search on skills.sh** | "Search on skills.sh" tab in the Skill install box; install = clone GitHub, take only the skill directory, with undo; no telemetry | ✅ verified for real: search 0.8s, install `mattpocock/skills/code-review` 1.4s, clean undo (temp directory) |
| **UI learning from skills-manager** | tool logos + brand colors, Skills screen as cards/table + filter chips + toggles, skills.sh as cards, suggestion cards on Overview | ✅ screenshots `docs/screens/skills-cards-*.png`, `overview-insights-*.png` |
| **M6** Packaging | installers, autostart, single-instance, CI | 🟡 Ubuntu `.deb` (9.6M) + `.AppImage` (86M) rebuilt 2026-09-25 with the Conversations build (`docs/build-ubuntu.md`, needs glibc 2.39 = Ubuntu 24.04+); **no** Windows/macOS, autostart, CI yet |
| **M7** Terminal | `bum tui`, `bum daemon`, `bum prompts --json` | ❌ not started |
| **M8** VS Code | extension, webview panel reusing `app/ui` | ❌ not started |

**Current test status (2026-09-28):** `cargo test --workspace` = **180 tests pass** · Vitest = **64/64** · `tsc --noEmit` clean.

## 5. Recently finished (branch feat/conversations)

- Conversation titles (Claude `ai-title`, Kiro `title`, schema v4); if none, use the first question.
- Backend registry: `LogSource::info()` → `SourceInfo` (label, letter, color, capabilities); `bum_adapters::registry()`; app RPC `sources` = registry + `tool_stats`.
- RPC `list_conversations` (grouped by local date; filter q/project/models[]/tool/from/to/quick), `conversation_turns` (returns `{title, turns}`), `conversation_transcript` (app command, reads the log).
- `project_stats`/`day_stats` take `tool` (tool scope).
- i18n: `Config.language` (vi/en); notifications, tray, statusline, `bum status` translate accordingly; the backend returns **codes** for "why it cost" (`Explain`) and session-analysis suggestions (`Suggestion`), the UI translates them itself.
- UI: tool scope at the top of the sidebar (remembered via localStorage); sidebar = Overview / Conversations / Skills / Settings; Conversations screen + turn detail (question/answer, tool calls, why it cost, ⭐/📝); Overview has 3 suggestions; language picked in Settings.

## 5b. UI redesign + rename to Bumeter (branch feat/bumeter)

- Done: shared tokens/components (orange, flat buttons, no gradients, Be Vietnam Pro font embedded offline), the Bum mascot with 5 expressions, width-based layout (wide ≥1200 / medium 700–1199 with a drawer / narrow <700), reworked Overview, Conversations, Skills, Settings; light/dark/system theme in config; tray changes Bum's expression; CLI `bum`, data directory `bumeter` migrated automatically (with rollback, keeps the old directory if migration fails), statusline hook points to `bum`.
- UI review (Task 12): dialogs have `aria-labelledby`, Esc closes, focus restored; autocomplete disabled on the source/search fields; fixed the shared `.bar` rule overriding the day-chart columns; the language select was clipping text.
- Screenshots: `docs/screens/bumeter-<view>-<w>-<lang>-<theme>.png` (Overview + Conversations at 1360/1000/640 × vi/en × light/dark; Skills + Settings at 1360 vi light/dark). Captured with `vite preview` + headless Chrome with mock data; the URL supports `?theme=&lang=&view=&scope=&conversation=&turn=&skillView=&open=1`. The old images (m2–m5, conversations-*, overview-*, skills-*) are kept because the old verification docs still point to them.
- Installers: `target/release/bundle/deb/Bumeter_1.0.1_amd64.deb`, `target/release/bundle/appimage/Bumeter_1.0.1_amd64.AppImage` (sidecar `binaries/bum-x86_64-unknown-linux-gnu`).
- Upgrading from an older version: the old package was named `thongke` (identifier `dev.thongke.app`) and differs from the new `bumeter` package, so it installs alongside — you need `sudo apt remove thongke`. The old localStorage (tool scope, Skills view mode) is reset because the keys changed `thongke.*` → `bumeter.*`. Details in `docs/build-ubuntu.md`.
- Real-machine check (2026-10-15, with backup `~/.local/share/thongke-backup-20261015`): `bum status` migrated `thongke/` → `bumeter/` the first time (`thongke.db` → `bumeter.db`, 11 MB, data still complete); the `bumeter` app runs, IPC `bum status` returns real numbers; the tray shows Bum `worry` at 85% of the 5-hour quota; the statusline hook rewrites `tk` → `bum` (a bug found during the real run: an early backup copy was missing `previous_status_line`, which broke the hook migration — fixed, with a test). Not yet checked by hand: the drawer when shrinking a real window (verified via the 1000px screenshot).

## 6. Unfinished work (priority top to bottom)

0. **Compare with skills-manager** (`docs/research/2026-09-28-skills-manager-comparison.md`): should do first (1) hand skill operations over to skills-manager when a skill is managed by it — this machine has `~/.skills-manager`; (2) CI to build the installers, Linux on ubuntu-22.04 to remove the glibc 2.39 limit, add Windows/macOS.
1. The backend strings of `bum-skills` (operation plan descriptions, warnings) are still Vietnamese only.
2. Search within answers (needs storing/indexing the transcript).
3. Onboarding + a "Tools" screen in Settings + per-tool monitoring toggle (spec section 6).
4. Incremental engine for SQLite-type sources (needed for OpenCode), then integrate OpenCode — researched (section 2), **deferred**.
5. Remaining M6: autostart, Windows/macOS builds, CI; M7 (TUI), M8 (VS Code).

## 7. Commands to remember

```bash
cargo test --workspace                 # 180 Rust tests
cd app/ui && npm test                  # 64 UI tests (vitest run)
cd app/ui && npx tsc --noEmit          # typecheck (npm run build runs it too)
cd app/ui && npm run build             # UI → app/ui/dist (must run BEFORE tauri build)
cd app/ui && npm run dev               # view the UI in a browser with mock.ts (?theme=dark&view=skills)
./target/release/bumeter              # run the built app
# Linux bundle: see docs/build-ubuntu.md
#   cargo build --release -p bum-cli
#   cp target/release/bum app/src-tauri/binaries/bum-x86_64-unknown-linux-gnu
#   (cd app/ui && npm run build) && (cd app && ./ui/node_modules/.bin/tauri build --bundles deb appimage)
```

## 8. Conventions when working on code here

- **Tool logos**: `SourceInfo.icon` = the file name in `app/ui/public/tool-icons/` (only simple file names accepted); record the source + trademark owner in the `NOTICE.md` there. Before a public release, replace them with files from the official brand page.
- **Don't hard-code tool names in views** — all detection (name, color, capabilities) goes through `app/ui/src/tools.ts`, with data from the adapter's `SourceInfo`. Adding a new tool = a new adapter + 1 line in `bum_adapters::registry()`; the UI needs no change (colors go through the CSS variable `--tc`).
- **Display strings go through `t()`** — add the key to both `locales/vi.ts` and `locales/en.ts`. The backend returns codes + params, not prewritten sentences, to the UI.
- Adding a new RPC method: edit `api.ts` (interface + method) **and** `mock.ts` (fake-returning case) **and** the handler `crates/bum-core/src/handlers.rs`; if there's real data, add a test in `crates/bum-core/tests/`.
- Plain TS UI + `h()`; a view = `mountX(el)` + `cleanup`; Vitest tests follow the existing view pattern (`app/ui/tests/`).
- Docs, comments: **English**; UI content: vi + en. Project/client names in docs/mocks have been replaced with placeholders (commit `6bb915b`) — **do not bring the real names back**.

## 9. Traps already hit (read before "debugging" the wrong thing)

- **Don't `grep` for Vietnamese strings in the binary** — Tauri compresses assets (brotli) into the binary, so `grep "Công cụ" bumeter` = 0 even when the UI is correct. Verify by running the app + taking a screenshot (`import -window <id> /tmp/shot.png`).
- The app **runs in the tray background**; the window is only created when `window::open` is called (`app/src-tauri/src/window.rs`); a process in `ps` ≠ a window. `single-instance`: a second launch only focuses the old window.
- A 10×10 `InputOnly` window in `xwininfo` = a window that hasn't opened, not a broken app.
- After rebuilding, you must open the **new** app (close the old build first) or you'll be looking at the old UI.

## 10. How to verify quickly

```bash
cargo test --workspace && (cd app/ui && npm test && npx tsc --noEmit)   # green = safe to commit
# view the UI with fake data: cd app/ui && npm run dev → ?lang=en&theme=dark&view=overview&scope=kiro&conversation=s1&turn=a4
```
