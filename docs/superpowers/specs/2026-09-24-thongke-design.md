# thongke — Thiết kế ứng dụng thống kê sử dụng AI

- Ngày: 2026-09-24
- Trạng thái: Chờ duyệt bản viết (nội dung đã duyệt từng phần)
- Người dùng: cá nhân (developer outsource làm theo ticket), dùng Claude Code hằng ngày

## 1. Mục tiêu

### 1.1 Vấn đề

Người dùng không biết mỗi lần hỏi AI tốn bao nhiêu token, dùng skill nào, và vì sao có lần hỏi rất đắt, nên không biết cần tối ưu gì. `/context` chỉ cho biết context của phiên hiện tại tại một thời điểm, không có lịch sử và không so sánh được.

### 1.2 Kết quả mong muốn

Một app desktop nhẹ, **chạy nền liên tục**, đọc log của các công cụ AI theo thời gian thực. Mở app lên là trả lời được ngay 3 câu:

1. Lần hỏi nào tốn nhất, và vì sao?
2. Skill nào tốn nhiều hay ít trên mỗi lần dùng, được dùng ở đâu?
3. Nên thay đổi thói quen gì?

Ngoài ra app còn làm được:

- Cảnh báo khi sắp hết giới hạn sử dụng (5 giờ / 7 ngày) và khi có lần hỏi bất thường.
- Cài, tắt, bật, gỡ skill và plugin ngay trong app.
- Phân tích sâu từng phiên: thành phần context, biểu đồ compact, xem nội dung tool call.
- Kiến trúc cho phép thêm công cụ khác sau này (Codex, Kiro, Gemini…).
- Chạy được trên Windows, macOS và Linux.

### 1.3 Tiêu chí thành công

- Số token hiển thị khớp 100% với một cách tính độc lập (`tk audit`).
- Từ lúc log có dòng mới đến lúc giao diện hoặc tray cập nhật: dưới 100ms.
- RAM khi chạy nền (cửa sổ đóng): dưới 20MB. CPU gần 0% khi không có log mới.
- Quét lần đầu khoảng 34MB log (máy hiện tại): dưới 2 giây.

### 1.4 Nguyên tắc sản phẩm

**Một app duy nhất, quản lý tập trung.** Người dùng không phải ghép nhiều tool lại với nhau (ccusage, claude-devtools, TokenTracker…). Phân tích từng lần hỏi, phân tích thành phần context, quota, thông báo và quản lý skill đều nằm trong cùng một app.

### 1.5 Ngoài phạm vi v1

Adapter Codex, Kiro và Gemini (chỉ thiết kế sẵn trait); xuất báo cáo; đồng bộ skill giữa các công cụ; sửa nội dung skill trong app; duyệt kho skill công khai; dùng nhiều máy hoặc cho team.

### 1.6 Khảo sát các tool tương tự (09/2026)

Thông tin lấy từ README. Riêng ccusage đã chạy thử trên máy người dùng.

| Tool | Dạng | Có | Không có |
|---|---|---|---|
| ccusage | CLI | Thống kê theo ngày, phiên, khối 5h, khoảng 18 công cụ, có bỏ trùng (khớp số liệu: 726M) | Thống kê theo lần hỏi, skill, tray, thông báo |
| claude-devtools | Electron, 3 hệ điều hành | Chia token từng lượt theo 7 nhóm (CLAUDE.md, skill, file, tool I/O, thinking…), biểu đồ compaction, thông báo khi tốn nhiều | Tray và quota 5h/7d, quản lý skill; nặng vì là Electron |
| TokenTracker | Tray, 3 hệ điều hành | Giới hạn realtime, hơn 40 công cụ, bật/tắt và đồng bộ skill có Undo | Thống kê theo lần hỏi, chi phí skill so với mức dùng |
| Claude-Code-Usage-Monitor v4 | TUI | Đọc `rate_limits` từ statusline, dự đoán tốc độ dùng | Thông báo hệ điều hành, tray |
| ccboard | TUI và web | Tab Skills/Plugins, cảnh báo ngân sách | Chưa rõ có quản lý skill không |

**Điểm khác biệt của thongke:**
1. Gom tất cả vào một app nhẹ (Tauri).
2. Giải thích **"vì sao tốn và nên làm gì"** ngay khi đang làm việc.
3. Thông báo quota theo % thật từ statusline, chạy được cả trên Linux và Windows.
4. Quản lý skill **dựa trên số liệu**: so chi phí nền với mức dùng thật, rồi mới tắt/bật/gỡ.

## 2. Phát hiện từ dữ liệu thật

Khảo sát `~/.claude/projects` trên máy người dùng (24 file `.jsonl`, khoảng 34MB, 595 lần hỏi):

