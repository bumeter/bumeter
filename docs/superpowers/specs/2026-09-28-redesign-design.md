# Bumeter — UI redesign + rename (Spec, 2026-09-28)

> **Standard mockup:** `2026-09-28-redesign-mockups/bumeter-ui-mascot.html` (approved). Brand: `docs/brand/README.md` + `docs/brand/bum/`. The other mockups in the `2026-09-28-redesign-mockups/` directory are earlier trial rounds: **reference the layout only; wherever they differ from the standard version, follow the standard version.**

## 1. Goals

The user finds the old UI **cramped/cluttered** and **not modern**, and wants its own brand. This round:

- Rename the project **thongke → Bumeter** (Bum = the author's nickname, meter = a measuring gauge), with the **Bum** mascot as the logo and a status indicator.
- Redo the UI layer per the standard mockup: primary orange, friendly, modern, flat buttons; usable well in wide windows and split-screen.
- No functional changes, no changes to how data is fetched; the app stays light (plain TypeScript + `h()`, no framework).

Success when: every screen matches the standard mockup in light/dark, vi/en; at ~1000px the 3 columns are no longer squeezed; the tray and notifications use Bum expressions; the `bum` command and the new data directory work and old data is migrated; all tests pass.

## 2. Decided decisions

| Topic | Decision |
|---|---|
| Name | **Bumeter**; wordmark logo lowercase `bumeter`; CLI **`bum`** (replacing `tk`); data directory **`<data_dir>/bumeter`** (replacing `thongke`), with automatic migration of old data |
| Logo | Mascot **Bum** (a round flame with a face), 5 expressions: `happy`, `calm`, `worry`, `blaze`, `sleep` (SVG in `docs/brand/bum/`) |
| Colors | Primary orange `#FF7A1A` (dark: `#FF8A33`); dark orange for text `#E8640A`; light orange background `#FFF0E3`; red-orange `#FF4B2B` for extreme cost; green `#22A05A` for reduction/savings. Light background warm `#FAF7F4`, block surface `#FFFFFF`; dark background charcoal-brown `#15120F`, block surface `#1F1A16`. **No gradients anywhere** |
| Font | **Be Vietnam Pro** (embedded via `@fontsource/be-vietnam-pro`, runs offline), tabular numbers; titles 800, wordmark 900 |
| Corner radii by level | Large frames (panel, list) 16px; number boxes/sub-frames 12px; buttons, chips, search box 9–10px; small tags 5px |
| Buttons | Flat: primary button orange background with white text; secondary button `sunk` background; no drop shadow, no gradient. Shadows only for floating layers (drawer, dialogs, menus) |
| Tool identity | Tool logos (Claude Code, Kiro) — the orange color is reserved for "consumption" and primary actions |
| Width-responsive layout | `wide` ≥ 1200px: 3 columns. `medium` 700–1199px: the sidebar becomes an icon column, and the detail is a **drawer sliding in from the right** (~60%, closed with Esc/✕/backdrop click). `narrow` < 700px: a single column, the detail is a separate page with "← back" |
| Spirit | "macOS-inspired, friendly, modern": keep the native title bar, sidebar → list → detail layout, segmented control, keyboard shortcuts (Ctrl/⌘+K search, ↑↓, Esc); do not imitate vibrancy/Aqua |

## 3. Common foundation

### 3.1 Tokens (`app/ui/src/styles/tokens.css`)

Variables: `--bg`, `--surf`, `--sunk`, `--line`, `--ink`, `--ink2`, `--ink3`, `--acc`, `--acc2`, `--tint`, `--red`, `--grn` (light/dark values as in section 2 and the standard mockup), `--r-panel: 16px`, `--r-box: 12px`, `--r-ctl: 10px`, `--r-tag: 5px`, spacing in 4px steps. Modes: follow the system, or `data-theme="light|dark"` (Settings).

### 3.2 Components (`app/ui/src/components/`)

| Component | Job |
|---|---|
| `bum.ts` | `bum(state, size)` returns the mascot SVG (5 expressions, colors per token); `bumStateFor(...)` selects the expression from data (section 3.4) |
| `drawer.ts` | Right drawer: open/close, Esc, backdrop click, ✕; focus moves inside, focus is restored on close; `role="dialog"` |
| `chipMenu.ts` | A chip with a dropdown menu (single value) and `chipMultiMenu` (multiple values, the chip shows the selected count); keyboard ↑↓/Enter/Esc |
| `ui.ts` | `countPill`, `kpi` (label, big number, sub-line), `insightRow` (severity-based icon, title, description, action button) |

### 3.3 App frame

- `main.ts` watches the width (ResizeObserver) and sets `#app[data-layout]`; CSS switches the grid.
- **Sidebar:** Bum logo + `bumeter` text; tool-scope picker; nav with icons (Overview, Conversations, Skill, Settings); a **Bum card** at the bottom: Bum by state + "5-hour quota X%" + reset time + quota bar (only when the scope has quota; if not, Bum + a general status).
- Quick search: a search box with a `Ctrl K` shortcut hint (opens conversation search; a full command palette is deferred).

### 3.4 Bum expression by state

Priority from top to bottom:

1. `sleep`: quota data is stale **and** there is no new turn for 30 minutes, or there is no data at all.
2. `blaze`: 5-hour quota ≥ the second threshold (`quota_thresholds[1]`, default 90%), or an anomalous-cost turn in the last 10 minutes.
3. `worry`: 5-hour quota ≥ the first threshold (default 70%).
4. `happy`: today's tokens < the 30-day average daily usage.
5. `calm`: otherwise.

Used in: sidebar, Overview (next to the main token number), tray (icon), notifications (icon), empty screens (`sleep`), and the "why it's costly" explanation (`worry`).

## 4. Each screen (per the standard mockup)

### 4.1 Overview

- Range selection: Today / 7 days / 30 days / 90 days.
- A row of 3 cards: **Tokens** (Bum + big number + "Less/More by X% than the previous period", green/red), **Turns** (average per day of use), **Conversations** (count by tool when the scope is All).
- A **Tokens by day** card (bars with rounded tops, the costliest bar in dark orange, clicking a bar filters Conversations) next to a **Bum suggests** card (at most 3 lines: an anomalous-cost turn → red; a skill costly but unused → orange, from `api.skills(false)`, hidden on error; a project taking many tokens → gray). Each line has an action button.
- Project and Model: separate cards below (lists with horizontal bars), stacked when narrow.
- "Previous period" = exactly the preceding number of days; if the previous period is 0, the comparison part is hidden.

### 4.2 Conversations

- Title + count + search box; **one filter row**: Project chip menu, time chip menu, Model chip menu (multiple), quick-filter chips (Anomalous cost, Large context, Interrupted, ⭐).
- A list on a white 16px-rounded frame: grouped by day (bold title, normal text), each row: tool logo (when the scope is All) + title, sub-line (project, time or credits), a turn-count column, a **fuel bar** (orange; red when the turn/conversation is extremely costly) + token count. The selected row has a `--tint` background.
- Detail: title, meta (project, model, time range — no repeated token/turn), 3 number boxes (Token orange, Turns, Largest context), a "Analyze context" button (primary) and "Mark"; the turns: time, question, quick view of the answer, tokens; an expanded turn keeps the full question/answer/tool call; "why it's costly" in a `--tint` frame with a `worry` Bum.

### 4.3 Skill

Keep the existing features (table/cards, toggle, filter chips, warning label, search on skills.sh); switch to the new tokens/components; add an "All" chip; a skill card that is costly but unused gets an orange border; drop the status dot on the card (the toggle shows on/off).

### 4.4 Settings

Group into frames: **General** (language, UI Light/Dark/System — `Config.theme`), **Tools** (logo, log directory, usage count, status from `sources`), **Limits & notifications**, **Diagnostics**. **Save immediately** on change (debounce 400ms; an invalid value is reported inline and not saved); thresholds that need an app restart are still stated clearly.

### 4.5 Context analysis

The chart, the call composition, and the suggestions are separate frames using the new tokens; suggestions use `insightRow`.

### 4.6 Empty and error screens

A `sleep` Bum + a sentence stating clearly what to do + an action button (for example "Bum is waiting for the first conversation").

## 5. Tray, app icon, notifications

- App icon (1024 → the Tauri sizes, `.icns`, `.ico`): a `calm` Bum on a white background (per `docs/brand/bum/`). The PNG is exported with Chrome headless from the SVG (the machine has no rsvg/inkscape), committed to `app/src-tauri/icons/`.
- Tray: replace the colored dot with a **Bum by state** (section 3.4) at 32px (PNG embedded for the 5 expressions); the tooltip keeps the current status line.
- Notifications: title "Bumeter · …"; icon by type (quota → `worry`/`blaze`, anomalous cost → `blaze`).

## 6. Rename

| Item | Old | New |
|---|---|---|
| Display name, window, installer | thongke | Bumeter |
| Tauri `productName` / `identifier` | `thongke` / `dev.thongke.app` | `Bumeter` / `dev.bumeter.app` |
| App binary | `thongke-app` | `bumeter` |
| CLI | `tk` | `bum` (sidecar `binaries/bum-<triple>`) |
| Data directory | `<data_dir>/thongke`, `thongke.db`, socket `thongke-*.sock` | `<data_dir>/bumeter`, `bumeter.db`, `bumeter-*.sock` |
| Environment variables | `TK_DATA_DIR`, `TK_CLAUDE_DIR`, `TK_KIRO_DIR` | kept as they are (compatibility), plus new `BUM_*` names that take priority |

- **Data migration:** when the app/CLI opens for the first time, if `<data_dir>/bumeter` does not exist but `<data_dir>/thongke` does: rename the directory and the DB files (`thongke.db*` → `bumeter.db*`) in one step; on failure, keep the old directory and use it, and log it.
- **Statusline hook:** if `~/.claude/settings.json` has a statusline command pointing to the old `tk` binary, the app rewrites it to `bum` (backing up first, like the existing statusline operation).
- Internal crate names (`tk-core`, `tk-adapters`…) **stay as they are** to reduce churn; only change what the user sees.
- Documentation: README, PROGRESS, build-ubuntu updated with the new name.

## 7. Out of scope

New data features (conversation tags, skill presets, measuring the impact when a skill is edited, the full Ctrl+K command palette); changing tool colors; the Windows/macOS build and CI.

## 8. Testing

- Rust: migrating the old data directory → new (with/without the new directory, error keeps the old), `Config.theme`, rewriting the hook `tk` → `bum`, selecting the tray expression.
- Vitest: `bum.ts` (`bumStateFor` for each condition and priority order), `drawer`, `chipMenu`/`chipMultiMenu`, `data-layout`, saving immediately in Settings, comparing with the previous period; update the selectors of existing tests; keep the XSS checks.
- Chrome headless screenshots (fake data) at 1360/1000/640px × vi/en × light/dark, compared with the standard mockup.
- Run the real app: IPC, the drawer when the window is shrunk, the tray switching Bum by quota, the `bum status` command, old data still present after the rename. Build the `.deb`/`.AppImage` installer named Bumeter.
