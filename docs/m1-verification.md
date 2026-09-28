# M1: Kiểm chứng trên dữ liệu thật

- Ngày đo: 2026-09-24. Máy: Ubuntu GNOME, bản build release.
- Dữ liệu: `~/.claude/projects`, 38MB, 16.482 dòng.

| Tiêu chí (spec mục 1.3) | Mục tiêu | Kết quả | Đạt? |
|---|---|---|---|
| Token khớp cách đếm độc lập (`tk audit`) | 100% | input 4.984 · cache_write 18.149.215 · cache_read 735.396.956 · output 3.830.159. Tổng 757.381.314, **khớp 100%**, 0 lỗi parse | ✅ |
| Quét lần đầu | < 2 giây | 326ms (`tk audit`), 221ms (app) | ✅ |
| Độ trễ từ ghi log đến handler | < 100ms | 31ms (test `host`, release) | ✅ |
| CPU khi không có log mới | ~0% | 0–0,5% | ✅ |
| RAM khi chạy nền (cửa sổ đóng) | < 20MB | App: RSS 59MB, **bộ nhớ riêng 13,9MB**, PSS 30MB. `tk daemon`: RSS 6,4MB | ⚠️ Xem ghi chú |

## Ghi chú về RAM

- App Tauri nạp sẵn GTK và WebKitGTK ngay cả khi chưa mở cửa sổ, chiếm khoảng 45MB bộ nhớ ánh xạ từ file. Phần này là thư viện dùng chung với các app GTK khác. Bộ nhớ riêng của app chỉ 13,9MB.
- Nếu tính theo RSS, app **chưa đạt** mức dưới 20MB. `tk daemon` thì đạt (6,4MB).
- Hướng xử lý (để quyết ở M6): tách tray thành tiến trình nhẹ, còn cửa sổ Tauri chỉ khởi động khi bấm "Mở dashboard". Cách này tận dụng được kiến trúc host/client sẵn có.

## Kiểm thử chạy thật (Task 10)

- `tk status` lấy dữ liệu qua socket từ app đang chạy ✅
- `tk statusline` hiện `🔴5h 92% ↻07:00 · 7d 41% · hôm nay 38.6M` ✅
- Khởi động host thứ hai thì bị từ chối ✅
- Thông báo quota mức 90% được ghi vào `alerts_sent` (chỉ báo mốc cao nhất) ✅
- App chiếm quyền host khi `tk daemon` đang chạy ✅
- Tìm ra và sửa lỗi: data dir dài làm đường dẫn socket vượt giới hạn `sun_path`. Đã chuyển sang dùng `$XDG_RUNTIME_DIR` và có test.
