# M2: Dashboard "Lần hỏi" — Kế hoạch triển khai (bản gọn)

- Spec: `docs/superpowers/specs/2026-09-24-thongke-design.md` (mục 4, 6, 6.0, 7.1, 14.4)
- Thiết kế tham chiếu: `2026-09-24-thongke-mockup-macos.html` và `2026-09-24-thongke-wireframes.md` (màn 1, 6, 8)
- Mục tiêu: cửa sổ dashboard dùng thật, gồm sidebar, danh sách lần hỏi gom theo phiên và panel chi tiết. Cập nhật realtime, có ⭐/📝, có màn Cài đặt cơ bản và thiết lập statusline. Phong cách macOS, chạy trên mọi hệ điều hành.
- Stack UI: Vite + TypeScript thuần (không framework), `@tauri-apps/api` 2, test bằng Vitest + happy-dom, font Inter nhúng sẵn (`@fontsource-variable/inter`).
- Làm theo TDD, mỗi task một commit. Nhánh: `feat/m2-dashboard`.

## Quy tắc chung

- Giao diện chỉ gọi lệnh qua `ui/src/api.ts`, để M8 (VS Code) có thể thay bằng socket.
- Giờ **luôn hiển thị theo giờ máy** (sửa lỗi M1 còn để lại: dashboard hiện giờ UTC).
- Mọi dữ liệu chèn vào DOM đều qua `textContent` hoặc `esc()`. Không chèn chuỗi thô vào `innerHTML`.
- Thao tác sửa file của người dùng (statusline, settings) phải sao lưu trước, có hộp xác nhận, và hoàn tác được.
- Màu và khoảng cách lấy từ token trong `tokens.css`, có đủ chế độ sáng và tối.

## Task

| # | Task | File chính | Test cần qua |
|---|---|---|---|
| 1 | **Truy vấn danh sách lần hỏi** | `tk-core/src/store/queries.rs`, `handlers.rs` | `list_prompts(filter)` hỗ trợ: tìm theo nội dung, project, skill (trực tiếp và làm tiếp), khoảng thời gian, lọc nhanh (bất thường / ctx>300K / ngắt ngang / ⭐), sắp xếp theo thời gian hoặc token, phân trang. Kết quả gom theo phiên, mỗi phiên có tổng. Có `sidebar_counts()`. Test trên fixture |
| 2 | **Chi tiết lần hỏi** | `queries.rs`, `handlers.rs` | `prompt_detail(key)` trả về từng lượt gọi (ts, model, context, output, tools), tỉ lệ đọc lại context, phần "vì sao tốn", chi phí và ghi chú. Test: p1 có 3 lượt (gồm 1 subagent) |
| 3 | **Ghi chú ⭐/📝** | `store/schema.rs` (migration v2), `store/notes.rs` | `set_note` ghi DB và sao lưu `notes.json`. DB bị tạo lại thì khôi phục từ `notes.json`. Migration từ v1 lên v2 không mất dữ liệu |
| 4 | **Đẩy realtime lên UI** | `app/src-tauri/src/runtime.rs`, `commands.rs` | Host phát sự kiện `data-changed {prompt_keys, session_ids}` (gộp tối đa 1 lần mỗi 250ms). Có các lệnh Tauri mới cho task 1–3 và 7. Test thuần cho bộ gộp sự kiện |
| 5 | **Khung UI + design token** | `app/ui/{index.html,vite.config.ts,package.json}`, `src/{main,api,state}.ts`, `src/styles/{tokens,layout}.css` | `npm run build` chạy được và Tauri nạp `dist/`. Có token sáng/tối, font Inter nhúng, bố cục 3 cột, thanh tiêu đề gốc. Vitest: `fmt.ts` (token, tiền, giờ máy, "X phút trước") |
| 6 | **Sidebar + danh sách** | `src/views/{sidebar,prompts}.ts`, `src/components/{row,chips,search}.ts` | Vitest + happy-dom: gom theo phiên, thanh màu theo mức trung bình, tag skill (đậm = trực tiếp, nét đứt = làm tiếp), phím ↑/↓, tìm kiếm có debounce, chip lọc. Dòng mới tự thêm khi nhận `data-changed` và giữ nguyên dòng đang chọn. Chuỗi có ký tự đặc biệt không phá DOM (test XSS) |
| 7 | **Panel chi tiết** | `src/views/detail.ts`, `src/charts/bars.ts` | Hiện số lớn, các thẻ số liệu, biểu đồ SVG từng lượt gọi, khung "Vì sao tốn", nút ⭐/📝 (gọi `set_note`, cập nhật ngay trên giao diện). Vitest cho `bars.ts` (tỉ lệ, 0 lượt, 1 lượt) |
| 8 | **Cài đặt + thiết lập statusline** | `tk-adapters/src/claude_code/statusline.rs`, `src/views/settings.ts` | `plan/apply/undo` của `setup_statusline`: sao lưu script gốc; chèn `echo "$input" \| tk statusline --quiet` đúng một lần. Nếu chưa có statusLine thì tạo mới và sửa `settings.json` (có sao lưu). Gỡ thì khôi phục bản sao lưu. Test trên HOME tạm. UI: các ngưỡng, bật/tắt thông báo, trạng thái statusline |
| 9 | **Tray hiện "X phút trước"** | `app/src-tauri/src/tray.rs`, `tk-core/src/limits.rs` | Sửa lỗi M1 còn để lại: dữ liệu cũ hiện "cập nhật X phút trước", icon chuyển xám thay vì đỏ hoặc vàng. Unit test cho nhãn và màu |
| 10 | **Kiểm chứng giao diện** | `docs/m2-verification.md` | Chụp ảnh sáng và tối bằng Chrome headless trên bản build UI chạy với API giả. Chạy app thật: mở cửa sổ, dữ liệu hiện ra (xác nhận IPC chạy được khi bật CSP), realtime khi đang chat, đóng cửa sổ thì RAM giảm lại. So với mockup |

## Review Focus

1. Danh sách hàng trăm lần hỏi và sự kiện realtime dồn dập khi Claude đang chạy → UI không giật, không mất dòng đang chọn (Task 4, 6).
2. Tên project, câu hỏi hoặc skill chứa `<script>` hay ký tự lạ → hiển thị đúng dạng chữ, không chạy code (Task 6, 7).
3. Người dùng đã có script statusline riêng → chèn đúng một lần, chạy lại thiết lập không chèn trùng, gỡ ra thì trả về đúng file gốc (Task 8).
4. Máy không hỗ trợ `backdrop-filter`, hoặc chưa cài font → giao diện vẫn đọc được và đúng màu (Task 5).
5. DB đang ghi khi UI truy vấn nhiều lần liên tục → không lỗi và không đứng (Task 1, 4).

## Để sau

Màn Skill + quản lý skill (M3), Project/Ngày (M4), Phân tích phiên (M5), đóng gói (M6), tối ưu RSS của app (M6).
