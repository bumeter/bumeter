# Find and install skills from skills.sh — Plan (condensed)

- The approach follows skills-manager (MIT): skills.sh is only an index; search with `GET https://skills.sh/api/search?q=&limit=`; install = clone `github.com/<source>` then pick the right skill directory by `skillId`. See `docs/research/2026-09-28-skills-manager-comparison.md`.
- No telemetry, no reading the leaderboard HTML. Network calls use the machine's `curl` (like `git`), with no added HTTP/TLS library.
- Branch: `feat/skill-search` (branched off `feat/conversations`). TDD, one commit per task.
- **Status: 5 tasks done.** Actually verified on a temporary directory: searching "code review" = 5 results (0.8 seconds); installing `mattpocock/skills/code-review` = 1.4 seconds (only `SKILL.md` + `agents/`); undo is clean, the temporary directory is cleaned up.

| # | Task | Test |
|---|---|---|
| 1 | `tk-skills/src/market.rs`: `parse_search` (accepts an array or `{skills: […]}`, drops missing/invalid items), `valid_source` (`owner/repo`), query encoding, `search()` via `curl -fsS --max-time 15` | Parse from a real sample JSON; odd source/id rejected |
| 2 | `find_skill_dir(repo, id)`: try `<id>`, `skills/<id>`, `.agents/skills/<id>`, then search recursively (depth ≤ 6, skip `.git`/`node_modules`, don't follow symlinks, fixed order) by directory name, finally by `name` in SKILL.md. If not found, error, and **do not** install the whole repo | Various repo structures; symlinks pointing outside are skipped |
| 3 | Operation `install_market` (`source` + `skill`): plan checks the name/duplicates; apply = clone `--depth 1` into a temp dir → pick the skill directory → move it into `skills/<id>` → clean the temp dir; undo = move to trash | A local git repo (`file://`) instead of GitHub |
| 4 | App: `skills(search)`; UI with an "Install skill" dialog that has a **Search on skills.sh** tab (debounce, install count, an Install button → plan confirmation dialog; an existing skill shows "Installed"); install into the tool of the current scope; vi/en | Vitest |
| 5 | Verification: really find and install a skill on a temporary HOME, then undo | — |
