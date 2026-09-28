# Giao diện học từ skills-manager — Kế hoạch (bản gọn)

- Nguồn ý tưởng: ảnh `~/outsource/skills-manager/assets/demo/{library,agent-workspace,install-skills}.png`. Xem `docs/research/2026-09-28-skills-manager-comparison.md` (mục 8).
- Nhánh `feat/skill-search` (làm tiếp). TDD, mỗi task một commit.
- **Trạng thái: xong 5 task.** Ảnh: `docs/screens/skills-cards-vi-light.png`, `skills-table-en-dark.png`, `overview-insights-vi-dark.png`, `conversations-logos-vi-light.png`.

| # | Task | Test |
|---|---|---|
| 1 | **Logo công cụ**: `SourceInfo.icon` (tên file), logo Claude Code/Kiro trong `app/ui/public/tool-icons/` (lấy từ skills-manager, ghi chú nhãn hiệu trong `NOTICE.md`); `glyph()` hiện logo, lỗi tải thì quay về ô chữ cái. Màu nhận diện theo màu thương hiệu (Claude `#D77655`, Kiro `#9046FF`) | Rust registry; Vitest glyph + fallback |
| 2 | **Màn Skill**: tiêu đề có số đếm dạng viên + nhãn cảnh báo "tốn nền mà không dùng" (bấm để lọc); hàng chip lọc (trạng thái, nguồn, chip viền đứt "Tốn nền mà không dùng"); nút chuyển **Bảng / Thẻ** (nhớ bằng localStorage); thẻ có mô tả, chi phí nền, số lần dùng, **công tắc bật/tắt** (vẫn qua hộp xác nhận + hoàn tác), chân thẻ = nguồn + logo công cụ | Vitest |
| 3 | **Tìm trên skills.sh**: kết quả dạng thẻ, nhãn "✓ Đã cài", chip lọc theo repo nguồn trong kết quả | Vitest |
| 4 | **Hội thoại**: số đếm dạng viên ở tiêu đề + dòng trạng thái theo công cụ. **Tổng quan**: 3 gợi ý thành thẻ có nút hành động | Vitest |
| 5 | Kiểm chứng: ảnh vi/en sáng/tối, build app | — |

Không làm: hàng logo của mọi agent trên thẻ, lưới thẻ cho danh sách Hội thoại, đổi màu nhấn chính của app.
