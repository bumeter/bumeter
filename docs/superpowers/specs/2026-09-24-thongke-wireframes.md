# thongke — Wireframe các màn hình

- Ngày: 2026-09-24
- Trạng thái: Đã duyệt
- Đi kèm spec: `2026-09-24-thongke-design.md`
- Tên app tạm để là `[App]`. Số liệu trong wireframe chỉ mang tính minh họa.

## 1. Màn hình chính: Lần hỏi (danh sách và chi tiết)

```
┌─[App]──────────────────────────────────────────────────────────────── ─ □ ✕ ┐
│ ● LIVE  5h: ████████░░ 78% (reset 14:30)   7d: ████░░░░░░ 41%   Hôm nay 2.1M │
├──────────────┬─────────────────────────────────────┬─────────────────────────┤
│ XEM THEO     │ 🔎 Tìm trong câu hỏi...             │ "giờ bạn cập nhật tiến  │
│ ▸🕒 Lần hỏi  │ [Project: project-shop ✕] [Skill ▾]     │  độ nhé, và tôi cần..." │
│  🧩 Skill    │ [30 ngày ▾] [Sắp xếp: Thời gian ▾]  │ TICKET-101 · 11/09    │
│  📁 Project  │─────────────────────────────────────│ 11:28 · 4m40s · —skill  │
│  📅 Ngày     │ 231 lần hỏi · 184M tok · TB 796K    │┌──────────┬──────────┐ │
│  ⚙ Cài đặt   │                                     ││Tổng      │Lượt API  │ │
│              │ ▾ Phiên 11/09 09:10→12:40 · 71M     ││ 9.4M 🔴  │ 11       │ │
│ LỌC NHANH    │ 09:52 test chạy e2e với lambda… ███ ││Context   │Output    │ │
│  🔥 Bất thường│      [run]                   14.9M ││ 841K 🔴  │ 17K      │ │
│  🧠 Ctx>300K │ 11:02 tức là ntn ?              ▌   │└──────────┴──────────┘ │
│  ✋ Ngắt ngang│                              620K   │ Từng lượt gọi API:     │
│  ⭐ Xem lại  │▶11:28 giờ bạn cập nhật tiến độ… ██  │ █ █ █ █ █ █ █ █ █ █ █  │
│              │                              9.4M   │ ▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔  │
│ CÔNG CỤ      │ 11:40 ok                        ▌   │ ■ đọc context ■ output │
│  ☑ Claude    │                              870K   │ Tool: Bash×10          │
│              │ ▸ Phiên 10/09 14:05→17:30 · 52M     │┌ Vì sao tốn ──────────┐│
│ HÔM NAY      │ ▸ Phiên 09/09 ...                   ││99.8% là đọc lại 841K ││
│  11 lần hỏi  │                                     ││context cũ.           ││
│  2.1M tok    │                                     ││→ /compact hoặc mở    ││
│  $3.2        │                                     ││  phiên mới trước khi ││
│              │                                     ││  tổng kết.           ││
│              │                                     │└──────────────────────┘│
│              │                                     │[⭐Xem lại][📝Ghi chú]  │
│              │                                     │[🔬 Phân tích sâu →]    │
└──────────────┴─────────────────────────────────────┴─────────────────────────┘
  Thanh màu: ▌xanh <2× TB · ██vàng 2–5× · ███đỏ ≥5×
```

## 2. Phân tích phiên (phân tích sâu, M5)

