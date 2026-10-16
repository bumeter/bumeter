# PROGRESS — Bumeter (trước là thongke) (cập nhật 2026-09-28, chiều)

> File này để người/AI **tiếp tục công việc đúng ngay** khi mở repo. Đọc hết file là hiểu dự án đang ở đâu, việc gì xong, việc gì dang dở. Cập nhật mỗi khi xong một mảng lớn.

---

## 1. Dự án là gì

**Bumeter** (tên cũ: thongke; CLI `bum`, thư mục dữ liệu `~/.local/share/bumeter`, tự chuyển từ `thongke` khi khởi động; biến `BUM_*` thắng `TK_*`) — app desktop (Tauri 2) chạy nền, đọc log của các công cụ AI lập trình và trả lời: hội thoại/lượt nào tốn token, dùng skill nào, vì sao đắt, quota 5 giờ/7 ngày còn bao nhiêu.

**Thuật ngữ:** Hội thoại (một session) → Lượt (một câu hỏi + trả lời) → Lượt gọi (một request tới model). UI song ngữ **vi** (mặc định) / **en**.

- Người dùng: cá nhân (developer), dùng **gói subscription** — UI **không hiện $** (chỉ token, và credit của Kiro).
- Hệ điều hành chính: Ubuntu Linux (dev trên máy này). Windows/macOS theo spec nhưng chưa build.
- Spec gốc: `docs/superpowers/specs/2026-09-24-thongke-design.md` (458 dòng, mục 13 = bảng cột mốc M1–M8).
- Wireframe/mockup: `docs/superpowers/specs/2026-09-24-thongke-wireframes.md`, `...-mockup-macos.html`.
- Kế hoạch theo milestone: `docs/superpowers/plans/2026-09-24-m1…m5*.md`, `...-kiro.md`.

**Stack:** Rust workspace (edition 2021, rustc 1.94) + Tauri 2 + UI TypeScript thuần (Vite 8, Vitest 5, **không framework**, render bằng helper `h()`).

## 2. Cấu trúc repo (đường dẫn quan trọng)

```
Cargo.toml                  # workspace: bum-core, bum-adapters, bum-service, bum-cli, bum-skills, app/src-tauri
crates/bum-core/             # lõi: engine đọc log, SQLite, handlers (toàn bộ query RPC), models, limits
crates/bum-adapters/         # nguồn log: claude_code/ (JSONL ~/.claude/projects), kiro/ (whole-file ~/.kiro/sessions/cli)
crates/bum-service/          # bum-service JSON-RPC qua socket (client TUI/VS Code dùng sau này)
crates/bum-cli/              # lệnh bum: scan, audit, session, skills, statusline, prompts…
crates/bum-skills/           # liệt kê/tắt bật gỡ skill có hoàn tác (Claude + Kiro); market.rs = tìm trên skills.sh (qua `curl`)
app/src-tauri/              # app Tauri: commands (gọi `call {method, params}` → bum-core handlers), tray, window
app/ui/src/                 # toàn bộ UI
  api.ts                    # LỚP GỌI DUY NHẤT → Tauri IPC; ngoài Tauri tự rơi về mock.ts
  mock.ts                   # dữ liệu giả (browser dev) — THÊM METHOD MỚI PHẢI CẬP NHẬT Ở ĐÂY
  tools.ts                  # ⭐ nhận diện tool — nạp từ RPC `sources` (backend registry), toolDef/toolTag/glyph/modelTag
  i18n.ts, locales/{vi,en}.ts  # t('khoá', {tham số}); en.ts có kiểu Record<khoá vi> nên thiếu khoá = lỗi typecheck
  views/{sidebar,conversations,conversation,overview,skills,session,settings}.ts
  tests/*.test.ts           # Vitest (fake.ts = dữ liệu test), 64 test
docs/build-ubuntu.md        # quy trình build bundle Linux
docs/m{1,2,3,5}-verification.md, docs/kiro-verification.md, docs/conversations-verification.md   # biên bản kiểm chứng
docs/research/              # nghiên cứu, so sánh với dự án khác
docs/screens/*.png          # ảnh chụp bằng dữ liệu giả
```

**Luồng dữ liệu:** watcher đọc log → engine (`bum-core/src/engine.rs`) ghi SQLite → UI gọi RPC `list_conversations`, `conversation_turns`, `prompt_detail`, `sources`… → `onDataChanged` push khi có log mới. Nội dung hỏi/đáp (`conversation_transcript`) đọc thẳng từ file log lúc cần, không lưu DB.

