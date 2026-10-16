# M2: Dashboard verification

- Date: 2026-09-24. Machine: Ubuntu GNOME, debug build.
- Screenshots use Chrome headless with fake data (`vite preview`, `?theme=light|dark`): `screens/m2-light.png`, `screens/m2-dark.png`.

| Check | Result |
|---|---|
| Matches the approved macOS mockup (3-column layout, sidebar, blue selected row, stat cards, "Why it's costly" box) | ✅ Both light and dark |
| Native OS title bar | ✅ Does not draw its own window buttons |
| **IPC works with CSP enabled** (leftover M1 bug) | ✅ Real app with real logs (17,174 lines): the UI calls `list_prompts`, `prompt_detail`, `sidebar_counts`, `today_summary`, `limit_state` (log `tk: ui call …` in the debug build) |
| Realtime while chatting | ✅ While this Claude Code session writes logs, the UI reloads the list and detail panel automatically (a second call round appears after a few seconds) |
| Time in local time | ✅ `fmt` test and screenshots (11/09 correctly shows Friday) |
| XSS protection (questions, skills, explanations, statusline operation descriptions) | ✅ Vitest tests: content `<img …>`, `<b>`, `<script>` is shown as text |
| Tests | `cargo test --workspace` 97/97 · Vitest 24/24 |

## Not verified / notes

- **RAM with the window open:** RSS 189MB (debug build, WebKitGTK). Closing the window destroys the WebView (in place since M1), but RAM after closing could not be measured because there is no tool to close the window automatically (no `xdotool`). Manual measurement needed.
- The first scan in the debug build takes 2.4 seconds for 17,174 lines. The release build in M1 is 0.3 seconds.
- **Thresholds and notification on/off only take effect after reopening the app.** The UI states this clearly.
- The "Enable limit tracking" button has only been tested on a temporary HOME, not on the real `~/.claude/settings.json`. Enabling it on the real machine is left for you to click.
