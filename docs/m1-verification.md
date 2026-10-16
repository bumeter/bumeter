# M1: Verification on real data

- Measurement date: 2026-09-24. Machine: Ubuntu GNOME, release build.
- Data: `~/.claude/projects`, 38MB, 16,482 lines.

| Criterion (spec section 1.3) | Target | Result | Met? |
|---|---|---|---|
| Tokens match an independent count (`tk audit`) | 100% | input 4,984 · cache_write 18,149,215 · cache_read 735,396,956 · output 3,830,159. Total 757,381,314, **100% match**, 0 parse errors | ✅ |
| First scan | < 2 seconds | 326ms (`tk audit`), 221ms (app) | ✅ |
| Latency from log write to handler | < 100ms | 31ms (`host` test, release) | ✅ |
| CPU when there is no new log | ~0% | 0–0.5% | ✅ |
| RAM while running in the background (window closed) | < 20MB | App: RSS 59MB, **private memory 13.9MB**, PSS 30MB. `tk daemon`: RSS 6.4MB | ⚠️ See notes |

## Notes on RAM

- The Tauri app preloads GTK and WebKitGTK even before the window opens, taking about 45MB of file-mapped memory. This part is shared libraries also used by other GTK apps. The app's private memory is only 13.9MB.
- Measured by RSS, the app **does not meet** the under-20MB level. `tk daemon` does (6.4MB).
- Direction (to be decided in M6): split the tray into a lightweight process, and have the Tauri window start only when "Open dashboard" is clicked. This approach leverages the existing host/client architecture.

## Real-run tests (Task 10)

- `tk status` fetches data over the socket from the running app ✅
- `tk statusline` shows `🔴5h 92% ↻07:00 · 7d 41% · today 38.6M` ✅
- Starting a second host is rejected ✅
- The 90% quota notification is written to `alerts_sent` (only reports the highest threshold) ✅
- The app takes over the host when `tk daemon` is running ✅
- Found and fixed a bug: a long data dir made the socket path exceed the `sun_path` limit. Switched to using `$XDG_RUNTIME_DIR`, with a test.