- **Log lặp bản ghi:** mỗi content block (text, tool_use…) của một lần gọi API được ghi thành một dòng `type=assistant` riêng, và các dòng này mang **cùng `usage`**. Bắt buộc phải bỏ trùng theo `requestId` (dự phòng: `message.id`). Chưa bỏ trùng thì tổng là 1,40 tỉ token, bỏ trùng rồi thì còn 721 triệu.
- Trung vị khoảng 678K token mỗi lần hỏi, p90 khoảng 2,86M.
- Lần hỏi đắt chủ yếu vì **context lớn**, không phải vì output. Ví dụ: một lần hỏi tốn 9,4M token qua 11 lần gọi API với context khoảng 841K, output chỉ 17K.
- Skill được ghi ở `attributionSkill` (trên dòng assistant) và ở tool_use `Skill` (`input.skill`). Slash command nằm trong `<command-name>` ở message của user.
- Log chỉ ghi giới hạn **khi đã bị chặn**: dòng `isApiErrorMessage` có `error: "rate_limit"`, kèm `quotaLimits.rateLimitType` (`five_hour`…) và `resetsAt`.
- Phần trăm giới hạn **trước khi bị chặn** chỉ có trong JSON stdin của statusLine (có tài liệu chính thức): `rate_limits.five_hour.used_percentage`, `rate_limits.five_hour.resets_at`, `rate_limits.seven_day.*`. Chỉ có với gói Pro/Max, và có sau lần API trả lời đầu tiên. Hook, `/usage` và OpenTelemetry đều không cung cấp số liệu này.
- `claude plugin details <name>` cho biết token chi phí nền (always-on) và token mỗi lần gọi (on-invoke) của từng skill con trong plugin.
- Trên máy đã có `~/.codex/sessions`, `~/.kiro/{sessions,skills}`, `~/.gemini` để làm adapter sau này.

## 3. Kiến trúc

### 3.1 Công nghệ

- **Tauri 2 + Rust** cho lõi. Giao diện là **HTML/CSS/TypeScript thuần** chạy trong WebView của hệ điều hành, không dùng framework, build bằng Vite.
- **Lý do chọn:** nhẹ nhất trong các lựa chọn vẫn có giao diện đầy đủ. Theo dõi file bằng sự kiện của hệ điều hành (crate `notify`: inotify / FSEvents / ReadDirectoryChangesW). Có sẵn tray, thông báo và tự khởi động.
- **Tiết kiệm RAM:** đóng cửa sổ thì **hủy WebView**, chỉ còn lõi Rust và tray chạy nền. Mở lại thì tạo lại WebView (khoảng 0,3 giây).
- Lưu trữ bằng **SQLite** (crate `rusqlite`, bản bundled). Vị trí file theo crate `dirs`: `data_dir()/thongke/thongke.db`, ví dụ `~/.local/share/thongke/`.

### 3.2 Sơ đồ

```
┌──────────── Cửa sổ Tauri (WebView) — HTML/TS thuần ─────────────┐
│   Lần hỏi · Skill · Project · Ngày · Cài đặt                    │
└──────────▲ event "data-changed"  │ invoke(query_*, skill_*, …) ──┘
           │                       ▼
┌────────────────────────── Rust core (chạy nền) ─────────────────┐
│ Watcher(notify) → Adapter.parse → Aggregator → Store(SQLite)    │
│                                          │                       │
│                         Insight/Alert ◄──┘ → Tray + Notification │
│  LimitWatcher (limits.json) ─────────────┘                       │
│                                                                  │
│  trait LogSource    trait SkillSource                            │
│   └─ claude_code    └─ claude_code        (v1)                   │
│   └─ codex / kiro   └─ kiro               (sau)                  │
└──────────────────────────────────────────────────────────────────┘
```

Giao diện và lõi giao tiếp qua IPC nội bộ của Tauri. **Không mở cổng mạng nào.**

### 3.2.1 Nhiều giao diện, một lõi (host + client)

```
                    ┌──────────── tk-core + tk-adapters + tk-skills ────────────┐
                    │  watcher → store(SQLite) → insight/alerts   (1 nơi GHI)   │
                    └──────────────────────────┬────────────────────────────────┘
                                 tk-service: API local (JSON-RPC)
                     Unix socket (Linux/macOS) · Named pipe (Windows)
            ┌──────────────┬───────────────┼────────────────┬──────────────────┐
            ▼              ▼               ▼                ▼                  ▼
     App desktop     Terminal TUI     VS Code ext     Statusline Claude    (sau) Web
     (Tauri, tray)   `tk tui`         status bar +    `tk statusline`
                                      webview panel
```

- **Host** là tiến trình duy nhất theo dõi log và ghi DB. Host là app desktop nếu app đang chạy, còn không thì là `tk daemon` chạy không giao diện. Dùng file lock `data_dir()/thongke/host.lock` để bảo đảm chỉ có một host. DB bật chế độ WAL.
- **Client** gồm TUI, VS Code và statusline. Client gọi `tk-service` qua socket hoặc pipe. Nếu chưa có host, client tự khởi động `tk daemon`.
- Socket chỉ cho user hiện tại truy cập (quyền `0600`, Windows dùng pipe ACL theo user), không mở cổng mạng.
- Bộ lệnh của `tk-service` **giống hệt** các lệnh Tauri ở mục 14.4. Cả hai cùng gọi một lớp `handlers` nên mỗi lệnh chỉ viết logic một lần. Tauri UI gọi trực tiếp trong tiến trình, không đi qua socket.

