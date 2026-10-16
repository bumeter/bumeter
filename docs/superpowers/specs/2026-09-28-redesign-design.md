# Bumeter — Thiết kế lại giao diện + đổi tên (Spec, 2026-09-28)

> **Mockup chuẩn:** `2026-09-28-redesign-mockups/bumeter-ui-mascot.html` (đã duyệt). Thương hiệu: `docs/brand/README.md` + `docs/brand/bum/`. Các mockup khác trong thư mục `2026-09-28-redesign-mockups/` là các vòng thử trước đó: **chỉ tham khảo bố cục; chỗ nào khác bản chuẩn thì theo bản chuẩn.**

## 1. Mục tiêu

Người dùng thấy giao diện cũ **chật/rối** và **chưa hiện đại**, và muốn một thương hiệu riêng. Đợt này:

- Đổi tên dự án **thongke → Bumeter** (Bum = biệt danh tác giả, meter = đồng hồ đo), linh vật **Bum** làm logo và đèn báo trạng thái.
- Làm lại lớp giao diện theo mockup chuẩn: cam chủ đạo, thân thiện, hiện đại, nút phẳng; dùng tốt ở cửa sổ rộng lẫn nửa màn hình.
- Không đổi chức năng, không đổi cách lấy dữ liệu; app vẫn nhẹ (TypeScript thuần + `h()`, không framework).

Thành công khi: mọi màn khớp mockup chuẩn ở sáng/tối, vi/en; ở ~1000px không còn 3 cột bị bóp; tray và thông báo dùng biểu cảm Bum; lệnh `bum` và thư mục dữ liệu mới hoạt động, dữ liệu cũ được chuyển sang; toàn bộ test qua.

## 2. Quyết định đã chốt

| Chủ đề | Chốt |
|---|---|
| Tên | **Bumeter**; logo chữ viết thường `bumeter`; CLI **`bum`** (thay `tk`); thư mục dữ liệu **`<data_dir>/bumeter`** (thay `thongke`), tự chuyển dữ liệu cũ |
| Logo | Linh vật **Bum** (ngọn lửa tròn có mặt), 5 biểu cảm: `happy`, `calm`, `worry`, `blaze`, `sleep` (SVG trong `docs/brand/bum/`) |
| Màu | Cam chủ đạo `#FF7A1A` (tối: `#FF8A33`); cam đậm cho chữ `#E8640A`; nền nhạt cam `#FFF0E3`; đỏ cam `#FF4B2B` cho cực tốn; xanh `#22A05A` cho giảm/tiết kiệm. Nền sáng ấm `#FAF7F4`, mặt khối `#FFFFFF`; nền tối than nâu `#15120F`, mặt khối `#1F1A16`. **Không gradient ở đâu cả** |
| Chữ | **Be Vietnam Pro** (nhúng qua `@fontsource/be-vietnam-pro`, chạy offline), số dạng tabular; tiêu đề 800, logo chữ 900 |
| Bo góc theo cấp | Khung lớn (panel, danh sách) 16px; ô số/khung phụ 12px; nút, chip, ô tìm 9–10px; tag nhỏ 5px |
| Nút | Phẳng: nút chính nền cam chữ trắng; nút phụ nền `sunk`; không bóng đổ, không gradient. Bóng chỉ cho lớp nổi (ngăn kéo, hộp thoại, menu) |
| Nhận diện công cụ | Logo công cụ (Claude Code, Kiro) — màu cam dành cho "tiêu hao" và hành động chính |
| Bố cục theo độ rộng | `wide` ≥ 1200px: 3 cột. `medium` 700–1199px: sidebar thành cột icon, chi tiết là **ngăn kéo trượt từ phải** (~60%, Esc/✕/bấm nền để đóng). `narrow` < 700px: một cột, chi tiết là trang riêng có "← quay lại" |
| Tinh thần | "macOS-inspired, thân thiện, hiện đại": giữ thanh tiêu đề gốc, bố cục sidebar → danh sách → chi tiết, segmented control, phím tắt (Ctrl/⌘+K tìm, ↑↓, Esc); không bắt chước vibrancy/Aqua |

## 3. Nền tảng chung

### 3.1 Token (`app/ui/src/styles/tokens.css`)

Biến: `--bg`, `--surf`, `--sunk`, `--line`, `--ink`, `--ink2`, `--ink3`, `--acc`, `--acc2`, `--tint`, `--red`, `--grn` (giá trị sáng/tối như mục 2 và mockup chuẩn), `--r-panel: 16px`, `--r-box: 12px`, `--r-ctl: 10px`, `--r-tag: 5px`, khoảng cách bậc 4px. Chế độ: theo hệ thống, hoặc `data-theme="light|dark"` (Cài đặt).

### 3.2 Component (`app/ui/src/components/`)

