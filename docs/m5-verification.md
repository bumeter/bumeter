# M5: Session Analysis screen verification

- Screenshots use fake data: `screens/m5-light.png`, `screens/m5-dark.png`.
- Real run: `tk session <id>` on the largest session (TICKET-101): 224 prompts, 726 turns, 1 compact. **Analysis takes 48ms.**

## Context composition at the largest call (963K tokens, estimated)

| Group | Tokens | % |
|---|---|---|
| System & unmeasurable | 354K | 37% |
| Tool I/O | 247K | 26% |
| **Thinking** | **280K** | **29%** |
| Output | 63K | 7% |
| Files read | 15K | 2% |

## Findings during a real run (fixed, with tests)

1. Attachments contain content in various fields (`snippet`, `text`, `content` as a list…). Now the app reads them all. `prompt_snapshot` is a system prompt snapshot repeated many times, so it is skipped.
2. **Thinking content is hidden, but the encrypted signature (`signature`) is still sent back in the context.** The signature alone is 979K characters, the most in the log. Now the app estimates thinking by signature length. Before the fix, this part was lumped entirely into "System" (66%).

## Limitations

- The ratio of 3.5 characters/token is only an estimate. Code and Vietnamese tend to cost more tokens per character, so the "unmeasurable" part also includes this error margin. The system prompt, tool definitions, and CLAUDE.md are not in the log.
- The real session is only run through the CLI. This screen in the app has been tested with Vitest and screenshotted with fake data.

Tests: `cargo test --workspace` and Vitest 41/41.