### 3.3 Các module (Rust)

| Module | Trách nhiệm | Phụ thuộc |
|---|---|---|
| `adapters::LogSource` (trait) | `id()`, `watch_roots()`, `parse_line(ctx, line) -> Vec<RawEvent>` | — |
| `adapters::SkillSource` (trait) | `list()`, `enable()`, `disable()`, `install()`, `uninstall()`, `cost()` | — |
| `adapters::claude_code` | Implement cả hai trait cho Claude Code | `serde_json` |
| `watcher` | Nhận sự kiện từ `notify`, đọc tiếp file từ offset, quét đối chiếu mỗi 60 giây | `notify`, `store` |
| `aggregator` | Chuyển `RawEvent` thành Session / Prompt / ApiCall / SkillUse; bỏ trùng; gán skill | `store` |
| `store` | Schema SQLite, migration, truy vấn | `rusqlite` |
| `insight` | Đánh dấu bất thường, gợi ý tối ưu, tính chi phí USD | `store`, `pricing` |
| `limits` | Đọc `limits.json` và các sự kiện 429, tính trạng thái giới hạn | `notify` |
| `alerts` | Quyết định khi nào gửi thông báo (có chống lặp), cập nhật tray | `tauri-plugin-notification` |
| `skills_ops` | Chạy thao tác skill có xác nhận, nhật ký thao tác, hoàn tác, thùng rác | `SkillSource`, `store` |
| `composition` | Ước lượng thành phần context của từng ApiCall, đọc lại nội dung tool I/O từ log theo offset (mục 6.1) | `adapters`, `store` |
| `handlers` | Lớp xử lý chung cho mọi lệnh (dùng bởi cả Tauri commands và `tk-service`) | tất cả |
| `service` | JSON-RPC qua Unix socket / named pipe, host lock | `handlers` |
| `commands` | Các lệnh Tauri `invoke`, chỉ bọc `handlers` | `handlers` |

Giao diện (`ui/`) gồm một file cho mỗi màn hình: `prompts.ts`, `skills.ts`, `projects.ts`, `days.ts`, `settings.ts`, `session.ts` (Phân tích phiên), và `detail.ts` cho panel chi tiết. Biểu đồ vẽ bằng SVG tự viết, không dùng thư viện chart để giữ app nhẹ.

## 4. Mô hình dữ liệu chung

### 4.1 Khái niệm

| Khái niệm | Trường chính | Claude Code |
|---|---|---|
| **Session** | id, tool, project, cwd, file_path, started_at, ended_at, deleted | Một file `.jsonl` / `sessionId`. Project lấy từ tên thư mục cha |
| **Prompt** | id, session_id, ts, text (tối đa 2.000 ký tự), interrupted, compacted_after | Dòng `type=user` có `promptId`, không phải `isMeta`, không có `toolUseResult`, không bắt đầu bằng `<` và không phải `[Request interrupted…]` |
| **ApiCall** | request_id (unique), prompt_id, ts, model, input, cache_write, cache_read, output, context, tools_json | Dòng `type=assistant`, **bỏ trùng theo `requestId`**. `context = input + cache_write + cache_read`. Tool gộp từ mọi dòng cùng `requestId` |
| **SkillUse** | prompt_id, skill, kind (`direct` / `continued` / `command`), source | `Skill` tool_use, `attributionSkill`, `<command-name>` |

Dòng `[Request interrupted by user]` đánh dấu `interrupted=true` cho lần hỏi ngay trước nó. Dòng `isCompactSummary` đánh dấu `compacted_after` và cắt chuỗi "làm tiếp" của skill.

### 4.2 Số liệu mỗi lần hỏi (tính khi truy vấn hoặc cache trong bảng `prompt_stats`)

Tổng từng loại token, `total`, `context_at_start` (của ApiCall đầu tiên), `api_calls`, `tools` (đếm theo tên), `duration` (từ lúc hỏi đến ApiCall cuối), `cost_usd`.

### 4.3 Quy tắc gán skill

- **direct:** trong lần hỏi có tool_use `Skill`, có `attributionSkill`, hoặc có slash command (kind `command`).
- **continued:** các lần hỏi sau trong cùng phiên kế thừa skill direct gần nhất, cho đến khi gặp một trong ba trường hợp: có skill direct khác, có compact, hoặc lần hỏi cách lần trước hơn 30 phút.
- Giao diện: tag đậm là direct, tag nhạt là continued.

### 4.4 Quy tắc bất thường (ngưỡng chỉnh được)

`total > 3 × trung vị (30 ngày gần nhất)`, **hoặc** `context_at_start > 300_000`.

Thanh màu hiển thị: xanh khi dưới 2× trung vị, vàng khi từ 2× đến dưới 5×, đỏ khi từ 5× trở lên.

