# Tích hợp Kiro CLI: kiểm chứng

Dữ liệu thật trên máy (`~/.kiro/sessions/cli`, 113MB) được nạp vào một DB tạm bằng `tk scan`:

| | Claude Code | Kiro |
|---|---|---|
| Phiên | 23 | 164 (gồm cả subagent) |
| Lần hỏi | 633 (24 lần ngắt ngang) | 755 (34 lần huỷ hoặc từ chối tool) |
| Chi phí | $ theo bảng giá | **448,02 credit** |
| Model | opus-5, opus-5-5, sonnet-5, haiku-4-5 | auto, claude-opus-4.6, claude-sonnet-4.6 |

- Quét cả hai công cụ (khoảng 150MB) mất 1,06 giây, 0 lỗi parse.
- **Kiro không ghi số token.** Con số token của Kiro là context ước lượng: `% context × cửa sổ context`, cộng dồn theo từng lượt. Chi phí tính bằng credit, không quy ra $. Màn Phân tích phiên không áp dụng cho Kiro.
- Skill của Kiro (`~/.kiro/skills`, 10 skill) hiện trên màn Skill với tag Kiro. Tắt, bật, gỡ và hoàn tác dùng thư mục `~/.kiro`, còn thùng rác và nhật ký nằm riêng ở `data_dir/kiro`. Kiro không có số lần dùng skill và không bao giờ bị gợi ý tắt.
- Màn **Model** mới gộp model của cả hai công cụ.
- Test: `cargo test --workspace` 156/156 · Vitest 50/50.
- Lần này chưa có reviewer độc lập. Thao tác skill của Kiro dùng lại `Manager` đã được review ở M3.
