# So sánh với skills-manager và những phần có thể áp dụng (2026-09-28)

- Dự án tham khảo: `~/outsource/skills-manager` (github.com/xingkongliang/skills-manager), bản v1.40.0.
- Giấy phép **MIT** (Tianliang Zhang): được dùng lại code/dữ liệu, chỉ cần ghi nguồn.
- Stack: Tauri 2, React 19, Tailwind, Rust, SQLite (rusqlite), react-i18next. Khoảng 42.600 dòng Rust, 526 test.

## 1. Khác nhau ở mục đích

| | skills-manager | thongke |
|---|---|---|
| Câu hỏi trả lời | Skill nào đang được cài cho agent nào? | Dùng AI tốn bao nhiêu, tốn vào đâu, có hiệu quả không? |
| Dữ liệu | Thư mục skill của 54 agent | Log hội thoại (Claude Code, Kiro) và quota |
| Skill | Thư viện trung tâm (`~/.skills-manager`); triển khai sang từng agent bằng symlink/copy; preset, cập nhật từ git, backup lên GitHub, marketplace skills.sh | Liệt kê skill, **chi phí nền (tok/phiên), số lần dùng, dùng ở đâu**; tắt/bật/gỡ có hoàn tác |
| Phân tích token, quota | Không có | Là phần chính |

Hai app **bổ sung cho nhau**: thongke biết skill nào *đáng giữ*, skills-manager biết skill *nằm ở đâu* và phân phối nó ra sao.

## 2. Cảnh báo: có thể xung đột trên máy này

- Máy đang có `~/.skills-manager` (thư viện hiện chỉ có `frontend-design`).
- Skill trong `~/.claude/skills` hiện **không** phải symlink từ đó, nên chưa xung đột.
- Rủi ro: thongke tắt skill bằng cách chuyển thư mục sang `~/.claude/skills-disabled` (`crates/tk-skills/src/ops.rs`). Nếu skill do skills-manager triển khai, nó sẽ thấy trạng thái lệch và có thể triển khai lại.

## 3. Nên áp dụng (theo mức lợi ích)

