# M3: Kiểm chứng màn Skill

- Ngày: 2026-09-24. Máy: Ubuntu GNOME.
- Ảnh chụp dùng dữ liệu giả: `screens/m3-light.png`, `screens/m3-dark.png`.

## Chạy trên dữ liệu thật (chỉ đọc: `tk skills`)

| Kiểm tra | Kết quả |
|---|---|
| Skill cá nhân khớp với `ls ~/.claude/skills` | ✅ 11 skill: pj, pj-*, estimate. `pj-clarify` là symlink |
| Skill synced từ claude.ai | ✅ 8 skill (docs, docx, pdf, pptx, xlsx, morning, import-memory, skill-creator). Chỉ đọc, không gợi ý tắt |
| Plugin qua `claude plugin list --json` / `details` | ✅ superpowers có 15 skill con, chi phí thật (ví dụ brainstorming ~70 nền / ~6,3K mỗi lần gọi) |
| Tổng chi phí nền của skill đang bật | ~3.871 token mỗi phiên. Có 12 skill chưa dùng 30 ngày, tốn ~1.198 token mỗi phiên |
| Không sửa file nào | ✅ Lệnh chỉ đọc |

## Lỗi tìm ra khi chạy thật, đã sửa (có test)

1. `/superpowers:brainstorming` (gõ lệnh) và `superpowers:brainstorming` (skill) bị tách thành 2 dòng. Nay đã gộp chung (`slash_command_of_an_installed_skill_counts_toward_that_skill`).
2. Gọi skill bằng `/X` thì log ghi cả lệnh lẫn skill với cùng `promptId`, làm số lần dùng bị **đếm đôi**. Nay chỉ đếm một lần (`a_slash_invoked_skill_is_counted_once`).

## Thao tác tắt/bật/gỡ/cài

- Đã kiểm chứng bằng 11 test trên HOME tạm (`crates/tk-skills/tests/ops.rs`): tắt, bật, gỡ vào thùng rác, hoàn tác, không hoàn tác được hai lần, symlink chỉ di chuyển link (thư mục gốc còn nguyên), cài từ thư mục hoặc git (repo không có `SKILL.md` thì bị từ chối và dọn sạch), trả lại hết khi một bước lỗi, plugin chạy qua CLI (hoàn tác thì chạy lệnh ngược), dọn thùng rác sau 30 ngày.
- **Chưa chạy trên `~/.claude` thật.** Thao tác có sửa file nên để bạn tự bấm trong app, và mọi thao tác đều có hộp xác nhận và nút hoàn tác.
- Giới hạn: nếu thùng rác nằm khác phân vùng ổ đĩa với `~/.claude` thì thao tác chuyển thư mục sẽ báo lỗi (không tự sao chép).

## Test

`cargo test --workspace` 132/132 · Vitest 34/34.
