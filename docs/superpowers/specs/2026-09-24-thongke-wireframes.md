# thongke — Screen wireframes

- Date: 2026-09-24
- Status: Approved
- Accompanying spec: `2026-09-24-thongke-design.md`
- The temporary app name is `[App]`. The numbers in the wireframes are illustrative only.

## 1. Main screen: Prompts (list and detail)

```
┌─[App]──────────────────────────────────────────────────────────────── ─ □ ✕ ┐
│ ● LIVE  5h: ████████░░ 78% (reset 14:30)   7d: ████░░░░░░ 41%   Today 2.1M │
├──────────────┬─────────────────────────────────────┬─────────────────────────┤
│ VIEW BY      │ 🔎 Search in questions...            │ "now update the progress,│
│ ▸🕒 Prompts  │ [Project: project-shop ✕] [Skill ▾] │  and I need the..."     │
│  🧩 Skill    │ [30 days ▾] [Sort: Time ▾]          │ TICKET-101 · 11/09      │
│  📁 Project  │─────────────────────────────────────│ 11:28 · 4m40s · —skill  │
│  📅 Day      │ 231 prompts · 184M tok · avg 796K   │┌──────────┬──────────┐ │
│  ⚙ Settings  │                                     ││Total     │API calls │ │
│              │ ▾ Session 11/09 09:10→12:40 · 71M   ││ 9.4M 🔴  │ 11       │ │
│ QUICK FILTER │ 09:52 test ran e2e with lambda… ███ ││Context   │Output    │ │
│  🔥 Anomalous│      [run]                   14.9M ││ 841K 🔴  │ 17K      │ │
│  🧠 Ctx>300K │ 11:02 so what does that mean?   ▌   │└──────────┴──────────┘ │
│  ✋ Interrupted 620K                              │ Per API call:          │
│  ⭐ Review   │▶11:28 now update the progress… ██   │ █ █ █ █ █ █ █ █ █ █ █  │
│              │                              9.4M   │ ▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔  │
│ TOOLS        │ 11:40 ok                        ▌   │ ■ read context ■ output│
│  ☑ Claude    │                              870K   │ Tool: Bash×10          │
│              │ ▸ Session 10/09 14:05→17:30 · 52M   │┌ Why it's costly ─────┐│
│ TODAY        │ ▸ Session 09/09 ...                 ││99.8% is re-reading   ││
│  11 prompts  │                                     ││841K of old context.  ││
│  2.1M tok    │                                     ││→ /compact or start a ││
│  $3.2        │                                     ││  new session before  ││
│              │                                     ││  summarizing.        ││
│              │                                     │└──────────────────────┘│
│              │                                     │[⭐Review][📝Note]      │
│              │                                     │[🔬 Deep analysis →]    │
└──────────────┴─────────────────────────────────────┴─────────────────────────┘
  Color bar: ▌green <2× avg · ██yellow 2–5× · ███red ≥5×
```

## 2. Session analysis (deep analysis, M5)

```
┌─[App] › TICKET-101 › Session 11/09 09:10─────────────── ─ □ ✕ ┐
│ ← Back         64 prompts · 118 API calls · 71M tok · 2 compacts      │
├────────────────────────────────────────────────────────────────────────┤
│ CONTEXT OVER TIME (estimated)                              1M ┤       │
│                                              ▄▆█           ┤          │
│                                     ▂▄▆███████▌  ← compact             │
│                        ▂▄▆█▌                 ▌  ▂▄▆█                   │
│            ▂▄▆████████▌   ▌ ← compact        ▌▂▄████                   │
│   ▂▄▆█████▌               ▌▂▄▆█████████████  ▌                         │
│  └──┬────────┬────────┬────────┬────────┬────────┬──── API calls       │
│     ▲Q1      ▲Q12     ▲Q25     ▲Q40     ▲Q52     ▲Q64   (▲ = prompt)  │
│  ■ System/CLAUDE.md  ■ Skill  ■ Files read  ■ Tool I/O  ■ Thinking     │
│  ■ Assistant output    ■ Your questions                                │
├───────────────────────────────────┬────────────────────────────────────┤
│ COMPOSITION at the selected call  │ TOP HEAVIEST BLOCKS                │
│ Files read   ██████████░  48% 404K│ 1. Bash 11:30 cat logs…  ~120K  [↗]│
│ Tool I/O     █████░░░░░░  24% 202K│ 2. Read handler.ts       ~ 85K  [↗]│
│ System       ██░░░░░░░░░  11%  92K│ 3. Read handler.ts (2nd) ~ 85K  [↗]│
│ Output       ██░░░░░░░░░   9%  76K│ 4. Skill pj-review       ~ 12K  [↗]│
│ Skill        █░░░░░░░░░░   5%  42K│                                    │
│ Questions    ░░░░░░░░░░░   3%  25K│ SUGGESTIONS                        │
│                                   │ ⚠ handler.ts was read 4× (~340K)   │
│                                   │ ⚠ Bash output is long; use | head  │
├───────────────────────────────────┴────────────────────────────────────┤
│ TOOL CALL (call #57)                                                   │
│ ▸ Bash  cd /home/.../TICKET-158 && cat result.log      out 3.2K  [+]  │
│ ▾ Edit  src/handler.ts                                                 │
│   - const status = res.code                                            │
│   + const status = res.statusCode ?? 404                               │
│ ▸ Read  docs/investigation_vi.md  (1–420)                out 18K  [+]  │
└────────────────────────────────────────────────────────────────────────┘
```

