# M2: Kiểm chứng dashboard

- Ngày: 2026-09-24. Máy: Ubuntu GNOME, bản debug.
- Ảnh chụp dùng Chrome headless với dữ liệu giả (`vite preview`, `?theme=light|dark`): `screens/m2-light.png`, `screens/m2-dark.png`.

| Kiểm tra | Kết quả |
|---|---|
| Giống mockup macOS đã duyệt (bố cục 3 cột, sidebar, hàng chọn xanh, thẻ số liệu, khung "Vì sao tốn") | ✅ Cả sáng và tối |
| Thanh tiêu đề gốc của hệ điều hành | ✅ Không tự vẽ nút cửa sổ |
| **IPC chạy được khi bật CSP** (lỗi M1 còn để lại) | ✅ App thật với log thật (17.174 dòng): UI gọi `list_prompts`, `prompt_detail`, `sidebar_counts`, `today_summary`, `limit_state` (log `tk: ui call …` ở bản debug) |
| Realtime khi đang chat | ✅ Phiên Claude Code này ghi log thì UI tự tải lại danh sách và panel chi tiết (vòng gọi thứ 2 xuất hiện sau vài giây) |
| Giờ theo giờ máy | ✅ Test `fmt` và ảnh chụp (11/09 hiện đúng Thứ Sáu) |
| Chống XSS (câu hỏi, skill, lời giải thích, mô tả thao tác statusline) | ✅ Test Vitest: nội dung `<img …>`, `<b>`, `<script>` hiện ra dạng chữ |
| Test | `cargo test --workspace` 97/97 · Vitest 24/24 |

## Chưa kiểm chứng được / ghi chú

- **RAM khi mở cửa sổ:** RSS 189MB (bản debug, WebKitGTK). Đóng cửa sổ là hủy WebView (đã có từ M1), nhưng chưa đo được RAM sau khi đóng vì không có công cụ tự đóng cửa sổ (không có `xdotool`). Cần đo tay.
- Quét lần đầu bản debug mất 2,4 giây cho 17.174 dòng. Bản release ở M1 là 0,3 giây.
- **Ngưỡng và bật/tắt thông báo chỉ có hiệu lực sau khi mở lại app.** Giao diện đã ghi rõ điều này.
- Nút "Bật theo dõi giới hạn" mới được test trên HOME tạm, chưa chạy trên `~/.claude/settings.json` thật. Việc bật cho máy thật để bạn tự bấm.
