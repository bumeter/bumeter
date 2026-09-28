# Tìm và cài skill từ skills.sh — Kế hoạch (bản gọn)

- Cách làm theo skills-manager (MIT): skills.sh chỉ là danh mục; tìm bằng `GET https://skills.sh/api/search?q=&limit=`; cài = clone `github.com/<source>` rồi chọn đúng thư mục skill theo `skillId`. Xem `docs/research/2026-09-28-skills-manager-comparison.md`.
- Không gửi telemetry, không đọc HTML bảng xếp hạng. Gọi mạng bằng `curl` có sẵn trên máy (như `git`), không thêm thư viện HTTP/TLS.
- Nhánh: `feat/skill-search` (tách từ `feat/conversations`). TDD, mỗi task một commit.
- **Trạng thái: xong 5 task.** Kiểm chứng thật trên thư mục tạm: tìm "code review" = 5 kết quả (0,8 giây); cài `mattpocock/skills/code-review` = 1,4 giây (chỉ lấy `SKILL.md` + `agents/`); hoàn tác sạch, thư mục tạm được dọn.

| # | Task | Test |
|---|---|---|
| 1 | `tk-skills/src/market.rs`: `parse_search` (nhận mảng hoặc `{skills: […]}`, bỏ mục thiếu/không hợp lệ), `valid_source` (`owner/repo`), mã hoá query, `search()` qua `curl -fsS --max-time 15` | Parse từ JSON mẫu thật; source/id lạ bị loại |
| 2 | `find_skill_dir(repo, id)`: thử `<id>`, `skills/<id>`, `.agents/skills/<id>`, rồi tìm đệ quy (sâu ≤ 6, bỏ `.git`/`node_modules`, không đi theo symlink, thứ tự cố định) theo tên thư mục, cuối cùng theo `name` trong SKILL.md. Không thấy thì báo lỗi, **không** cài cả repo | Các cấu trúc repo; symlink trỏ ra ngoài bị bỏ qua |
| 3 | Thao tác `install_market` (`source` + `skill`): plan kiểm tra tên/trùng; apply = clone `--depth 1` vào thư mục tạm → chọn thư mục skill → chuyển vào `skills/<id>` → dọn thư mục tạm; hoàn tác = đưa vào thùng rác | Repo git cục bộ (`file://`) thay cho GitHub |
| 4 | App: `skills(search)`; UI hộp "Cài skill" có tab **Tìm trên skills.sh** (debounce, số lượt cài, nút Cài → hộp xác nhận plan; skill đã có thì hiện "Đã cài"); cài vào công cụ theo phạm vi đang chọn; vi/en | Vitest |
| 5 | Kiểm chứng: tìm và cài thật một skill trên HOME tạm, rồi hoàn tác | — |
