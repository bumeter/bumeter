# UI learned from skills-manager — Plan (condensed)

- Idea source: images `~/outsource/skills-manager/assets/demo/{library,agent-workspace,install-skills}.png`. See `docs/research/2026-09-28-skills-manager-comparison.md` (section 8).
- Branch `feat/skill-search` (continued). TDD, one commit per task.
- **Status: 5 tasks done.** Screenshots: `docs/screens/skills-cards-vi-light.png`, `skills-table-en-dark.png`, `overview-insights-vi-dark.png`, `conversations-logos-vi-light.png`.

| # | Task | Test |
|---|---|---|
| 1 | **Tool logos**: `SourceInfo.icon` (file name), Claude Code/Kiro logos in `app/ui/public/tool-icons/` (taken from skills-manager, brand notice in `NOTICE.md`); `glyph()` shows the logo, and falls back to the letter box on load failure. Identity colors use the brand colors (Claude `#D77655`, Kiro `#9046FF`) | Rust registry; Vitest glyph + fallback |
| 2 | **Skill screen**: a title with a pill-style count + a "costly but unused" warning label (click to filter); a filter chip row (status, source, a dashed-border "Costly but unused" chip); a **Table / Cards** toggle button (remembered via localStorage); cards with a description, baseline cost, usage count, an **enable/disable toggle** (still through a confirmation dialog + undo), the card footer = source + tool logo | Vitest |
| 3 | **Search on skills.sh**: card-style results, an "✓ Installed" label, a filter chip by source repo within the results | Vitest |
| 4 | **Conversations**: pill-style counts in the title + a status line by tool. **Overview**: 3 suggestions as cards with action buttons | Vitest |
| 5 | Verification: vi/en light/dark screenshots, build the app | — |

Not doing: a row of logos for every agent on the card, a card grid for the Conversations list, changing the app's primary accent color.
