# M3: Skill screen verification

- Date: 2026-09-24. Machine: Ubuntu GNOME.
- Screenshots use fake data: `screens/m3-light.png`, `screens/m3-dark.png`.

## Run on real data (read-only: `tk skills`)

| Check | Result |
|---|---|
| Personal skills match `ls ~/.claude/skills` | ✅ 11 skills: pj, pj-*, estimate. `pj-clarify` is a symlink |
| Skills synced from claude.ai | ✅ 8 skills (docs, docx, pdf, pptx, xlsx, morning, import-memory, skill-creator). Read-only, never suggested for disabling |
| Plugins via `claude plugin list --json` / `details` | ✅ superpowers has 15 sub-skills, real cost (for example brainstorming ~70 baseline / ~6.3K per call) |
| Total baseline cost of enabled skills | ~3,871 tokens per session. 12 skills unused for 30 days, costing ~1,198 tokens per session |
| Does not modify any file | ✅ Read-only command |

## Bugs found during a real run, fixed (with tests)

1. `/superpowers:brainstorming` (typing the command) and `superpowers:brainstorming` (the skill) were split into 2 rows. Now merged (`slash_command_of_an_installed_skill_counts_toward_that_skill`).
2. Invoking a skill with `/X` made the log record both the command and the skill with the same `promptId`, so the usage count was **double-counted**. Now counted once (`a_slash_invoked_skill_is_counted_once`).

## Disable/enable/remove/install operations

- Verified with 11 tests on a temporary HOME (`crates/bum-skills/tests/ops.rs`): disable, enable, move to trash, undo, can't undo twice, a symlink only moves the link (the source directory is left intact), install from a directory or git (a repo without `SKILL.md` is rejected and cleaned up), roll back everything when a step fails, plugin runs through the CLI (undo runs the reverse command), clean the trash after 30 days.
- **Not run on the real `~/.claude`.** The operations modify files, so they are left for you to click in the app, and every operation has a confirmation dialog and an undo button.
- Limitation: if the trash is on a different disk partition than `~/.claude`, moving the directory will error (it does not copy).

## Tests

`cargo test --workspace` 132/132 · Vitest 34/34.