```
┌─[App] › TICKET-101 › Phiên 11/09 09:10──────────────────────── ─ □ ✕ ┐
│ ← Quay lại    64 lần hỏi · 118 lượt API · 71M tok · 2 lần compact      │
├────────────────────────────────────────────────────────────────────────┤
│ CONTEXT THEO THỜI GIAN (ước lượng)                          1M ┤       │
│                                              ▄▆█           ┤          │
│                                     ▂▄▆███████▌  ← compact             │
│                        ▂▄▆█▌                 ▌  ▂▄▆█                   │
│            ▂▄▆████████▌   ▌ ← compact        ▌▂▄████                   │
│   ▂▄▆█████▌               ▌▂▄▆█████████████  ▌                         │
│  └──┬────────┬────────┬────────┬────────┬────────┬──── lượt API       │
│     ▲Q1      ▲Q12     ▲Q25     ▲Q40     ▲Q52     ▲Q64   (▲ = lần hỏi) │
│  ■ Hệ thống/CLAUDE.md  ■ Skill  ■ File đã đọc  ■ Tool I/O  ■ Thinking  │
│  ■ Output assistant    ■ Câu hỏi của bạn                               │
├───────────────────────────────────┬────────────────────────────────────┤
│ THÀNH PHẦN tại lượt đang chọn     │ TOP KHỐI NẶNG NHẤT                 │
│ File đã đọc  ██████████░  48% 404K│ 1. Bash 11:30 cat logs…  ~120K  [↗]│
│ Tool I/O     █████░░░░░░  24% 202K│ 2. Read handler.ts       ~ 85K  [↗]│
│ Hệ thống     ██░░░░░░░░░  11%  92K│ 3. Read handler.ts (lần 2)~85K  [↗]│
│ Output       ██░░░░░░░░░   9%  76K│ 4. Skill pj-review       ~ 12K  [↗]│
│ Skill        █░░░░░░░░░░   5%  42K│                                    │
│ Câu hỏi      ░░░░░░░░░░░   3%  25K│ GỢI Ý                              │
│                                   │ ⚠ handler.ts bị đọc 4 lần (~340K) │
│                                   │ ⚠ Output Bash dài, nên dùng | head│
├───────────────────────────────────┴────────────────────────────────────┤
│ TOOL CALL (lượt #57)                                                   │
│ ▸ Bash  cd /home/.../TICKET-158 && cat result.log      out 3.2K  [+]  │
│ ▾ Edit  src/handler.ts                                                 │
│   - const status = res.code                                            │
│   + const status = res.statusCode ?? 404                               │
│ ▸ Read  docs/investigation_vi.md  (1–420)                out 18K  [+]  │
└────────────────────────────────────────────────────────────────────────┘
```

## 3. Skill: danh sách và "dùng ở đâu" (M3)

```
┌─[App] › Skill──────────────────────────────────────────────────── ─ □ ✕ ┐
│ [+ Cài skill]  [↶ Hoàn tác]  Lọc: [Tất cả ▾] [Nguồn ▾]  🔎 Tìm skill... │
│ Chi phí nền tổng: ~2.4K tok/phiên · 16 skill bật · 5 chưa dùng 30 ngày  │
├───────────────────────────────────────────┬─────────────────────────────┤
│ SKILL          NGUỒN   BẬT NỀN  /LẦN DÙNG TB/LẦN  CUỐI  │ pj-review                   │
│ ▶pj-review     cá nhân ● ~90  ~4K    5   5.7M   16/09   │ cá nhân · ~/.claude/skills/ │
│  pj-estimate   cá nhân ● ~80  ~5K    4   2.3M   27/08   │ Nền ~90 tok · Gọi ~4K tok   │
│  pj-investigate cá nhân ● ~85 ~6K    4   1.9M   21/09   │ 5 lần · 5 project · 28.6M   │
│  run           plugin  ● ~60  ~3K    1  14.9M   11/09   │ [Tắt] [Gỡ] [Mở thư mục]     │
│  brainstorming superpw ● ~70  ~6.3K  4   0.5M   hôm nay │─────────────────────────────│
│  writing-skills superpw● ~40  ~9.7K  1   0.8M   28/08   │ DÙNG Ở ĐÂU                  │
│  /model        lệnh    –   –    –   14    –     23/09   │ 16/09 TICKET-201   4.4M  │
│ ─ chưa dùng ─────────────────────────────────           │  "đọc chi tiết các phần…"   │
│  pj-debug      cá nhân ● ~75  ~3K    0    –     –   💡  │ 10/09 project-shop        5.6M  │
│  pj-design     cá nhân ● ~80  ~8K    0    –     –   💡  │  "review lại xem đúng ko"   │
│  pj-code       cá nhân ● ~90  ~7K    0    –     –   💡  │ 09/09 project-shop        0.6M  │
│  estimate      cá nhân ● ~70  ~5K    0    –     –   💡  │ 29/08 PROJECT-SUMMARY 🔴15.4M │
│ ─ đã tắt ────────────────────────────────────           │ 27/08 project-shop        3.1M  │
│  old-skill     cá nhân ○  –    –     2   1.1M   02/08   │  (click → mở lần hỏi đó)    │
├───────────────────────────────────────────┴─────────────────────────────┤
│ 💡 5 skill chưa dùng 30 ngày tốn ~390 tok/phiên. [Xem & tắt hàng loạt]  │
└─────────────────────────────────────────────────────────────────────────┘
  ● bật  ○ tắt   NỀN = token luôn nạp mỗi phiên   /LẦN = token mỗi lần gọi
```