**Nguồn dữ liệu thật trên máy này:**

| Nguồn | Đường dẫn | Quy mô |
|---|---|---|
| Claude Code | `~/.claude/projects` (JSONL) | 38MB, 16.482 dòng |
| Kiro | `~/.kiro/sessions/cli` (whole-file) | 113MB |
| Quota | `~/.claude/settings.json` statusline → `rate_limits` 5h/7d | chỉ Pro/Max |
| OpenCode (chưa tích hợp) | `~/.local/share/opencode/opencode.db` (SQLite WAL, có `cost`) | 522MB |

## 3. Trạng thái Git

- Nhánh hiện tại: **`feat/skill-search`** (tách từ `feat/conversations`, chứa toàn bộ M1–M5 + Kiro + Hội thoại). Working tree sạch, chưa merge vào `main`.
- Đợt **Tìm skill trên skills.sh** (plan `docs/superpowers/plans/2026-09-28-skill-search.md`): `8e0d629` market.rs (parse + tìm thư mục skill) · `d4d5b3d` op `install_market` · UI tab skills.sh.
- 7 commit của đợt Hội thoại (cũ → mới): `aa82a72` tiêu đề · `6e93cd3` registry + tool_stats · `a06f3ce` list_conversations/turns/sources · `8567e8a` transcript · `b9054e2` i18n backend + stats theo tool · `177be5f` UI Hội thoại/Tổng quan/vi-en/bỏ $ · `94aac58` docs.
- Không có TODO/FIXME trong code.

## 4. Tiến độ theo cột mốc (spec mục 13)

| Cột mốc | Nội dung | Trạng thái |
|---|---|---|
| **M1** Lõi + Tray | đọc log, bỏ trùng, gán skill, SQLite, `bum audit`, tray, quota, thông báo | ✅ `docs/m1-verification.md` (token khớp 100%) |
| **M2** Lần hỏi | khung 3 cột, danh sách, tìm kiếm/lọc, chi tiết, ⭐/📝, cài đặt | ✅ `docs/m2-verification.md` — màn này đã được thay bằng **Hội thoại** |
| **M3** Skill | liệt kê, chi phí nền, dùng ở đâu, tắt/bật/cỡ/gỡ có hoàn tác | ✅ `docs/m3-verification.md` |
| **M4** Project + Ngày | 2 màn tổng hợp, bấm để lọc | ✅ — đã gộp vào **Tổng quan** (cùng màn Model) |
| **M5** Phân tích phiên | thành phần context, biểu đồ compact, tool call | ✅ `docs/m5-verification.md` (phân tích 224 lần hỏi = 48ms) |
| **Kiro** | adapter whole-file, ước lượng context từ %, credit, skill riêng, màn Model | ✅ `docs/kiro-verification.md` |
| **Hội thoại + i18n** | tiêu đề hội thoại, registry công cụ ở backend (`sources`), màn Hội thoại nhóm theo ngày + transcript, phạm vi công cụ, Tổng quan (gộp Project/Ngày/Model), vi/en, bỏ $ | ✅ `docs/conversations-verification.md` |
| **Tìm skill trên skills.sh** | tab "Tìm trên skills.sh" trong hộp Cài skill; cài = clone GitHub, chỉ lấy thư mục skill, có hoàn tác; không telemetry | ✅ kiểm chứng thật: tìm 0,8 giây, cài `mattpocock/skills/code-review` 1,4 giây, hoàn tác sạch (thư mục tạm) |
| **Giao diện học từ skills-manager** | logo công cụ + màu thương hiệu, màn Skill dạng thẻ/bảng + chip lọc + công tắc, skills.sh dạng thẻ, thẻ gợi ý ở Tổng quan | ✅ ảnh `docs/screens/skills-cards-*.png`, `overview-insights-*.png` |
| **M6** Đóng gói | file cài, autostart, single-instance, CI | 🟡 Ubuntu `.deb` (9.6M) + `.AppImage` (86M) build lại 2026-09-25 với bản Hội thoại (`docs/build-ubuntu.md`, cần glibc 2.39 = Ubuntu 24.04+); **chưa** Windows/macOS, autostart, CI |
| **M7** Terminal | `bum tui`, `bum daemon`, `bum prompts --json` | ❌ chưa bắt đầu |
| **M8** VS Code | extension, panel webview dùng lại `app/ui` | ❌ chưa bắt đầu |

**Kiểm thử hiện trạng (2026-09-28):** `cargo test --workspace` = **180 test pass** · Vitest = **64/64** · `tsc --noEmit` sạch.

