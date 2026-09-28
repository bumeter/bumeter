# Thiết kế lại giao diện thongke — Spec (2026-09-28)

## 1. Mục tiêu

Người dùng nêu hai vấn đề: **bố cục chật/rối** và **trông chưa hiện đại**. Mục tiêu của đợt này:

- App dùng thoải mái ở cả cửa sổ rộng lẫn **nửa màn hình cạnh terminal/editor**.
- Phong cách hiện đại, sắc nét, nhất quán giữa các màn.
- Không đổi chức năng, không đổi cách lấy dữ liệu; app vẫn nhẹ (TypeScript thuần + `h()`, không thêm framework).

Thành công khi: ở cửa sổ ~1000px không còn 3 cột bị bóp; mọi màn dùng chung một bộ token và vài component; 64 test hiện có (sau khi cập nhật selector) và test mới đều qua; ảnh vi/en sáng/tối khớp mockup.

## 2. Quyết định đã chốt

| Chủ đề | Chốt | Mockup |
|---|---|---|
| Phong cách | **B. Thẻ hiện đại**: nền xám nhạt, mỗi mục là một thẻ trắng | `2026-09-28-redesign-mockups/visual-style.html` |
| Bo góc | **4px** (thẻ, chip, nút, số đếm), 3px cho tag nhỏ, 6px cho khung lớn. **Viền mảnh thay cho bóng đổ** | `radius.html` |
| Nút và bề mặt | **Phẳng, không gradient**: nút chính = nền xanh đặc; nút phụ = nền thẻ + viền mảnh; nút đang chọn trong nhóm (segmented) = nền đặc, không đổ bóng. Không dùng gradient ở bất cứ đâu (kể cả ô "∗ Tất cả công cụ" → màu xám đậm đặc) | — |
| Màu nhấn | **Xanh dương macOS** (`#0a84ff` tối / `#007aff` sáng). Màu công cụ giữ nguyên: Claude `#d77655`, Kiro `#9046ff` | `accent.html` |
| Bố cục theo độ rộng | Rộng ≥ 1200px: 3 cột. Vừa 700–1199px: sidebar thành **cột icon**, chi tiết là **ngăn kéo trượt từ phải** (~60%, Esc/✕ để đóng). Hẹp < 700px: một cột, chi tiết là trang riêng có "← quay lại" | `detail-layout.html`, `design-1-conversations.html` |
| Cách làm | Làm lại **lớp giao diện**, giữ logic/API; lần lượt từng màn | — |

> **Mockup chuẩn: `2026-09-28-redesign-mockups/design-v2.html`** (bo 4px, nút phẳng, viền mảnh, sáng + tối). Các mockup phần 1–3 vẽ trước đó (12px + bóng) chỉ dùng để tham khảo bố cục; chỗ nào khác v2 thì theo v2.
>
> Chỉnh sửa trong v2 (đã duyệt): (1) Model gộp thành chip menu chọn nhiều → **1 hàng lọc**; (2) chi tiết hội thoại bỏ token/lượt khỏi dòng meta (đã có ở KPI); (3) KPI Tổng quan "Tốn bất thường" → **"Hội thoại"** (tách theo công cụ); (4) thẻ Skill bỏ chấm trạng thái, chỉ giữ công tắc; (5) hội thoại đang chọn = viền xanh + vạch xanh bên trái.

### 2.1 Tinh thần: "macOS-inspired, phẳng và hiện đại"

App chạy chính trên Ubuntu (sau có Windows), nên **giữ nguyên tắc của macOS, không bắt chước bề ngoài** (gần cách Linear, Raycast, Arc làm).

- **Giữ:** thanh tiêu đề gốc của hệ điều hành; chữ SF/Inter với thứ bậc rõ; màu nhấn xanh dương theo sáng/tối hệ thống; bố cục sidebar → danh sách → chi tiết; segmented control; Cài đặt nhóm kiểu System Settings; phím tắt (⌘/Ctrl+F, ↑↓, Esc); khoảng trắng thoáng, chuyển động nhẹ.
- **Không bắt chước:** sidebar trong mờ (vibrancy), bo góc lớn, bóng đổ và gradient, control giả dáng Aqua.

