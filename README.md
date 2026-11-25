# LifeOS

**Trợ lý AI cá nhân cho việc cần làm, thói quen và nhật ký, với dữ liệu lưu trên máy bạn.**

![Bìa LifeOS với đồi xanh lúc bình minh và một cuốn sổ mở](docs/assets/cover.webp)

LifeOS dùng Claude Code để trò chuyện với bạn và quản lý một vault Markdown + SQLite. Bạn có thể xem kế hoạch hôm nay, nhờ agent cập nhật dữ liệu và theo dõi thay đổi trên dashboard.

- **Tổ chức ngày của bạn** với việc cần làm, mục tiêu, thói quen, nhật ký, ghi chú và inbox.
- **Theo dõi điều quan trọng** như người cần liên lạc, chi tiêu theo ngân sách và xu hướng sức khỏe.
- **Làm việc với một crew** gồm các agent chuyên trách, mỗi agent có vai trò, bộ công cụ, bộ nhớ và lịch sử trò chuyện riêng.
- **Dùng qua nhiều kênh** gồm web, terminal, Telegram và MCP.

## Bắt đầu

Cần Rust qua rustup, C toolchain, CMake, GNU Make, Node.js 22.12 trở lên và npm. Cài Claude Code, đăng nhập trước khi dùng các chức năng AI. Xem [hướng dẫn cài đặt](docs/getting-started.md) để biết phiên bản và cách xử lý lỗi build.

```sh
git clone https://github.com/nguyenvanduocit/lifeos.git
cd lifeos
rustup toolchain install nightly --target wasm32-unknown-unknown
cargo +nightly install wasm-bindgen-cli --version 0.2.130 --locked
./scripts/setup.sh
./target/release/lifeos init
./target/release/lifeos web
```

Mở [dashboard local](http://127.0.0.1:7420). Thử nhắn **“Thêm việc đọc sách tối nay rồi cho tôi xem kế hoạch hôm nay.”** Bạn có thể viết thêm thông tin về mình trong `~/lifeos/CLAUDE.md` để agent dùng khi trò chuyện.

Web mặc định nghe trên máy local. Khi cần dùng từ thiết bị khác, hãy đọc [hướng dẫn triển khai qua Tailscale](docs/deployment.md) vì web và WebTransport không có đăng nhập tích hợp.

## Tài liệu

| Tài liệu | Nội dung |
| --- | --- |
| [Cài đặt và sử dụng](docs/getting-started.md) | Build, các lệnh CLI, web, Telegram, MCP và cấu hình |
| [Dữ liệu và quyền riêng tư](docs/data.md) | Cấu trúc vault, sao lưu và luồng dữ liệu qua Claude Code |
| [Triển khai](docs/deployment.md) | Local, Tailscale, LaunchAgent và kiểm tra WebTransport |
| [Đóng góp](CONTRIBUTING.md) | Chạy môi trường dev, kiểm thử và gửi pull request |
| [Kiến trúc](CLAUDE.md) | Các crate, giao thức WebTransport và quy tắc đồng bộ kiểu |

Mã nguồn theo giấy phép [MIT](LICENSE).
