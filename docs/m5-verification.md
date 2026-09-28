# M5: Kiểm chứng màn Phân tích phiên

- Ảnh chụp dùng dữ liệu giả: `screens/m5-light.png`, `screens/m5-dark.png`.
- Chạy thật: `tk session <id>` trên phiên lớn nhất (TICKET-101): 224 lần hỏi, 726 lượt gọi, 1 lần compact. **Phân tích mất 48ms.**

## Thành phần context ở lượt gọi lớn nhất (963K token, ước lượng)

| Nhóm | Token | % |
|---|---|---|
| Hệ thống & không đo được | 354K | 37% |
| Tool I/O | 247K | 26% |
| **Thinking** | **280K** | **29%** |
| Output | 63K | 7% |
| File đã đọc | 15K | 2% |

## Phát hiện khi chạy thật (đã sửa, có test)

1. Attachment chứa nội dung ở nhiều trường khác nhau (`snippet`, `text`, `content` dạng list…). Nay app đọc hết. `prompt_snapshot` là ảnh chụp system prompt lặp lại nhiều lần nên được bỏ qua.
2. **Nội dung thinking bị ẩn, nhưng chữ ký mã hoá (`signature`) vẫn được gửi lại trong context.** Riêng chữ ký đã là 979K ký tự, nhiều nhất trong log. Nay app ước lượng thinking theo độ dài chữ ký. Trước khi sửa, phần này bị dồn cả vào "Hệ thống" (66%).

## Giới hạn

- Tỉ lệ 3,5 ký tự/token chỉ là ước lượng. Code và tiếng Việt thường tốn nhiều token hơn trên mỗi ký tự, nên phần "không đo được" cũng gồm cả sai số này. System prompt, định nghĩa tool và CLAUDE.md không có trong log.
- Phiên thật chỉ được chạy qua CLI. Màn này trong app đã test bằng Vitest và chụp ảnh với dữ liệu giả.

Test: `cargo test --workspace` và Vitest 41/41.
