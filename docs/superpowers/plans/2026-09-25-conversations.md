# Hội thoại + đa ngôn ngữ + registry công cụ — Kế hoạch (bản gọn)

- Mockup đã duyệt: `docs/superpowers/specs/2026-09-25-conversations-mockup.html`. Bộ lọc dùng phương án **B**: nút "Bộ lọc (n)" mở bảng chọn, các bộ lọc đang bật hiện thành chip có ✕.
- Thuật ngữ: **Hội thoại** (một session/thread) → **Lượt** (một câu hỏi và câu trả lời) → **Lượt gọi** (một request tới model). Hai ngôn ngữ: **vi** (mặc định) và **en**.
- Nhánh `feat/conversations` (tách từ `feat/kiro`).
- **Trạng thái: xong cả 8 task** — xem `docs/conversations-verification.md` (có ghi các điểm khác plan).

| # | Task | Test |
|---|---|---|
| 1 | Tiêu đề hội thoại: `RawEvent::Title` (Claude lấy từ `ai-title`, Kiro lấy từ `title`), `sessions.title` (schema v4) | Parser/store |
| 2 | Registry công cụ ở backend: `LogSource::info()` trả nhãn, ký hiệu, màu, khả năng. RPC `sources` gồm trạng thái (thư mục, số phiên, lần cập nhật cuối) | tk-core/adapters |
| 3 | RPC `list_conversations(filter)`: nhóm theo ngày giờ máy. Bộ lọc gồm q, project, `models[]`, tool, khoảng thời gian, lọc nhanh, ⭐. Mỗi hội thoại có tiêu đề, tool, project, model, số lượt, token, credit, cờ bất thường. `conversation_turns(session_id)` | tk-core |
| 4 | Transcript đọc thẳng từ log: `claude_code::transcript`, `kiro::transcript` trả các lượt gồm câu hỏi, câu trả lời (text), tool call (có cắt bớt). Lệnh app `conversation_transcript` | adapters |
| 5 | i18n cho UI: `i18n.ts` (`t()`, định dạng ngày/số theo `Intl`), `locales/vi.ts`, `locales/en.ts`, chọn ngôn ngữ trong Cài đặt (lưu vào config). Backend trả **mã + tham số** thay cho câu viết sẵn (lời giải thích, gợi ý). Thông báo và tray dịch theo `Config.language` | Vitest + Rust |
| 6 | UI: bộ chọn phạm vi công cụ ở sidebar (thay mục "Công cụ"), màn **Hội thoại** (nhóm theo ngày, tag tool ở phạm vi "Tất cả", nút Bộ lọc gồm Project/Model/lọc nhanh), panel chi tiết gồm danh sách lượt và mở rộng một lượt để xem transcript | Vitest |
| 7 | Bỏ $ trên toàn bộ UI. Gộp Project/Ngày/Model thành **Tổng quan** (biểu đồ ngày, top project, model theo tool, 3 gợi ý). Sidebar còn: Tổng quan, Hội thoại, Skill, Cài đặt | Vitest |
| 8 | Kiểm chứng trên dữ liệu thật, chụp ảnh vi/en, build bộ cài | — |
