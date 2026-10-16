# Build bộ cài Ubuntu

Yêu cầu có sẵn: Rust (stable), Node 20+, và `libwebkit2gtk-4.1-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`.

```bash
cargo build --release -p bum-cli   # tạo target/release/bum
mkdir -p app/src-tauri/binaries
cp target/release/bum app/src-tauri/binaries/bum-x86_64-unknown-linux-gnu   # bum đi kèm app (sidecar)
(cd app/ui && npm install && npm run build)
cd app && ./ui/node_modules/.bin/tauri build --bundles deb appimage
```

Kết quả:

- `target/release/bundle/deb/Bumeter_1.0.0_amd64.deb` (khoảng 9,5MB). Cài bằng `sudo apt install ./Bumeter_1.0.0_amd64.deb`. Cài xong có `bumeter` và `bum` trong `/usr/bin`, kèm mục trong menu ứng dụng.
- `target/release/bundle/appimage/Bumeter_1.0.0_amd64.AppImage` (khoảng 86MB, đã gồm sẵn thư viện). Chạy trực tiếp, không cần cài.

## Nâng cấp từ bản cũ (tên `thongke`)

- Bản cũ có tên gói `thongke` (identifier `dev.thongke.app`), bản mới là `bumeter` (identifier `dev.bumeter.app`), nên `apt` coi là hai ứng dụng khác nhau và cài song song: menu có hai mục, có thể chạy hai app cùng lúc. Gỡ bản cũ trước (hoặc sau) khi cài: `sudo apt remove thongke`. Dữ liệu ở `~/.local/share/thongke` không bị xoá khi gỡ gói; lần đầu chạy `bumeter` hoặc `bum` sẽ tự chuyển sang `~/.local/share/bumeter`.
- Tuỳ chọn giao diện lưu trong trình duyệt của app (localStorage) bị đặt lại vì thư mục dữ liệu webview (theo identifier) và khoá đổi (`thongke.*` → `bumeter.*`): phạm vi công cụ đang chọn quay về "Tất cả", kiểu xem Skill quay về "Bảng". Chọn lại một lần là được. Ngôn ngữ, theme, ngưỡng nằm trong `config.toml` nên không mất.

Icon gốc nằm ở `app/src-tauri/icons/design/a-bars.svg`. Muốn tạo lại bộ icon thì chạy: `./ui/node_modules/.bin/tauri icon src-tauri/icons/source.png -o src-tauri/icons`.
