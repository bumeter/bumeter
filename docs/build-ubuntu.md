# Build the Ubuntu installer

Prerequisites: Rust (stable), Node 20+, and `libwebkit2gtk-4.1-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`.

```bash
(cd app && npm install)   # @bumeter/ui (the screens) and the Tauri CLI
scripts/fetch-bum.sh x86_64-unknown-linux-gnu app/src-tauri/binaries/bum-x86_64-unknown-linux-gnu   # bum ships with the app (sidecar), from bumeter/statusline
cd app && npx tauri build --bundles deb appimage
```

Result:

- `target/release/bundle/deb/Bumeter_1.3.0_amd64.deb` (about 9.5MB). Install with `sudo apt install ./Bumeter_1.3.0_amd64.deb`. After installing, `bumeter` and `bum` are in `/usr/bin`, with an entry in the applications menu.
- `target/release/bundle/appimage/Bumeter_1.3.0_amd64.AppImage` (about 86MB, libraries included). Run directly, no installation needed.

The source icon is at `app/src-tauri/icons/design/flame-tile.svg` (rendered to `source.png` at 1024 px). To regenerate the icon set, run: `npx tauri icon src-tauri/icons/source.png -o src-tauri/icons`.
