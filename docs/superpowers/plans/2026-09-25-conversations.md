# Conversations + multi-language + tool registry — Plan (condensed)

- Approved mockup: `docs/superpowers/specs/2026-09-25-conversations-mockup.html`. Filters use option **B**: the "Filters (n)" button opens a picker, and active filters appear as chips with ✕.
- Terminology: **Conversation** (one session/thread) → **Turn** (one question and answer) → **Call** (one request to the model). Two languages: **vi** (default) and **en**.
- Branch `feat/conversations` (branched off `feat/kiro`).
- **Status: all 8 tasks done** — see `docs/conversations-verification.md` (which notes the differences from the plan).

| # | Task | Test |
|---|---|---|
| 1 | Conversation titles: `RawEvent::Title` (Claude from `ai-title`, Kiro from `title`), `sessions.title` (schema v4) | Parser/store |
| 2 | Tool registry in the backend: `LogSource::info()` returns label, symbol, color, capabilities. The `sources` RPC includes status (directory, session count, last update) | tk-core/adapters |
| 3 | `list_conversations(filter)` RPC: group by local date-time. The filter includes q, project, `models[]`, tool, time range, quick filters, ⭐. Each conversation has a title, tool, project, model, turn count, tokens, credits, an anomaly flag. `conversation_turns(session_id)` | tk-core |
| 4 | Transcripts read straight from the log: `claude_code::transcript`, `kiro::transcript` return turns with the question, answer (text), tool calls (truncated). App command `conversation_transcript` | adapters |
| 5 | i18n for the UI: `i18n.ts` (`t()`, date/number formatting via `Intl`), `locales/vi.ts`, `locales/en.ts`, language selection in Settings (saved to config). The backend returns **codes + parameters** instead of ready-made sentences (explanations, suggestions). Notifications and the tray translate per `Config.language` | Vitest + Rust |
| 6 | UI: a tool-scope picker in the sidebar (replacing the "Tools" item), a **Conversations** screen (grouped by day, tool tags in the "All" scope, a Filters button with Project/Model/quick filters), a detail panel with the turn list and one turn expandable to see the transcript | Vitest |
| 7 | Drop $ across the whole UI. Merge Project/Day/Model into **Overview** (day chart, top projects, model by tool, 3 suggestions). The sidebar keeps: Overview, Conversations, Skill, Settings | Vitest |
| 8 | Verify on real data, screenshot vi/en, build the installer | — |
