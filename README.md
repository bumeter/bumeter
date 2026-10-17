<p align="center">
  <img src="docs/brand/bum/bum-calm.svg" width="96" alt="Bum, the Bumeter mascot">
</p>

<h1 align="center">Bumeter</h1>

<p align="center">See where your AI coding tokens go — per conversation, per turn, per skill — and how close you are to your limits.</p>

Bumeter is a desktop app for Linux that reads the local logs of AI coding assistants
(**Claude Code**, **Kiro** and **OpenCode** (beta) today) and turns them into something you can act on:
which conversations and turns were expensive, which skills cost tokens in every session,
and how much of your 5-hour, 7-day and monthly quota is left.

Everything is read from your own machine. Nothing is uploaded.

![Overview](docs/screens/readme-overview.png)

## Features

- **Tray with a quota bar per tool.** Bum, the mascot, changes face with your usage
  (calm, happy, worried, on fire, asleep), and the highest percentage sits next to it in the top bar.

  ```
  Claude Code
  ▰▰▰▰▰▱▱▱▱▱ 🟢  5 hours · 49% · reset 13:10
  ▰▰▰▰▰▰▰▰▰▱ 🔴  7 days · 90% · reset Oct 16
  Kiro
  ▰▰▰▱▱▱▱▱▱▱ 🟢  Month · 27% · 271/1000 credits · reset Nov 1
  ```
- **Limit forecast.** When your current pace would hit the 5-hour, 7-day or monthly limit before it resets,
  the tray, `bum status` and the app show when (`out ~14:20`), Bum gets worried, and you get one notification
  (lead time in Settings).
- **Overview** for today, 7, 30 or 90 days: tokens compared with the previous period,
  tokens per day, costliest projects and models, and suggestions from Bum
  (unusually costly turns, a project eating most of the budget, skills that cost tokens but are never used).
- **Conversations** grouped by day, with a fuel bar per conversation, filters
  (project, time range, model, costly, large context, interrupted, starred) and the full list of turns.