### 4.5 Chi phí

Bảng giá model lưu ở `pricing.toml` (trong data dir, có bản mặc định đi kèm app, người dùng sửa được). Giá tính riêng cho input, cache_write (5 phút / 1 giờ), cache_read và output. Model không có trong bảng thì hiện "—", không đoán giá. Bảng mặc định lấy từ trang giá chính thức lúc triển khai.

### 4.6 Bảng SQLite phụ

- `file_offsets(path, inode/file_id, size, offset, mtime)`
- `notes(prompt_id, starred, note, updated_at)`, sao lưu song song ra `notes.json`
- `skill_ops(id, ts, op, target, before_json, after_json, status)` dùng cho hoàn tác
- `alerts_sent(key, ts)` để chống gửi thông báo lặp
- `schema_version`

## 5. Đọc log realtime

1. Khi khởi động: liệt kê các file trong `watch_roots()`. Với mỗi file, so với `file_offsets`, rồi đọc từ `offset` đến hết.
2. `notify` báo file thay đổi thì đọc tiếp từ `offset`. **Chỉ xử lý đến ký tự `\n` cuối cùng.** Phần còn lại giữ lại cho lần đọc sau.
3. Nếu `size < offset` hoặc inode/file_id đổi thì đọc lại file từ đầu. Xóa ApiCall cũ của file đó trước khi đọc lại (bỏ trùng theo `request_id` giúp tránh đếm hai lần).
4. File bị xóa: giữ dữ liệu và đánh dấu `Session.deleted = true`.
5. Quét đối chiếu mỗi 60 giây (theo size và mtime) để bắt các sự kiện bị lỡ, ví dụ khi máy ngủ hoặc inotify bị tràn.
6. Mỗi lô dữ liệu được ghi trong một transaction, sau đó gửi event `data-changed { session_ids, prompt_ids }` cho giao diện (nếu đang mở) và cho module `alerts`.

## 6. Giao diện

Bố cục 3 cột (theo mockup v4 đã duyệt):

- **Sidebar trái:**
  - Các màn hình: Lần hỏi, Skill, Project, Ngày, Cài đặt.
  - Lọc nhanh: 🔥 tốn bất thường, 🧠 context trên 300K, ✋ bị ngắt ngang, ⭐ cần xem lại.
  - Số liệu hôm nay.
  - Bộ lọc "Công cụ" (ẩn khi mới có một adapter).
- **Cột giữa, danh sách lần hỏi:**
  - Gom theo phiên.
  - Ô tìm kiếm theo nội dung câu hỏi. Bộ lọc dạng chip: project, skill, khoảng thời gian, sắp xếp.
  - Mỗi dòng gồm: giờ, câu hỏi, thanh màu, tag skill, token.
  - Lần hỏi mới tự thêm vào khi đang chạy.
- **Cột phải, chi tiết một lần hỏi:**
  - Tổng token, số lần gọi API, context lúc hỏi, output thực sự, chi phí.
  - Biểu đồ cột từng lần gọi API (phần đọc context và phần output).
  - Các tool đã dùng.
  - Mục **"Vì sao tốn / Nên làm"** do `insight` sinh ra.
  - Nút ⭐, 📝 và "Mở vị trí file log".
  - Nút **"Phân tích sâu"** để mở màn hình Phân tích phiên (mục 6.1).
- **Skill:** bảng gồm skill cá nhân, skill trong plugin, slash command, và skill **đã cài nhưng chưa dùng**. Có các cột nguồn, trạng thái, chi phí nền, chi phí mỗi lần gọi, số lần dùng, TB token mỗi lần, lần cuối dùng, và các công cụ có skill đó. Bấm vào một skill thì cột giữa lọc theo skill đó. Các nút thao tác xem ở mục 8.
- **Project:** mỗi project gồm tổng token, số phiên, các skill đã dùng, và lần hỏi đắt nhất. Bấm vào thì lọc theo project.
- **Ngày:** biểu đồ cột token theo ngày. Bấm vào một cột thì lọc theo ngày đó.
- **Cài đặt:** các ngưỡng, đường dẫn cho từng adapter, tự khởi động, bảng giá, bật/tắt từng loại thông báo, thiết lập statusline (mục 7), số dòng log lỗi, nhật ký thao tác skill.

Múi giờ hiển thị theo hệ thống. Giao diện theo sáng/tối của hệ thống.

### 6.0 Ngôn ngữ thiết kế (đã duyệt 2026-09-24)

App **đa nền tảng** (Linux, Windows, macOS). Giao diện theo **phong cách macOS**, nhưng không phải app riêng cho macOS. Mockup tham chiếu: `2026-09-24-thongke-mockup-macos.html`.