## 4. Hộp thoại cài skill và xác nhận thao tác

```
┌─ Cài skill ─────────────────────────────┐   ┌─ Xác nhận: Tắt skill ───────────────┐
│ Nguồn:  (•) Thư mục   ( ) Git URL       │   │ Tắt "pj-debug"?                     │
│         ( ) Plugin (marketplace)        │   │                                     │
│ ┌─────────────────────────────┐[Chọn…] │   │ Sẽ thực hiện:                       │
│ │ ~/skills-repo/pj-new  │        │   │  mv ~/.claude/skills/pj-debug       │
│ └─────────────────────────────┘        │   │   → ~/.claude/skills-disabled/      │
│ Cách cài: (•) Symlink  ( ) Copy         │   │                                     │
│                                         │   │ Tiết kiệm ~75 tok mỗi phiên.        │
│ ✔ Tìm thấy SKILL.md                     │   │ Hoàn tác được trong Nhật ký.        │
│ ✔ name: pj-new                          │   │ ⓘ Có hiệu lực từ phiên Claude mới.  │
│ ✔ description: 142 ký tự (~40 tok nền)  │   │                                     │
│ ⚠ Trùng tên? Không                      │   │            [Huỷ]  [Tắt skill]       │
│                  [Huỷ]  [Cài]           │   └─────────────────────────────────────┘
└─────────────────────────────────────────┘
```

## 5. Project và Ngày (M4)

```
┌─[App] › Project──────────────────────────────────────────────────┐
│ PROJECT               PHIÊN  LẦN HỎI  TOKEN   SKILL HAY DÙNG  ĐẮT NHẤT│
│ PROJECT-X-TICKET-101    3    231   184M ███  pj-review,run   14.9M ▸│
│ project-shop...-TICKET-102    2     88    98M ██   pj-spec,clarify  8.7M ▸│
│ PROJECT-X-TICKET-103    2     64    61M █▌   pj-investigate   4.1M ▸│
│ PROJECT-SUMMARY 1     52    54M █▌   pj-review       15.4M ▸│
│ ...                         (click → Lần hỏi lọc theo project)       │
└──────────────────────────────────────────────────────────────────┘

┌─[App] › Ngày────────────────────────────── [Tuần ▾] [Theo: Token ▾]┐
│ 200M┤                  █                                            │
│     ┤                  █                                            │
│ 100M┤        █         █                                            │
│     ┤     █  █     █   █              █        █                    │
│   0 ┼──▁──▂──█──▃──█───█───▂──▁──▁────█───▂────█──▁                 │
│      03 04 07 08 09 10 11  16 17 18   21  22   23 24   (09/2026)    │
│ Click cột → Lần hỏi của ngày đó                                     │
│ 11/09: 201M · 3 phiên · lần hỏi đắt nhất "test e2e lambda" 14.9M    │
└─────────────────────────────────────────────────────────────────────┘
```

## 6. Cài đặt

