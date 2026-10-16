# Kiro CLI integration: verification

Real data on the machine (`~/.kiro/sessions/cli`, 113MB) was loaded into a temporary DB with `tk scan`:

| | Claude Code | Kiro |
|---|---|---|
| Sessions | 23 | 164 (including subagents) |
| Prompts | 633 (24 interrupts) | 755 (34 cancellations or tool rejections) |
| Cost | $ per pricing table | **448.02 credits** |
| Model | opus-5, opus-5-5, sonnet-5, haiku-4-5 | auto, claude-opus-4.6, claude-sonnet-4.6 |

- Scanning both tools (about 150MB) takes 1.06 seconds, 0 parse errors.
- **Kiro does not record token counts.** Kiro's token number is an estimated context: `% context × context window`, accumulated per turn. Cost is measured in credits, not converted to $. The Session Analysis screen does not apply to Kiro.
- Kiro's skills (`~/.kiro/skills`, 10 skills) appear on the Skill screen with a Kiro tag. Disable, enable, remove, and undo use the `~/.kiro` directory, while the trash and journal live separately in `data_dir/kiro`. Kiro has no skill usage count and is never suggested for disabling.
- The new **Model** screen merges the models from both tools.
- Tests: `cargo test --workspace` 156/156 · Vitest 50/50.
- This time there was no independent reviewer. Kiro skill operations reuse the `Manager` that was reviewed in M3.