## 5. Việc vừa xong (nhánh feat/conversations)

- Tiêu đề hội thoại (Claude `ai-title`, Kiro `title`, schema v4); không có thì lấy câu hỏi đầu.
- Backend registry: `LogSource::info()` → `SourceInfo` (nhãn, chữ cái, màu, khả năng); `bum_adapters::registry()`; RPC app `sources` = registry + `tool_stats`.
- RPC `list_conversations` (nhóm theo ngày giờ máy; lọc q/project/models[]/tool/from/to/quick), `conversation_turns` (trả `{title, turns}`), `conversation_transcript` (app command, đọc log).
- `project_stats`/`day_stats` nhận `tool` (phạm vi công cụ).
- i18n: `Config.language` (vi/en); thông báo, tray, statusline, `bum status` dịch theo đó; backend trả **mã** cho "vì sao tốn" (`Explain`) và gợi ý phân tích phiên (`Suggestion`), UI tự dịch.
- UI: phạm vi công cụ ở đầu sidebar (nhớ bằng localStorage); sidebar = Tổng quan / Hội thoại / Skill / Cài đặt; màn Hội thoại + chi tiết lượt (hỏi/đáp, tool call, vì sao tốn, ⭐/📝); Tổng quan có 3 gợi ý; chọn ngôn ngữ trong Cài đặt.

## 5b. Thiết kế lại giao diện + đổi tên Bumeter (nhánh feat/bumeter)

- Xong: token/component dùng chung (cam, nút phẳng, không gradient, chữ Be Vietnam Pro nhúng offline), linh vật Bum 5 biểu cảm, bố cục theo độ rộng (wide ≥1200 / medium 700–1199 có ngăn kéo / narrow <700), làm lại Tổng quan, Hội thoại, Skill, Cài đặt; theme sáng/tối/hệ thống trong config; tray đổi biểu cảm Bum; CLI `bum`, thư mục dữ liệu `bumeter` tự chuyển (có rollback, giữ thư mục cũ nếu không chuyển được), hook statusline trỏ `bum`.
- Rà giao diện (Task 12): hộp thoại có `aria-labelledby`, Esc đóng, trả focus; tắt autocomplete ô nguồn/tìm; sửa quy tắc `.bar` chung đè cột biểu đồ ngày; ô chọn ngôn ngữ bị cắt chữ.
- Ảnh: `docs/screens/bumeter-<view>-<w>-<lang>-<theme>.png` (Tổng quan + Hội thoại ở 1360/1000/640 × vi/en × sáng/tối; Skill + Cài đặt ở 1360 vi sáng/tối). Chụp bằng `vite preview` + Chrome headless với dữ liệu mock; URL hỗ trợ `?theme=&lang=&view=&scope=&conversation=&turn=&skillView=&open=1`. Ảnh cũ (m2–m5, conversations-*, overview-*, skills-*) giữ lại vì các tài liệu kiểm chứng cũ còn trỏ tới.
- Bộ cài: `target/release/bundle/deb/Bumeter_1.0.0_amd64.deb`, `target/release/bundle/appimage/Bumeter_1.0.0_amd64.AppImage` (sidecar `binaries/bum-x86_64-unknown-linux-gnu`).
- Nâng cấp từ bản cũ: gói cũ tên `thongke` (identifier `dev.thongke.app`) khác gói mới `bumeter` nên cài song song — cần `sudo apt remove thongke`. localStorage cũ (phạm vi công cụ, kiểu xem Skill) bị đặt lại vì khoá đổi `thongke.*` → `bumeter.*`. Chi tiết ở `docs/build-ubuntu.md`.
- Kiểm tra máy thật (2026-10-15, có sao lưu `~/.local/share/thongke-backup-20261015`): `bum status` lần đầu chuyển `thongke/` → `bumeter/` (`thongke.db` → `bumeter.db`, 11 MB, dữ liệu còn đủ); app `bumeter` chạy, IPC `bum status` trả số thật; tray hiện Bum `worry` khi quota 5 giờ 85%; hook statusline viết lại `tk` → `bum` (lỗi tìm thấy khi chạy thật: bản sao lưu đời đầu thiếu `previous_status_line` làm hỏng chuyển hook — đã sửa, có test). Chưa kiểm tra bằng tay: ngăn kéo khi kéo nhỏ cửa sổ thật (đã kiểm qua ảnh 1000px).

## 6. Việc dang dở (ưu tiên trên xuống dưới)