```
┌─[App] › Cài đặt──────────────────────────────────────────────────┐
│ THEO DÕI GIỚI HẠN (statusline)                                    │
│  Trạng thái: ● Đang bật · cập nhật 12 giây trước                  │
│  Script: ~/.claude/statusline-command.sh (đã sao lưu)  [Gỡ]       │
│ NGƯỠNG                                                            │
│  Quota cảnh báo   [70]% và [90]%                                  │
│  Bất thường       > [3]× trung vị   hoặc context > [300]K         │
│ THÔNG BÁO   ☑ Quota  ☑ Bị chặn/Reset  ☑ Lần hỏi bất thường  ☑ Ctx │
│ NGUỒN DỮ LIỆU  Claude Code: [~/.claude/projects      ] [Đổi]      │
│ HỆ THỐNG    ☑ Khởi động cùng máy   Múi giờ: [Hệ thống ▾]          │
│ BẢNG GIÁ    pricing.toml  [Mở file]  · 2 model chưa có giá ⚠       │
│ CHẨN ĐOÁN   Dòng log lỗi: 0 · Phiên bản log đã test: 2.1.247      │
│             [Chạy audit]  [Nhật ký thao tác skill]                │
└───────────────────────────────────────────────────────────────────┘
```

## 7. Tray và thông báo

```
 Thanh hệ thống:  … 🔊 📶 [🟡78%] 🕐 14:02        ← icon đổi màu: 🟢<70 🟡≥70 🔴≥90/bị chặn
                            │
               ┌────────────┴──────────────────┐
               │ 5 giờ   ████████░░ 78%  ↻14:30 │
               │ 7 ngày  ████░░░░░░ 41%  ↻T2    │
               │ Hôm nay 2.1M tok · $3.2        │
               │ Lần hỏi cuối: 620K "tức là ntn"│
               ├────────────────────────────────┤
               │ Mở dashboard                   │
               │ Tạm dừng thông báo 1 giờ       │
               │ Thoát                          │
               └────────────────────────────────┘

 ┌───────────────────────────────────┐  ┌───────────────────────────────────┐
 │ [App] · Quota                     │  │ [App] · Lần hỏi tốn bất thường    │
 │ Đã dùng 90% giới hạn 5 giờ.       │  │ "giờ bạn cập nhật tiến độ…" tốn   │
 │ Reset lúc 14:30 (còn 28 phút).    │  │ 9.4M (14× TB) – do đọc lại 841K   │
 └───────────────────────────────────┘  │ context. Nên /compact.     [Xem]  │
 ┌───────────────────────────────────┐  └───────────────────────────────────┘
 │ [App] · Context lớn               │  ┌───────────────────────────────────┐
 │ Phiên project-shop đang ở 450K.       │  │ [App] · Đã reset                  │
 │ Nên /compact hoặc mở phiên mới.   │  │ Giới hạn 5 giờ đã mở lại.         │
 └───────────────────────────────────┘  └───────────────────────────────────┘
```

## 8. Lần chạy đầu tiên (thiết lập)

```
┌─ Chào mừng đến [App] ───────────────────────────────────────┐
│ Bước 1/3  Nguồn dữ liệu                                     │
│  ✔ Tìm thấy Claude Code: ~/.claude/projects (24 phiên)      │
│  ○ Codex, Kiro: sẽ hỗ trợ sau                               │
│ Bước 2/3  Theo dõi giới hạn 5h/7d                           │
│  Cần thêm 1 dòng vào statusline để lấy % quota.             │
│  ┌──────────────────────────────────────────────────────┐   │
│  │ + echo "$input" | jq '.rate_limits' > ~/.local/…json │   │
│  └──────────────────────────────────────────────────────┘   │
│  Script gốc sẽ được sao lưu.     [Bỏ qua]  [Bật]            │
│ Bước 3/3  ☑ Khởi động cùng máy   ☑ Bật thông báo            │
│                                            [Bắt đầu →]      │
│  Đang quét log lần đầu... ████████████░░░░ 78%              │
└─────────────────────────────────────────────────────────────┘
```

Ghi chú: đoạn `jq` ở bước 2 chỉ minh họa. Bản thật sẽ ghi file theo kiểu ghi ra file tạm rồi đổi tên, và không phụ thuộc `jq` nếu máy không có (xem spec mục 7.1).
