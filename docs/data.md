# Dữ liệu và quyền riêng tư

LifeOS lưu vault trên máy chạy ứng dụng. `lifeos init` tạo cấu trúc vault và một file `CLAUDE.md` để bạn mô tả thông tin muốn agent biết.

| Vị trí trong vault | Dữ liệu |
| --- | --- |
| `CLAUDE.md` | Thông tin và hướng dẫn dành cho agent |
| `tasks/`, `goals/`, `people/`, `notes/` | Việc cần làm, mục tiêu, hồ sơ liên hệ và ghi chú |
| `journal/<năm>/`, `reviews/` | Nhật ký, bản tin và các bản review |
| `crew/` | Cấu hình agent chuyên trách |
| `routines/` | Định nghĩa routine trong vault |
| `.lifeos/lifeos.db` | Habit logs, tương tác, giao dịch, ngân sách, chỉ số, inbox, bộ nhớ, lịch sử chat và index tìm kiếm |

Bạn có thể chỉnh Markdown bằng trình soạn thảo hoặc Obsidian. Chạy `lifeos reindex` nếu cần dựng lại index từ Markdown. SQLite cũng lưu dữ liệu gốc như giao dịch và lịch sử chat, nên giữ cả thư mục `.lifeos/` khi sao lưu.

Để sao lưu nhất quán, dừng các tiến trình LifeOS đang ghi dữ liệu rồi sao chép toàn bộ vault, kể cả thư mục ẩn. Khi khôi phục, trỏ `LIFEOS_HOME` đến bản vault đã phục hồi và kiểm tra bằng `lifeos today`.

Khi dùng chức năng AI, Claude Code xử lý nội dung chat và ngữ cảnh mà agent đọc từ vault. Khi dùng bot, tin nhắn và phản hồi đi qua Telegram. Giữ vault và các giá trị token ở ngoài checkout công khai.
