# Verification: Conversations + multi-language + tool registry (2026-09-25)

Plan: `docs/superpowers/plans/2026-09-25-conversations.md`. Approved mockup: `specs/2026-09-25-conversations-mockup.html`.

## Automated

- `cargo test --workspace`: 171 tests pass. Newly added: `tests/conversations.rs` (grouping by day, filtering by project/model/tool/search, turns in order, stats by tool scope), `bum-adapters/tests/transcript.rs`, language tests in `insight.rs`, tray vi/en, `sources_merge_registry_with_stats`.
- Vitest: 53 tests pass (`conversations`, `conversation`, `overview`, `i18n`, `sidebar`, `fmt` newly written; old tests for the Prompts, Project, Day, and Model screens were removed along with those screens). `tsc --noEmit` is clean.
- `i18n.test.ts` checks that vi and en have the same set of keys, the same parameters, and that the English version has no Vietnamese text left.

## On real data (release, numbers only)

| RPC | Result | Time |
|---|---|---|
| `list_conversations` (all) | 63 conversations, 31 days | 71ms |
| by tool | Claude Code 20, Kiro 43 (all have titles) | 17ms / 11ms |
| search `test` | 15 conversations | 15ms |
| `conversation_turns` (largest conversation) | 225 turns | 11ms |
| `day_stats` + `project_stats` (Kiro) | 87 days, 17 projects | 24ms |
| Claude transcript (15MB file) | 68 turns, 613 items | 70ms |
| Kiro transcript (largest file) | 196 turns | 32ms |

- The number of turns in the transcript matches the DB. The only difference is recently added turns that had not been scanned yet (the app was off at the time).
- `tool_stats` only counts conversations that have turns, so Kiro subagent sessions are not counted.
- The real app (`target/release/thongke-app`) opens a window and shows the Overview from real data: IPC works with CSP enabled.

## Screenshots (fake data)

`docs/screens/conversations-vi-light.png`, `conversations-en-dark.png` (with a turn expanded), `conversations-kiro-vi.png` (Kiro scope, tool tag hidden), `overview-vi-light.png`, `overview-en-dark.png`.

## Differences from the plan

- The filters follow the approved mockup (Project select and time select right above the list, multi-select Model tag, quick-filter chips), not the "Filters (n)" button of option B.
- The search box searches **titles and questions**. Answers are not stored in the DB yet, so they cannot be searched.
- The plan's description of the skill operations and warnings of `bum-skills` (strings from the backend) is still only in Vietnamese.