### 2.2 Skill hỗ trợ khi triển khai

| Skill | Dùng khi |
|---|---|
| `frontend-design` (anthropics/skills, triển khai qua skills-manager) | Dựng token, component và từng màn: kiểu chữ, khoảng cách, tránh giao diện "mẫu có sẵn" |
| `dataviz` | Biểu đồ ngày, biểu đồ context, meter: màu, trục, chú thích, nhất quán sáng/tối |
| `web-design-guidelines` (vercel-labs/agent-skills, cài qua thongke) | Rà lại sau khi xong mỗi màn: khả năng truy cập, focus, trạng thái rỗng/lỗi |

## 3. Nền tảng chung

### 3.1 Token (`app/ui/src/styles/tokens.css`)

- Nền app `--bg-app` (sáng `#f5f6f8`, tối `#141518`), nền thẻ `--bg-card` (sáng `#fff`, tối `#1d1f23`), viền `--line` (sáng `#e7e9ee`, tối `#2a2d33`).
- Chữ `--t1/--t2/--t3` (chính / phụ / mờ); màu trạng thái giữ `--red/--orange/--green`.
- Bo góc `--r-sm: 3px`, `--r: 4px`, `--r-lg: 6px`. Khoảng cách theo bậc 4px (`--s1`…`--s6`).
- **Không gradient, không bóng đổ cho nút/chip/thẻ.** Bóng chỉ dùng cho lớp nổi (ngăn kéo, hộp thoại, menu thả xuống) để tách khỏi nền.
- Tag công cụ vẫn tô bằng `--tc` (từ `SourceInfo.color`).
- Giữ chế độ sáng/tối theo hệ thống; thêm lựa chọn **Sáng / Tối / Theo hệ thống** trong Cài đặt (lưu `Config.theme`, UI đặt `data-theme`).

### 3.2 Component dùng chung (`app/ui/src/components/`)

| Component | Việc | Dùng ở |
|---|---|---|
| `card` (CSS `.card`) | Thẻ trắng, viền mảnh, bo 4px; trạng thái chọn = viền xanh + vòng mờ | mọi màn |
| `chip` + `chipMenu` | Chip bật/tắt; chip có menu thả xuống (thay `<select>` thô cho Project, thời gian) | Hội thoại, Skill, Tổng quan |
| `countPill`, `warnBadge` | Số đếm dạng viên, nhãn cảnh báo bấm được | tiêu đề các màn |
| `drawer` | Ngăn kéo phải: mở/đóng, Esc, bấm nền mờ để đóng, giữ focus bên trong | Hội thoại (cỡ vừa) |
| `rail` | Sidebar dạng cột icon (tooltip tên mục, % quota ở đáy) | khung app (cỡ vừa) |
| `kpi` | Ô số chính (nhãn, số, dòng phụ/so sánh) | Tổng quan, chi tiết hội thoại |
| `insightCard` | Thẻ gợi ý có viền trái theo mức độ (đỏ/cam/xanh) + nút hành động | Tổng quan, Phân tích context |

### 3.3 Khung app và bố cục theo độ rộng

- `main.ts` theo dõi độ rộng cửa sổ (`ResizeObserver`/`matchMedia`) và đặt `data-layout="wide|medium|narrow"` trên `#app`; CSS đổi grid theo đó. Không dùng hai bộ view riêng.
- **Rộng:** sidebar 200px | danh sách ~400px | chi tiết còn lại.
- **Vừa:** cột icon 52px | danh sách; chọn hội thoại thì mở `drawer` chứa chi tiết.
- **Hẹp:** cột icon ẩn thành thanh tab trên cùng; chi tiết thay cho danh sách, có "← Hội thoại".

## 4. Từng màn

### 4.1 Hội thoại (`design-1-conversations.html`)

