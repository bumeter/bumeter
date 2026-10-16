# Build the Ubuntu installer

Prerequisites: Rust (stable), Node 20+, and `libwebkit2gtk-4.1-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`.

```bash
cargo build --release -p bum-cli   # creates target/release/bum
mkdir -p app/src-tauri/binaries
cp target/release/bum app/src-tauri/binaries/bum-x86_64-unknown-linux-gnu   # bum ships with the app (sidecar)
(cd app/ui && npm install && npm run build)
cd app && ./ui/node_modules/.bin/tauri build --bundles deb appimage
```

Result:

- `target/release/bundle/deb/Bumeter_1.0.1_amd64.deb` (about 9.5MB). Install with `sudo apt install ./Bumeter_1.0.1_amd64.deb`. After installing, `bumeter` and `bum` are in `/usr/bin`, with an entry in the applications menu.
- `target/release/bundle/appimage/Bumeter_1.0.1_amd64.AppImage` (about 86MB, libraries included). Run directly, no installation needed.

## Upgrading from the old version (named `thongke`)

- The old version had the package name `thongke` (identifier `dev.thongke.app`), the new one is `bumeter` (identifier `dev.bumeter.app`), so `apt` treats them as two different applications and installs them side by side: the menu has two entries, and both apps can run at once. Remove the old version before (or after) installing: `sudo apt remove thongke`. Data in `~/.local/share/thongke` is not deleted when the package is removed; the first time `bumeter` or `bum` runs it will migrate to `~/.local/share/bumeter`.
- UI options stored in the app's browser (localStorage) are reset because the webview data directory (keyed by identifier) and the keys change (`thongke.*` → `bumeter.*`): the selected tool scope returns to "All", and the Skill view mode returns to "Table". Re-select once and it's fine. Language, theme, and thresholds are in `config.toml` so they are not lost.

The source icon is at `app/src-tauri/icons/design/a-bars.svg`. To regenerate the icon set, run: `./ui/node_modules/.bin/tauri icon src-tauri/icons/source.png -o src-tauri/icons`.
