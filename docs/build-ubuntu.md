# Build bộ cài Ubuntu

Yêu cầu có sẵn: Rust (stable), Node 20+, và `libwebkit2gtk-4.1-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`.

```bash
cargo build --release -p tk-cli
mkdir -p app/src-tauri/binaries
cp target/release/tk app/src-tauri/binaries/tk-x86_64-unknown-linux-gnu   # tk đi kèm app (sidecar)
(cd app/ui && npm install && npm run build)
cd app && ./ui/node_modules/.bin/tauri build --bundles deb appimage
```

Kết quả:

- `target/release/bundle/deb/thongke_0.1.0_amd64.deb` (khoảng 9,5MB). Cài bằng `sudo apt install ./thongke_0.1.0_amd64.deb`. Cài xong có `thongke-app` và `tk` trong `/usr/bin`, kèm mục trong menu ứng dụng.
- `target/release/bundle/appimage/thongke_0.1.0_amd64.AppImage` (khoảng 86MB, đã gồm sẵn thư viện). Chạy trực tiếp, không cần cài.

Icon gốc nằm ở `app/src-tauri/icons/design/a-bars.svg`. Muốn tạo lại bộ icon thì chạy: `./ui/node_modules/.bin/tauri icon src-tauri/icons/source.png -o src-tauri/icons`.
