# M2: Dashboard "Prompts" — Implementation plan (condensed)

- Spec: `docs/superpowers/specs/2026-09-24-thongke-design.md` (sections 4, 6, 6.0, 7.1, 14.4)
- Reference design: `2026-09-24-thongke-mockup-macos.html` and `2026-09-24-thongke-wireframes.md` (screens 1, 6, 8)
- Goal: a real dashboard window with a sidebar, a list of prompts grouped by session, and a detail panel. Realtime updates, ⭐/📝, a basic Settings screen, and statusline setup. macOS style, runnable on every OS.
- UI stack: Vite + plain TypeScript (no framework), `@tauri-apps/api` 2, tested with Vitest + happy-dom, Inter font embedded (`@fontsource-variable/inter`).
- Follow TDD, one commit per task. Branch: `feat/m2-dashboard`.

## General rules

- The UI only calls commands through `ui/src/api.ts`, so M8 (VS Code) can swap it for a socket.
- Times are **always shown in local time** (fixes a leftover M1 bug: the dashboard showed UTC time).
- All data inserted into the DOM goes through `textContent` or `esc()`. Never inject raw strings into `innerHTML`.
- User file-modifying operations (statusline, settings) must back up first, have a confirmation dialog, and be undoable.
- Colors and spacing come from tokens in `tokens.css`, with full light and dark modes.

## Tasks

| # | Task | Main files | Tests to pass |
|---|---|---|---|
| 1 | **Prompt list query** | `tk-core/src/store/queries.rs`, `handlers.rs` | `list_prompts(filter)` supports: search by content, project, skill (direct and continued), time range, quick filters (anomalous / ctx>300K / interrupted / ⭐), sort by time or tokens, pagination. Results grouped by session, each session with a total. Has `sidebar_counts()`. Tests on fixtures |
| 2 | **Prompt detail** | `queries.rs`, `handlers.rs` | `prompt_detail(key)` returns each call (ts, model, context, output, tools), the context re-read ratio, the "why it's costly" part, cost, and notes. Test: p1 has 3 calls (including 1 subagent) |
| 3 | **Notes ⭐/📝** | `store/schema.rs` (migration v2), `store/notes.rs` | `set_note` writes to the DB and backs up `notes.json`. If the DB is recreated, restore from `notes.json`. Migration from v1 to v2 loses no data |
| 4 | **Push realtime to the UI** | `app/src-tauri/src/runtime.rs`, `commands.rs` | The host emits a `data-changed {prompt_keys, session_ids}` event (coalesced at most once per 250ms). New Tauri commands for tasks 1–3 and 7. Pure tests for the event coalescer |
| 5 | **UI skeleton + design tokens** | `app/ui/{index.html,vite.config.ts,package.json}`, `src/{main,api,state}.ts`, `src/styles/{tokens,layout}.css` | `npm run build` works and Tauri loads `dist/`. Has light/dark tokens, embedded Inter font, 3-column layout, native title bar. Vitest: `fmt.ts` (tokens, money, local time, "X minutes ago") |
| 6 | **Sidebar + list** | `src/views/{sidebar,prompts}.ts`, `src/components/{row,chips,search}.ts` | Vitest + happy-dom: group by session, color bar by average level, skill tags (bold = direct, dashed = continued), ↑/↓ keys, debounced search, filter chips. A new row is added automatically on `data-changed` while keeping the selected row. Strings with special characters don't break the DOM (XSS test) |
| 7 | **Detail panel** | `src/views/detail.ts`, `src/charts/bars.ts` | Show big numbers, stat cards, an SVG chart per call, the "Why it's costly" box, ⭐/📝 buttons (call `set_note`, update the UI immediately). Vitest for `bars.ts` (ratio, 0 calls, 1 call) |
| 8 | **Settings + statusline setup** | `tk-adapters/src/claude_code/statusline.rs`, `src/views/settings.ts` | `plan/apply/undo` of `setup_statusline`: back up the original script; insert `echo "$input" \| tk statusline --quiet` exactly once. If there is no statusLine yet, create one and edit `settings.json` (with a backup). Uninstall restores the backup. Tests on a temporary HOME. UI: thresholds, notification on/off, statusline status |
| 9 | **Tray shows "X minutes ago"** | `app/src-tauri/src/tray.rs`, `tk-core/src/limits.rs` | Fixes a leftover M1 bug: stale data shows "updated X minutes ago", the icon turns gray instead of red or yellow. Unit tests for the label and color |
| 10 | **UI verification** | `docs/m2-verification.md` | Take light and dark screenshots with Chrome headless on a UI build running against a fake API. Run the real app: open the window, data appears (confirm IPC works with CSP enabled), realtime while chatting, RAM drops after closing the window. Compare with the mockup |

## Review Focus

1. A list of hundreds of prompts and a burst of realtime events while Claude is running → the UI doesn't stutter and doesn't lose the selected row (Tasks 4, 6).
2. A project name, question, or skill containing `<script>` or odd characters → shown as text, not executed (Tasks 6, 7).
3. A user who already has their own statusline script → inserted exactly once, re-running setup doesn't insert a duplicate, uninstalling restores the exact original file (Task 8).
4. A machine that doesn't support `backdrop-filter`, or lacks the font → the UI is still readable and correctly colored (Task 5).
5. The DB is writing while the UI queries repeatedly → no errors and no freezing (Tasks 1, 4).

## Deferred

Skill screen + skill management (M3), Project/Day (M4), session analysis (M5), packaging (M6), app RSS optimization (M6).