| Component | Việc |
|---|---|
| `bum.ts` | `bum(state, size)` trả SVG linh vật (5 biểu cảm, màu theo token); `bumStateFor(...)` chọn biểu cảm từ dữ liệu (mục 3.4) |
| `drawer.ts` | Ngăn kéo phải: mở/đóng, Esc, bấm nền mờ, ✕; focus vào trong, trả focus khi đóng; `role="dialog"` |
| `chipMenu.ts` | Chip có menu thả xuống (một giá trị) và `chipMultiMenu` (nhiều giá trị, chip hiện số đang chọn); bàn phím ↑↓/Enter/Esc |
| `ui.ts` | `countPill`, `kpi` (nhãn, số lớn, dòng phụ), `insightRow` (icon theo mức độ, tiêu đề, mô tả, nút hành động) |

### 3.3 Khung app

- `main.ts` theo dõi độ rộng (ResizeObserver) và đặt `#app[data-layout]`; CSS đổi grid.
- **Sidebar:** logo Bum + chữ `bumeter`; bộ chọn phạm vi công cụ; nav có icon (Tổng quan, Hội thoại, Skill, Cài đặt); **thẻ Bum** ở đáy: Bum theo trạng thái + "Quota 5 giờ X%" + giờ reset + thanh quota (chỉ khi phạm vi có quota; không có thì Bum + trạng thái chung).
- Tìm nhanh: ô tìm kiếm có gợi ý phím `Ctrl K` (mở tìm hội thoại; bảng lệnh đầy đủ để sau).

### 3.4 Biểu cảm Bum theo trạng thái

Ưu tiên từ trên xuống:

1. `sleep`: dữ liệu quota đã cũ (`stale`) **và** không có lượt mới trong 30 phút, hoặc chưa có dữ liệu nào.
2. `blaze`: quota 5 giờ ≥ ngưỡng thứ hai (`quota_thresholds[1]`, mặc định 90%), hoặc có lượt tốn bất thường trong 10 phút gần nhất.
3. `worry`: quota 5 giờ ≥ ngưỡng thứ nhất (mặc định 70%).
4. `happy`: token hôm nay < trung bình mỗi ngày dùng của 30 ngày.
5. `calm`: còn lại.

Dùng ở: sidebar, Tổng quan (cạnh số token chính), tray (icon), thông báo (icon), màn trống (`sleep`), lời giải thích "vì sao tốn" (`worry`).

## 4. Từng màn (theo mockup chuẩn)

### 4.1 Tổng quan

- Chọn khoảng: Hôm nay / 7 ngày / 30 ngày / 90 ngày.
- Hàng 3 thẻ: **Token** (Bum + số lớn + "Ít/Nhiều hơn X% so với kỳ trước", màu xanh/đỏ), **Lượt** (TB mỗi ngày dùng), **Hội thoại** (số theo từng công cụ khi phạm vi Tất cả).
- Thẻ **Token theo ngày** (cột bo đầu, cột tốn nhất cam đậm, bấm cột để lọc Hội thoại) cạnh thẻ **Bum gợi ý** (tối đa 3 dòng: lượt tốn bất thường → đỏ; skill tốn nền mà không dùng → cam, lấy từ `api.skills(false)`, lỗi thì ẩn; project chiếm nhiều token → xám). Mỗi dòng có nút hành động.
- Project và Model: thẻ riêng bên dưới (danh sách có thanh ngang), xếp chồng khi hẹp.
- "Kỳ trước" = đúng số ngày liền trước; kỳ trước bằng 0 thì không hiện phần so sánh.

### 4.2 Hội thoại

- Tiêu đề + số đếm + ô tìm; **một hàng lọc**: chip menu Project, chip menu thời gian, chip menu Model (nhiều), chip lọc nhanh (Tốn bất thường, Context lớn, Ngắt ngang, ⭐).
- Danh sách trên một khung trắng bo 16px: nhóm theo ngày (tiêu đề đậm, chữ thường), mỗi dòng: logo công cụ (khi phạm vi Tất cả) + tiêu đề, dòng phụ (project, giờ hoặc credit), cột số lượt, **thanh nhiên liệu** (cam; đỏ khi lượt/hội thoại cực tốn) + số token. Dòng đang chọn nền `--tint`.
- Chi tiết: tiêu đề, meta (project, model, khoảng giờ — không lặp token/lượt), 3 ô số (Token cam, Lượt, Context lớn nhất), nút "Phân tích context" (chính) và "Đánh dấu"; các lượt: giờ, câu hỏi, xem nhanh câu trả lời, token; lượt mở rộng giữ đầy đủ hỏi/đáp/tool call; "vì sao tốn" trong khung `--tint` có Bum `worry`.

### 4.3 Skill

