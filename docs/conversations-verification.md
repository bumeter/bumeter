# Kiểm chứng: Hội thoại + đa ngôn ngữ + registry công cụ (2026-09-25)

Plan: `docs/superpowers/plans/2026-09-25-conversations.md`. Mockup đã duyệt: `specs/2026-09-25-conversations-mockup.html`.

## Tự động

- `cargo test --workspace`: 171 test pass. Mới thêm: `tests/conversations.rs` (nhóm theo ngày, lọc project/model/tool/tìm kiếm, lượt theo thứ tự, thống kê theo phạm vi công cụ), `bum-adapters/tests/transcript.rs`, test ngôn ngữ trong `insight.rs`, tray vi/en, `sources_merge_registry_with_stats`.
- Vitest: 53 test pass (`conversations`, `conversation`, `overview`, `i18n`, `sidebar`, `fmt` viết mới; test cũ của màn Lần hỏi, Project, Ngày, Model đã bỏ cùng các màn đó). `tsc --noEmit` sạch.
- `i18n.test.ts` kiểm tra vi và en có cùng bộ khoá, cùng tham số, và bản tiếng Anh không sót chữ tiếng Việt.

## Trên dữ liệu thật (release, chỉ ghi số liệu)

| RPC | Kết quả | Thời gian |
|---|---|---|
| `list_conversations` (tất cả) | 63 hội thoại, 31 ngày | 71ms |
| theo tool | Claude Code 20, Kiro 43 (đều có tiêu đề) | 17ms / 11ms |
| tìm kiếm `test` | 15 hội thoại | 15ms |
| `conversation_turns` (hội thoại lớn nhất) | 225 lượt | 11ms |
| `day_stats` + `project_stats` (Kiro) | 87 ngày, 17 project | 24ms |
| transcript Claude (file 15MB) | 68 lượt, 613 mục | 70ms |
| transcript Kiro (file lớn nhất) | 196 lượt | 32ms |

- Số lượt trong transcript khớp với DB. Chênh lệch duy nhất là các lượt mới chưa kịp quét (lúc đó app đang tắt).
- `tool_stats` chỉ đếm hội thoại có lượt hỏi, nên phiên subagent của Kiro không bị tính.
- App thật (`target/release/thongke-app`) mở cửa sổ và hiện Tổng quan từ dữ liệu thật: IPC chạy được với CSP.

## Ảnh (dữ liệu giả)

`docs/screens/conversations-vi-light.png`, `conversations-en-dark.png` (đang mở rộng một lượt), `conversations-kiro-vi.png` (phạm vi Kiro, tag công cụ ẩn), `overview-vi-light.png`, `overview-en-dark.png`.

## Khác so với plan

- Bộ lọc làm theo mockup đã duyệt (select Project và select thời gian nằm ngay trên danh sách, tag Model chọn nhiều, chip lọc nhanh), không làm nút "Bộ lọc (n)" của phương án B.
- Ô tìm kiếm tìm trong **tiêu đề và câu hỏi**. Câu trả lời chưa được lưu DB nên chưa tìm được.
- Mô tả plan thao tác skill và cảnh báo của `bum-skills` (chuỗi từ backend) vẫn chỉ có tiếng Việt.