- **Thanh tiêu đề:** dùng thanh tiêu đề **gốc của từng hệ điều hành**, không tự vẽ nút đóng/thu nhỏ. Bỏ 3 nút màu của macOS có trong mockup.
- **Font:** `-apple-system, "SF Pro Text"` trên macOS. Các hệ điều hành khác dùng **Inter**, nhúng sẵn trong app (woff2, dùng được khi offline). Số liệu dùng `font-variant-numeric: tabular-nums`.
- **Màu:** khai báo dạng token trên `:root`, có đủ chế độ sáng và tối theo hệ thống (`prefers-color-scheme`). Màu nhấn xanh hệ thống (`#007aff` khi sáng, `#0a84ff` khi tối). Màu trạng thái: xanh lá, cam, đỏ, tím (skill).
- **Chi tiết:** đường kẻ 0,5px; bo góc 6–12px; hàng đang chọn tô màu nhấn với chữ trắng; nhãn nhóm 11px màu xám; nút có viền mảnh; thẻ số liệu nền xám nhạt; không dùng viền đậm hay màu gắt.
- **Sidebar trong mờ:** dùng `backdrop-filter` nếu WebView hỗ trợ, không thì dùng màu nền đặc.
- **Tương tác:** ↑/↓ để chọn lần hỏi, `⌘/Ctrl+F` để vào ô tìm kiếm, `Esc` để bỏ lọc.

### 6.1 Phân tích phiên (phân tích sâu kiểu devtools)

Màn hình toàn trang cho một phiên, mở từ panel chi tiết hoặc từ danh sách phiên.

- **Biểu đồ context theo thời gian:** trục X là từng lượt gọi API, trục Y là context. Các vùng chồng lên nhau thể hiện **thành phần context**. Có đánh dấu các điểm compact (context tụt xuống) và các lần hỏi của người dùng.
- **Thành phần context của từng lượt (ước lượng):**

  | Nhóm | Cách nhận diện trong log |
  |---|---|
  | Hệ thống / CLAUDE.md / memory | Các `attachment` và message `isMeta` ở đầu phiên hoặc sau compact |
  | Skill | Nội dung được nạp khi gọi tool `Skill` (tool_result của Skill) |
  | File đã đọc | tool_result của Read, Grep, Glob, và các lệnh Bash đọc file |
  | Tool I/O khác | tool_use input và tool_result còn lại |
  | Thinking | Các block `thinking` của assistant |
  | Output của assistant | Các block text và tool_use của assistant |
  | Câu hỏi của người dùng | Message user có `promptId` |

  **Cách ước lượng:**
  - Đếm số ký tự của từng khối rồi quy ra token theo hệ số (mặc định 1 token ≈ 3,5 ký tự; chỉnh được).
  - Cộng dồn theo thứ tự hội thoại. Sau mỗi lần compact thì đặt lại, lúc này tổng tóm tắt được tính vào nhóm "Hệ thống".
  - Hiệu chỉnh theo `usage` thật: nhân mọi nhóm với cùng một tỉ lệ, sao cho tổng bằng `context` thật của lượt đó.
  - Giao diện luôn ghi rõ nhãn **"ước lượng"**.
- **Top khối nặng nhất** trong context hiện tại, ví dụ: "tool_result Bash lúc 11:30, khoảng 120K token". Bấm vào để xem nội dung.
- **Xem tool call:** nội dung input/output của từng tool. Edit và Write hiện dạng diff, Bash hiện lệnh và output (tự cắt, có nút xem hết), Read hiện đường dẫn và số dòng.
- **Gợi ý từ phân tích:**
  - "File X được đọc 4 lần, tốn khoảng 200K."
  - "Output Bash quá dài, nên dùng `| head`."
  - "Skill Y nạp 9.7K token mỗi lần gọi."

Nội dung tool I/O **không lưu vào SQLite**. Khi mở màn hình này, app đọc lại trực tiếp từ file log theo offset đã lưu, để DB luôn nhỏ. DB chỉ lưu số liệu tổng hợp theo nhóm cho từng ApiCall, trong bảng `call_composition(request_id, category, est_tokens)`.

## 7. Giới hạn sử dụng, tray và thông báo

### 7.1 Nguồn dữ liệu giới hạn

- **Trước khi bị chặn:** lấy từ statusLine qua lệnh **`tk statusline`**. Lệnh này đọc JSON từ stdin, ghi `rate_limits` ra `data_dir()/thongke/limits.json` (ghi ra file tạm rồi đổi tên), rồi in ra **một dòng trạng thái** gọn, ví dụ `🟡5h 78% ↻14:30 · lượt này 620K · ctx 450K · pj-review`. Không cần `jq`, chạy được trên cả Windows.
  - Người dùng đã có script statusLine (máy này là `~/.claude/statusline-command.sh`): trong Cài đặt có nút "Bật theo dõi giới hạn". Khi bấm, app hiện chính xác đoạn sẽ thêm vào script, rồi **sao lưu** script. Đoạn thêm vào là `echo "$input" | tk statusline --quiet >/dev/null` (`--quiet` chỉ ghi file, không in gì), nên phần hiển thị hiện tại của script giữ nguyên.
  - Người dùng muốn dùng luôn dòng trạng thái của app: đặt `statusLine.command = "tk statusline"`. Trường hợp chưa có statusLine: app hỏi trước khi tạo script mới và thêm `statusLine` vào `~/.claude/settings.json`. Việc sửa settings là thao tác có xác nhận, có sao lưu và hoàn tác được. Có nút gỡ để khôi phục bản sao lưu.
