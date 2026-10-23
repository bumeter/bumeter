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

- `target/release/bundle/deb/Bumeter_1.1.0_amd64.deb` (about 9.5MB). Install with `sudo apt install ./Bumeter_1.1.0_amd64.deb`. After installing, `bumeter` and `bum` are in `/usr/bin`, with an entry in the applications menu.
- `target/release/bundle/appimage/Bumeter_1.1.0_amd64.AppImage` (about 86MB, libraries included). Run directly, no installation needed.

The source icon is at `app/src-tauri/icons/design/a-bars.svg`. To regenerate the icon set, run: `./ui/node_modules/.bin/tauri icon src-tauri/icons/source.png -o src-tauri/icons`.