| # | Việc | Tham khảo bên skills-manager | Áp dụng vào thongke |
|---|---|---|---|
| 1 | **Nhường thao tác cho skills-manager** khi skill là symlink vào `~/.skills-manager` | CLI `skills-manager-cli skills undeploy/deploy/status … --json`; bản CLI được publish ở `~/.skills-manager/bin/skills-manager-cli` (kèm file `.version`) | `tk-skills`: nhận diện nguồn "Skills Manager", gọi CLI của nó thay vì tự chuyển thư mục; UI ghi "quản lý bởi Skills Manager". Không có CLI hợp lệ (`.version` thiếu) thì chỉ đọc |
| 2 | **CI build bộ cài** | `.github/workflows/release.yml`: matrix macOS arm64/x64, **ubuntu-22.04** x64, ubuntu-24.04-arm, Windows x64; `test.yml` chạy `cargo test` | Build Linux trên ubuntu-22.04 để gỡ giới hạn glibc 2.39 (bộ cài hiện chỉ chạy trên Ubuntu 24.04+). Cùng matrix đó thêm Windows/macOS (phần M6 còn thiếu) |
| 3 | **Bảng đường dẫn skill của 54 agent** | `src-tauri/src/core/tool_adapters.rs` (`default_tool_adapters`: key, `relative_skills_dir`, `relative_detect_dir`, `additional_scan_dirs`, project path riêng; ưu tiên `~/.config/...` hoặc `dirs::config_dir()`) | Chỉ lấy **dữ liệu đường dẫn** để màn Skill liệt kê skill theo từng công cụ. Code đọc log vẫn phải viết riêng cho từng công cụ (adapter trong `tk-adapters`). Ghi nguồn MIT |
| 4 | **Chống lỗi khi thao tác file** | `core/path_guard.rs` (`sanitize_name`, `is_path_safe`: canonicalize, kiểm tra đích nằm trong base); `core/sync_engine.rs` chặn nguồn/đích lồng nhau (issue #61 lồng vô hạn, #199 xoá mất nguồn) | Bổ sung vào `tk-skills/src/ops.rs` cho `install_dir`, `uninstall`, `undo`, kèm test |
| 5 | **Chẩn đoán cho app chạy nền** | `core/panic_log.rs` (hook ghi lần crash), `core/timing.rs` (chỉ log lần gọi đầu và các lần chậm), Settings → Export Logs, `core/log_sanitize.rs` (lọc thông tin nhạy cảm) | Hook ghi crash cho app tray; đo thời gian RPC theo kiểu first-or-slow; nút "Xuất log" trong Cài đặt (zip, đã lọc) |
| 6 | **CLI cho agent + skill kèm theo** | `core/cli_bridge.rs`, `skills/manage-skills/SKILL.md`; lỗi dạng `{"ok": false, "code": …, "message": …}` | Hợp với M7: `tk … --json` có mã lỗi, cùng một skill để Claude tự hỏi "hôm nay tốn bao nhiêu, hội thoại nào đắt nhất" |
| 7 | **Tiện ích UI nhỏ** | `components/CommandPalette.tsx` (Ctrl+K), `SkillMarkdown.tsx` (xem SKILL.md), cài đặt `show_tray_icon` / `close_action`, `tauri-plugin-updater`, cỡ chữ | Bảng lệnh Ctrl+K; xem trước SKILL.md trong màn Skill; tuỳ chọn tray (ẩn icon, đóng cửa sổ thì thu vào tray hay thoát); tự cập nhật app |

## 4. Không nên lấy

- **Backup/đồng bộ qua Git, marketplace, preset**: ngoài mục đích của thongke. Nếu cần thì dùng thẳng skills-manager.
- **Bộ logo agent** (`public/agent-icons/`): logo thương hiệu, không nằm trong giấy phép MIT. thongke giữ ô chữ cái màu (`SourceInfo.letter`/`color`).
- **Code UI React**: thongke dùng TypeScript thuần + `h()`; chỉ lấy ý tưởng.

## 5. Đề xuất thứ tự làm

1. Mục 1 (tránh xung đột thật trên máy này) và mục 2 (giải quyết phần lớn M6).
2. Mục 4 (an toàn file) đi cùng mục 1, vì cùng sửa `tk-skills/ops.rs`.
3. Mục 5 và 7 khi làm M6.
4. Mục 3 khi mở rộng sang công cụ mới (OpenCode…); mục 6 khi làm M7.

## 6. Ý tưởng tính năng có thể mang sang (kết hợp với dữ liệu thongke)

Phần 3 ở trên là kỹ thuật. Phần này là **tính năng người dùng thấy được**. Điểm mạnh riêng của thongke là số liệu sử dụng, nên mỗi ý tưởng đều được gắn thêm số liệu đó.

| # | Bên skills-manager | Mang sang thongke | Giá trị |
|---|---|---|---|
| A | **Preset**: nhóm skill, bấm một lần để bật/tắt cả nhóm cho một agent | **Bộ skill theo ngữ cảnh kèm chi phí**: mỗi bộ hiện tổng chi phí nền (tok/phiên). Tự **đề xuất bộ theo project** từ log: "project X 30 ngày chỉ dùng pj-review, pj-estimate → bộ này tiết kiệm ~2K tok/phiên" | Cao: giảm chi phí nền, đúng mục tiêu tối ưu |
| B | **Theo dõi cập nhật + so sánh với bản upstream** | **Đo tác động khi skill thay đổi**: ghi lại lúc SKILL.md đổi (hash nội dung), so sánh TB token/lần dùng trước và sau. Ví dụ: "sau khi sửa pj-review ngày 20/09, TB giảm từ 5.7M xuống 3.1M" | Cao: công cụ khác không có |
| C | **Nhật ký hoạt động** | **Đánh dấu sự kiện trên biểu đồ ngày**: tắt/bật skill, đổi model, bật statusline. Nhìn được ngay thay đổi đó có làm giảm token không | Cao, làm nhanh (đã có `skill_ops`) |
| D | **Project Workspace** (skill riêng trong repo `.claude/skills`) | Tự quét skill cấp project từ `cwd` đã có trong log, không cần người dùng tự thêm project. Hiện chi phí nền **thật** của từng project (skill toàn cục + skill project) | Trung bình–cao |
| E | **Huy hiệu agent trên thẻ skill** (skill đang có ở agent nào) | Mỗi skill hiện huy hiệu công cụ **kèm số lần dùng theo từng công cụ** | Trung bình |
| F | **Tag + lọc "chưa gắn tag"** | Gắn tag cho **hội thoại** (bug, tài liệu, review…), thống kê token theo tag trong Tổng quan: "việc review tốn 40% token" | Trung bình–cao |
| G | **Bảng lệnh ⌘K** | Nhảy nhanh tới hội thoại, project, skill, model, cài đặt; lệnh "lọc tốn bất thường", "đổi phạm vi Kiro" | Trung bình, làm nhanh |
| H | **Cho agent tự quản lý** (skill `manage-skills` + CLI) | Skill **`thongke`** cho Claude: "hôm nay tôi tốn bao nhiêu", "quota còn bao nhiêu trước khi chạy việc dài", "hội thoại này đang ở context bao nhiêu, có nên /compact" | Cao, gắn với M7 |
| I | **Lần mở đầu** (bắt đầu mới hay khôi phục; tự nhận ngôn ngữ từ OS) | Onboarding: nhận diện công cụ có trên máy, chọn ngôn ngữ (mặc định theo OS), bật statusline quota trong một bước | Trung bình (đang nằm trong việc dang dở) |
| J | **Hỏi khi đóng cửa sổ**: thu vào tray hay thoát, nhớ lựa chọn | Làm giống vậy; hiện tại đóng cửa sổ là luôn chạy nền, người mới dễ tưởng app đã tắt | Thấp, làm rất nhanh |
| K | **Chọn nhiều + thao tác hàng loạt** | Chọn nhiều hội thoại để gắn tag hoặc ⭐; chọn nhiều skill để tắt/bật | Thấp–trung bình |
| L | **Hướng dẫn nhanh** | Hộp "cách đọc số liệu": token thật và ước lượng, chi phí nền, vì sao đọc lại context tốn | Thấp |

**Nên làm trước:** C (nhanh, thấy hiệu quả ngay), A (đúng mục tiêu giảm chi phí), B (điểm khác biệt), H (khi làm M7).

## 7. Cách quản lý skill (cài, tắt, gỡ): có nên làm theo không?

| | thongke (`tk-skills`) | skills-manager |
|---|---|---|
| Nơi chứa skill | Ngay trong thư mục của từng công cụ (`~/.claude/skills`, …) | **Thư viện trung tâm** `~/.skills-manager/skills`; *triển khai* sang từng agent bằng symlink hoặc copy |
| Cài | Từ thư mục (symlink) hoặc từ git | Git (có URL thư mục con `…/tree/main/skills/x`), thư mục, `.zip`/`.skill`, marketplace, `owner/repo@skill` |
| Tắt | Chuyển sang `skills-disabled`, có hoàn tác | Không có. Chỉ "undeploy" (gỡ khỏi agent), thư viện vẫn giữ |
| Gỡ | Thùng rác 30 ngày, có hoàn tác | Xoá khỏi thư viện và mọi agent (hỏi xác nhận) |
| Cập nhật | Không có | Hash nội dung so với upstream, xem diff, báo file sẽ mất (`removals.rs`) |
| An toàn | Kế hoạch → xác nhận → thực hiện → nhật ký → hoàn tác; symlink chỉ đụng link | `TargetState` (Absent / LinkToSource / ForeignLink / RealDir / RealFile, đọc bằng `symlink_metadata`) + `ReplacePolicy` (NoClobber / Recorded / UserConfirmed); `preflight_replace` kiểm tra cả loạt trước (issue #363) |

**Kết luận:** **không** chép mô hình thư viện trung tâm. Đó là lõi của skills-manager và rất nặng (`commands/skills.rs` ~4.200 dòng, `git_fetcher.rs` ~2.700 dòng). Làm lại chẳng khác gì viết lại app của họ, và dễ xung đột trên máy dùng cả hai app. Giữ cách tắt/gỡ có hoàn tác hiện tại của thongke.

**Lấy từng phần:**

1. Skill do skills-manager triển khai → nhường thao tác cho CLI của nó (xem mục 3.1). Cài cho nhiều agent thì dùng skills-manager.
2. Phân loại đích + mặc định không ghi đè (`TargetState`/`ReplacePolicy`) cho `tk-skills/ops.rs`: nhận ra link lạ, link hỏng; từ chối khi không chắc thứ đó là của mình.
3. Kiểm tra trước cả loạt cho "tắt hàng loạt": có một skill không làm được thì dừng hết trước khi đụng vào bất kỳ skill nào.
4. Thêm nguồn cài: URL GitHub trỏ tới thư mục con, `owner/repo@skill`, `.zip`.
5. Kiểm tra cập nhật cho skill cài từ git bằng hash nội dung (đi cùng ý tưởng 6B: token trước và sau khi skill đổi).
6. Báo trước những file sẽ mất khi cập nhật skill.

**Không lấy:** thư viện trung tâm, preset đồng bộ, backup Git, marketplace.

## 8. Giao diện: đã mang sang (2026-09-28)

Plan: `docs/superpowers/plans/2026-09-28-ui-from-skills-manager.md`.

- **Logo công cụ** Claude Code, Kiro (`app/ui/public/tool-icons/`, ghi chú nhãn hiệu trong `NOTICE.md`); thiếu hoặc lỗi thì quay về ô chữ cái. Màu nhận diện theo thương hiệu: Claude `#d77655`, Kiro `#9046ff`.
- **Màn Skill**: số đếm dạng viên, nhãn cảnh báo "tốn nền mà không dùng" (bấm để lọc), chip lọc trạng thái/nguồn, chip viền đứt; chuyển **Bảng / Thẻ**; thẻ có công tắc bật/tắt (vẫn qua xác nhận + hoàn tác).
- **Tìm trên skills.sh**: kết quả dạng thẻ, "✓ Đã cài", chip lọc theo repo nguồn.
- **Hội thoại**: số đếm dạng viên, số hội thoại theo từng công cụ. **Tổng quan**: 3 gợi ý thành thẻ có nút hành động.
- Không làm: hàng logo mọi agent trên thẻ, lưới thẻ cho Hội thoại, đổi màu nhấn chính, sidebar thu gọn (sidebar hiện chỉ có một nhóm).