- Tiêu đề: "Hội thoại" + số đếm + ô tìm kiếm cùng hàng.
- Bộ lọc **một hàng**: chip menu **Project**, chip menu **thời gian**, chip menu **Model** (chọn nhiều, mỗi mục có chấm màu công cụ, chip hiện số model đang chọn), chip lọc nhanh (🔥, Context lớn, Ngắt ngang, ⭐).
- Mỗi hội thoại là một thẻ: tiêu đề trọn dòng; **tag công cụ ở dòng meta** (chỉ khi phạm vi Tất cả) + project · số lượt · giờ; bên phải token (Kiro: `~` + credit) và cờ ⚠🔥⭐.
- Chi tiết: tag công cụ, tiêu đề, meta (project · model · giờ · số lần compact — không lặp token/lượt); **3 ô KPI** (token, lượt, context lớn nhất); nút "Phân tích context", "☆ Xem lại"; các lượt là thẻ xám nhạt, lượt đang mở là thẻ trắng có viền, "Vì sao tốn?" nằm trong lượt.

### 4.2 Tổng quan (`design-2-overview.html`)

- Chọn khoảng: **Hôm nay / 7 ngày / 30 ngày / 90 ngày**.
- Hàng 4 KPI: Token (**so với kỳ trước ▲/▼**), Lượt (TB/ngày dùng), **Hội thoại** (số theo từng công cụ khi phạm vi Tất cả), **Quota 5 giờ** (+ giờ reset, 7 ngày) khi phạm vi có quota.
- 3 thẻ gợi ý theo mức độ: tốn bất thường (đỏ), skill tốn nền mà không dùng (cam), project chiếm nhiều token (xanh).
- Thẻ biểu đồ ngày; thẻ Project và thẻ Model cạnh nhau khi rộng, xếp chồng khi hẹp.
- Backend: `day_stats` đã đủ để tính kỳ trước ở UI (lấy thêm cùng độ dài trước đó); số skill tốn nền lấy từ `skills:list` đã có.

### 4.3 Skill (`design-3-others.html`)

- Giữ tính năng vừa làm (thẻ/bảng, công tắc, chip, nhãn cảnh báo, tìm trên skills.sh); đổi sang token/component mới.
- Thêm chip **"Tất cả"**; thẻ tốn nền mà không dùng có viền cam; **thẻ bỏ chấm trạng thái** (công tắc đã thể hiện bật/tắt; skill không có công tắc thì ghi "đã tắt" bằng chữ).

### 4.4 Cài đặt

- Nhóm thành thẻ kiểu System Settings: **Chung** (ngôn ngữ, giao diện), **Công cụ** (mỗi công cụ: logo, thư mục log, số lượt, trạng thái — lấy từ RPC `sources`), **Giới hạn & thông báo**, **Chẩn đoán**.
- **Lưu ngay** khi đổi (bỏ nút Lưu); giữ thông báo "có hiệu lực sau khi mở lại app" cho các ngưỡng còn cần khởi động lại.

### 4.5 Phân tích context

- Biểu đồ, thành phần của lượt gọi, gợi ý là thẻ riêng; gợi ý dùng `insightCard`.

## 5. Ngoài phạm vi

- Tính năng mới về dữ liệu (tag hội thoại, preset skill, đo tác động khi sửa skill…) — xem `docs/research/2026-09-28-skills-manager-comparison.md` mục 6.
- Đổi màu nhận diện công cụ, thêm logo agent khác.
- Hiệu ứng chuyển động ngoài ngăn kéo (ngăn kéo tôn trọng `prefers-reduced-motion`).

## 6. Kiểm thử

- Vitest: component mới (`drawer` mở/đóng/Esc/focus, `chipMenu` chọn giá trị, `rail`), chuyển `data-layout` theo độ rộng, lưu ngay ở Cài đặt, so sánh kỳ trước ở Tổng quan. Cập nhật selector trong test hiện có; giữ các kiểm tra XSS (mọi chữ qua `textContent`).
- Chụp ảnh Chrome headless (dữ liệu giả) ở 3 độ rộng (1360 / 1000 / 640px), vi/en, sáng/tối; so với mockup.
- Chạy app thật: IPC, ngăn kéo khi thu nhỏ cửa sổ, đổi giao diện sáng/tối.