0. **Đối chiếu skills-manager** (`docs/research/2026-09-28-skills-manager-comparison.md`): nên làm trước (1) nhường thao tác skill cho skills-manager khi skill do nó quản lý — máy này có `~/.skills-manager`; (2) CI build bộ cài, Linux trên ubuntu-22.04 để gỡ giới hạn glibc 2.39, thêm Windows/macOS.
1. Chuỗi backend của `bum-skills` (mô tả plan thao tác, cảnh báo) vẫn chỉ có tiếng Việt.
2. Tìm kiếm trong câu trả lời (cần lưu/đánh chỉ mục transcript).
3. Onboarding + màn "Công cụ" trong Cài đặt + bật/tắt theo dõi từng tool (spec mục 6).
4. Engine incremental cho nguồn dạng SQLite (cần cho OpenCode), rồi tích hợp OpenCode — đã research (mục 2), **hoãn**.
5. M6 còn lại: autostart, bản Windows/macOS, CI; M7 (TUI), M8 (VS Code).

## 7. Lệnh cần nhớ

```bash
cargo test --workspace                 # 180 test Rust
cd app/ui && npm test                  # 64 test UI (vitest run)
cd app/ui && npx tsc --noEmit          # typecheck (npm run build cũng chạy sẵn)
cd app/ui && npm run build             # UI → app/ui/dist (phải chạy TRƯỚC khi tauri build)
cd app/ui && npm run dev               # xem UI trên trình duyệt bằng mock.ts (?theme=dark&view=skills)
./target/release/bumeter              # chạy app đã build
# bundle Linux: xem docs/build-ubuntu.md
#   cargo build --release -p bum-cli
#   cp target/release/bum app/src-tauri/binaries/bum-x86_64-unknown-linux-gnu
#   (cd app/ui && npm run build) && (cd app && ./ui/node_modules/.bin/tauri build --bundles deb appimage)
```

## 8. Quy ước khi làm code ở đây

- **Logo công cụ**: `SourceInfo.icon` = tên file trong `app/ui/public/tool-icons/` (chỉ nhận tên file đơn giản); ghi nguồn + chủ sở hữu nhãn hiệu vào `NOTICE.md` ở đó. Trước khi phát hành công khai nên thay bằng file từ trang brand chính thức.
- **Không hard-code tên tool ở view** — mọi nhận diện (tên, màu, khả năng) qua `app/ui/src/tools.ts`, dữ liệu từ `SourceInfo` của adapter. Thêm tool mới = adapter mới + 1 dòng trong `bum_adapters::registry()`; UI không phải sửa (màu đi qua biến CSS `--tc`).
- **Chuỗi hiển thị qua `t()`** — thêm khoá vào cả `locales/vi.ts` và `locales/en.ts`. Backend trả mã + tham số, không trả câu viết sẵn cho UI.
- Thêm RPC method mới: sửa `api.ts` (interface + method) **và** `mock.ts` (case trả giả) **và** handler `crates/bum-core/src/handlers.rs`; nếu có dữ liệu thật thêm test trong `crates/bum-core/tests/`.
- UI thuần TS + `h()`; view = `mountX(el)` + `cleanup`; test Vitest theo mẫu view hiện có (`app/ui/tests/`).
- Docs, comment: **tiếng Việt**; nội dung UI: vi + en. Tên dự án/client trong docs/mock đã bị thay bằng placeholder (commit `6bb915b`) — **không đưa tên thật trở lại**.

## 9. Bẫy đã gặp (đọc trước khi “debug” oan)

- **Không `grep` chuỗi tiếng Việt trong binary** — Tauri nén asset (brotli) vào binary nên `grep "Công cụ" bumeter` = 0 dù UI đúng. Kiểm chứng bằng cách chạy app + chụp màn (`import -window <id> /tmp/shot.png`).
- App **chạy nền tray**, cửa sổ chỉ sinh khi gọi `window::open` (`app/src-tauri/src/window.rs`); có process trong `ps` ≠ có cửa sổ. `single-instance`: launch lần 2 chỉ focus cửa sổ cũ.
- Window 10×10 `InputOnly` trong `xwininfo` = cửa sổ chưa mở, không phải app hỏng.
- Sau khi build lại, phải mở app **mới** (đóng bản cũ trước) nếu không sẽ xem UI cũ.

## 10. Cách kiểm chứng nhanh

```bash
cargo test --workspace && (cd app/ui && npm test && npx tsc --noEmit)   # xanh = an toàn commit
# xem UI bằng dữ liệu giả: cd app/ui && npm run dev → ?lang=en&theme=dark&view=overview&scope=kiro&conversation=s1&turn=a4
```
