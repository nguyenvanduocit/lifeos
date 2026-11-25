# Cài đặt và sử dụng

## Build từ mã nguồn

Chuẩn bị Git, rustup, C compiler, CMake, GNU Make, Node.js 22.12 trở lên và npm. Trên macOS có thể cài C compiler bằng `xcode-select --install`. Cài Claude Code 2.1.329 trở lên theo yêu cầu của dự án và đăng nhập trước khi chat hoặc chạy bản tin.

Từ thư mục checkout:

```sh
rustup toolchain install nightly --target wasm32-unknown-unknown
cargo +nightly install wasm-bindgen-cli --version 0.2.130 --locked
./scripts/setup.sh
```

Script dùng `npm ci` và build binary có nhúng dashboard. Nếu build Wasm báo sai phiên bản, so sánh `wasm-bindgen --version` với phiên bản ghim trong `crates/lifeos-web-client/Cargo.toml`.

## Tạo vault và dùng terminal

```sh
./target/release/lifeos init
./target/release/lifeos today
./target/release/lifeos chat
./target/release/lifeos chat --resume
./target/release/lifeos ask "Tóm tắt các việc cần làm hôm nay"
```

`today` in dashboard dạng JSON và chạy được khi chưa đăng nhập Claude Code. Để đổi vault, dùng `--home` hoặc đặt `LIFEOS_HOME`.

## Dashboard web

```sh
./target/release/lifeos web --brief-at 06:30
```

Mở [dashboard](http://127.0.0.1:7420). `--brief-at` là giờ địa phương và có thể bỏ qua nếu không cần bản tin hằng ngày. Dùng bảng Assistant để thêm hay cập nhật dữ liệu. Nếu đã tạo crew, chọn agent trong bảng Assistant để chuyển cuộc trò chuyện.

## Telegram và MCP

Đặt `LIFEOS_TELEGRAM_TOKEN` trong môi trường của tiến trình và chạy `./target/release/lifeos telegram`. Khi chưa có danh sách cho phép, bot chỉ trả về chat ID. Nhắn bot để lấy ID, rồi đặt `LIFEOS_TELEGRAM_ALLOWED_CHATS` thành danh sách ID cách nhau bằng dấu phẩy và khởi động lại.

Đăng ký MCP qua stdio từ thư mục repo:

```sh
claude mcp add lifeos -- "$PWD/target/release/lifeos" mcp
```

Nếu muốn dùng vault riêng, thêm `--home "$HOME/my-lifeos-vault"` trước `mcp` trong lệnh đăng ký.

## Cấu hình thường dùng

| Biến môi trường | Mặc định hoặc tác dụng |
| --- | --- |
| `LIFEOS_HOME` | `~/lifeos` |
| `LIFEOS_MODEL` | Model mặc định của Claude Code |
| `LIFEOS_EFFORT` | `low`, `medium`, `high`, `xhigh` hoặc `max` |
| `LIFEOS_WEB_HOST` / `LIFEOS_WEB_PORT` | HTTP tại `127.0.0.1:7420` |
| `LIFEOS_LINK_HOST` / `LIFEOS_LINK_PORT` | WebTransport dùng host HTTP và cổng UDP `7421` theo mặc định |
| `LIFEOS_BRIEF_AT` | Giờ địa phương để viết bản tin, ví dụ `06:30` |
| `LIFEOS_TELEGRAM_TOKEN` | Token bot Telegram |
| `LIFEOS_TELEGRAM_ALLOWED_CHATS` | Danh sách chat ID được dùng agent |