- **Đã bị chặn:** lấy từ dòng log có `error: "rate_limit"`, dùng `quotaLimits.resetsAt` và `rateLimitType`.
- Nếu `limits.json` cũ hơn 10 phút thì tray hiện "cập nhật X phút trước". Không báo động khi dữ liệu đã cũ.

### 7.2 Tray

- Icon đổi màu theo mức giới hạn cao nhất: xanh dưới 70%, vàng từ 70%, đỏ từ 90% hoặc khi đã bị chặn.
- Menu gồm: 5 giờ và 7 ngày (% và giờ reset), token và chi phí hôm nay, lần hỏi gần nhất, Mở dashboard, Tạm dừng thông báo 1 giờ, Thoát.

### 7.3 Thông báo (bật/tắt và chỉnh ngưỡng được từng loại)

| Loại | Điều kiện | Chống lặp |
|---|---|---|
| Giới hạn | 5h hoặc 7 ngày vượt 70% / 90% | Mỗi ngưỡng chỉ báo 1 lần cho mỗi chu kỳ reset |
| Bị chặn | Có lỗi 429, và báo lại khi đến `resetsAt` | Mỗi `resetsAt` chỉ báo 1 lần |
| Lần hỏi bất thường | Lần hỏi thỏa mục 4.4 | Mỗi lần hỏi chỉ báo 1 lần, khi lần hỏi đó kết thúc (không có ApiCall mới trong 5 giây) |
| Context lớn | `context` vượt 300K trong phiên đang chạy | Mỗi phiên chỉ báo 1 lần cho mỗi ngưỡng |

Bấm vào thông báo thì mở dashboard tại đúng lần hỏi hoặc phiên đó.

## 8. Quản lý skill

### 8.1 Liệt kê

- Skill cá nhân: các thư mục con của `~/.claude/skills/` có `SKILL.md` (đọc frontmatter `name` và `description`). Skill đang tắt nằm ở `~/.claude/skills-disabled/`.
- Plugin: `claude plugin list` và `claude plugin details <name>` (lấy danh sách skill con và chi phí nền / chi phí mỗi lần gọi).
- Chi phí của skill cá nhân là ước lượng: chi phí nền dựa trên độ dài `description`, chi phí mỗi lần gọi dựa trên độ dài `SKILL.md`. Công thức ký tự sang token được ghi rõ là ước lượng.
- Gợi ý: skill không được dùng trong 30 ngày và có chi phí nền lớn hơn 0 thì gợi ý tắt.

### 8.2 Thao tác

| Thao tác | Skill cá nhân | Plugin |
|---|---|---|
| Tắt | Chuyển `skills/<n>` sang `skills-disabled/<n>`. Nếu là symlink thì chỉ chuyển link | `claude plugin disable` |
| Bật | Chuyển ngược lại | `claude plugin enable` |
| Cài | Từ thư mục (Linux/macOS: symlink; Windows: copy) hoặc git URL (`git clone`). Kiểm tra có `SKILL.md` với frontmatter hợp lệ | `claude plugin install <plugin@marketplace>` |
| Gỡ | Chuyển vào `data_dir()/thongke/trash/<ts>-<n>`, giữ 30 ngày. Nếu là symlink thì chỉ xóa link | `claude plugin uninstall` |

**Nguyên tắc:**
- Plugin chỉ được thao tác qua CLI chính thức, app không tự sửa file cấu hình plugin.
- Mọi thao tác đều qua hộp xác nhận ghi rõ các file bị ảnh hưởng, và được ghi vào `skill_ops` để **hoàn tác** được.
- Tên skill trùng thì hỏi người dùng muốn ghi đè hay đổi tên.
- Nếu thao tác thất bại thì hoàn tác phần đã làm và hiện nguyên thông báo lỗi.
- Sau mỗi thao tác, app nhắc: "Có hiệu lực từ phiên Claude Code mới."

## 9. Mở rộng sang công cụ khác

Thêm một công cụ mới gồm các bước:
1. Tạo `adapters/<tool>.rs` implement `LogSource` (và `SkillSource` nếu công cụ có khái niệm tương đương).
2. Đăng ký adapter vào registry.
3. Thêm đường dẫn mặc định cho từng hệ điều hành.

Core, store và giao diện không phải sửa. Trường `Session.tool` cho phép lọc theo công cụ. Giới hạn sử dụng là tùy chọn của từng adapter (`fn limits() -> Option<LimitState>`).

## 10. Đa nền tảng

| | Windows | macOS | Linux |
|---|---|---|---|
| File cài | `.msi` / NSIS `.exe` | `.dmg` | `.deb` / `.AppImage` |
| Log Claude | `%USERPROFILE%\.claude\projects` | `~/.claude/projects` | `~/.claude/projects` |
| Tray | Có | Thanh menu | Cần AppIndicator. Nếu không có tray thì vẫn chạy bằng cửa sổ và thông báo |
| Tự khởi động | `tauri-plugin-autostart` | như Windows | như Windows |