## 3. Skill: list and "where used" (M3)

```
┌─[App] › Skill──────────────────────────────────────────────────── ─ □ ✕ ┐
│ [+ Install skill]  [↶ Undo]  Filter: [All ▾] [Source ▾]  🔎 Search skills…│
│ Total baseline cost: ~2.4K tok/session · 16 skills on · 5 unused 30 days │
├───────────────────────────────────────────┬─────────────────────────────┤
│ SKILL          SOURCE  ON  BASE  /CALL USES  AVG/USE  LAST│ pj-review                   │
│ ▶pj-review     personal● ~90  ~4K    5   5.7M   16/09   │ personal · ~/.claude/skills/ │
│  pj-estimate   personal● ~80  ~5K    4   2.3M   27/08   │ Baseline ~90 tok · Call ~4K  │
│  pj-investigate personal● ~85 ~6K    4   1.9M   21/09   │ 5 uses · 5 projects · 28.6M  │
│  run           plugin  ● ~60  ~3K    1  14.9M   11/09   │ [Disable] [Remove] [Open dir]│
│  brainstorming superpw ● ~70  ~6.3K  4   0.5M   today   │─────────────────────────────│
│  writing-skills superpw● ~40  ~9.7K  1   0.8M   28/08   │ WHERE USED                  │
│  /model        command –   –    –   14    –     23/09   │ 16/09 TICKET-201   4.4M     │
│ ─ unused ─────────────────────────────────              │  "read the details of the…"  │
│  pj-debug      personal● ~75  ~3K    0    –     –   💡  │ 10/09 project-shop          │
│  pj-design     personal● ~80  ~8K    0    –     –   💡  │  "review it again to check" │
│  pj-code       personal● ~90  ~7K    0    –     –   💡  │ 09/09 project-shop        0.6M│
│  estimate      personal● ~70  ~5K    0    –     –   💡  │ 29/08 PROJECT-SUMMARY 🔴15.4M │
│ ─ disabled ──────────────────────────────────           │ 27/08 project-shop        3.1M│
│  old-skill     personal○  –    –     2   1.1M   02/08   │  (click → open that prompt)  │
├───────────────────────────────────────────┴─────────────────────────────┤
│ 💡 5 skills unused 30 days cost ~390 tok/session. [Review & batch-disable]│
└─────────────────────────────────────────────────────────────────────────┘
  ● on  ○ off   BASE = tokens always loaded per session   /CALL = tokens per call
```

## 4. Install-skill and confirm-operation dialogs

```
┌─ Install skill ─────────────────────────┐   ┌─ Confirm: Disable skill ────────────┐
│ Source:  (•) Directory  ( ) Git URL     │   │ Disable "pj-debug"?                 │
│          ( ) Plugin (marketplace)       │   │                                     │
│ ┌─────────────────────────────┐[Choose…]│   │ Will do:                            │
│ │ ~/skills-repo/pj-new  │        │   │  mv ~/.claude/skills/pj-debug       │
│ └─────────────────────────────┘        │   │   → ~/.claude/skills-disabled/      │
│ Install as: (•) Symlink  ( ) Copy       │   │                                     │
│                                         │   │ Saves ~75 tok per session.          │
│ ✔ Found SKILL.md                        │   │ Undoable in the Journal.            │
│ ✔ name: pj-new                          │   │ ⓘ Takes effect from the next Claude session.│
│ ✔ description: 142 chars (~40 baseline tok)│ │                                     │
│ ⚠ Duplicate name? No                    │   │            [Cancel]  [Disable skill]│
│                  [Cancel]  [Install]    │   └─────────────────────────────────────┘
└─────────────────────────────────────────┘
```

## 5. Project and Day (M4)

