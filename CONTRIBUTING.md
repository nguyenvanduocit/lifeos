# Đóng góp cho LifeOS

Đọc [hướng dẫn cài đặt](docs/getting-started.md) để chuẩn bị Rust nightly, wasm-bindgen và npm. Đọc [kiến trúc](CLAUDE.md) trước khi thay đổi wire types, tool hoặc luồng chat.

```sh
export RUSTUP_TOOLCHAIN=nightly
(cd web && npm ci)
LIFEOS_HOME="$(mktemp -d)" make dev
```

Mở Vite tại `http://localhost:5173`. Dùng vault thử nghiệm và dữ liệu giả khi gửi bug report, ảnh chụp hoặc log.

```sh
make test                           # kiểm thử Rust, build Wasm, typecheck Vue
make web                            # build frontend production
cargo test -p lifeos-web --test link # tích hợp WebTransport
```

Nếu thay đổi Rust wire types hoặc input của tool, chạy `make gen` và commit TypeScript được sinh ra cùng thay đổi Rust. Với pull request, mô tả hành vi thay đổi và các lệnh đã kiểm tra.