Giữ tính năng hiện có (bảng/thẻ, công tắc, chip lọc, nhãn cảnh báo, tìm trên skills.sh); đổi sang token/component mới; thêm chip "Tất cả"; thẻ skill tốn nền mà không dùng có viền cam; bỏ chấm trạng thái trên thẻ (công tắc thể hiện bật/tắt).

### 4.4 Cài đặt

Nhóm thành khung: **Chung** (ngôn ngữ, giao diện Sáng/Tối/Theo hệ thống — `Config.theme`), **Công cụ** (logo, thư mục log, số lượt, trạng thái từ `sources`), **Giới hạn & thông báo**, **Chẩn đoán**. **Lưu ngay** khi đổi (debounce 400ms; giá trị không hợp lệ thì báo lỗi tại chỗ, không lưu); ngưỡng cần mở lại app thì vẫn ghi rõ.

### 4.5 Phân tích context

Biểu đồ, thành phần lượt gọi, gợi ý là các khung riêng theo token mới; gợi ý dùng `insightRow`.

### 4.6 Màn trống và lỗi

Bum `sleep` + một câu nói rõ việc cần làm + nút hành động (ví dụ "Bum đang chờ hội thoại đầu tiên").

## 5. Tray, icon app, thông báo

- Icon app (1024 → các cỡ Tauri, `.icns`, `.ico`): Bum `calm` trên nền trắng (theo `docs/brand/bum/`). PNG xuất bằng Chrome headless từ SVG (máy không có rsvg/inkscape), commit vào `app/src-tauri/icons/`.
- Tray: thay chấm màu bằng **Bum theo trạng thái** (mục 3.4) ở cỡ 32px (PNG nhúng sẵn cho 5 biểu cảm); tooltip giữ dòng trạng thái hiện tại.
- Thông báo: tiêu đề "Bumeter · …"; icon theo loại (quota → `worry`/`blaze`, tốn bất thường → `blaze`).

## 6. Đổi tên

| Hạng mục | Cũ | Mới |
|---|---|---|
| Tên hiển thị, cửa sổ, bộ cài | thongke | Bumeter |
| Tauri `productName` / `identifier` | `thongke` / `dev.thongke.app` | `Bumeter` / `dev.bumeter.app` |
| Binary app | `thongke-app` | `bumeter` |
| CLI | `tk` | `bum` (sidecar `binaries/bum-<triple>`) |
| Thư mục dữ liệu | `<data_dir>/thongke`, `thongke.db`, socket `thongke-*.sock` | `<data_dir>/bumeter`, `bumeter.db`, `bumeter-*.sock` |
| Biến môi trường | `TK_DATA_DIR`, `TK_CLAUDE_DIR`, `TK_KIRO_DIR` | giữ nguyên (tương thích), thêm tên mới `BUM_*` được ưu tiên |

- **Chuyển dữ liệu:** khi mở app/CLI lần đầu, nếu `<data_dir>/bumeter` chưa có mà `<data_dir>/thongke` có: đổi tên thư mục và file DB (`thongke.db*` → `bumeter.db*`) trong một bước; lỗi thì giữ nguyên thư mục cũ và dùng nó, ghi log.
- **Hook statusline:** nếu `~/.claude/settings.json` có lệnh statusline trỏ tới binary `tk` cũ, app viết lại sang `bum` (sao lưu trước, như thao tác statusline hiện có).
- Tên crate nội bộ (`tk-core`, `tk-adapters`…) **giữ nguyên** để giảm xáo trộn; chỉ đổi thứ người dùng nhìn thấy.
- Tài liệu: README, PROGRESS, build-ubuntu cập nhật tên mới.

## 7. Ngoài phạm vi

Tính năng dữ liệu mới (tag hội thoại, preset skill, đo tác động khi sửa skill, bảng lệnh Ctrl+K đầy đủ); đổi màu công cụ; bản Windows/macOS và CI.

## 8. Kiểm thử

- Rust: chuyển dữ liệu thư mục cũ → mới (có/không có thư mục mới, lỗi giữ cũ), `Config.theme`, viết lại hook `tk` → `bum`, chọn biểu cảm tray.
- Vitest: `bum.ts` (`bumStateFor` theo từng điều kiện và thứ tự ưu tiên), `drawer`, `chipMenu`/`chipMultiMenu`, `data-layout`, lưu ngay ở Cài đặt, so sánh kỳ trước; cập nhật selector các test hiện có; giữ kiểm tra XSS.
- Ảnh Chrome headless (dữ liệu giả) ở 1360/1000/640px × vi/en × sáng/tối, so với mockup chuẩn.
- Chạy app thật: IPC, ngăn kéo khi thu nhỏ cửa sổ, tray đổi Bum theo quota, lệnh `bum status`, dữ liệu cũ vẫn còn sau khi đổi tên. Build bộ cài `.deb`/`.AppImage` tên Bumeter.