- **Context analysis** for a session: what fills the context window, when it was compacted, which tool calls were large.
- **Skills**: always-on token cost and real usage of every skill; install (also from
  [skills.sh](https://skills.sh)), disable, change size or remove, all with undo — and a read-only scan
  that flags a `SKILL.md` whose instructions try to hijack the model, reach your credentials, or hide
  what it does.
- **Per-tool tabs** on every screen, so adding Codex, Cursor or OpenCode later is one more tab, not a redesign.
- **Notifications** when a quota passes your thresholds (70% / 90% by default) or a turn costs far more than usual.
- **Light and dark themes**, Vietnamese and English.

| Conversations | Skills |
|---|---|
| ![Conversations](docs/screens/readme-conversations.png) | ![Skills](docs/screens/readme-skills.png) |

## Install

Bumeter needs a desktop with WebKitGTK 4.1, i.e. **Ubuntu 24.04 or newer** (or an equivalent distribution).

**Debian/Ubuntu package**

```bash
sudo apt install ./Bumeter_1.0.5_amd64.deb
```

This installs `bumeter` (the app) and `bum` (the command line tool) in `/usr/bin`.

**AppImage** (no install, libraries included)

```bash
chmod +x Bumeter_1.0.5_amd64.AppImage
./Bumeter_1.0.5_amd64.AppImage
```

**Snap** — the `bumeter` snap is waiting for the Snap Store's classic-confinement review.
Once it is published: `sudo snap install bumeter --classic`.

**macOS** (Homebrew)

```bash
brew install --cask chieund/tap/bumeter
```

The cask lives in [chieund/homebrew-tap](https://github.com/chieund/homebrew-tap). Bumeter is not
signed with an Apple Developer ID, so macOS quarantines it: open it once with right-click → Open (the
cask prints this when it installs). That is also why it is not in `homebrew/cask`, which requires
notarized applications. The `.dmg` on [Releases](https://github.com/chieund/bumeter/releases) works
the same way.

**Windows** — the `.msi` and `.exe` are on Releases. The WinGet manifest is generated from those
files (`scripts/packaging.py`) but the package is not in `winget-pkgs` yet; see
[docs/packaging.md](docs/packaging.md) for what publishing it takes.

## How it gets the numbers

| Source | What Bumeter reads |
|---|---|
| Claude Code | Session logs in `~/.claude/projects/**` (tokens, turns, tools, skills). The 5-hour and 7-day quota comes from Claude Code's status line: Bumeter adds `bum statusline --tee` in front of your existing status line command in `~/.claude/settings.json` (with a backup and an undo button in Settings). |
| Kiro | Session files in `~/.kiro/sessions/cli/**`. Kiro does not log tokens, so context is estimated from each turn's context percentage, and cost is in credits. The monthly credit limit comes from `kiro-cli chat --no-interactive /usage`, run only when `kiro-cli` is installed and `kiro-cli whoami` says you are logged in. You can also type the limit in Settings. |
| OpenCode (beta) | Its SQLite database `~/.local/share/opencode/opencode.db` (OpenCode 2.x), opened **read-only** and read incrementally. Real token counts per model call, including reasoning tokens. OpenCode also reports a USD cost per call; Bumeter stores it but shows tokens only for now. No quota and no conversation transcript yet. |

Tools that are not installed are simply not shown.

Data lives in `~/.local/share/bumeter/` (a SQLite database, `config.toml`, backups).

## Command line

```
bum status       today's summary and quota bars from the running app
bum statusline   used by the Claude Code status line hook
bum scan         read the logs into the database without the app
bum daemon       run the background host without a window
bum session      context breakdown of one session
bum audit        compare Bumeter's token totals with an independent count
```

`bum skills` works on what Claude Code (or `--tool kiro`) loads, from a terminal or from an agent:

```
bum skills list                always-on cost and usage of each skill
bum skills disable <name>      plan to move a skill out of the way (--yes applies it)
bum skills enable <name>       plan to move it back
bum skills undo <id>           put the files of one journal entry back
bum skills log                 what was changed, newest first
bum skills audit               look for hijacked instructions, credentials and duplicates in SKILL.md
```

`disable` and `enable` always print the plan first: without `--yes` nothing moves and the exit code is
non-zero, so a typo cannot silently change what your next session loads. `--json` puts the plan, the
journal entry or the report on stdout for scripts, and `bum skills audit` exits non-zero when it finds
something high-risk (an agent that reads your files, a credential next to a way to send it out, or
instructions that tell the model to hide something from you). The scan is a heuristic over the text:
every finding shows the line it matched, so read that line before acting on it.

## Build from source

Requirements: Rust (stable), Node 20+, `libwebkit2gtk-4.1-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`.

```bash
cargo build --release -p bum-cli
mkdir -p app/src-tauri/binaries
cp target/release/bum app/src-tauri/binaries/bum-x86_64-unknown-linux-gnu
(cd app/ui && npm install && npm run build)
cd app && ./ui/node_modules/.bin/tauri build --bundles deb appimage
```

The packages land in `target/release/bundle/`. Snap: `snapcraft pack` (uses `snap/snapcraft.yaml`, built from the `.deb`).
More detail in [docs/build-ubuntu.md](docs/build-ubuntu.md).

Tests: `cargo test --workspace` and `cd app/ui && npx vitest run && npx tsc --noEmit`.

### CI and releases

- `.github/workflows/ci.yml` runs the Rust tests on Linux, macOS and Windows, plus the UI tests, on every push and pull request.
- `.github/workflows/release.yml` builds the installers with `tauri-action`: `.deb` + `.AppImage` (Ubuntu 22.04+),
  `.dmg` (Apple Silicon and Intel), `.exe` (NSIS) + `.msi` (Windows). Run it from the Actions tab to get the files as
  artifacts, or push a tag (`git tag v1.0.5 && git push origin v1.0.5`) to get a published GitHub Release with the
  files attached. The macOS and Windows builds are not code-signed yet, so Gatekeeper and SmartScreen will warn on
  first launch.
- Auto-update goes through the same GitHub Release (Settings → Updates, plus a "Check for updates" tray item).
  Signing the updater bundles needs two repository secrets:
  `TAURI_SIGNING_PRIVATE_KEY` (contents of the private key) and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` (empty if the
  key has no password). Generate the pair once and keep the private key safe forever — losing it means no further
  updates for existing installs:

  ```bash
  cd app && ./ui/node_modules/.bin/tauri signer generate -w ~/.tauri/bumeter.key
  ```

  Put the public key (`~/.tauri/bumeter.key.pub`) into `plugins.updater.pubkey` in
  `app/src-tauri/tauri.conf.json`. `createUpdaterArtifacts` lives in `app/src-tauri/tauri.ci.conf.json` and is only
  passed to CI (`--config`), so a local `tauri build` still works without the private key.
  Linux auto-update only works for the `.AppImage` (a `.deb` install updates through the package manager).

### Project layout

```
crates/bum-core       database, ingestion engine, quotas, tray text, insights
crates/bum-adapters   readers for Claude Code and Kiro logs and the OpenCode database, Kiro /usage
crates/bum-service    background host and IPC
crates/bum-skills     skill catalog, install/disable/remove with undo, skills.sh
crates/bum-cli        the `bum` command
app/src-tauri         the Tauri app: window, tray, notifications
app/ui                the interface (TypeScript, no framework)
docs/build-ubuntu.md  build the Ubuntu package (deb / AppImage)
docs/brand            Bum, the mascot (used in this README)
docs/screens          screenshots used in this README
```

## Credits

Tool logos in `app/ui/public/tool-icons/` belong to their owners; see the
[NOTICE](app/ui/public/tool-icons/NOTICE.md) file there.

## License

Bumeter is free software under the **GNU General Public License v3.0 or later** — see [LICENSE](LICENSE).
You may use, study, change and share it. If you distribute a modified version, it must stay open source under the same license.

**Name and logo.** The license covers the code, not the Bumeter name or the Bum mascot.
Forks are welcome, but a fork you distribute must use a different name and logo, so users can tell it apart from the official Bumeter.