- Chỉ chạy một bản app: dùng `tauri-plugin-single-instance`.
- Build bằng GitHub Actions với ma trận 3 hệ điều hành.
- Phát triển và test thật trên Linux. Test tay trên Windows và macOS thì cần máy thật hoặc máy ảo.

## 11. Xử lý lỗi

| Tình huống | Xử lý |
|---|---|
| Dòng JSON hỏng hoặc đang ghi dở | Chỉ đọc đến `\n` cuối. Dòng hỏng thì bỏ qua và đếm vào `parse_errors` |
| Định dạng log thay đổi | Bỏ qua trường lạ, trường không bắt buộc để trống. Gặp `version` chưa từng thử thì hiện cảnh báo nhẹ |
| File bị ghi lại nhỏ hơn hoặc bị xóa | Xem mục 5 |
| Watcher bị lỡ sự kiện | Quét đối chiếu mỗi 60 giây |
| DB hỏng hoặc migration lỗi | Đổi tên DB cũ, tạo lại từ log, khôi phục ghi chú từ `notes.json` |
| `limits.json` không có hoặc đã cũ | Hiện "chưa có dữ liệu" hoặc "X phút trước", không báo động |
| Thao tác skill lỗi | Hoàn tác và hiện nguyên lỗi |
| Không có tray | Chạy bằng cửa sổ và thông báo |

## 12. Test

- **Unit test adapter Claude Code** trên bộ file mẫu `tests/fixtures/` (cắt từ log thật, **đã xóa nội dung**, chỉ giữ cấu trúc). Gồm: dòng trùng `requestId`, compact, interrupt, lỗi 429, slash command, dòng meta, dòng ghi dở.
- **Aggregator:** gán skill direct và continued, quy tắc bất thường, chi phí, context lúc hỏi.
- **Watcher:** ghi thêm dòng vào file trong lúc đang theo dõi, ghi một dòng làm 2 lần, file bị ghi lại nhỏ hơn, file bị xóa. Kiểm tra không trùng, không sót.
- **composition:** trên bộ fixture, tổng các nhóm sau khi hiệu chỉnh phải bằng `context` thật; compact phải đặt lại đúng; phân loại đúng tool_result của Read, Bash và Skill.
- **skills_ops:** chạy trên thư mục `HOME` tạm. Làm lần lượt tắt, bật, gỡ, hoàn tác, trùng tên, symlink, rồi kiểm tra file thật trên đĩa. Phần plugin dùng CLI giả (script `claude` giả nằm đầu `PATH`).
- **`tk audit`:** so tổng token của DB với một cách tính độc lập trực tiếp từ log. Phải khớp 100%.
- **Hiệu năng:** đo thời gian quét lần đầu, độ trễ realtime và RAM khi ẩn cửa sổ, so với tiêu chí ở mục 1.3.
- **CI:** chạy `cargo test` trên 3 hệ điều hành. Test giao diện làm tay trên Linux.

## 13. Phạm vi v1 và các cột mốc

Mỗi cột mốc xong là **dùng được ngay**. M1 đến M6 khoảng 4 tuần làm việc. M7 và M8 thêm khoảng 1,5 tuần.

| Cột mốc | Nội dung | Ước lượng |
|---|---|---|
| **M1: Lõi + Tray** | Adapter Claude Code (phần log), watcher, bỏ trùng, gán skill, SQLite, `tk audit`; `tk-service` + host lock; `tk statusline`; tray; quota 5h/7d (mục 7); thông báo quota, context lớn, lần hỏi bất thường | 5–6 ngày |
| **M2: Lần hỏi** | Khung 3 cột, danh sách lần hỏi, tìm kiếm và lọc, panel chi tiết có "Vì sao tốn / Nên làm", ⭐ và 📝, Cài đặt cơ bản | 4–5 ngày |
| **M3: Skill** | Liệt kê và tính chi phí (mục 8.1), "dùng ở đâu", gợi ý; thao tác tắt/bật/cài/gỡ có hoàn tác (mục 8.2) | 4–5 ngày |
| **M4: Project, Ngày** | Hai màn hình tổng hợp, bấm vào để lọc danh sách lần hỏi | 2 ngày |
| **M5: Phân tích phiên** | Mục 6.1: thành phần context, biểu đồ context và compact, xem tool call, gợi ý | 4–5 ngày |
| **M6: Đóng gói** | File cài cho 3 hệ điều hành, tự khởi động, single-instance, CI | 2 ngày |
| **M7: Terminal** | `tk tui` (ratatui: Lần hỏi, chi tiết, Skill, quota; phím j/k, /, Tab), `tk daemon`, `tk prompts --json` | 3–4 ngày |
| **M8: VS Code** | Extension: thanh trạng thái quota và token, panel webview dùng lại `app/ui` (đổi `api.ts` sang socket), thông báo, tự lọc theo workspace | 4–5 ngày |

