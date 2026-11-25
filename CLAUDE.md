# lifeos

AI agent quản trị cuộc đời: mục tiêu và việc cần làm, thói quen và sức khoẻ, tài chính, nhật ký và tri thức. Dữ liệu nằm trên máy người dùng; người dùng trò chuyện với agent qua CLI, web, Telegram và MCP.

## Kiến trúc

```
crates/
  lifeos-wire        kiểu dữ liệu đi qua đường truyền và frame postcard (sans-IO, biên dịch cả wasm32)
  lifeos-core        vault Markdown + SQLite + bộ tool (`tools!`): mọi thao tác đều đi qua đây
  lifeos-agent       phiên Claude Code (crate claude-code-agent) với tool lifeos chạy trong process
  lifeos-mcp         bộ tool lifeos làm MCP server qua stdio
  lifeos-web         axum (app nhúng + /link.json) và WebTransport (web-transport-quinn) trên UDP
  lifeos-web-client  phía trang của link, biên dịch sang wasm (web-transport, wasm-bindgen)
  lifeos-telegram    bot Telegram dùng long polling
  lifeos             binary: init | today | reindex | chat | ask | mcp | web | telegram
web/                 Vue 3 + Vite, chỉ vẽ những gì client wasm đưa cho
```

Đường đi của trang web: Vue ⇄ wasm (`lifeos-web-client`) ⇄ WebTransport, frame = 4 byte độ dài big-endian + postcard ⇄ `lifeos-web` ⇄ `Life` / `LifeAgent`. Mỗi lệnh gọi dùng một bidi stream riêng: client ghi một `Call` rồi finish; server ghi các `Reply::Progress`, sau đó đúng một reply cuối, rồi finish.

## Quy tắc

- **Mỗi kiểu dữ liệu chỉ định nghĩa một lần.** Kiểu đi qua đường truyền nằm trong `lifeos-wire`, còn TypeScript được sinh ra bằng `make gen` (ts-rs → `web/src/core/generated/`). Không chép kiểu bằng tay sang TS và không sửa file đã sinh.
- **Postcard không tự mô tả.** Kiểu trong `lifeos-wire` không được dùng `skip_serializing_if`, `flatten`, enum untagged hay internally tagged, hay `serde_json::Value`. Mỗi kiểu mới phải có trong `crates/lifeos-wire/tests/round_trip.rs`.
- **Mỗi tool chỉ khai báo một lần**, trong `tools!` (`crates/lifeos-core/src/tools.rs`): tên, quyền `read`/`write`, mô tả, struct input (schema sinh bằng schemars) và handler. Agent, MCP và web đều đọc `specs()` và gọi `call()`.
- **Mỗi loại dữ liệu có một nguồn gốc duy nhất.** Markdown trong vault giữ phần văn bản (tasks, goals, journal, notes); bảng `docs`/`docs_fts` chỉ là index, xoá đi dựng lại được (`lifeos reindex`). SQLite giữ phần số liệu (`habit_logs`, `transactions`, `metrics`, `kv`). Tiền lưu theo đơn vị nhỏ nhất của từng loại tiền (`amount_minor`).
- Người dùng có thể sửa file trong vault bằng tay. Gọi `sync_index()` trước khi đọc index, như các handler tool đang làm.
- Agent không có Bash. Tool built-in bị giới hạn trong vault (`cwd`), và `PermissionMode::DontAsk` từ chối mọi thứ chưa được cho phép.
- Web và link không có đăng nhập, nên chỉ chạy trên loopback. Telegram chỉ phục vụ các chat id được cho phép.
- Dự án còn sơ khai: refactor thoải mái, không cần giữ tương thích ngược.

## Lệnh

```sh
make deps        # npm install
make gen         # sinh lại TypeScript từ lifeos-wire
make core        # build wasm vào web/src/core/pkg (yêu cầu wasm-bindgen-cli đúng bản đã ghim)
make test        # test Rust, build wasm, typecheck Vue
make dev         # backend :7420 (link udp :7421) + Vite :5173
make build       # binary release có nhúng web app
```

Kiểm tra đầu-cuối mà không cần AI: `cargo test -p lifeos-web --test link` dựng server thật và gọi qua QUIC thật.