```
┌─[App] › Project──────────────────────────────────────────────────┐
│ PROJECT               SESSIONS PROMPTS TOKEN  COMMON SKILLS  MOST EXPENSIVE│
│ PROJECT-X-TICKET-101    3    231   184M ███  pj-review,run   14.9M ▸│
│ project-shop...-TICKET-102    2     88    98M ██   pj-spec,clarify  8.7M ▸│
│ PROJECT-X-TICKET-103    2     64    61M █▌   pj-investigate   4.1M ▸│
│ PROJECT-SUMMARY 1     52    54M █▌   pj-review       15.4M ▸│
│ ...                         (click → Prompts filtered by project)    │
└──────────────────────────────────────────────────────────────────┘
```

```
┌─[App] › Day─────────────────────────────── [Week ▾] [By: Token ▾]┐
│ 200M┤                  █                                            │
│     ┤                  █                                            │
│ 100M┤        █         █                                            │
│     ┤     █  █     █   █              █        █                    │
│   0 ┼──▁──▂──█──▃──█───█───▂──▁──▁────█───▂────█──▁                 │
│      03 04 07 08 09 10 11  16 17 18   21  22   23 24   (09/2026)    │
│ Click a bar → Prompts for that day                                  │
│ 11/09: 201M · 3 sessions · most expensive prompt "test e2e lambda" 14.9M│
└─────────────────────────────────────────────────────────────────────┘
```

## 6. Settings

```
┌─[App] › Settings─────────────────────────────────────────────────┐
│ LIMIT TRACKING (statusline)                                       │
│  Status: ● Enabled · updated 12 seconds ago                       │
│  Script: ~/.claude/statusline-command.sh (backed up)  [Remove]     │
│ THRESHOLDS                                                        │
│  Quota warning    [70]% and [90]%                                 │
│  Anomalous        > [3]× median   or context > [300]K             │
│ NOTIFICATIONS ☑ Quota  ☑ Blocked/Reset  ☑ Anomalous prompt  ☑ Ctx │
│ DATA SOURCES  Claude Code: [~/.claude/projects      ] [Change]     │
│ SYSTEM     ☑ Start with the machine   Time zone: [System ▾]       │
│ PRICING    pricing.toml  [Open file]  · 2 models without a price ⚠ │
│ DIAGNOSTICS  Error log lines: 0 · Tested log version: 2.1.247     │
│             [Run audit]  [Skill operation journal]                │
└───────────────────────────────────────────────────────────────────┘
```

## 7. Tray and notifications

```
 System bar:  … 🔊 📶 [🟡78%] 🕐 14:02        ← icon changes color: 🟢<70 🟡≥70 🔴≥90/blocked
                            │
               ┌────────────┴──────────────────┐
               │ 5 hours ████████░░ 78%  ↻14:30 │
               │ 7 days  ████░░░░░░ 41%  ↻Mon   │
               │ Today 2.1M tok · $3.2          │
               │ Last prompt: 620K "so what…"   │
               ├────────────────────────────────┤
               │ Open dashboard                 │
               │ Pause notifications for 1 hour │
               │ Quit                           │
               └────────────────────────────────┘

 ┌───────────────────────────────────┐  ┌───────────────────────────────────┐
 │ [App] · Quota                     │  │ [App] · Anomalous prompt          │
 │ Used 90% of the 5-hour limit.     │  │ "now update the progress…" cost   │
 │ Resets at 14:30 (28 minutes left).│  │ 9.4M (14× avg) – from re-reading  │
 └───────────────────────────────────┘  │ 841K context. Recommend /compact. │
 ┌───────────────────────────────────┐  │                            [View] │
 │ [App] · Large context             │  └───────────────────────────────────┘
 │ Session project-shop is at 450K.  │  ┌───────────────────────────────────┐
 │ Recommend /compact or a new one.  │  │ [App] · Reset                     │
 └───────────────────────────────────┘  │ The 5-hour limit has reset.       │
                                        └───────────────────────────────────┘
```

## 8. First launch (setup)

```
┌─ Welcome to [App] ──────────────────────────────────────────┐
│ Step 1/3  Data sources                                      │
│  ✔ Found Claude Code: ~/.claude/projects (24 sessions)      │
│  ○ Codex, Kiro: supported later                             │
│ Step 2/3  Track 5h/7d limits                                │
│  One line must be added to the statusline to get the quota %.│
│  ┌──────────────────────────────────────────────────────┐   │
│  │ + echo "$input" | jq '.rate_limits' > ~/.local/…json │   │
│  └──────────────────────────────────────────────────────┘   │
│  The original script will be backed up.  [Skip]  [Enable]   │
│ Step 3/3  ☑ Start with the machine   ☑ Enable notifications │
│                                            [Start →]        │
│  Scanning logs for the first time... ████████████░░░░ 78%   │
└─────────────────────────────────────────────────────────────┘
```

Note: the `jq` snippet in step 2 is illustrative only. The real version writes the file temp-file-then-rename style, and does not depend on `jq` if the machine lacks it (see spec section 7.1).