- ⏭️ Để sau: adapter Codex, Kiro và Gemini; xuất báo cáo; đồng bộ skill giữa các công cụ.
- Khi làm M5: đọc cách claude-devtools (mã nguồn mở) phân loại context để tham khảo. **Không copy code** khi chưa kiểm tra license.

## 14. Cấu trúc dự án

Vì chưa có tên chính thức, các crate tạm dùng tiền tố `tk-`. Đổi tên sau bằng tìm và thay thế.

### 14.1 Thư mục

```
thongke/
├── Cargo.toml                  # workspace
├── crates/
│   ├── tk-core/                # Lõi: KHÔNG phụ thuộc Tauri/Claude
│   │   └── src/ lib.rs · model.rs · aggregator.rs · watcher.rs · insight.rs
│   │           composition.rs · limits.rs · pricing.rs · config.rs · handlers.rs
│   │           store/{mod,schema,queries}.rs
│   ├── tk-adapters/            # Mỗi công cụ AI là một module
│   │   └── src/ lib.rs (trait LogSource, SkillSource, registry)
│   │           claude_code/{mod,log,skills,statusline}.rs
│   ├── tk-skills/              # Thao tác skill an toàn
│   │   └── src/ ops.rs (plan/apply) · journal.rs (undo) · trash.rs
│   ├── tk-service/             # JSON-RPC qua socket/pipe, host lock
│   └── tk-cli/                 # tk audit|scan|statusline|daemon|tui|prompts
├── app/
│   ├── src-tauri/src/ main.rs · runtime.rs · commands.rs · tray.rs · alerts.rs · window.rs
│   └── ui/src/ main.ts · api.ts · state.ts
│              views/{prompts,detail,session,skills,projects,days,settings,onboarding}.ts
│              components/ · charts/ (SVG tự viết) · styles/{tokens,layout}.css
├── extensions/vscode/          # M8
├── tests/fixtures/claude_code/ # log mẫu đã xóa nội dung
├── pricing.default.toml
├── docs/superpowers/specs/
└── .github/workflows/ ci.yml · release.yml
```

### 14.2 Phụ thuộc giữa các thành phần

```
 app/ui ─invoke─► app/src-tauri ─┐        tk-cli / TUI / VS Code / statusline
                                 ▼                     │ socket
                           tk-core::handlers ◄── tk-service
                              ▲        ▲
                    tk-adapters    tk-skills
```

`tk-core` chỉ biết `RawEvent` và các trait, nên test được bằng `cargo test` mà không cần giao diện. `tk-adapters` là nơi duy nhất biết định dạng log của từng công cụ.

### 14.3 Luồng realtime

```
Claude ghi .jsonl ─► notify ─► watcher (offset → '\n' cuối) ─► adapter.parse_line
  ─► aggregator (bỏ trùng, gán skill) ─► store (1 transaction) ─► runtime:
        ├─► alerts → thông báo HĐH     ├─► tray (số hôm nay, màu)
        └─► event "data-changed" → UI Tauri / client socket (subscribe)
statusline ─► tk statusline ─► limits.json ─► notify ─► limits ─► tray/alerts
```

### 14.4 Bộ lệnh (Tauri invoke = tk-service JSON-RPC)

| Nhóm | Lệnh |
|---|---|
| Lần hỏi | `list_prompts(filter)`, `prompt_detail(id)`, `session_analysis(id)`, `set_note(id, starred, note)` |
| Skill | `list_skills()`, `skill_usage(name)`, `plan_skill_op(op)`, `apply_skill_op(plan_id)`, `undo_skill_op(id)`, `skill_ops_log()` |
| Tổng hợp | `project_stats()`, `day_stats(range)`, `today_summary()`, `limit_state()` |
| Cài đặt | `get_settings()`, `save_settings(s)`, `setup_statusline()`, `remove_statusline()`, `run_audit()` |
| Sự kiện | `data-changed`, `limits-changed`, `scan-progress` (client socket nhận qua `subscribe`) |

Thao tác skill luôn đi qua 2 bước: **`plan`** trả về danh sách thay đổi cụ thể để hiện trong hộp xác nhận, sau đó **`apply`** mới thực sự sửa file.

### 14.5 Thư viện chính

| Phần | Thư viện |
|---|---|
| Rust chung | `serde`, `serde_json`, `thiserror`, `anyhow`, `tracing`, `chrono`, `dirs`, `toml` |
| Lõi | `rusqlite` (bundled), `notify` |
| Service | `interprocess` (local socket / named pipe đa nền tảng), `fs4` (file lock) |
| Tauri | `tauri` 2, các plugin `notification`, `autostart`, `single-instance`, `dialog`, `opener` |
| TUI | `ratatui`, `crossterm` |
| UI | Vite, TypeScript, `@tauri-apps/api`. Không dùng framework, không dùng thư viện chart |
| Test | `tempfile`, `insta` |
