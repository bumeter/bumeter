# Bumeter for VS Code

Your Claude Code, Kiro and OpenCode usage where you code: everything is read from the logs on the machine the workspace is on, and nothing leaves it.

- **Status bar**: 5-hour and weekly quota, and the context of this workspace's latest session (`5h 62% · 7d 41% · ctx 312K`). Yellow from 70% or past the context threshold, red from 90% or when you are blocked. Hover for reset times and the forecast; click to open the Overview.
- **Explorer marks**: files of the workspace agents read 3 times or more in the last 7 days, without an edit in red (`9×`).
- **Overview panel**: the Bumeter app's own Overview, including the Effectiveness panel.

Works in remote windows (SSH, WSL, Dev Containers, Codespaces): the extension runs where the logs are. Also runs in Cursor, Windsurf and Kiro.

## How it works

The extension runs `bum rpc`, which answers from the Bumeter app or `bum daemon` when one is running, and otherwise reads the logs itself. The first start on a machine reads the whole history once (15–60 s).

`bum` is bundled with the extension. To use another one, set `bumeter.binaryPath`.

## Settings

| Setting | Default | |
|---|---|---|
| `bumeter.binaryPath` | empty | Path to `bum`; empty uses the bundled one, then `PATH` |
| `bumeter.decorations.enabled` | `true` | Mark files in the Explorer |
| `bumeter.decorations.minReads` | `3` | Reads in 7 days from which a file is marked |

## Develop

```
npm install
npm test              # unit tests (vscode is stubbed)
npm run build         # dist/extension.js
npm run package       # needs app/ui built and target/release/bum: copies them in, then makes the .vsix
```

Install the result with `code --install-extension bumeter-0.1.0.vsix`.
